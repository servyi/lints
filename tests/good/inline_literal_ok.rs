const INITIAL_BUFFER_SIZE: usize = 64;
const ZERO: u8 = 0;
const FIELD_SEP: &str = ",";
const EMPTY: &str = "";
const FIRST_INDEX: usize = 0;
const MESSAGE_DONE: &str = "done";
const CAPACITY_LIMIT: u64 = 4096;

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
    let buffer: [u8; INITIAL_BUFFER_SIZE] = [ZERO; INITIAL_BUFFER_SIZE];
    let fields: Vec<&str> = EMPTY.split(FIELD_SEP).collect();
    assert!(!fields.is_empty(), "there is always one field");
    log_line(MESSAGE_DONE);
    let outcome: Result<(), String> = Err("nothing to do".to_string());
    let next = FIRST_INDEX + 1;
    let half = capacity_limit() / 2;
    let neg: i8 = -1;
    seek(0);
    let stop = outcome.is_err();
    std::hint::black_box((buffer, next, half, neg, stop));
}
