//! `unsafe_scope`: report unsafe blocks that wrap more than the operations
//! that actually require unsafe — `unsafe { 1 + *p }` instead of
//! `1 + unsafe { *p }`.
//!
//! Used as a `RUSTC_WRAPPER`: cargo invokes this binary in place of rustc;
//! it registers one extra late lint and otherwise behaves exactly like the
//! compiler it ships with (same nightly toolchain).
#![feature(rustc_private)]
#![allow(internal_features)]
extern crate rustc_ast;
extern crate rustc_driver;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;

use rustc_ast::Mutability;
use rustc_hir as hir;
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{BlockCheckMode, ExprKind, HirId, Node, UnsafeSource};
use rustc_lint::{LateContext, LateLintPass, LintContext, declare_lint, impl_lint_pass};
use rustc_middle::ty::{self};
use rustc_hir::def_id::DefId;
use rustc_span::sym;

declare_lint! {
    /// Checks that `unsafe` blocks wrap only the operations that require
    /// unsafe. Larger blocks hide which operations are actually unsafe.
    pub UNSAFE_SCOPE,
    Warn,
    "unsafe block wraps more than the operations that require unsafe"
}

impl_lint_pass!(UnsafeScope => [UNSAFE_SCOPE]);

struct LintCallbacks;

impl rustc_driver::Callbacks for LintCallbacks {
    fn config(&mut self, config: &mut rustc_interface::Config) {
        let previous = config.register_lints.take();
        config.register_lints = Some(Box::new(move |sess, store| {
            if let Some(prev) = &previous {
                prev(sess, store);
            }
            store.register_lints(&[UNSAFE_SCOPE]);
            store.register_late_lint_pass(Box::new(|_| Box::new(UnsafeScope)));
        }));
    }
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    // As RUSTC_WRAPPER, cargo passes the real rustc path as the first arg.
    let rustc_path = if std::env::var_os("RUSTC_WRAPPER").is_some()
        && args
            .first()
            .is_some_and(|a| a.ends_with("rustc") || a.ends_with("clippy-driver"))
    {
        Some(args.remove(0))
    } else {
        None
    };

    // cargo probes the wrapper (`--print=file-names` et al.) to discover
    // target properties. Delegate such probes to the wrapped compiler
    // verbatim: this driver cannot answer them reliably across toolchains.
    if rustc_path.is_some() && args.iter().any(|a| a == "--print=file-names") {
        let real = rustc_path.as_deref().unwrap();
        let status = std::process::Command::new(real)
            .args(&args)
            .status()
            .unwrap_or_else(|e| {
                eprintln!("unsafe-scope-lint: probe delegation failed: {e}");
                std::process::exit(101);
            });
        std::process::exit(status.code().unwrap_or(101));
    }

    // The driver links librustc_driver.so from the toolchain it was built
    // with. Make the binary self-contained as a RUSTC_WRAPPER: if that
    // directory is not on the loader path, re-exec with it prepended. The
    // directory is baked in at build time — the wrapped compiler's
    // toolchain may differ from this binary's build toolchain.
    if std::env::var_os("UNSAFE_SCOPE_REEXEC").is_none() {
        {
            let sysroot_lib = option_env!("USCOPE_SYSROOT_LIB")
                .map(std::path::PathBuf::from)
                .filter(|l| l.exists());
            if let Some(lib) = sysroot_lib {
                let current = std::env::var("LD_LIBRARY_PATH").unwrap_or_default();
                if !current.split(':').any(|p| std::path::Path::new(p) == lib) {
                    let new_ld = if current.is_empty() {
                        lib.to_string_lossy().into_owned()
                    } else {
                        format!("{}:{}", lib.display(), current)
                    };
                    let mut cmd = std::process::Command::new(std::env::args().next().unwrap());
                    cmd.args(&args)
                        .env("LD_LIBRARY_PATH", new_ld)
                        .env("UNSAFE_SCOPE_REEXEC", "1");
                    let status = cmd.status().unwrap_or_else(|e| {
                        eprintln!("unsafe-scope-lint: re-exec failed: {e}");
                        std::process::exit(101);
                    });
                    std::process::exit(status.code().unwrap_or(101));
                }
            }
        }
    }
    // run_compiler expects argv including the program name.
    let mut at_args: Vec<String> = vec!["rustc".to_string()];
    at_args.extend(args);
    let early_dcx = rustc_session::EarlyDiagCtxt::new(
        rustc_session::config::ErrorOutputType::default(),
    );
    rustc_driver::init_rustc_env_logger(&early_dcx);
    let mut callbacks = LintCallbacks;
    let exit_code = rustc_driver::catch_fatal_errors(move || {
        rustc_driver::run_compiler(&at_args, &mut callbacks);
    })
    .map(|()| 0)
    .unwrap_or(101);
    std::process::exit(exit_code);
}

// -----------------------------------------------------------------------------
// Which expressions require unsafe?

struct UnsafeLeaf {
    hir_id: HirId,
    kind: &'static str,
    /// Whether the leaf can be individually wrapped (`unsafe { leaf }`)
    /// without changing meaning. A deref used as a place (borrow operand,
    /// assignment lhs, base of field access) cannot be moved out.
    movable: bool,
}

struct LeafFinder<'a, 'tcx> {
    cx: &'a LateContext<'tcx>,
    leaves: Vec<UnsafeLeaf>,
    inside_unsafe_block: bool,
}

impl<'a, 'tcx> Visitor<'tcx> for LeafFinder<'a, 'tcx> {
    type NestedFilter = rustc_middle::hir::nested_filter::OnlyBodies;

    fn maybe_tcx(&mut self) -> Self::MaybeTyCtxt {
        self.cx.tcx
    }

    fn visit_expr(&mut self, e: &'tcx hir::Expr<'tcx>) {
        if let ExprKind::Block(b, _) = e.kind {
            if matches!(b.rules, BlockCheckMode::UnsafeBlock(_)) {
                // Contents of an inner unsafe block do not require the
                // outer one.
                let saved = self.inside_unsafe_block;
                self.inside_unsafe_block = true;
                intravisit::walk_block(self, b);
                self.inside_unsafe_block = saved;
                return;
            }
        }
        if let Some((kind, movable)) = self.classify(e) {
            if !self.inside_unsafe_block {
                self.leaves.push(UnsafeLeaf { hir_id: e.hir_id, kind, movable });
            }
        }
        intravisit::walk_expr(self, e);
    }
}

impl<'a, 'tcx> LeafFinder<'a, 'tcx> {
    fn classify(&self, e: &'tcx hir::Expr<'tcx>) -> Option<(&'static str, bool)> {
        let cx = self.cx;
        match e.kind {
            ExprKind::Unary(hir::UnOp::Deref, inner) => {
                let ty = cx.typeck_results().expr_ty_opt(inner)?;
                if ty.is_raw_ptr() {
                    let movable = !self.parent_is_place_sensitive(e);
                    Some(("dereference of raw pointer", movable))
                } else {
                    None
                }
            }
            ExprKind::Call(f, _) => {
                let ty = cx.typeck_results().expr_ty_opt(f)?;
                match ty.kind() {
                    ty::FnDef(did, _) => {
                        if is_unsafe_callee(cx, *did) {
                            Some(("call to unsafe function", true))
                        } else {
                            None
                        }
                    }
                    ty::FnPtr(sig_tys, hdr) => {
                        if hdr.safety().is_unsafe() {
                            let _ = sig_tys;
                            Some(("call through unsafe function pointer", true))
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            }
            ExprKind::MethodCall(..) => {
                let did = cx.typeck_results().type_dependent_def_id(e.hir_id);
                match did {
                    Some(did) if is_unsafe_callee(cx, did) => {
                        Some(("call to unsafe method", true))
                    }
                    _ => None,
                }
            }
            ExprKind::Path(qpath) => {
                match cx.typeck_results().qpath_res(&qpath, e.hir_id) {
                    hir::def::Res::Def(
                        hir::def::DefKind::Static { mutability: Mutability::Mut, .. },
                        _,
                    ) => {
                        let movable = !self.parent_is_place_sensitive(e);
                        Some(("access to `static mut`", movable))
                    }
                    _ => None,
                }
            }
            ExprKind::Field(base, _) => {
                let ty = cx.typeck_results().expr_ty_opt(base)?;
                if let ty::Adt(def, _) = ty.kind() {
                    def.is_union().then(|| {
                        let movable = !self.parent_is_place_sensitive(e);
                        ("access to union field", movable)
                    })
                } else {
                    None
                }
            }
            ExprKind::InlineAsm(_) => Some(("inline assembly", true)),
            _ => None,
        }
    }

    /// `&*p`, `&raw const *p`, `*p = x`, `(*p).field` — the deref is a
    /// place and cannot be wrapped separately.
    fn parent_is_place_sensitive(&self, e: &hir::Expr<'_>) -> bool {
        for (_, node) in self.cx.tcx.hir_parent_iter(e.hir_id) {
            match node {
                Node::Expr(parent) => match parent.kind {
                    ExprKind::AddrOf(_, _, inner) => {
                        return inner.hir_id == e.hir_id;
                    }
                    ExprKind::Assign(lhs, _, _) | ExprKind::AssignOp(_, lhs, _) => {
                        return lhs.hir_id == e.hir_id;
                    }
                    ExprKind::Field(inner, _) => return inner.hir_id == e.hir_id,
                    _ => return false,
                },
                _ => continue,
            }
        }
        false
    }
}

fn is_unsafe_callee<'tcx>(cx: &LateContext<'tcx>, did: DefId) -> bool {
    let tcx = cx.tcx;
    tcx.fn_sig(did).skip_binder().safety().is_unsafe()
        || tcx.get_attrs(did, sym::target_feature).next().is_some()
}

fn leaves_in_expr<'a, 'tcx>(
    cx: &'a LateContext<'tcx>,
    e: &'tcx hir::Expr<'tcx>,
) -> Vec<UnsafeLeaf> {
    let mut f = LeafFinder { cx, leaves: Vec::new(), inside_unsafe_block: false };
    f.visit_expr(e);
    f.leaves
}

fn leaves_in_stmt<'a, 'tcx>(
    cx: &'a LateContext<'tcx>,
    s: &'tcx hir::Stmt<'tcx>,
) -> Vec<UnsafeLeaf> {
    let mut f = LeafFinder { cx, leaves: Vec::new(), inside_unsafe_block: false };
    f.visit_stmt(s);
    f.leaves
}

// -----------------------------------------------------------------------------
// The pass

pub struct UnsafeScope;

impl<'tcx> LateLintPass<'tcx> for UnsafeScope {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, e: &'tcx hir::Expr<'tcx>) {
        let ExprKind::Block(b, _) = e.kind else { return };
        if !matches!(b.rules, BlockCheckMode::UnsafeBlock(UnsafeSource::UserProvided)) {
            return;
        }

        // Skip work entirely where the lint is not active (e.g. dependencies
        // compiled with `--cap-lints allow`, or `#[allow]`'d scopes).
        let spec = cx.get_lint_level_spec(UNSAFE_SCOPE);
        if spec.is_allow() || spec.is_expect() {
            return;
        }

        let stmt_leaves: Vec<Vec<UnsafeLeaf>> =
            b.stmts.iter().map(|s| leaves_in_stmt(cx, s)).collect();
        let tail_leaves = b.expr.map(|t| leaves_in_expr(cx, t)).unwrap_or_default();
        let any_unsafe =
            !tail_leaves.is_empty() || stmt_leaves.iter().any(|l| !l.is_empty());
        if !any_unsafe {
            return; // entirely unnecessary — `unused_unsafe`'s job
        }

        // One diagnostic per block, emitted at the user's span. The primary
        // span must be local: macro-expanded tails (e.g. `format!`) can point
        // into a foreign crate, and emission there is silently dropped.
        let mut notes: Vec<(rustc_span::Span, String)> = Vec::new();

        // Safe padding statements around the unsafe core.
        for (s, leaves) in b.stmts.iter().zip(&stmt_leaves) {
            if leaves.is_empty() {
                notes.push((
                    s.span,
                    "statement does not require unsafe; move it out of the block".to_string(),
                ));
            }
        }

        // Tail expression larger than the unsafe operations it contains.
        if let Some(tail) = b.expr {
            if matches!(tail.kind, ExprKind::Block(_, _)) {
                return; // covered by its own check_expr invocation
            }
            let is_leaf_itself = tail_leaves.len() == 1 && tail_leaves[0].hir_id == tail.hir_id;
            if tail_leaves.is_empty() || is_leaf_itself {
                return;
            }
            let all_movable = tail_leaves.iter().all(|l| l.movable);
            if !all_movable && !tail_leaves.iter().any(|l| l.movable) {
                // Every unsafe operation is a place-expression that cannot
                // be individually wrapped; the block cannot shrink here.
                return;
            }
            notes.push((
                tail.span,
                if all_movable {
                    format!(
                        "unsafe block is larger than necessary: only {} wrapped \
                         sub-expression(s) require unsafe; wrap those instead, e.g. \
                         `safe_part + unsafe {{ *p }}`",
                        tail_leaves.len()
                    )
                } else {
                    "unsafe block is larger than necessary: only the operations noted \
                     below require unsafe; the marked place-expression must stay inside"
                        .to_string()
                },
            ));
            for l in &tail_leaves {
                notes.push((
                    cx.tcx.hir_span(l.hir_id),
                    format!("{} requires unsafe", l.kind),
                ));
            }
        }

        if notes.is_empty() {
            return;
        }
        cx.opt_span_lint(
            UNSAFE_SCOPE,
            Some(b.span),
            rustc_errors::DiagDecorator(|diag| {
                for (span, msg) in &notes {
                    diag.span_note(*span, msg.clone());
                }
            }),
        );
    }
}
