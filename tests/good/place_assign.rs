const INITIAL: u32 = 7;
const STORED: u32 = 5;

fn main() {
    let mut v: u32 = INITIAL;
    let mp: *mut u32 = &mut v;
    // Pure values — literals and constants — are permitted on the
    // value side, same as binding reads (review on #17).
    unsafe { *mp = STORED };
    unsafe { *mp += 1 };
}
