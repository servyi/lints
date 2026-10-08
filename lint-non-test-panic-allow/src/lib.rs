//! `non_test_panic_allow` (issue servyi/lints#6): `allow`/`expect` of
//! the panic/unwrap lints is a test-only exemption.

#![feature(rustc_private)]
#![allow(internal_features)]
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_span;

use rustc_errors::DiagDecorator;
use rustc_lint::{LateContext, LateLintPass, LintContext, declare_lint, impl_lint_pass};
use rustc_span::sym;

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

pub fn register(store: &mut rustc_lint::LintStore) {
    store.register_lints(&[NON_TEST_PANIC_ALLOW]);
    store.register_late_lint_pass(Box::new(|_| Box::new(Pass)));
}

pub struct Pass;

impl_lint_pass!(Pass => [NON_TEST_PANIC_ALLOW]);

impl<'tcx> LateLintPass<'tcx> for Pass {
    fn check_attribute(
        &mut self,
        cx: &LateContext<'tcx>,
        attr: &'tcx rustc_hir::attrs::Attribute,
    ) {
        // Skip where the lint is not active (`--cap-lints allow` for
        // dependencies, `#[allow]`'d scopes).
        if !servyi_lint_core::is_active(cx, NON_TEST_PANIC_ALLOW) {
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
                DiagDecorator(|diag| {
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
}
