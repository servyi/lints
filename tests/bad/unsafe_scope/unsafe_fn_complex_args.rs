unsafe fn read_at(buf: *const u32, idx: u32) -> u32 {
    let i = idx as usize;
    let q = unsafe { buf.add(i) };
    unsafe { *q }
}

fn compute(x: u32) -> u32 { x * 2 + 1 }

fn main() {
    let v: [u32; 4] = [10, 20, 30, 40];
    let p: *const u32 = v.as_ptr();
    let x = 1u32;
    // Bad: computed argument and arithmetic inside the block. Hoist:
    //   let idx = compute(x);
    //   let _ = unsafe { read_at(p, idx) };
    let _ = unsafe { x + read_at(p, compute(x)) };
}
