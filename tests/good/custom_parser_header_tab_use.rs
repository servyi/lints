use	std::io::Write as _;

/// WARNING: CUSTOM PARSER — APPROVED BY: <https://github.com/servyi/lints/pull/11#issuecomment-6019322829>
const KV_SEP: char = '=';
const SAMPLE_LINE: &str = "k=v";

fn parse(line: &str) -> Option<(&str, &str)> {
    line.split_once(KV_SEP)
}

fn main() {
    let _ = parse(SAMPLE_LINE);
    let _ = std::io::stdout().flush();
}
