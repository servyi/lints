//! servyi-lint — the Servyi policy driver.
//!
//! One `RUSTC_WRAPPER` binary wrapping the compiler, registering the
//! servyi lint collection (issue servyi/lints#12: one crate per lint,
//! composed here; shared plumbing in `servyi-lint-core`):
//!
//! - `lint-unsafe-scope` — `unsafe_scope`: strict shape of `unsafe`
//!   blocks (reads of existing bindings plus exactly one operation
//!   that requires unsafe, everything else hoisted out).
//! - `lint-custom-parser` — `custom_parser` (issue #9): hand-parsing
//!   primitives only inside sanctioned, maintainer-approved modules.
//! - `lint-inline-literal` — `inline_literal` (issue #7): inline
//!   literals outside const declarations, except message strings and
//!   the trivial numerics.
//! - `lint-unsound-constructor` — `unsound_constructor` (issue #8):
//!   `#[servyi::unsound_constructor]` types carry SAFETY comments at
//!   the type and at every construction site.
//! - `lint-non-test-panic-allow` — `non_test_panic_allow` (issue #6):
//!   `allow`/`expect` of the panic/unwrap lints is a test-only
//!   exemption.
//! - `lint-workspace-lints-table` — `workspace_lints_table`: the
//!   workspace manifest defines no lint tables.
//!
//! Used as a `RUSTC_WRAPPER`: cargo invokes this binary in place of
//! rustc; it registers the extra late lints and otherwise behaves
//! exactly like the compiler it ships with (same nightly toolchain).
#![feature(rustc_private)]
#![allow(internal_features)]
extern crate rustc_driver;
extern crate rustc_interface;
extern crate rustc_lint;
extern crate rustc_session;

struct LintCallbacks;

impl rustc_driver::Callbacks for LintCallbacks {
    fn config(&mut self, config: &mut rustc_interface::Config) {
        let previous = config.register_lints.take();
        config.register_lints = Some(Box::new(move |sess, store| {
            if let Some(prev) = &previous {
                prev(sess, store);
            }
            lint_custom_parser::register(store);
            lint_inline_literal::register(store);
            lint_non_test_panic_allow::register(store);
            lint_unsafe_scope::register(store);
            lint_unsound_constructor::register(store);
            lint_workspace_lints_table::register(store);
        }));
    }
}

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    // As RUSTC_WRAPPER, cargo passes the real rustc path as the first
    // arg. The probe recognizes it by FILE NAME equality via std's
    // path componentization (review on #11: no string suffix match).
    let is_compiler = |a: &String| {
        std::path::Path::new(a)
            .file_name()
            .is_some_and(|f| f == "rustc" || f == "clippy-driver")
    };
    let rustc_path = if std::env::var_os("RUSTC_WRAPPER").is_some()
        && args.first().is_some_and(is_compiler)
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
                eprintln!("servyi-lint: probe delegation failed: {e}");
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
