fn main() {
    let v: u32 = 7;
    let p: *const u32 = &v;
    let _a2 = 1 + unsafe { *p };
}
