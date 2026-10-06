fn take_flag<'a>(s: &'a str, flag: &str) -> Option<&'a str> {
    s.strip_prefix(flag)
}

fn main() {
    let _ = take_flag("--verbose", "--");
}
