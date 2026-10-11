const FIELD_SEP: char = ',';
const SAMPLE_LINE: &str = "a,b,c";

fn fields(line: &str) -> Vec<&str> {
    line.split(FIELD_SEP).collect()
}

fn main() {
    let _ = fields(SAMPLE_LINE);
}
