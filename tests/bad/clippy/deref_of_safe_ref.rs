// Bad case for the shared clippy/rustc lint list, NOT for `unsafe_scope`:
// `add_ref` returns a plain `&u32`, so the block performs no operation that
// requires unsafe. rustc's `unused_unsafe` forbids it (see
// example-workspace-lints.toml); `unsafe_scope` deliberately passes this
// file.
fn add_ref(v: &[u32], idx: usize) -> &u32 {
    &v[idx]
}

fn main() {
    let data = [7u32, 8, 9];
    let idx = 1usize;
    let r = add_ref(&data, idx);
    let _ = unsafe { *r };
}
