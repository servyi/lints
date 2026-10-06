//! The shared-policy lints ride one driver:
//!
//! - `unsafe_scope`: enforce the strict shape of `unsafe` blocks — the
//!   block body must be reads of existing bindings plus exactly one
//!   operation that requires unsafe whose operands are those binding
//!   reads; everything else (argument computation, arithmetic, `let`)
//!   must be hoisted out and the unsafe blocks let-chained:
//!   `let q = unsafe { buf.add(i) }; unsafe { *q }`.
//! - `custom_parser` (issue #9): calls to typical hand-parsing
//!   primitives (`split_once`, `strip_prefix`, `trim_*_matches`, ...)
//!   must not appear outside a sanctioned custom-parser module — one
//!   headed by a `/// WARNING: CUSTOM PARSER ...` explanation with
//!   only `use` directives above it.
//! - `unsound_constructor` (issue #8): constructing an
//!   `#[unsound_constructor]` type carries SAFETY comments at the type
//!   and at every construction site.
//! - `non_test_panic_allow` (issue #6): `allow`/`expect` of the
//!   panic/unwrap lints is a test-only exemption.
//! - `workspace_lints_table`: the workspace manifest defines no lint
//!   tables (the shared policy is the CI flag list).
//!
//! Used as a `RUSTC_WRAPPER`: cargo invokes this binary in place of
//! rustc; it registers the extra late lints and otherwise behaves
//! exactly like the compiler it ships with (same nightly toolchain).
#![feature(rustc_private)]
#![allow(internal_features)]
extern crate rustc_ast;
extern crate rustc_driver;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_lexer;
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

declare_lint! {
    /// Checks that "typical parser functions" (string-parsing
    /// primitives people reach for when hand-rolling a parser —
    /// `split_once`, `strip_prefix`, `trim_matches`, ...; plain
    /// `split` is exempt) are only used inside a sanctioned
    /// custom-parser module: one headed by a
    /// `/// WARNING: CUSTOM PARSER ...` explanation with only `use`
    /// directives above it (issue #9). `#[allow(custom_parser)]`
    /// remains the per-site escape.
    pub CUSTOM_PARSER,
    Warn,
    "hand-rolled parser primitive used outside a WARNING: CUSTOM PARSER module"
}

declare_lint! {
    /// `#[allow]`/`#[expect]` of `clippy::panic`/`clippy::unwrap_used` is a
    /// test-only exemption: a panicking test IS the failure signal, but
    /// production code must handle or propagate the case instead of
    /// silencing the lint. The sanctioned forms —
    /// `#![cfg_attr(test, allow(...))]`, `#[cfg(test)]` modules,
    /// integration-test targets — never trip this: without `--test` the
    /// first is not expanded and the others are not compiled at all.
    pub NON_TEST_PANIC_ALLOW,
    Warn,
    "`allow`/`expect` of panic/unwrap lints outside test code"
}

declare_lint! {
    /// `#[unsound_constructor]` on a type marks its constructor (a struct
    /// literal) as unsound from safe code: constructing it can create
    /// aliasing references without any `unsafe` block at the site. The
    /// attribute is only legal together with (a) a `/// SAFETY` comment
    /// right above the struct explaining the constructor's safety
    /// preconditions, and (b) a `// SAFETY` comment directly inside every
    /// place that constructs the type, arguing why those preconditions
    /// hold there. This lint enforces both.
    pub UNSOUND_CONSTRUCTOR,
    Warn,
    "construction of an #[unsound_constructor] type must carry SAFETY comments"
}

declare_lint! {
    /// Checks that the workspace-level Cargo.toml defines no lint tables.
    /// The shared Servyi policy is the explicit flag list the CI runs
    /// (servyi/lints lint-policy.yml); a manifest table is a second place
    /// to configure lints that silently drifts from it.
    pub WORKSPACE_LINTS_TABLE,
    Warn,
    "the workspace-level Cargo.toml defines lints"
}

impl_lint_pass!(UnsafeScope => [UNSAFE_SCOPE, CUSTOM_PARSER, UNSOUND_CONSTRUCTOR, WORKSPACE_LINTS_TABLE, NON_TEST_PANIC_ALLOW]);

struct LintCallbacks;

impl rustc_driver::Callbacks for LintCallbacks {
    fn config(&mut self, config: &mut rustc_interface::Config) {
        let previous = config.register_lints.take();
        config.register_lints = Some(Box::new(move |sess, store| {
            if let Some(prev) = &previous {
                prev(sess, store);
            }
            store.register_lints(&[UNSAFE_SCOPE, CUSTOM_PARSER, UNSOUND_CONSTRUCTOR, WORKSPACE_LINTS_TABLE, NON_TEST_PANIC_ALLOW]);
            store.register_late_lint_pass(Box::new(|_| Box::new(UnsafeScope { unsound_types: Vec::new() })));
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

impl UnsafeScope {
    /// True when a SAFETY comment sits in the contiguous comment block
    /// directly above `span` (attributes between it and the span are
    /// skipped). Comment extraction uses `rustc_lexer` — the compiler's
    /// own lexer — so strings, chars, and nested comments cannot produce
    /// false positives.
    fn has_safety_comment(&self, cx: &LateContext<'_>, span: rustc_span::Span) -> bool {
        let sm = cx.tcx.sess.source_map();
        let Ok(prefix) = sm.span_to_prev_source(span) else { return false };
        // Token offsets are cumulative over `Token::len`. Only tokens on
        // EARLIER LINES count: when the span starts mid-line (`let h =
        // Struct { .. }`), the `let`/ident/`=` tokens of its own line
        // would break the backward walk before it can reach the comment
        // block attached to the STATEMENT above.
        let cut = prefix.rfind('\n').unwrap_or(0);
        let mut tokens: Vec<(std::ops::Range<usize>, rustc_lexer::TokenKind)> = Vec::new();
        let mut pos = 0usize;
        for t in rustc_lexer::tokenize(&prefix, rustc_lexer::FrontmatterAllowed::No) {
            let start = pos;
            pos += t.len as usize;
            if pos <= cut {
                tokens.push((start..pos, t.kind));
            }
        }
        let mut iter = tokens.into_iter().rev().peekable();
        while let Some((range, kind)) = iter.next() {
            match kind {
                rustc_lexer::TokenKind::Whitespace => continue,
                rustc_lexer::TokenKind::LineComment { doc_style: _ }
                | rustc_lexer::TokenKind::BlockComment { doc_style: _, terminated: _ } => {
                    // Contiguous comment block: any line in it may carry
                    // the SAFETY note; keep walking through plain lines.
                    if prefix[range].contains("SAFETY") {
                        return true;
                    }
                }
                // An attribute chain between the comment and the span is
                // fine (e.g. `#[servyi::unsound_constructor]` above the
                // struct): skip backwards through the balanced brackets.
                rustc_lexer::TokenKind::CloseBracket => {
                    let mut depth = 1usize;
                    for (r2, k2) in iter.by_ref() {
                        match k2 {
                            rustc_lexer::TokenKind::CloseBracket => depth += 1,
                            rustc_lexer::TokenKind::OpenBracket => {
                                depth -= 1;
                                if depth == 0 {
                                    // also skip the `#` (and `!`) of the attribute
                                    while let Some((_, k3)) = iter.peek() {
                                        match k3 {
                                            rustc_lexer::TokenKind::Pound
                                            | rustc_lexer::TokenKind::Bang
                                            | rustc_lexer::TokenKind::Whitespace => {
                                                let _ = r2;
                                                iter.next();
                                            }
                                            _ => break,
                                        }
                                    }
                                    break;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                _ => break,
            }
        }
        self.has_safety_comment_inside(sm, span)
    }

    /// A SAFETY comment INSIDE the expression's own braces (next to the
    /// fields) also counts; tokenized with the same lexer.
    fn has_safety_comment_inside(
        &self,
        sm: &rustc_span::source_map::SourceMap,
        span: rustc_span::Span,
    ) -> bool {
        match sm.span_to_snippet(span) {
            Ok(s) => {
                let mut pos = 0usize;
                rustc_lexer::tokenize(&s, rustc_lexer::FrontmatterAllowed::No).any(|t| {
                    let hit = matches!(
                        t.kind,
                        rustc_lexer::TokenKind::LineComment { doc_style: _ }
                            | rustc_lexer::TokenKind::BlockComment { doc_style: _, terminated: _ }
                    ) && s[pos..pos + t.len as usize].contains("SAFETY");
                    pos += t.len as usize;
                    hit
                })
            }
            Err(_) => false,
        }
    }
}

fn _unused(_: &[(std::ops::Range<usize>, rustc_lexer::TokenKind)]) {}

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
        if let ExprKind::Block(b, _) = e.kind
            && matches!(b.rules, BlockCheckMode::UnsafeBlock(_))
        {
            // Contents of an inner unsafe block do not require the
            // outer one.
            let saved = self.inside_unsafe_block;
            self.inside_unsafe_block = true;
            intravisit::walk_block(self, b);
            self.inside_unsafe_block = saved;
            return;
        }
        if let Some(kind) = self.classify(e)
            && !self.inside_unsafe_block
        {
            self.leaves.push(UnsafeLeaf { hir_id: e.hir_id, kind });
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

// Querying attributes by DefId: the parsed-attrs replacement for
// `get_attrs` does not cover this shape on the pinned nightly.
#[allow(deprecated)]
fn is_unsafe_callee<'tcx>(cx: &LateContext<'tcx>, did: DefId) -> bool {
    let tcx = cx.tcx;
    tcx.fn_sig(did).skip_binder().safety().is_unsafe()
        || tcx.get_attrs(did, sym::target_feature).next().is_some()
}

fn leaves_in_expr<'tcx>(
    cx: &LateContext<'tcx>,
    e: &'tcx hir::Expr<'tcx>,
) -> Vec<UnsafeLeaf> {
    let mut f = LeafFinder { cx, leaves: Vec::new(), inside_unsafe_block: false };
    f.visit_expr(e);
    f.leaves
}

fn leaves_in_stmt<'tcx>(
    cx: &LateContext<'tcx>,
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
// Manifest hygiene: the workspace-level Cargo.toml must define no lints.

/// Walk up from `manifest_dir` to the workspace-level Cargo.toml the way
/// cargo does: skip directories without a manifest, stop at the nearest
/// ancestor manifest with a `[workspace]` table (the workspace root), or
/// at an ancestor package manifest without one (a nested package — the
/// crate stands alone, its own manifest is the root). Return that
/// manifest when it defines lints (`[workspace.lints]` or a `[lints]`
/// table beyond a plain `workspace = true` inheritance marker).
fn conflicting_workspace_manifest_from(
    manifest_dir: &std::path::Path,
) -> Option<std::path::PathBuf> {
    let mut own: Option<(std::path::PathBuf, toml::Table)> = None;
    let mut dir = Some(manifest_dir);
    while let Some(d) = dir {
        let manifest = d.join("Cargo.toml");
        if let Ok(src) = std::fs::read_to_string(&manifest)
            && let Ok(cfg) = src.parse::<toml::Table>()
        {
            if cfg.contains_key("workspace") {
                return defines_lints(&cfg).then_some(manifest);
            }
            if d == manifest_dir {
                own = Some((manifest, cfg));
            } else {
                break; // nested package without [workspace]
            }
        }
        dir = d.parent();
    }
    own.and_then(|(manifest, cfg)| defines_lints(&cfg).then_some(manifest))
}

fn defines_lints(cfg: &toml::Table) -> bool {
    let workspace_lints = cfg
        .get("workspace")
        .and_then(|w| w.get("lints"))
        .and_then(|l| l.as_table())
        .is_some_and(|t| !t.is_empty());
    let package_lints = cfg
        .get("lints")
        .and_then(|l| l.as_table())
        .is_some_and(|t| !(t.len() == 1 && t.contains_key("workspace")));
    workspace_lints || package_lints
}

fn conflicting_workspace_manifest() -> Option<std::path::PathBuf> {
    // Only cargo sets this; plain rustc invocations have no manifest.
    let manifest_dir = std::env::var_os("CARGO_MANIFEST_DIR")?;
    conflicting_workspace_manifest_from(std::path::Path::new(&manifest_dir))
}

#[cfg(test)]
mod tests {
    use super::conflicting_workspace_manifest_from;
    use std::path::{Path, PathBuf};

    fn scratch(name: &str, files: &[(&str, &str)]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "unsafe-scope-lint-test-{}-{}",
            name,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        for (rel, content) in files {
            let p = dir.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, content).unwrap();
        }
        dir
    }

    fn member(dir: &Path) -> PathBuf {
        dir.join("crates/foo")
    }

    #[test]
    fn clean_workspace_is_silent() {
        let dir = scratch(
            "clean",
            &[
                ("Cargo.toml", "[workspace]\nmembers = [\"crates/foo\"]\n"),
                ("crates/foo/Cargo.toml", "[package]\nname = \"foo\"\n"),
            ],
        );
        assert!(conflicting_workspace_manifest_from(&member(&dir)).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn workspace_lints_table_is_flagged() {
        let dir = scratch(
            "ws-lints",
            &[
                (
                    "Cargo.toml",
                    "[workspace]\nmembers = [\"crates/foo\"]\n\n[workspace.lints.rust]\nunused = \"deny\"\n",
                ),
                ("crates/foo/Cargo.toml", "[package]\nname = \"foo\"\n"),
            ],
        );
        let hit = conflicting_workspace_manifest_from(&member(&dir)).unwrap();
        assert_eq!(hit, dir.join("Cargo.toml"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn package_lints_in_root_are_flagged() {
        let dir = scratch(
            "pkg-lints",
            &[
                (
                    "Cargo.toml",
                    "[workspace]\nmembers = [\"crates/foo\"]\n\n[lints.rust]\nunused = \"deny\"\n",
                ),
                ("crates/foo/Cargo.toml", "[package]\nname = \"foo\"\n"),
            ],
        );
        assert!(conflicting_workspace_manifest_from(&member(&dir)).is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn single_crate_root_is_checked() {
        let dir = scratch(
            "single",
            &[
                ("src/main.rs", "fn main() {}\n"),
                (
                    "Cargo.toml",
                    "[package]\nname = \"foo\"\n\n[lints.clippy]\nunwrap_used = \"deny\"\n",
                ),
            ],
        );
        let hit = conflicting_workspace_manifest_from(&dir).unwrap();
        assert_eq!(hit, dir.join("Cargo.toml"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn pure_inheritance_marker_is_not_a_definition() {
        // A root manifest whose [lints] is only the inheritance marker is
        // meaningless but harmless; only real definitions are flagged.
        let dir = scratch(
            "marker",
            &[
                ("Cargo.toml", "[package]\nname = \"foo\"\n\n[lints]\nworkspace = true\n"),
                ("src/main.rs", "fn main() {}\n"),
            ],
        );
        assert!(conflicting_workspace_manifest_from(&dir).is_none());
        let _ = std::fs::remove_dir_all(&dir);
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

pub struct UnsafeScope {
    /// DefIds of structs marked #[servyi::unsound_constructor], collected
    /// by `check_item` (which the HIR traversal visits before the bodies
    /// that construct them).
    unsound_types: Vec<DefId>,
}

// -----------------------------------------------------------------------------
// custom_parser (issue #9)

/// The "typical functions used for parsing strings" (plain `split` is
/// deliberately absent — the one accepted everyday idiom). Def-path
/// SUFFIXES against the resolved callee (inherent impls resolve to
/// e.g. `core::str::<impl str>::split_once`), with the human-facing
/// name for the diagnostic.
const PARSER_PRIMITIVES: &[(&str, &str)] = &[
    // cursor-style consumption and delimiting
    ("<impl str>::split_once", "str::split_once"),
    ("<impl str>::rsplit_once", "str::rsplit_once"),
    ("<impl str>::split_terminator", "str::split_terminator"),
    ("<impl str>::rsplit_terminator", "str::rsplit_terminator"),
    ("<impl str>::splitn", "str::splitn"),
    ("<impl str>::rsplitn", "str::rsplitn"),
    ("<impl str>::split_at", "str::split_at"),
    ("<impl [T]>::split_at", "slice::split_at"),
    ("<impl [T]>::split_at_mut", "slice::split_at_mut"),
    ("<impl str>::strip_prefix", "str::strip_prefix"),
    ("<impl str>::strip_suffix", "str::strip_suffix"),
    // custom-delimiter stripping (whitespace has trim())
    ("<impl str>::trim_matches", "str::trim_matches"),
    ("<impl str>::trim_start_matches", "str::trim_start_matches"),
    ("<impl str>::trim_end_matches", "str::trim_end_matches"),
    // prefix sniffing / whitespace-run stripping by hand (review on #11:
    // starts_with and trim_start are the same hand-rolling family)
    ("<impl str>::starts_with", "str::starts_with"),
    ("<impl str>::trim_start", "str::trim_start"),
    // scanner idioms
    ("<impl str>::match_indices", "str::match_indices"),
    ("<impl str>::char_indices", "str::char_indices"),
];

/// The sanctioned-module header (issue #9): a `/// WARNING: CUSTOM
/// PARSER` explanation at the top of the file, with only `use`
/// directives (and crate attributes / blank lines) above it.
const PARSER_MARKER: &str = "WARNING: CUSTOM PARSER";

fn check_parser_primitive<'tcx>(cx: &LateContext<'tcx>, e: &'tcx hir::Expr<'tcx>) {
    // Skip where the lint is not active (`--cap-lints allow` for
    // dependencies, `#[allow]`'d scopes).
    let spec = cx.get_lint_level_spec(CUSTOM_PARSER);
    if spec.is_allow() || spec.is_expect() {
        return;
    }
    let path = match &e.kind {
        hir::ExprKind::MethodCall(..) => {
            let Some(def) = cx.typeck_results().type_dependent_def_id(e.hir_id) else {
                return;
            };
            cx.tcx.def_path_str(def)
        }
        hir::ExprKind::Call(callee, _) => {
            let hir::ExprKind::Path(qpath) = &callee.kind else { return };
            match cx.typeck_results().qpath_res(qpath, callee.hir_id) {
                hir::def::Res::Def(_, def) => cx.tcx.def_path_str(def),
                _ => return,
            }
        }
        _ => return,
    };
    if std::env::var_os("CUSTOM_PARSER_DEBUG").is_some() {
        eprintln!("custom_parser: resolved call: {path}");
    }
    let Some(&(_, why)) = PARSER_PRIMITIVES.iter().find(|(p, _)| path.ends_with(p)) else {
        return;
    };
    if file_has_custom_parser_header(cx, e.span) {
        return;
    }
    cx.opt_span_lint(
        CUSTOM_PARSER,
        Some(e.span),
        rustc_errors::DiagDecorator(|diag| {
            diag.note(format!(
                "`{path}` is a typical hand-parsing primitive (issue servyi/lints#9): \
                 first check that no std function or external crate already does this \
                 parsing job. If hand-rolling the parser is necessary, agree on the \
                 approach and design with a human supervisor, then isolate it in a \
                 submodule whose header carries `/// {PARSER_MARKER} ...` explaining why \
                 it must be hand-written (only `use` directives above the explanation). \
                 `{why}` was the matched primitive."
            ));
        }),
    );
}

/// Does the file containing `span` carry the sanctioned header? The
/// marker line is the FIRST `///` line of the file; every line above
/// it must be blank, a `use` directive, or a crate attribute.
fn file_has_custom_parser_header(cx: &LateContext<'_>, span: rustc_span::Span) -> bool {
    use std::io::BufRead as _;
    let filename = cx.sess().source_map().span_to_filename(span);
    let rustc_span::FileName::Real(real) = &filename else {
        return false; // macro expansions / synthetic spans: no header
    };
    let Some(path) = real.local_path() else {
        return false; // remapped/virtual path (e.g. rustc internals)
    };
    let Ok(file) = std::fs::File::open(path) else {
        return false;
    };
    for line in std::io::BufReader::new(file).lines() {
        let Ok(line) = line else { return false };
        let t = line.trim_start();
        if t.is_empty() || t.starts_with("use ") || t.starts_with("#![") {
            continue;
        }
        if t.starts_with("///") {
            return t.contains(PARSER_MARKER);
        }
        // Anything else before the first `///` doc: not a sanctioned
        // header (the explanation must sit at the top, above the
        // module's items).
        return false;
    }
    false
}

impl<'tcx> LateLintPass<'tcx> for UnsafeScope {
    fn check_crate(&mut self, cx: &LateContext<'tcx>) {
        // Skip where the lint is not active (`--cap-lints allow` for
        // dependencies, `#[allow]`'d scopes).
        let spec = cx.get_lint_level_spec(WORKSPACE_LINTS_TABLE);
        if spec.is_allow() || spec.is_expect() {
            return;
        }
        if let Some(manifest) = conflicting_workspace_manifest() {
            cx.opt_span_lint(
                WORKSPACE_LINTS_TABLE,
                None::<rustc_span::Span>,
                rustc_errors::DiagDecorator(|diag| {
                    diag.note(format!(
                        "`{}` defines lints; the workspace-level cargo.toml could conflict \
                         with the actual CI toml, resulting in confusion",
                        manifest.display()
                    ));
                }),
            );
        }
    }

    fn check_attribute(&mut self, cx: &LateContext<'tcx>, attr: &'tcx rustc_hir::attrs::Attribute) {
        // Skip where the lint is not active (`--cap-lints allow` for
        // dependencies, `#[allow]`'d scopes).
        let spec = cx.get_lint_level_spec(NON_TEST_PANIC_ALLOW);
        if spec.is_allow() || spec.is_expect() {
            return;
        }
        // In a --test compilation the sanctioned cfg_attr(test, ...)
        // forms have expanded into plain allows: those are fine.
        if cx.tcx.sess.is_test_crate() {
            return;
        }
        let kind = match attr.name() {
            Some(n) if n == sym::allow => "allow",
            Some(n) if n == sym::expect => "expect",
            _ => return,
        };
        let Some(items) = attr.meta_item_list() else { return };
        for item in &items {
            let Some(mi) = item.meta_item() else { continue };
            let segments: Vec<_> = mi.path.segments.iter().map(|s| s.ident.name.as_str()).collect();
            let last = segments.last().copied().unwrap_or_default();
            if last != "panic" && last != "unwrap_used" {
                continue;
            }
            let scoped = match segments.as_slice() {
                [.., "clippy", last] => *last,
                [last] => *last,
                _ => continue,
            };
            cx.opt_span_lint(
                NON_TEST_PANIC_ALLOW,
                Some(mi.path.span),
                rustc_errors::DiagDecorator(|diag| {
                    diag.note(format!(
                        "`{kind}({scoped})` silences the panic policy outside test code: \
                         a panicking test is the failure signal, but production code must \
                         handle or propagate the case instead; gate exemptions with \
                         `cfg_attr(test, ...)` or move them into `#[cfg(test)]` code"
                    ));
                }),
            );
        }
    }

    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx hir::Item<'tcx>) {
        let hir::ItemKind::Struct(..) = item.kind else { return };
        let attrs = cx.tcx.hir_attrs(rustc_hir::HirId::make_owner(item.owner_id.def_id));
        let marked = attrs.iter().any(|a| {
            a.path_matches(&[rustc_span::Symbol::intern("servyi"), rustc_span::Symbol::intern("unsound_constructor")])
        });
        if !marked {
            return;
        }
        self.unsound_types.push(item.owner_id.to_def_id());
        let documented = self.has_safety_comment(cx, item.span);
        if !documented {
            cx.opt_span_lint(
                UNSOUND_CONSTRUCTOR,
                Some(item.span),
                rustc_errors::DiagDecorator(|diag| {
                    diag.note(format!(
                        "`{}` is #[unsound_constructor]: document the constructor's \
                         safety preconditions in a SAFETY comment right above the \
                         struct (either `///`/`//` line comments or a `/* */` block \
                         comment). Spell out: WHAT can go wrong if constructed \
                         wrongly (e.g. aliasing references), and WHAT a construction \
                         site must prove about its arguments for the construction to \
                         be sound (the type invariant)",
                        cx.tcx.item_name(item.owner_id.to_def_id())
                    ));
                }),
            );
        }
    }

    fn check_expr(&mut self, cx: &LateContext<'tcx>, e: &'tcx hir::Expr<'tcx>) {
        check_parser_primitive(cx, e);

        if let ExprKind::Struct(qpath, _, _) = e.kind
            && let hir::def::Res::Def(hir::def::DefKind::Struct, did) =
                cx.typeck_results().qpath_res(&qpath, e.hir_id)
            && self.unsound_types.contains(&did)
            && !self.has_safety_comment(cx, e.span)
        {
            cx.opt_span_lint(
                UNSOUND_CONSTRUCTOR,
                Some(e.span),
                rustc_errors::DiagDecorator(|diag| {
                    diag.note(format!(
                        "construction of `{}` is #[unsound_constructor]: argue, for \
                         THIS site, why the constructor's safety preconditions hold — \
                         a SAFETY comment (line `//` or block `/* */` style) either \
                         directly above the literal or inside its braces, next to the \
                         fields. Say what makes the arguments satisfy the type \
                         invariant (owned, exclusive, disjoint, ...)",
                        cx.tcx.item_name(did)
                    ));
                }),
            );
        }
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
