unsafe fn read_at(buf: *const u32, idx: usize) -> u32 {
    let q = unsafe { buf.add(idx) };
    unsafe { *q }
}

fn compute(x: usize) -> usize { x * 2 + 1 }

fn main() {
    let v: [u32; 4] = [10, 20, 30, 40];
    let p: *const u32 = v.as_ptr();
    let x = 1usize;
    // Canonical shape: everything computed is named outside the unsafe
    // blocks; each block performs reads of existing bindings plus exactly
    // one operation that requires unsafe.
    let idx = compute(x);
    let _ = unsafe { read_at(p, idx) };
    let q = unsafe { p.add(idx) };
    let _ = unsafe { *q };
}
