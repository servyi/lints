# unsafe-scope-lint

A custom rustc lint (`unsafe_scope`) enforcing the strict shape of `unsafe`
blocks: the block body may only

* read bindings that already exist (place expressions rooted at a local,
  built from field accesses and reference dereferences), and
* perform exactly one operation that requires unsafe, whose operands are
  such binding reads.

Everything else — computing arguments, arithmetic, casts, literals in
operand position, `let` statements — must be hoisted out of the block, and
the unsafe blocks are then let-chained:

```rust
let x = unsafe { 1 + *p };                   // bad: arithmetic inside
let x = 1 + unsafe { *p };                   // good
let _ = unsafe { read_at(p, compute(x)) };   // bad: computed argument
let idx = compute(x);
let _ = unsafe { read_at(p, idx) };          // good
let q = unsafe { buf.add(idx) };
let v = unsafe { *q };                       // good (let-chain)
unsafe { *mp = value };                      // good (value side is a read)
unsafe { *mp = 5 };                          // bad: literal operand
```

Type-aware: raw-pointer derefs, unsafe fn/method/`FnPtr` calls,
`#[target_feature]` functions, `static mut`, union fields and inline asm
count as unsafe operations; `&*p`, `(*p).f`, `*p = v` and `*p += v` are
recognized as single place-operations on binding reads.

Out of scope on purpose (classic rules live in rustc/clippy — see
[`example-workspace-lints.toml`](../example-workspace-lints.toml), run both
tools in CI):

* blocks containing no operation that requires unsafe at all →
  `rustc::unused_unsafe`;
* more than one unsafe operation per block →
  `clippy::multiple_unsafe_ops_per_block`.

The binary is a rustc driver: cargo invokes it in place of rustc via
`RUSTC_WRAPPER` (the same mechanism clippy uses). It re-execs itself with
the correct loader path derived from cargo's rustc argument, so no
environment setup is needed beyond the toolchain.

## Requirements

- A nightly toolchain with the `rustc-dev` component. `rust-toolchain.toml`
  pins the tested nightly; building the tool with rustup installs it
  automatically.
- The project being linted is compiled by the wrapper's own nightly (like
  clippy-driver), so run it in a separate `CARGO_TARGET_DIR` to avoid
  clobbering stable artifacts.

## Usage

```bash
cargo build --release
RUSTC_WRAPPER="$PWD/target/release/unsafe-scope-lint" \
RUSTFLAGS="-D unsafe_scope" \
CARGO_TARGET_DIR=target/unsafe-scope \
cargo check --workspace --all-targets
```

Omit `-D` to see warnings without failing the build.

## Self-test

`tests/bad/unsafe_scope/*.rs` must be denied, `tests/bad/clippy/*.rs` must
pass this driver (they are forbidden by the classic lints instead), and
`tests/good/*.rs` must stay silent under both — CI runs exactly those
loops.

## Version coupling

`rustc_private` ties the driver to the toolchain it was built against. The
pinned nightly in `rust-toolchain.toml` is the supported combination;
bump it deliberately (rebuild + fixture check) when moving to a newer
nightly.
