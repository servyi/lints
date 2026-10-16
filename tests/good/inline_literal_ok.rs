const INITIAL_BUFFER_SIZE: usize = 64;
const INITIAL_FILL: u8 = 0;
const FIELD_SEP: &str = ",";
const FIRST_INDEX: usize = 0;
const MESSAGE_DONE: &str = "done";
const CAPACITY_LIMIT: u64 = 4096;
const FIRST_INDEX_USIZE: usize = 1;
static COUNTER_RELAXED: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
struct Zeroed {
    count: usize,
    base: usize,
}

fn log_line(message: &str) {
    println!("{message}");
}

fn capacity_limit() -> u64 {
    CAPACITY_LIMIT
}

fn seek(position: u64) {
    std::hint::black_box(position);
}

fn main() {
    let buffer: [u8; INITIAL_BUFFER_SIZE] = [INITIAL_FILL; INITIAL_BUFFER_SIZE];
    let fields: Vec<&str> = "".split(FIELD_SEP).collect();
    assert!(!fields.is_empty(), "there is always one field");
    log_line(MESSAGE_DONE);
    log_line("");
    let outcome: Result<(), String> = Err("nothing to do".to_string());
    let next = FIRST_INDEX + 1;
    let half = capacity_limit() / 2;
    let neg: i8 = -1;
    seek(0);
    let stop = outcome.is_err();
    // Range starts 0/1 are iteration idiom, fetch_add/fetch_sub carry
    // the +/-1 exception (review on states#7).
    let first_half: Vec<u32> = (0..buffer.len() as u32).take(FIRST_INDEX_USIZE).collect();
    let _ = first_half;
    for _ in 1..CAPACITY_LIMIT {}
    for _ in 0..=CAPACITY_LIMIT {}
    COUNTER_RELAXED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    COUNTER_RELAXED.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    // Boundary comparisons, saturating steps, and defaulted method
    // args are trivial too (review on servatui#9).
    let boundary_hi = next > 1;
    let boundary_lo = half > 0;
    let boundary_ge = half >= 1;
    let stepped = next.saturating_sub(1) + 1;
    let defaulted = boundary_hi.then_some(half).map_or(0, |v| v);
    // The whole addition/subtraction method family carries the ±1
    // exception on its first argument (review follow-up on #21).
    // A struct-literal field initialized to zero is the
    // zero-initialization idiom (review follow-up).
    let _ = Zeroed { count: 0, base: 0 };
    // A let binding initialized to 0, 1, 0.0, 1.0 or "" is
    // initialization, not a magic constant (review on
    // agent-scripts#115).
    let mut attempts = 0;
    let mut index = 1;
    let mut ratio = 0.0;
    let mut scale = 1.0;
    let mut label = "";
    let mut suffix = "".to_string();
    let typed_zero: u64 = 0;
    let typed_one: usize = 1;
    let _ = (
        next.checked_add(1),
        next.checked_sub(1),
        next.wrapping_add(1),
        next.wrapping_sub(1),
        next.overflowing_add(1),
        next.overflowing_sub(1),
        next.rotate_left(1),
        next.rotate_right(1),
    );
    let _ = (stepped, defaulted);
    std::hint::black_box((buffer, next, half, neg, stop, attempts, index, ratio, scale, label, suffix, typed_zero, typed_one));
}
