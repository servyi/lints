fn value_of(line: &str) -> &str {
    let i = line.find('=');
    &line[i.map(|i| i + 1).unwrap_or(0)..]
}

fn has_suffix(s: &str) -> bool {
    s.ends_with(".tmp")
}

fn unindent(s: &str) -> &str {
    s.trim_end()
}

fn main() {
    let _ = value_of("k=v");
    let _ = has_suffix("x.tmp");
    let _ = unindent("x  ");
    let _ = "a.b".rfind('.');
}
