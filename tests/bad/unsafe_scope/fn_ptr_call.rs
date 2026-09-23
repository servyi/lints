unsafe fn raw_read(p: *const u32) -> u32 { unsafe { *p } }

fn main() {
    let v: u32 = 7;
    let p: *const u32 = &v;
    let f: unsafe fn(*const u32) -> u32 = raw_read;
    let _ = unsafe { f(p) + 1 };
}
