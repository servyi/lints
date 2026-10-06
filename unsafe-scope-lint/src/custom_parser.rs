use rustc_errors::DiagDecorator;
use rustc_hir as hir;
use rustc_lint::{LateContext, LintContext};
use rustc_span::Span;

/// WARNING: CUSTOM PARSER — this module implements the `custom_parser`
/// lint itself (issue servyi/lints#9, supervisor sign-off: Jonas's
/// review on PR #11): it matches resolved def-paths against a suffix
/// table and line-scans source files for the sanctioned-module header.
/// No std function or external crate does either job — def-path
/// matching rides the compiler internals, and the header scan is the
/// protocol's own enforcement — so the parsing here is hand-written on
/// purpose (supervisor discussion: PR #11 review thread, 2026-10-06).
///
/// The "typical functions used for parsing strings" (plain `split` is
/// deliberately absent — the one accepted everyday idiom, like `trim`
/// for whitespace and `parse`/`from_str_radix` as the sanctioned
/// conversions). Def-path SUFFIXES against the resolved callee
/// (inherent impls resolve to e.g. `core::str::<impl str>::split_once`),
/// with the human-facing name for the diagnostic.
pub(crate) const PARSER_PRIMITIVES: &[(&str, &str)] = &[
    // cursor-style consumption and delimiting
    ("<impl str>::split_once", "str::split_once"),
    ("<impl str>::rsplit_once", "str::rsplit_once"),
    ("<impl str>::split_terminator", "str::split_terminator"),
    ("<impl str>::rsplit_terminator", "str::rsplit_terminator"),
    ("<impl str>::split_inclusive", "str::split_inclusive"),
    ("<impl str>::splitn", "str::splitn"),
    ("<impl str>::rsplitn", "str::rsplitn"),
    ("<impl str>::split_at", "str::split_at"),
    ("<impl str>::split_at_checked", "str::split_at_checked"),
    ("<impl [T]>::split_at", "slice::split_at"),
    ("<impl [T]>::split_at_mut", "slice::split_at_mut"),
    ("<impl [T]>::split_at_mut_checked", "slice::split_at_mut_checked"),
    ("<impl str>::strip_prefix", "str::strip_prefix"),
    ("<impl str>::strip_suffix", "str::strip_suffix"),
    ("<impl [T]>::split_first", "slice::split_first"),
    ("<impl [T]>::split_last", "slice::split_last"),
    // custom-delimiter stripping (whitespace has trim())
    ("<impl str>::trim_matches", "str::trim_matches"),
    ("<impl str>::trim_start_matches", "str::trim_start_matches"),
    ("<impl str>::trim_end_matches", "str::trim_end_matches"),
    // prefix/suffix sniffing and whitespace-run stripping by hand
    // (review on #11): starts_with/ends_with, trim_start/trim_end
    ("<impl str>::starts_with", "str::starts_with"),
    ("<impl str>::ends_with", "str::ends_with"),
    ("<impl str>::trim_start", "str::trim_start"),
    ("<impl str>::trim_end", "str::trim_end"),
    // index-search cursors: `&s[..i]` after find is THE hand-parser
    // slice step (containment checks should use `contains`)
    ("<impl str>::find", "str::find"),
    ("<impl str>::rfind", "str::rfind"),
    // scanner idioms
    ("<impl str>::match_indices", "str::match_indices"),
    ("<impl str>::char_indices", "str::char_indices"),
    // the chars-cursor lexer advance (clone/next/as_str loops); the
    // lifetime segment is part of the def path (Chars::<'a>::as_str)
    ("str::Chars::<'a>::as_str", "Chars::as_str"),
    // manual UTF-8 cursor walking
    ("<impl str>::is_char_boundary", "str::is_char_boundary"),
    // radix digit parsing by hand (use str::parse / from_str_radix)
    ("<impl char>::to_digit", "char::to_digit"),
];

/// The sanctioned-module header (issue #9): a `/// WARNING: CUSTOM
/// PARSER` explanation at the top of the file, with only `use`
/// directives (and crate attributes / blank lines) above it.
const PARSER_MARKER: &str = "WARNING: CUSTOM PARSER";

pub(crate) fn check_parser_primitive<'tcx>(
    cx: &LateContext<'tcx>,
    e: &'tcx hir::Expr<'tcx>,
) {
    // Skip where the lint is not active (`--cap-lints allow` for
    // dependencies, `#[allow]`'d scopes).
    let spec = cx.get_lint_level_spec(crate::CUSTOM_PARSER);
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
        crate::CUSTOM_PARSER,
        Some(e.span),
        DiagDecorator(|diag| {
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
fn file_has_custom_parser_header(cx: &LateContext<'_>, span: Span) -> bool {
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
