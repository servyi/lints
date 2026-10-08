fn first(b: &[u8]) -> u8 {
    b[0]
}

fn main() {
    std::hint::black_box(first);
}
