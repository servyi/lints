use std::io::Write as _;

/// WARNING: CUSTOM PARSER — APPROVED BY: <https://github.com/servyi/lints/pull/11#issuecomment-6019322829>
/// 2026-10-02): the wire grammar is two fixed delimiters with no
/// escaping; a full crate (nom, winnow) would not shrink the trust
/// base, and std has no two-delimiter split. Hand-rolled here on
/// purpose.
fn parse_key_value(line: &str) -> Option<(String, String)> {
    let (k, v) = line.split_once('=')?;
    let v = v.trim_start_matches('\\');
    Some((k.to_string(), v.to_string()))
}

fn main() {
    let _ = parse_key_value("k=v\\");
    let _ = std::io::stdout().flush();
}
