unsafe fn read_at(buf: *const u32, idx: usize) -> u32 {
    let q = unsafe { buf.add(idx) };
    unsafe { *q }
}

const WORD_COUNT: usize = 4;
const WORDS: [u32; WORD_COUNT] = [10, 20, 30, 40];
const FIRST_WORD: usize = 1;

fn compute(x: usize) -> usize { x * 2 + 1 }

fn main() {
    let v: [u32; WORD_COUNT] = WORDS;
    let p: *const u32 = v.as_ptr();
    let x = FIRST_WORD;
    // Canonical shape: everything computed is named outside the unsafe
    // blocks; each block performs reads of existing bindings plus exactly
    // one operation that requires unsafe.
    let idx = compute(x);
    let _ = unsafe { read_at(p, idx) };
    let q = unsafe { p.add(idx) };
    let _ = unsafe { *q };
}
