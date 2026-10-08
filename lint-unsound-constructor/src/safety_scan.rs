use rustc_lint::LateContext;

    /// True when a SAFETY comment sits in the contiguous comment block
    /// directly above `span` (attributes between it and the span are
    /// skipped). Comment extraction uses `rustc_lexer` — the compiler's
    /// own lexer — so strings, chars, and nested comments cannot produce
    /// false positives.
pub(crate) fn has_safety_comment(cx: &LateContext<'_>, span: rustc_span::Span) -> bool {
        let sm = cx.tcx.sess.source_map();
        let Ok(prefix) = sm.span_to_prev_source(span) else { return false };
        // Token offsets are cumulative over `Token::len`. Only tokens on
        // EARLIER LINES count: when the span starts mid-line (`let h =
        // Struct { .. }`), the `let`/ident/`=` tokens of its own line
        // would break the backward walk before it can reach the comment
        // block attached to the STATEMENT above. The cut comes from the
        // SOURCE MAP's own positions (review on #11: rustc's lexing and
        // positions, no line-searching) — the bytes of the span's own
        // line before the span are excluded.
        let same_line_before_span = sm
            .lookup_line(span.lo())
            .ok()
            .and_then(|sl| {
                let line_start = sl.sf.line_bounds(sl.line).start;
                (span.lo() >= line_start).then(|| (span.lo() - line_start).0 as usize)
            })
            .unwrap_or(0);
        let cut = prefix.len().saturating_sub(same_line_before_span);
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
        has_safety_comment_inside(sm, span)
    }

    /// A SAFETY comment INSIDE the expression's own braces (next to the
    /// fields) also counts; tokenized with the same lexer.
fn has_safety_comment_inside(
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
