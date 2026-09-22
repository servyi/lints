unsafe fn read_at(buf: *const u32, idx: usize) -> u32 {
    let q = unsafe { buf.add(idx) };
    unsafe { *q }
}

fn compute(x: usize) -> usize { x * 2 + 1 }

fn main() {
    let v: [u32; 4] = [10, 20, 30, 40];
    let p: *const u32 = v.as_ptr();
    let x = 1usize;
    // Bad: the argument is computed inside the block. Operands of the
    // unsafe operation must be plain reads of existing bindings. Hoist:
    //   let idx = compute(x);
    //   let _ = unsafe { read_at(p, idx) };
    let _ = unsafe { read_at(p, compute(x)) };
}
