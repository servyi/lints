fn main() {
    let mut v: u32 = 7;
    let mp: *mut u32 = &mut v;
    unsafe { *mp = 5 };
}
