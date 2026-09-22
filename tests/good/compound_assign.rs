static mut COUNTER: u32 = 0;

fn main() {
    unsafe { COUNTER += 1 };
}
