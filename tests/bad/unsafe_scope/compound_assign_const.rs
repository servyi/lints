static mut COUNTER: u32 = 0;

fn main() {
    // Bad: the increment must be an existing binding. Fix:
    //   let one = 1u32;
    //   unsafe { COUNTER += one };
    unsafe { COUNTER += 1 };
}
