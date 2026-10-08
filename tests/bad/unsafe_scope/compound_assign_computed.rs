static mut COUNTER: u32 = 0;

fn step() -> u32 {
    1
}

fn main() {
    // Bad: the increment is a call — computation inside the unsafe
    // block. Literals and consts are fine (pure values); hoist calls:
    //   let s = step();
    //   unsafe { COUNTER += s };
    unsafe { COUNTER += step() };
}
