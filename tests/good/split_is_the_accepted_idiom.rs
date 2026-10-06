fn fields(line: &str) -> Vec<&str> {
    line.split(',').collect()
}

fn main() {
    let _ = fields("a,b,c");
}
