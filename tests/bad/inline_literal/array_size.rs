fn scratch() -> [u8; 512] {
    [0; 512]
}

fn main() {
    std::hint::black_box(scratch());
}
