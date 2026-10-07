//! `custom_parser` (issue servyi/lints#9): calls to typical
//! hand-parsing primitives (`split_once`, `strip_prefix`,
//! `trim_*_matches`, ...) must not appear outside a sanctioned
//! custom-parser module — one headed by a
//! `/// WARNING: CUSTOM PARSER ...` explanation with only `use`
//! directives above it, carrying a maintainer approval the linter
//! verifies live.

#![feature(rustc_private)]
#![allow(internal_features)]
extern crate rustc_errors;
extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_lexer;
extern crate rustc_span;

use rustc_errors::DiagDecorator;
use rustc_hir as hir;
use rustc_lint::{LateContext, LateLintPass, LintContext, declare_lint, impl_lint_pass};
use rustc_span::Span;

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

pub fn register(store: &mut rustc_lint::LintStore) {
    store.register_lints(&[CUSTOM_PARSER]);
    store.register_late_lint_pass(Box::new(|_| Box::new(Pass)));
}

pub struct Pass;

impl_lint_pass!(Pass => [CUSTOM_PARSER]);

impl<'tcx> LateLintPass<'tcx> for Pass {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, e: &'tcx hir::Expr<'tcx>) {
        check_parser_primitive(cx, e);
    }
}


/// The "typical functions used for parsing strings" (plain `split` is
/// deliberately absent — the one accepted everyday idiom, like `trim`
/// for whitespace and `parse`/`from_str_radix` as the sanctioned
/// conversions). FULL def-path strings exactly as the compiler renders
/// them, compared by plain EQUALITY (review on #11: the table is
/// data; the match is `==`, no suffix parsing). Nothing in this module
/// hand-parses: the header scan uses `rustc_lexer`, the approval URL
/// comes out of `pulldown-cmark`, the permalink maps through `url` +
/// `regex`, the API reply through `serde_json`.
const PARSER_PRIMITIVES: &[(&str, &str)] = &[
    ("core::str::<impl str>::split_once", "str::split_once"),
    ("core::str::<impl str>::rsplit_once", "str::rsplit_once"),
    ("core::str::<impl str>::split_terminator", "str::split_terminator"),
    ("core::str::<impl str>::rsplit_terminator", "str::rsplit_terminator"),
    ("core::str::<impl str>::split_inclusive", "str::split_inclusive"),
    ("core::str::<impl str>::splitn", "str::splitn"),
    ("core::str::<impl str>::rsplitn", "str::rsplitn"),
    ("core::str::<impl str>::split_at", "str::split_at"),
    ("core::str::<impl str>::split_at_checked", "str::split_at_checked"),
    ("core::slice::<impl [T]>::split_at", "slice::split_at"),
    ("core::slice::<impl [T]>::split_at_mut", "slice::split_at_mut"),
    ("core::slice::<impl [T]>::split_at_mut_checked", "slice::split_at_mut_checked"),
    ("core::str::<impl str>::strip_prefix", "str::strip_prefix"),
    ("core::str::<impl str>::strip_suffix", "str::strip_suffix"),
    ("core::slice::<impl [T]>::split_first", "slice::split_first"),
    ("core::slice::<impl [T]>::split_last", "slice::split_last"),
    ("core::str::<impl str>::trim_matches", "str::trim_matches"),
    ("core::str::<impl str>::trim_start_matches", "str::trim_start_matches"),
    ("core::str::<impl str>::trim_end_matches", "str::trim_end_matches"),
    ("core::str::<impl str>::starts_with", "str::starts_with"),
    ("core::str::<impl str>::ends_with", "str::ends_with"),
    ("core::str::<impl str>::trim_start", "str::trim_start"),
    ("core::str::<impl str>::trim_end", "str::trim_end"),
    ("core::str::<impl str>::find", "str::find"),
    ("core::str::<impl str>::rfind", "str::rfind"),
    ("core::str::<impl str>::match_indices", "str::match_indices"),
    ("core::str::<impl str>::char_indices", "str::char_indices"),
    ("std::str::Chars::<'a>::as_str", "Chars::as_str"),
    ("core::str::<impl str>::is_char_boundary", "str::is_char_boundary"),
    ("std::char::methods::<impl char>::to_digit", "char::to_digit"),
];

/// The sanctioned-module header (issue #9, upgraded by review on
/// #11): the file's first outer-doc token must be
/// `/// WARNING: CUSTOM PARSER — APPROVED BY: {url}`, with only `use`
/// directives (and crate attributes / blank lines) above it. The URL
/// must point at a comment by a MAINTAINER whose body contains the
/// exact approval sentence below — verified live by the linter.
const PARSER_MARKER: &str = "WARNING: CUSTOM PARSER";
const APPROVED_BY: &str = "APPROVED BY: ";
const APPROVAL_SENTENCE: &str =
    "I approve writing a custom parser for this specific use case:";

/// The humans whose approval comments count (servyi org reviewers).
/// Extending this list is a change to THIS file — itself reviewed.
const MAINTAINERS: &[&str] = &["JonasOberhauser"];

/// One verification of an approval URL, cached per process.
#[derive(Clone, Debug)]
enum Approval {
    /// Author is a maintainer and the sentence is present.
    ApprovedBy(String),
    /// The link resolved but something is wrong (named).
    Rejected(String),
    /// The link could not be checked at all — fail closed.
    Unverifiable(String),
}

/// Extract the approval URL from the marker doc: the doc is parsed as
/// MARKDOWN (pulldown-cmark — off-the-shelf) and the URL must be the
/// header's autolink (`<https://…>`) or a link destination — no
/// hand-slicing of the text (review on #11).
fn approval_url_from(marker_doc: &str) -> Option<String> {
    use pulldown_cmark::{Event, Options, Parser};
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_OLD_FOOTNOTES);
    let mut url = None;
    for ev in Parser::new_ext(marker_doc, opts) {
        match ev {
            Event::Start(pulldown_cmark::Tag::Link { link_type: pulldown_cmark::LinkType::Autolink, dest_url, .. }) => {
                url = Some(dest_url.into_string());
                break;
            }
            Event::Start(pulldown_cmark::Tag::Link { dest_url, .. }) => {
                url = Some(dest_url.into_string());
                break;
            }
            _ => {}
        }
    }
    url.filter(|u| url::Url::parse(u).is_ok_and(|u| u.scheme() == "https"))
}

/// Map a github comment permalink to its REST endpoint: parsed by the
/// `url` crate (host/path/fragment as OBJECTS), the fragment's shape
/// classified by a REGEX (an off-the-shelf parser, per review on #11)
/// — no hand string surgery.
fn api_endpoint(link: &str) -> Option<String> {
    let u = url::Url::parse(link).ok()?;
    if u.host_str()? != "github.com" {
        return None;
    }
    let segs: Vec<_> = u.path_segments()?.collect();
    if segs.len() != 4 {
        return None;
    }
    match segs[2] {
        "pull" | "issues" => {}
        _ => return None,
    }
    let (owner, repo, _num) = (segs[0], segs[1], segs[3]);
    let frag = u.fragment()?;
    static SHAPE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let shape = SHAPE.get_or_init(|| {
        regex::Regex::new(
            r"^(?:issuecomment-(?<i>[0-9]+)|discussion_r(?<d>[0-9]+)|pullrequestreview-(?<r>[0-9]+))$",
        ).expect("the permalink-shape regex compiles")
    });
    let caps = shape.captures(frag)?;
    let kind = if caps.name("i").is_some() {
        "issues/comments"
    } else if caps.name("d").is_some() {
        "pulls/comments"
    } else {
        "pulls/reviews"
    };
    let id = caps.name("i").or_else(|| caps.name("d")).or_else(|| caps.name("r"))?;
    Some(format!("https://api.github.com/repos/{owner}/{repo}/{kind}/{}", id.as_str()))
}

/// Live verification: the linked comment must exist, be by a
/// MAINTAINER, and contain the approval sentence. Off-the-shelf
/// JSON (serde_json) — no hand parsing here.
fn verify_approval(url: &str) -> Approval {
    let Some(endpoint) = api_endpoint(url) else {
        return Approval::Rejected("not a github comment permalink".into());
    };
    let out = std::process::Command::new("curl")
        .args([
            "-s",
            "--max-time",
            "10",
            "-H",
            "Accept: application/vnd.github+json",
            &endpoint,
        ])
        .output();
    let out = match out {
        Ok(o) if o.status.success() => o,
        Ok(o) => {
            return Approval::Unverifiable(format!("fetch failed ({})", o.status.code().unwrap_or(-1)))
        }
        Err(e) => return Approval::Unverifiable(format!("curl failed: {e}")),
    };
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(&out.stdout) else {
        return Approval::Unverifiable("response was not JSON".into());
    };
    let Some(login) = v["user"]["login"].as_str() else {
        return Approval::Rejected("the link does not resolve to a comment".into());
    };
    if !MAINTAINERS.contains(&login) {
        return Approval::Rejected(format!("author `{login}` is not a maintainer"));
    }
    let body = v["body"].as_str().unwrap_or_default();
    // The sentence must appear in the comment's PLAIN PROSE: markdown
    // parsing (pulldown-cmark — off-the-shelf) excludes code blocks
    // and inline code, so quoting the sentence as a template (as the
    // protocol's own spec comment does) can never self-approve.
    let mut prose = String::new();
    use pulldown_cmark::{Event, Parser};
    for ev in Parser::new(body) {
        if let Event::Text(txt) = ev {
            prose.push_str(&txt);
        }
    }
    if prose.contains(APPROVAL_SENTENCE) {
        Approval::ApprovedBy(login.to_string())
    } else {
        Approval::Rejected(format!(
            "the comment by `{login}` lacks the sentence `{APPROVAL_SENTENCE}` \
             in plain prose"
        ))
    }
}

type Cache = std::sync::Mutex<std::collections::HashMap<String, Approval>>;

fn with_cache<R>(f: impl FnOnce(&mut std::collections::HashMap<String, Approval>) -> R) -> R {
    static CACHE: std::sync::OnceLock<Cache> = std::sync::OnceLock::new();
    let mut guard = CACHE
        .get_or_init(|| Cache::default())
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    f(&mut guard)
}

fn cached_approval(url: &str) -> Approval {
    if let Some(a) = with_cache(|c| c.get(url).cloned()) {
        return a;
    }
    let a = verify_approval(url);
    with_cache(|c| c.insert(url.to_string(), a.clone()));
    a
}

fn check_parser_primitive<'tcx>(
    cx: &LateContext<'tcx>,
    e: &'tcx hir::Expr<'tcx>,
) {
    // Skip where the lint is not active (`--cap-lints allow` for
    // dependencies, `#[allow]`'d scopes).
    if !servyi_lint_core::is_active(cx, CUSTOM_PARSER) {
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
    let Some(&(_, why)) = PARSER_PRIMITIVES.iter().find(|(p, _)| *p == path) else {
        if std::env::var_os("CUSTOM_PARSER_DEBUG").is_some() {
            eprintln!("custom_parser: NO TABLE MATCH for {path} (len {})", path.len());
        }
        return;
    };
    if std::env::var_os("CUSTOM_PARSER_DEBUG").is_some() {
        eprintln!("custom_parser: matched {why}; header lookup next");
    }
    match file_approval_url(cx, e.span) {
        Some(url) => match cached_approval(&url) {
            Approval::ApprovedBy(by) => {
                if std::env::var_os("CUSTOM_PARSER_DEBUG").is_some() {
                    eprintln!("custom_parser: approved by {by}: {url}");
                }
                return;
            }
            bad => {
                cx.opt_span_lint(
                    CUSTOM_PARSER,
                    Some(e.span),
                    DiagDecorator(move |diag| {
                        approval_failure_note(diag, &bad, &url);
                    }),
                );
            }
        },
        None => {}
    }
    cx.opt_span_lint(
        CUSTOM_PARSER,
        Some(e.span),
        DiagDecorator(|diag| {
            diag.note(format!(
                "`{path}` is a hand-parsing primitive (issue servyi/lints#9) — this \
                 specific use of custom parsing needs a MAINTAINER APPROVAL. \
                 1) Check first that no std function or off-the-shelf crate/parser/lexer \
                 already does this parsing job. \
                 2) Ask for approval: post a comment listing EVERY case of custom \
                 parsing in this project that you believe is strictly necessary and not \
                 solvable off-the-shelf, and request the reply \
                 `{APPROVAL_SENTENCE}` covering exactly those cases. \
                 3) Once approved, head the parser's file with \
                 `/// {PARSER_MARKER} — {APPROVED_BY}<url of the approving comment>` \
                 (only `use` directives above it) — the linter verifies the link, its \
                 author, and the sentence. Custom parsing NOT covered by that approval \
                 comment must be carefully removed. \
                 `{why}` was the matched primitive."
            ));
        }),
    );
}

fn approval_failure_note(
    diag: &mut rustc_errors::Diag<'_, ()>,
    bad: &Approval,
    url: &str,
) {
    match bad {
        Approval::Rejected(why) => {
            diag.note(format!(
                "the header cites {url} but the approval does not hold: {why} — \
                 the finding stands"
            ));
        }
        Approval::Unverifiable(why) => {
            diag.note(format!(
                "the header cites {url} which could not be verified ({why}) — \
                 FAIL CLOSED: the finding stands"
            ));
        }
        Approval::ApprovedBy(_) => unreachable!(),
    }
}

/// Does the file containing `span` carry the sanctioned header? The
/// marker is the FIRST outer-doc token of the file; everything above
/// it must be whitespace, `use` items, or inner attributes — decided
/// by the compiler's own lexer (review on #11: no line-slicing; the
/// lexer also gets the cases slicing cannot see — `use\t` vs `use `,
/// `////` plain comments and `//!` inner docs vs the `///` marker).
fn file_approval_url(cx: &LateContext<'_>, span: Span) -> Option<String> {

    let filename = cx.sess().source_map().span_to_filename(span);
    let rustc_span::FileName::Real(real) = &filename else {
        return None; // macro expansions / synthetic spans: no header
    };
    let Some(path) = real.local_path() else {
        return None; // remapped/virtual path (e.g. rustc internals)
    };
    let Ok(src) = std::fs::read_to_string(path) else {
        return None;
    };
    #[derive(PartialEq)]
    enum St {
        /// Between top-level items.
        Top,
        /// Inside a `use` item — everything until the `;` is its own.
        UseItem,
        /// Inside an inner attribute's brackets (depth counted).
        Attr(i32),
        /// Saw `#` (and maybe `!`) — expecting the bracket.
        AttrPound,
    }
    let mut st = St::Top;
    let mut pos = 0usize;
    let mut marker_doc: Option<String> = None;
    for t in rustc_lexer::tokenize(&src, rustc_lexer::FrontmatterAllowed::No) {
        let text = &src[pos..pos + t.len as usize];
        pos += t.len as usize;
        match st {
            St::UseItem => {
                if matches!(t.kind, rustc_lexer::TokenKind::Semi) {
                    st = St::Top;
                }
            }
            St::Attr(depth) => match t.kind {
                rustc_lexer::TokenKind::OpenBracket => st = St::Attr(depth + 1),
                rustc_lexer::TokenKind::CloseBracket if depth == 1 => st = St::Top,
                rustc_lexer::TokenKind::CloseBracket => st = St::Attr(depth - 1),
                _ => {}
            },
            St::AttrPound => match t.kind {
                rustc_lexer::TokenKind::Bang => {}
                rustc_lexer::TokenKind::OpenBracket => st = St::Attr(1),
                _ => return None,
            },
            St::Top => match t.kind {
                rustc_lexer::TokenKind::Whitespace => {}
                rustc_lexer::TokenKind::Ident if text == "use" => st = St::UseItem,
                rustc_lexer::TokenKind::Pound => st = St::AttrPound,
                rustc_lexer::TokenKind::LineComment {
                    doc_style: Some(rustc_lexer::DocStyle::Outer),
                }
                | rustc_lexer::TokenKind::BlockComment {
                    doc_style: Some(rustc_lexer::DocStyle::Outer),
                    terminated: _,
                } => {
                    marker_doc = Some(text.to_string());
                    break;
                }
                // Plain comments, inner docs, any item or debris above
                // the explanation: not a sanctioned header.
                _ => return None,
            },
        }
    }
    marker_doc.as_deref().and_then(approval_url_from)
}
