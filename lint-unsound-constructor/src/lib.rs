//! `unsound_constructor` (issue servyi/lints#8): constructing an
//! `#[unsound_constructor]` type carries SAFETY comments at the type
//! and at every construction site.

#![feature(rustc_private)]
#![allow(internal_features)]
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_lexer;
extern crate rustc_span;

mod safety_scan;

use rustc_errors::DiagDecorator;
use rustc_hir as hir;
use rustc_hir::ExprKind;
use rustc_hir::def_id::DefId;
use rustc_lint::{LateContext, LateLintPass, LintContext, declare_lint, impl_lint_pass};

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

pub fn register(store: &mut rustc_lint::LintStore) {
    store.register_lints(&[UNSOUND_CONSTRUCTOR]);
    store.register_late_lint_pass(Box::new(|_| Box::new(Pass { unsound_types: Vec::new() })));
}

pub struct Pass {
    /// DefIds of structs marked #[servyi::unsound_constructor], collected
    /// by `check_item` (which the HIR traversal visits before the bodies
    /// that construct them).
    unsound_types: Vec<DefId>,
}

impl_lint_pass!(Pass => [UNSOUND_CONSTRUCTOR]);

impl<'tcx> LateLintPass<'tcx> for Pass {
    fn check_item(&mut self, cx: &LateContext<'tcx>, item: &'tcx hir::Item<'tcx>) {
        if !servyi_lint_core::is_active(cx, UNSOUND_CONSTRUCTOR) {
            return;
        }
        let hir::ItemKind::Struct(..) = item.kind else { return };
        let attrs = cx.tcx.hir_attrs(rustc_hir::HirId::make_owner(item.owner_id.def_id));
        let marked = attrs.iter().any(|a| {
            a.path_matches(&[rustc_span::Symbol::intern("servyi"), rustc_span::Symbol::intern("unsound_constructor")])
        });
        if !marked {
            return;
        }
        self.unsound_types.push(item.owner_id.to_def_id());
        let documented = safety_scan::has_safety_comment(cx, item.span);
        if !documented {
            cx.opt_span_lint(
                UNSOUND_CONSTRUCTOR,
                Some(item.span),
                DiagDecorator(|diag| {
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
        if !servyi_lint_core::is_active(cx, UNSOUND_CONSTRUCTOR) {
            return;
        }
        if let ExprKind::Struct(qpath, _, _) = e.kind
            && let hir::def::Res::Def(hir::def::DefKind::Struct, did) =
                cx.typeck_results().qpath_res(&qpath, e.hir_id)
            && self.unsound_types.contains(&did)
            && !safety_scan::has_safety_comment(cx, e.span)
        {
            cx.opt_span_lint(
                UNSOUND_CONSTRUCTOR,
                Some(e.span),
                DiagDecorator(|diag| {
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
    }
}
