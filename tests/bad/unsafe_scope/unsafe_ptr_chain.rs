fn main() {
    let mut v: u32 = 7;
    let buf: *mut u32 = &mut v;
    let idx = 0usize;
    // Bad: `buf.add(idx)` (unsafe method) and the raw dereference are two
    // unsafe operations in one block. clippy::multiple_unsafe_ops_per_block
    // forbids this as well; `unsafe_scope` flags the deref operand being a
    // call. Fix:
    //   let q = unsafe { buf.add(idx) };
    //   let _ = unsafe { *q };
    let _ = unsafe { *buf.add(idx) };
}
