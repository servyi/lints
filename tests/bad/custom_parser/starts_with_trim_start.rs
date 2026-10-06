fn is_flag(s: &str) -> bool {
    s.starts_with('-')
}

fn drop_indent(s: &str) -> &str {
    s.trim_start()
}

fn main() {
    let _ = is_flag("-x");
    let _ = drop_indent("    x");
}
