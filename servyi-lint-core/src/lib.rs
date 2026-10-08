//! Shared plumbing for the servyi lint collection (issue
//! servyi/lints#12): the pieces every lint crate reuses. Lint logic
//! lives in one crate per lint; this crate holds what several of
//! them need — here, the active-check every pass consults before
//! doing any work.

#![feature(rustc_private)]
#![allow(internal_features)]
extern crate rustc_lint;

use rustc_lint::{LateContext, Lint, LintContext};

/// True where `lint` is actually enabled for the node being checked:
/// `--cap-lints allow` compiles dependencies with every lint off, and
/// `#[allow]`/`#![allow]`'d scopes are the sanctioned per-site escape.
pub fn is_active(cx: &LateContext<'_>, lint: &'static Lint) -> bool {
    let spec = cx.get_lint_level_spec(lint);
    !(spec.is_allow() || spec.is_expect())
}
