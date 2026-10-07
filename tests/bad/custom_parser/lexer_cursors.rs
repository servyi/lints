fn take_token<'a>(input: &'a [u8]) -> Option<(&'a [u8], &'a [u8])> {
    input.split_first().map(|(h, t)| (std::slice::from_ref(h), t))
}

fn advance<'a>(chars: &mut std::str::Chars<'a>) -> &'a str {
    let _ = chars.next();
    chars.as_str()
}

fn main() {
    let _ = take_token(b"ab");
    let _ = advance(&mut "ab".chars());
    let _ = "ab".is_char_boundary(1);
    let _ = '7'.to_digit(10);
    let _ = "a=b".split_inclusive('=').count();
    let _ = "a=b".split_at_checked(1);
}
