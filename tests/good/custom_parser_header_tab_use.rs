use	std::io::Write as _;

/// WARNING: CUSTOM PARSER — APPROVED BY: <https://github.com/servyi/lints/pull/11#issuecomment-6019322829>
fn parse(line: &str) -> Option<(&str, &str)> {
    line.split_once('=')
}

fn main() {
    let _ = parse("k=v");
    let _ = std::io::stdout().flush();
}
