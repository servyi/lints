static mut COUNTER: u32 = 0;
unsafe fn raw_read(p: *const u32) -> u32 { unsafe { *p } }

fn main() {
    let mut v: u32 = 7;
    let p: *const u32 = &v;
    let _a = unsafe { 1 + *p };
    let _c = unsafe { let t = 2; t + raw_read(p) };
    let _e = unsafe { COUNTER + 1 };
    let _f = unsafe { std::mem::transmute::<u32, i32>(7) + 1 };
}
