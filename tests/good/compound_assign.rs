static mut COUNTER: u32 = 0;

fn main() {
    // `+= 1` is fine: the literal is a pure value (review on #17).
    unsafe { COUNTER += 1 };
}
