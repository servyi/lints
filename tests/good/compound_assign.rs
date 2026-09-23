static mut COUNTER: u32 = 0;

fn main() {
    let one = 1u32;
    unsafe { COUNTER += one };
}
