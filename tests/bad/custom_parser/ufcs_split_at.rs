fn split_line(s: &str, at: usize) -> (&str, &str) {
    let _ = str::strip_prefix(s, "-");
    s.split_at(at)
}

fn main() {
    let _ = split_line("a-b", 2);
}
