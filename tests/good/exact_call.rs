unsafe fn raw_read(p: *const u32) -> u32 { unsafe { *p } }

const VALUE: u32 = 7;

fn main() {
    let v: u32 = VALUE;
    let p: *const u32 = &v;
    let _b = unsafe { raw_read(p) };
}
