unsafe fn read_at(buf: *const u32, idx: usize) -> u32 { unsafe { *buf.add(idx) } }

fn compute(x: usize) -> usize { x * 2 + 1 }

fn main() {
    let v: [u32; 4] = [10, 20, 30, 40];
    let p: *const u32 = v.as_ptr();
    let x = 1usize;
    // Minimal: the unsafe fn call is the whole tail, exactly one leaf.
    // Its arguments are evaluated as part of the call and are not
    // individually hoisted by this lint.
    let _ = unsafe { read_at(p, compute(x)) };
}
