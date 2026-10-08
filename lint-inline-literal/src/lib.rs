//! `inline_literal` (issue servyi/lints#7): inline literals are
//! unnamed constants. The value is either a CHOICE (the name of an
//! environment variable, an initial buffer size, a delimiter) — then
//! it must have a name describing its semantics — or a necessity of
//! the computation — then name it anyway, or silence the site with
//! `#[allow(inline_literal)]`.
//!
//! Sanctioned inline positions:
//!
//! * message strings: string literals inside `Err(...)`, `assert!`,
//!   `println!`, the log macros, ... — failure/report prose is not a
//!   reusable constant;
//! * the trivial numerics: `+1`/`-1` (stepping), `*2`/`/2` (doubling),
//!   and a `0` argument of a call (zero-initialization arguments);
//! * `bool` literals — `true`/`false` spell their own semantics.
//!
//! Forbidden anywhere — even inside the sanctioned positions and in
//! const declarations: string literals containing another string
//! literal.
//!
//! Everything else must live in a `const`/`static` initializer or an
//! explicit enum discriminant (the naming sites).

#![feature(rustc_private)]
#![allow(internal_features)]
extern crate rustc_ast;
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_span;

use rustc_ast::LitKind;
use rustc_errors::DiagDecorator;
use rustc_hir as hir;
use rustc_hir::ExprKind;
use rustc_lint::{LateContext, LateLintPass, LintContext, declare_lint, impl_lint_pass};
use rustc_span::hygiene::ExpnKind;

declare_lint! {
    /// Checks that literals appear only in const declarations (or the
    /// sanctioned positions: message strings, the trivial numerics
    /// `+1`/`-1`, `*2`/`/2`, a `0` call argument, and bools). An inline
    /// literal is a constant without a name: if its value is a choice it
    /// needs a semantically named const; if it is a necessity the site
    /// needs an `#[allow]` (issue #7). String literals inside other
    /// string literals are forbidden everywhere.
    pub INLINE_LITERAL,
    Warn,
    "inline literal outside a const declaration (issue #7)"
}

pub fn register(store: &mut rustc_lint::LintStore) {
    store.register_lints(&[INLINE_LITERAL]);
    store.register_late_lint_pass(Box::new(|_| Box::new(Pass)));
}

pub struct Pass;

impl_lint_pass!(Pass => [INLINE_LITERAL]);

/// The macros whose string arguments are messages (report/failure
/// prose), per issue #7: `Err("...")`, `assert!`, `println!`, log, etc.
const MESSAGE_MACROS: &[&str] = &[
    "assert", "assert_eq", "assert_ne",
    "debug_assert", "debug_assert_eq", "debug_assert_ne",
    "panic", "unreachable", "todo", "unimplemented",
    "print", "println", "eprint", "eprintln",
    "format", "write", "writeln",
    "info", "debug", "trace", "warn", "error",
];

impl<'tcx> LateLintPass<'tcx> for Pass {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, e: &'tcx hir::Expr<'tcx>) {
        let ExprKind::Lit(lit) = &e.kind else { return };
        if !servyi_lint_core::is_active(cx, INLINE_LITERAL) {
            return;
        }
        match lit.node {
            LitKind::Bool(_) => {}
            LitKind::Str(..) | LitKind::ByteStr(..) | LitKind::CStr(..) => {
                // A string inside a string is forbidden ANYWHERE (issue
                // #7) — checked before the sanctioned positions.
                if string_contains_string(cx, e) {
                    emit(cx, e.span, "a string literal inside another string literal");
                    return;
                }
                if in_const_context(cx, e) || in_message_position(cx, e) {
                    return;
                }
                emit(cx, e.span, "a string literal outside message position");
            }
            LitKind::Char(_) | LitKind::Byte(_) => {
                if in_const_context(cx, e) {
                    return;
                }
                emit(cx, e.span, "a char/byte literal outside a const declaration");
            }
            LitKind::Int(v, _) => {
                if in_const_context(cx, e) || trivial_int(cx, e, v.get()) {
                    return;
                }
                emit(cx, e.span, "an integer literal outside a const declaration");
            }
            LitKind::Float(v, _) => {
                if in_const_context(cx, e) {
                    return;
                }
                // The compiler keeps the literal as its symbol; the
                // value comparison needs the parsed f64.
                if let Ok(value) = v.as_str().parse::<f64>()
                    && trivial_float(cx, e, value)
                {
                    return;
                }
                emit(cx, e.span, "a float literal outside a const declaration");
            }
            LitKind::Err(_) => {}
        }
    }
}

fn emit(cx: &LateContext<'_>, span: rustc_span::Span, what: &str) {
    cx.opt_span_lint(
        INLINE_LITERAL,
        Some(span),
        DiagDecorator(|diag| {
            diag.note(format!(
                "{what} (issue servyi/lints#7): a literal inline in code is a \
                 constant without a name. 1) If the value is a CHOICE (the name \
                 of an environment variable, an initial buffer size, a \
                 delimiter, ...), it must have a `const` whose name describes \
                 its semantics. 2) If the value is a NECESSITY (e.g. `+= 8` \
                 for counting boxes of eights), put it in a named const or \
                 silence the site with `#[allow(inline_literal)]`. Sanctioned \
                 inline: message strings (inside `Err(...)`, `assert!`, \
                 `println!`, the log macros, ...), `+1`/`-1`, `*2`/`/2`, a \
                 `0` argument of a call, and `bool` literals."
            ));
        }),
    );
}

/// True when the literal's file snippet carries a `"` beyond the
/// opening/closing delimiters — the escaped (`"a \"b\""`) and raw
/// (`r#"a"b"#`) shapes of a string inside a string alike.
fn string_contains_string(cx: &LateContext<'_>, e: &hir::Expr<'_>) -> bool {
    match cx.sess().source_map().span_to_snippet(e.span) {
        Ok(s) => s.matches('"').count() > 2,
        Err(_) => false,
    }
}

/// True inside a `const`/`static` initializer or an explicit enum
/// discriminant — the naming sites. Anonymous consts (array lengths,
/// const-generic arguments) are deliberately NOT naming sites: the
/// initial buffer size is the canonical CHOICE that needs a name.
fn in_const_context<'tcx>(cx: &LateContext<'tcx>, e: &hir::Expr<'tcx>) -> bool {
    for (_id, node) in cx.tcx.hir_parent_iter(e.hir_id) {
        match node {
            hir::Node::Item(it) => {
                return matches!(it.kind, hir::ItemKind::Const(..) | hir::ItemKind::Static(..));
            }
            hir::Node::TraitItem(ti) => {
                return matches!(ti.kind, hir::TraitItemKind::Const(..));
            }
            hir::Node::ImplItem(ii) => {
                return matches!(ii.kind, hir::ImplItemKind::Const(..));
            }
            hir::Node::Variant(v) => {
                // explicit enum discriminant
                return v.disr_expr.is_some();
            }
            // Anonymous const: array length, const-generic argument,
            // discriminant wrapper — keep walking to the owner, which
            // decides.
            hir::Node::AnonConst(_) => {}
            hir::Node::Expr(_) | hir::Node::Stmt(_) | hir::Node::Block(_)
            | hir::Node::Ty(_) => {}
            _ => return false,
        }
    }
    false
}

/// True for a string literal inside a message position: a message
/// macro (`assert!`, `println!`, the log macros, ...) anywhere up the
/// expansion chain, or a `Result::Err(...)` call anywhere up the
/// expression chain (the string may be wrapped — `.to_string()`,
/// `format!`, ... — as in `Err("...".to_string())`).
fn in_message_position<'tcx>(cx: &LateContext<'tcx>, e: &hir::Expr<'tcx>) -> bool {
    let debug = std::env::var_os("INLINE_LITERAL_DEBUG").is_some();
    // (a) inside a message macro. The literal's own hygiene is root
    // (format-args capture keeps the original span), so the check
    // walks the ANCESTORS' expansion chains: some ancestor of the
    // literal is the expansion root carrying the macro's context.
    for (_id, node) in cx.tcx.hir_parent_iter(e.hir_id) {
        if let hir::Node::Expr(pe) = node {
            for expn in pe.span.macro_backtrace() {
                if let ExpnKind::Macro(_, name) = expn.kind {
                    if debug {
                        eprintln!("inline_literal: ancestor macro: {}", name.as_str());
                    }
                    if MESSAGE_MACROS.contains(&name.as_str()) {
                        return true;
                    }
                }
            }
            // (b) inside a `Result::Err(...)`: the callee must resolve
            // to THE Err variant constructor of core's Result — user
            // enums with their own `Err` variant do not count.
            if let ExprKind::Call(callee, _) = pe.kind
                && let ExprKind::Path(qpath) = &callee.kind
                && let hir::def::Res::Def(
                    hir::def::DefKind::Ctor(hir::def::CtorOf::Variant, _),
                    did,
                ) = cx.typeck_results().qpath_res(qpath, callee.hir_id)
            {
                let variant = cx.tcx.parent(did);
                if debug {
                    eprintln!(
                        "inline_literal: ctor parent kind: {:?}, name: {}, enum: {}",
                        cx.tcx.def_kind(variant),
                        cx.tcx.item_name(variant),
                        cx.tcx.def_path_str(cx.tcx.parent(variant))
                    );
                }
                if cx.tcx.def_kind(variant) == hir::def::DefKind::Variant
                    && cx.tcx.item_name(variant) == rustc_span::Symbol::intern("Err")
                {
                    // def_path_str renders the reexport the user wrote:
                    // `core::result::Result` or `std::result::Result`.
                    let owner = cx.tcx.def_path_str(cx.tcx.parent(variant));
                    if debug {
                        eprintln!("inline_literal: enum owner path: {owner}");
                    }
                    if owner == "core::result::Result" || owner == "std::result::Result" {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// The trivial integers (issue #7): `1` under `+`/`-`/unary `-`
/// (stepping), `2` under `*`/`/` (doubling), `0` as an argument of a
/// call. Casts on the literal's side are transparent
/// (`foo(x, 0 as *const u8)`).
fn trivial_int<'tcx>(cx: &LateContext<'tcx>, e: &hir::Expr<'tcx>, value: u128) -> bool {
    match trivial_shape_full(cx, e) {
        Some(Shape::AddSub) => value == 1,
        Some(Shape::MulDiv) => value == 2,
        Some(Shape::CallArg) => value == 0,
        None => false,
    }
}

/// The same exceptions for floats, by value (`1.0`, `2.0`).
fn trivial_float<'tcx>(cx: &LateContext<'tcx>, e: &hir::Expr<'tcx>, value: f64) -> bool {
    match trivial_shape_full(cx, e) {
        Some(Shape::AddSub) => value == 1.0,
        Some(Shape::MulDiv) => value == 2.0,
        _ => false,
    }
}

enum Shape {
    AddSub,
    MulDiv,
    CallArg,
}

/// Walk up through casts; classify the first enclosing operator.
fn trivial_shape_full<'tcx>(cx: &LateContext<'tcx>, e: &hir::Expr<'tcx>) -> Option<Shape> {
    let mut child = e;
    loop {
        let (_id, node) = cx.tcx.hir_parent_iter(child.hir_id).next()?;
        let hir::Node::Expr(pe) = node else { return None };
        match pe.kind {
            ExprKind::Cast(inner, _) if inner.hir_id == child.hir_id => {
                child = pe;
            }
            ExprKind::Unary(hir::UnOp::Neg, inner) if inner.hir_id == child.hir_id => {
                return Some(Shape::AddSub); // -1 / -1.0
            }
            ExprKind::Binary(op, l, r)
                if l.hir_id == child.hir_id || r.hir_id == child.hir_id =>
            {
                return Some(match op.node {
                    hir::BinOpKind::Add | hir::BinOpKind::Sub => Shape::AddSub,
                    hir::BinOpKind::Mul | hir::BinOpKind::Div => Shape::MulDiv,
                    _ => return None,
                });
            }
            ExprKind::AssignOp(op, _lhs, rhs) if rhs.hir_id == child.hir_id => {
                return Some(match op.node {
                    hir::AssignOpKind::AddAssign | hir::AssignOpKind::SubAssign => Shape::AddSub,
                    hir::AssignOpKind::MulAssign | hir::AssignOpKind::DivAssign => Shape::MulDiv,
                    _ => return None,
                });
            }
            ExprKind::Call(_, args) => {
                return (args.iter().any(|a| a.hir_id == child.hir_id)).then_some(Shape::CallArg);
            }
            ExprKind::MethodCall(_, recv, args, _) => {
                return (recv.hir_id == child.hir_id
                    || args.iter().any(|a| a.hir_id == child.hir_id))
                    .then_some(Shape::CallArg);
            }
            _ => return None,
        }
    }
}
