fn capacity_limit() -> usize {
    4096
}

fn main() {
    std::hint::black_box(capacity_limit());
}
