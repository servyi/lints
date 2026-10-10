fn compute() -> u32 {
    7
}

fn main() {
    let mut v: u32 = 7;
    let mp: *mut u32 = &mut v;
    // Bad: the value side is a call — computation inside the unsafe
    // block hides what the operation writes. Literals and consts are
    // fine (pure values); hoist calls out:
    //   let value = compute();
    //   unsafe { *mp = value };
    unsafe { *mp = compute() };
}
