fn main() {
    let v: u32 = 7;
    let p: *const u32 = &v;
    let _ = unsafe { *p > 3 };
}
