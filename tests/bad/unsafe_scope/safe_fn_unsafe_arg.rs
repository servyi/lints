unsafe fn load(p: *const u32) -> u32 { unsafe { *p } }
fn store(x: u32) -> u32 { x }

fn main() {
    let v: u32 = 7;
    let p: *const u32 = &v;
    // A safe fn call whose argument is an unsafe fn call: only the inner
    // call requires unsafe; it should be wrapped, not the outer statement.
    let _ = unsafe { store(load(p)) };
}
