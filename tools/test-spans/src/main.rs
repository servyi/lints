fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: test-spans <file.rs>");
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let spans = test_spans::spans(&text);
    println!("{}", serde_json::to_string(&spans).unwrap());
}
