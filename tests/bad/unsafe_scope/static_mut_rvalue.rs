static mut COUNTER: u32 = 0;

fn main() {
    let _e = unsafe { COUNTER + 1 };
}
