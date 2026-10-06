use	std::io::Write as _;

/// WARNING: CUSTOM PARSER — supervisor agreement: a tab after `use` is a token, not a space; the lexer accepts what line-slicing rejected.
fn parse(line: &str) -> Option<(&str, &str)> {
    line.split_once('=')
}

fn main() {
    let _ = parse("k=v");
    let _ = std::io::stdout().flush();
}
