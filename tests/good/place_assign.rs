fn main() {
    let mut v: u32 = 7;
    let mp: *mut u32 = &mut v;
    // Constants must be named before the block: the value side of the
    // unsafe operation is a plain read of an existing binding.
    let value = 5u32;
    unsafe { *mp = value };
}
