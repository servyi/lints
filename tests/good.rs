static mut COUNTER: u32 = 0;
unsafe fn raw_read(p: *const u32) -> u32 { unsafe { *p } }

fn main() {
    let mut v: u32 = 7;
    let p: *const u32 = &v;
    let mp: *mut u32 = &mut v;
    let _a2 = 1 + unsafe { *p };
    let _b = unsafe { raw_read(p) };
    unsafe { *mp = 5 };
    let _d = unsafe { &*p };
    unsafe { COUNTER += 1 };
}
