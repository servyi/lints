// anyhow-style context attachment is report prose (issue
// servyi/lints#7): the message argument of `.context("...")` and
// `.with_context(|| "...")` — direct or wrapped in the lazy closure —
// is a message position, exactly like `.expect("...")` and the
// `Err(...)`/`assert!`/log families.
//
// This fixture must PASS `-D inline_literal`. The `Context` trait
// here mirrors anyhow's method pair so the fixture stays std-only.
#![allow(dead_code)]

struct Site;

trait Context<T> {
    fn context<C: Into<String>>(self, msg: C) -> Result<T, String>;
    fn with_context<F: FnOnce() -> &'static str>(self, f: F) -> Result<T, String>;
}

impl<T> Context<T> for Option<T> {
    fn context<C: Into<String>>(self, msg: C) -> Result<T, String> {
        self.ok_or_else(|| msg.into())
    }
    fn with_context<F: FnOnce() -> &'static str>(self, f: F) -> Result<T, String> {
        self.ok_or_else(|| f().to_string())
    }
}

fn main() {
    let direct: Result<Site, String> = Some(Site).context("site lookup failed");
    let lazy: Result<Site, String> = Some(Site).with_context(|| "site lookup failed (lazy)");
    let _ = (direct, lazy);
}
