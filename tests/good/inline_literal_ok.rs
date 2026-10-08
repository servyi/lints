const INITIAL_BUFFER_SIZE: usize = 64;
const INITIAL_FILL: u8 = 0;
const FIELD_SEP: &str = ",";
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
    std::hint::black_box((buffer, next, half, neg, stop));
}
