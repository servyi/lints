fn main() {
    let mut v: u32 = 7;
    let mp: *mut u32 = &mut v;
    // Bad: the value side must be a plain read of an existing binding, not
    // a literal. Fix:
    //   let value = 5u32;
    //   unsafe { *mp = value };
    unsafe { *mp = 5 };
}
