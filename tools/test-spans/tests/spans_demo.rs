//! Behavioral tests for the span walker (the gate's eyes).  The
//! fault branch `/spans-fault` must make these fail — that is the
//! negative-control proof for this crate's own PR.

use test_spans::spans;

const SAMPLE: &str = "\
pub fn add(a: u32, b: u32) -> u32 { a + b }

#[cfg(test)]
mod tests {
    #[test]
    fn inline_add_works() { assert_eq!(super::add(1, 2), 3); }

    #[test]
    fn inline_mul_works() { assert_eq!(2 * 3, 6); }
}

#[test]
fn top_level_smoke() { assert!(true); }
";

fn find<'a>(s: &'a [test_spans::Span], path: &str) -> &'a test_spans::Span {
    s.iter()
        .find(|x| x.path == path)
        .unwrap_or_else(|| panic!("no span {path} in {:?}", s.iter().map(|x| &x.path).collect::<Vec<_>>()))
}

#[test]
fn mod_and_fn_spans_are_found_with_module_paths() {
    let s = spans(SAMPLE);
    // the whole cfg(test) region, module-qualified fns, and the
    // top-level test — with kinds.
    assert_eq!(find(&s, "tests").kind, "mod");
    assert_eq!(find(&s, "tests::inline_add_works").kind, "fn");
    assert_eq!(find(&s, "tests::inline_mul_works").kind, "fn");
    assert_eq!(find(&s, "top_level_smoke").kind, "fn");
}

#[test]
fn mod_region_encloses_its_fns() {
    let s = spans(SAMPLE);
    let m = s.iter().find(|x| x.path == "tests").unwrap();
    let f = s.iter().find(|x| x.path == "tests::inline_add_works").unwrap();
    assert!(m.start <= f.start && f.end <= m.end, "{m:?} {f:?}");
    // the span starts at the ATTRIBUTE, not the item
    assert_eq!(
        SAMPLE.lines().nth(m.start as usize - 1).unwrap().trim(),
        "#[cfg(test)]"
    );
}

#[test]
fn non_test_code_has_no_spans() {
    let s = spans(SAMPLE);
    // non-vacuous on purpose: an empty span set must fail here, not
    // pass two vacuous `all`/`any` filters (the absent-evidence trap)
    assert_eq!(s.len(), 4, "mod + 3 fns: {s:?}");
    assert!(s.iter().all(|x| x.path.contains("test") || x.path.contains("smoke")));
    assert!(!s.iter().any(|x| x.path.contains("add(a")));
}
