unsafe fn raw_read(p: *const u32) -> u32 { unsafe { *p } }

fn main() {
    let v: u32 = 7;
    let p: *const u32 = &v;
    let _c = unsafe { let t = 2; t + raw_read(p) };
}
