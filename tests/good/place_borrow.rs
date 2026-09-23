fn main() {
    let v: u32 = 7;
    let p: *const u32 = &v;
    let _d = unsafe { &*p };
}
