// Attribute-carried literals are declaration data, not code (issue
// servyi/lints#7): the attribute KEY already names the value, and a
// derive macro may re-emit the helper attribute's token into
// generated code with the attribute token's span — the consumer
// cannot name it. The `Conf` derive from fixture-derive reproduces
// exactly clap's flow (`#[derive(Parser)]` + `#[command(name = "set")]`
// / `#[arg(num_args = 0..)]`): the helper stays on the item, its
// literal is emitted into the generated impl.
//
// This fixture must PASS `-D inline_literal`.
#![allow(dead_code)]

use fixture_derive::Conf;

#[derive(Conf)]
#[conf(key = "declared-name")]
struct Documented {}

#[derive(Conf)]
#[conf(count = 9000)]
struct Counted {}

fn main() {
    let _ = (Documented::conf_value(), Counted::conf_value());
}

// builtin attributes carrying literals are declaration data too
#[deprecated(since = "0.1.0", note = "use Documented instead")]
fn legacy() {}

// the naming requirement in CODE is unchanged: a plain inline string
// is still denied unless named or in a message position — that side
// is guarded by the bad fixtures, this file only shows the sanctioned
// shapes.
const CODE_NAME: &str = "named-constant";

fn use_it() -> &'static str {
    CODE_NAME
}
