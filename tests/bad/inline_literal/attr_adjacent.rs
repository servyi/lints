// Attributes sanction their OWN tokens (issue servyi/lints#7), not
// the code around them: a literal in plain code near attributes —
// even inside a fn that carries attributes, or inside a struct whose
// fields carry helper attributes — must stay denied. Guards the
// attribute exemption against overreach.
#![allow(dead_code)]

use fixture_derive::Conf;

#[derive(Conf)]
#[conf(key = "declared-name")]
struct Documented {
    #[allow(dead_code)]
    field: u32,
}

#[deprecated(since = "0.1.0", note = "old")]
fn legacy() -> u32 {
    7000
}

fn main() {
    let _ = legacy();
    let _ = Documented { field: 400 };
}
