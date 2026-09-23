static mut COUNTER: u32 = 0;
unsafe fn raw_read(p: *const u32) -> u32 { unsafe { *p } }

fn main() {
    let v: u32 = 7;
    let p: *const u32 = &v;
    let _a = unsafe { 1 + *p };
    let _ = unsafe { &COUNTER };
}
