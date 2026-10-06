// an innocent plain comment above the marker
/// WARNING: CUSTOM PARSER — not sanctioned: a plain comment sits above
fn unquote(s: &str) -> &str {
    s.trim_matches('"')
}

fn main() {
    let _ = unquote("x");
}
