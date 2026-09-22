unsafe fn read_at(buf: *const u32, idx: u32) -> u32 { unsafe { *buf.add(idx as usize) } }

fn compute(x: u32) -> u32 { x * 2 + 1 }

fn main() {
    let v: [u32; 4] = [10, 20, 30, 40];
    let p: *const u32 = v.as_ptr();
    let x = 1u32;
    // The unsafe fn call is one leaf; the `x +` around it is safe padding.
    let _ = unsafe { x + read_at(p, compute(x)) };
}
