fn state_dir() -> &'static str {
    "SERVYI_STATE"
}

fn main() {
    std::hint::black_box(state_dir());
}
