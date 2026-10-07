fn unquote(s: &str) -> &str {
    s.trim_matches('"')
}

fn main() {
    let _ = unquote("\"x\"");
}
