//! `unsafe_scope`: enforce the strict shape of `unsafe` blocks — the block
//! body must be reads of existing bindings plus exactly one operation that
//! requires unsafe whose operands are those binding reads; everything else
//! (argument computation, arithmetic, `let`) must be hoisted out and the
//! unsafe blocks let-chained: `let q = unsafe { buf.add(i) }; unsafe { *q }`.
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
use rustc_hir::{BlockCheckMode, ExprKind, HirId, UnsafeSource};
use rustc_lint::{LateContext, LateLintPass, LintContext, declare_lint, impl_lint_pass};
use rustc_middle::ty::{self};
use rustc_hir::def_id::DefId;
use rustc_span::sym;

declare_lint! {
    /// Checks that `unsafe` blocks contain only reads of existing bindings
    /// and a single unsafe operation with binding operands. Anything else
    /// hides which operations are actually unsafe and where their values
    /// come from.
    pub UNSAFE_SCOPE,
    Warn,
    "unsafe block body must be binding reads plus a single unsafe operation"
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

#[derive(Clone)]
struct UnsafeLeaf {
    hir_id: HirId,
    kind: &'static str,
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
        if let Some(kind) = self.classify(e) {
            if !self.inside_unsafe_block {
                self.leaves.push(UnsafeLeaf { hir_id: e.hir_id, kind });
            }
        }
        intravisit::walk_expr(self, e);
    }
}

impl<'a, 'tcx> LeafFinder<'a, 'tcx> {
    fn classify(&self, e: &'tcx hir::Expr<'tcx>) -> Option<&'static str> {
        let cx = self.cx;
        match e.kind {
            ExprKind::Unary(hir::UnOp::Deref, inner) => {
                let ty = cx.typeck_results().expr_ty_opt(inner)?;
                ty.is_raw_ptr().then_some("dereference of raw pointer")
            }
            ExprKind::Call(f, _) => {
                let ty = cx.typeck_results().expr_ty_opt(f)?;
                match ty.kind() {
                    ty::FnDef(did, _) => {
                        is_unsafe_callee(cx, *did).then_some("call to unsafe function")
                    }
                    ty::FnPtr(sig_tys, hdr) => {
                        let _ = sig_tys;
                        hdr.safety()
                            .is_unsafe()
                            .then_some("call through unsafe function pointer")
                    }
                    _ => None,
                }
            }
            ExprKind::MethodCall(..) => {
                let did = cx.typeck_results().type_dependent_def_id(e.hir_id);
                did.filter(|did| is_unsafe_callee(cx, *did))
                    .map(|_| "call to unsafe method")
            }
            ExprKind::Path(qpath) => {
                match cx.typeck_results().qpath_res(&qpath, e.hir_id) {
                    hir::def::Res::Def(
                        hir::def::DefKind::Static { mutability: Mutability::Mut, .. },
                        _,
                    ) => Some("access to `static mut`"),
                    _ => None,
                }
            }
            ExprKind::Field(base, _) => {
                let ty = cx.typeck_results().expr_ty_opt(base)?;
                match ty.kind() {
                    ty::Adt(def, _) if def.is_union() => Some("access to union field"),
                    _ => None,
                }
            }
            ExprKind::InlineAsm(_) => Some("inline assembly"),
            _ => None,
        }
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

/// A "binding read": a place expression rooted at an existing local binding,
/// built only from field accesses, shared/mutable references taken to such
/// places, and dereferences of references (never raw pointers). Everything
/// else — calls, arithmetic, casts, literals, `let` — must be hoisted out of
/// an unsafe block.
fn is_binding_place<'tcx>(cx: &LateContext<'tcx>, e: &'tcx hir::Expr<'tcx>) -> bool {
    match e.kind {
        ExprKind::Path(qpath) => matches!(
            cx.typeck_results().qpath_res(&qpath, e.hir_id),
            hir::def::Res::Local(_)
        ),
        ExprKind::Field(base, _) => is_binding_place(cx, base),
        // Taking a reference to a binding read is itself a pure read
        // (`&x`, `&x.f`); `&raw`/`&*p` forms fall through the deref arm.
        ExprKind::AddrOf(_, _, inner) => is_binding_place(cx, inner),
        ExprKind::Unary(hir::UnOp::Deref, inner) => {
            // Dereferencing a raw pointer is itself the unsafe operation;
            // only reference derefs count as safe place reads.
            let is_ref = cx
                .typeck_results()
                .expr_ty_opt(inner)
                .is_some_and(|ty| matches!(ty.kind(), ty::Ref(..)));
            is_ref && is_binding_place(cx, inner)
        }
        _ => false,
    }
}

/// True when `e` is an ancestor expression of `leaf_hir_id`.
fn contains_leaf<'tcx>(cx: &LateContext<'tcx>, e: &'tcx hir::Expr<'tcx>, leaf_hir_id: HirId) -> bool {
    e.hir_id == leaf_hir_id || cx.tcx.hir_parent_iter(leaf_hir_id).any(|(id, _)| id == e.hir_id)
}

/// Walk a body expression down to its single unsafe operation, allowing only
/// place layers around the leaf: `&leaf`, `leaf.field`, reference derefs, and
/// `place_leaf = binding_read` assignments. Returns the leaf expression, or
/// None (with a note already pushed) when the shape is not allowed.
fn unwrap_to_leaf<'tcx>(
    cx: &LateContext<'tcx>,
    e: &'tcx hir::Expr<'tcx>,
    leaf_hir_id: HirId,
    notes: &mut Vec<(rustc_span::Span, String)>,
) -> Option<&'tcx hir::Expr<'tcx>> {
    if e.hir_id == leaf_hir_id {
        return Some(e);
    }
    match e.kind {
        // `&*p`, `(*p).field`, and reference-deref layers stay part of the
        // same place operation.
        ExprKind::AddrOf(_, _, inner)
        | ExprKind::Field(inner, _)
        | ExprKind::Unary(hir::UnOp::Deref, inner) => {
            if matches!(e.kind, ExprKind::Unary(hir::UnOp::Deref, _)) {
                let is_ref = cx
                    .typeck_results()
                    .expr_ty_opt(inner)
                    .is_some_and(|ty| matches!(ty.kind(), ty::Ref(..)));
                if !is_ref {
                    return None;
                }
            }
            unwrap_to_leaf(cx, inner, leaf_hir_id, notes)
        }
        // `*p = binding_read` / `*p += binding_read`: the assignment target
        // side carries the unsafe operation, the value side is a plain read.
        ExprKind::Assign(lhs, rhs, _) | ExprKind::AssignOp(_, lhs, rhs) => {
            let (place, value) = if contains_leaf(cx, lhs, leaf_hir_id) {
                (lhs, rhs)
            } else if contains_leaf(cx, rhs, leaf_hir_id) {
                (rhs, lhs)
            } else {
                return None;
            };
            let leaf = unwrap_to_leaf(cx, place, leaf_hir_id, notes)?;
            if !is_binding_place(cx, value) {
                notes.push((
                    value.span,
                    format!(
                        "{SHAPE_MSG}: the value side must be a plain read of an \
                         existing binding"
                    ),
                ));
            }
            Some(leaf)
        }
        _ => None,
    }
}

const SHAPE_MSG: &str = "unsafe block body must be a read of an existing binding, \
     or a single unsafe operation whose operands are existing bindings; \
     hoist everything else out and let-chain the unsafe blocks";

fn check_body_expr<'tcx>(
    cx: &LateContext<'tcx>,
    e: &'tcx hir::Expr<'tcx>,
    leaves: &[UnsafeLeaf],
    notes: &mut Vec<(rustc_span::Span, String)>,
) {
    if matches!(e.kind, ExprKind::Block(_, _)) {
        return; // inner block: covered by its own check_expr invocation
    }
    if leaves.is_empty() {
        if !is_binding_place(cx, e) {
            notes.push((
                e.span,
                format!("{SHAPE_MSG}: this expression does not require unsafe"),
            ));
        }
        return;
    }
    if leaves.len() == 1 {
        if let Some(leaf) = unwrap_to_leaf(cx, e, leaves[0].hir_id, notes) {
            check_leaf_operands(cx, leaf, notes);
        } else {
            notes.push((e.span, SHAPE_MSG.to_string()));
        }
        return;
    }
    notes.push((e.span, SHAPE_MSG.to_string()));
    for l in leaves {
        notes.push((
            cx.tcx.hir_span(l.hir_id),
            format!("{} requires unsafe", l.kind),
        ));
    }
}

/// The unsafe operation's operands must be plain reads of existing bindings.
fn check_leaf_operands<'tcx>(
    cx: &LateContext<'tcx>,
    leaf: &'tcx hir::Expr<'tcx>,
    notes: &mut Vec<(rustc_span::Span, String)>,
) {
    let mut operands: Vec<&hir::Expr<'tcx>> = Vec::new();
    match leaf.kind {
        ExprKind::Unary(hir::UnOp::Deref, inner) => operands.push(inner),
        ExprKind::Field(base, _) => operands.push(base),
        ExprKind::Call(_, args) => operands.extend(args),
        ExprKind::MethodCall(_, receiver, args, _) => {
            operands.push(receiver);
            operands.extend(args);
        }
        ExprKind::Path(_) | ExprKind::InlineAsm(_) => {}
        _ => {}
    }
    for op in operands {
        if !is_binding_place(cx, op) {
            notes.push((
                op.span,
                format!(
                    "{SHAPE_MSG}: operand must be a plain read of an existing \
                     binding; hoist it out of the unsafe block"
                ),
            ));
        }
    }
}

// -----------------------------------------------------------------------------
// The pass
//
// Strict shape model: inside an `unsafe` block only
//   * reads of bindings that already exist (place expressions rooted at a
//     local, built from field accesses and reference derefs), and
//   * exactly one operation that requires unsafe, whose operands are such
//     binding reads,
// may appear. Anything else — computing arguments, arithmetic, casts, `let`
// bindings — must be hoisted out of the block (let-chain style):
//
//     let idx = compute(x);
//     let q = unsafe { buf.add(idx) };
//     let v = unsafe { *q };
//
// Blocks containing no operation that requires unsafe at all are left to
// rustc's builtin `unused_unsafe` lint.

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
        let total: usize = stmt_leaves.iter().map(Vec::len).sum::<usize>() + tail_leaves.len();
        if total == 0 {
            return; // entirely unnecessary — `unused_unsafe`'s job
        }

        // One diagnostic per block, emitted at the user's span. The primary
        // span must be local: macro-expanded tails (e.g. `format!`) can point
        // into a foreign crate, and emission there is silently dropped.
        let mut notes: Vec<(rustc_span::Span, String)> = Vec::new();

        for (s, leaves) in b.stmts.iter().zip(&stmt_leaves) {
            match s.kind {
                hir::StmtKind::Expr(e) | hir::StmtKind::Semi(e) => {
                    check_body_expr(cx, e, leaves, &mut notes);
                }
                // `let` and item statements introduce definitions; hoist them.
                _ => {
                    notes.push((s.span, SHAPE_MSG.to_string()));
                }
            }
        }
        if let Some(tail) = b.expr {
            check_body_expr(cx, tail, &tail_leaves, &mut notes);
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
