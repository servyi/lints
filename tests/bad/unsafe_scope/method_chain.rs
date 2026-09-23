unsafe fn raw_read(p: *const u32) -> u32 { unsafe { *p } }

fn main() {
    let v: u32 = 7;
    let p: *const u32 = &v;
    let _ = unsafe { raw_read(p).leading_zeros() };
}
