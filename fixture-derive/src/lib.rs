//! Fixture support for the `inline_literal` good fixtures: a derive
//! that reads its `#[conf(...)]` helper attribute and re-emits the
//! attribute's literal token as the body of a generated associated
//! function.
//!
//! Why this exists: derive macros (clap's `#[derive(Parser)]` with
//! `#[command(name = "...")]` / `#[arg(num_args = 0..)]` helpers) read
//! the helper attribute's tokens and emit them into generated code
//! with the attribute token's span — and the helper attribute itself
//! stays on the item in HIR. A standalone fixture file cannot depend
//! on clap, so this crate reproduces exactly that flow:
//! `#[derive(Conf)] #[conf(key = "value")] struct S;` generates
//! `impl S { fn conf_value() -> &'static str { "value" } }` where the
//! literal carries the attribute token's span. The lint's attribute
//! exemption must hold for those derive-generated literals; without a
//! proc macro there is no way to produce one in a fixture.
//!
//! Not a text parser: it walks the token stream (token traversal, no
//! string parsing — no custom_parser seam involved).

use proc_macro::{Delimiter, Group, TokenStream, TokenTree};

const HELPER_ATTR: &str = "conf";
const STRUCT_KW: &str = "struct";
const PUNCT_EQ: char = '=';
const IMPL_KW: &str = "impl";
const CONF_VALUE_SIG: &str = "fn conf_value() -> impl std::fmt::Display";

#[proc_macro_derive(Conf, attributes(conf))]
pub fn derive_conf(input: TokenStream) -> TokenStream {
    // The literal is the token after `=` inside the parenthesized
    // arguments of a bracket group whose first ident is `conf` (the
    // helper attribute: `#[conf(key = "value")]`).
    let mut literal = None;
    for tt in input.clone() {
        if let TokenTree::Group(g) = &tt
            && g.delimiter() == Delimiter::Bracket
            && let mut inner = g.stream().into_iter()
            && let Some(TokenTree::Ident(id)) = inner.next()
            && id.to_string() == HELPER_ATTR
            && let Some(TokenTree::Group(args)) = inner.next()
        {
            literal = literal_after_eq(&args.stream());
        }
    }
    let Some(literal) = literal else {
        // No `#[conf(...)]` helper: nothing to re-emit.
        return TokenStream::new();
    };
    let Some(name) = struct_ident(&input) else {
        return TokenStream::new();
    };
    // The literal token is cloned, keeping its span inside the helper
    // attribute — the span mapping under test; the impl scaffolding
    // is built from lexed fragments and real brace Groups (spans
    // irrelevant).
    let (Ok(head), Ok(sig)) = (
        format!("{IMPL_KW} {name}").parse::<TokenStream>(),
        CONF_VALUE_SIG.parse::<TokenStream>(),
    ) else {
        return TokenStream::new();
    };
    let mut fn_ret = TokenStream::new();
    fn_ret.extend([TokenTree::Group(Group::new(Delimiter::Brace, {
        let mut body = TokenStream::new();
        body.extend([TokenTree::Literal(literal)]);
        body
    }))]);
    let mut impl_body = sig;
    impl_body.extend(fn_ret);
    let mut out = head;
    out.extend([TokenTree::Group(Group::new(Delimiter::Brace, impl_body))]);
    out
}

/// The first token after `=` in `key = "value"`-shaped attribute
/// arguments: token traversal, no string parsing.
fn literal_after_eq(args: &TokenStream) -> Option<proc_macro::Literal> {
    let mut it = args.clone().into_iter();
    while let Some(tt) = it.next() {
        if matches!(&tt, TokenTree::Punct(p) if p.as_char() == PUNCT_EQ)
            && let Some(TokenTree::Literal(lit)) = it.next()
        {
            return Some(lit);
        }
    }
    None
}

/// The ident after the `struct` keyword — the type to implement for.
/// Returned as an owned string for building the impl's head.
fn struct_ident(input: &TokenStream) -> Option<String> {
    let mut iter = input.clone().into_iter();
    while let Some(tt) = iter.next() {
        if matches!(&tt, TokenTree::Ident(id) if id.to_string() == STRUCT_KW)
            && let Some(TokenTree::Ident(name)) = iter.next()
        {
            return Some(name.to_string());
        }
    }
    None
}
