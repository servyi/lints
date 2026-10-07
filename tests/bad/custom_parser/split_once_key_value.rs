fn parse_pair(line: &str) -> Option<(&str, &str)> {
    line.split_once('=')
}

fn main() {
    let _ = parse_pair("k=v");
}
