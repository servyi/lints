// Review on #16: clients register their own message positions IN
// CODE. The `register_message!` macro (pasteable — README) leaves the
// marker const the linter recognizes; registration is per-crate.
const SPUN_DOWN: &str = "disk full";

macro_rules! register_message {
    ($m:ident) => {
        #[allow(dead_code)]
        const _: &[(&str, &str)] = &[("servyi::message", stringify!($m))];
    };
}

register_message!(log_error);
register_message!(fail);

macro_rules! log_error {
    ($m:expr) => { eprintln!("error: {}", $m) };
}

fn fail(message: &str) {
    eprintln!("{message}");
}

fn main() {
    log_error!(SPUN_DOWN);
    log_error!("motor stalled");
    fail("hardware fault");
}
