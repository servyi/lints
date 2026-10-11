const VALUE: u32 = 7;

fn main() {
    let v: u32 = VALUE;
    let p: *const u32 = &v;
    let _a2 = 1 + unsafe { *p };
}
