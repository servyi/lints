fn main() {
    // 0/1 (and 0.0/1.0, "") are permitted as let initializers; other
    // values still name themselves (review on agent-scripts#115).
    let retries = 3;
    let factor = 0.5;
    std::hint::black_box((retries, factor));
}
