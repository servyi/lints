const VALUE: u32 = 7;

fn main() {
    let v: u32 = VALUE;
    let p: *const u32 = &v;
    let _d = unsafe { &*p };
}
