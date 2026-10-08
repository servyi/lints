fn skip_two(s: &[u8]) -> &[u8] {
    &s[2..]
}

fn main() {
    std::hint::black_box(skip_two);
}
