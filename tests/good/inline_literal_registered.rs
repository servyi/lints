// This fixture declares two message positions of its own (below);
// it is compiled with SERVYI_MESSAGE_MACROS="log_error fail" in the
// registered-positions CI loop (review on #16).
const SPUN_DOWN: &str = "disk full";

macro_rules! log_error {
    ($m:expr) => { eprintln!("error: {}", $m) };
}

fn fail(message: &str) {
    eprintln!("{message}");
}

fn main() {
    log_error!(SPUN_DOWN);
    fail("hardware fault");
}
