# unsafe-scope-lint

A custom rustc lint (`unsafe_scope`) that reports `unsafe` blocks wrapping
more than the operations that actually require unsafe:

```rust
let x = unsafe { 1 + *p };     // warned: only `*p` requires unsafe
let x = 1 + unsafe { *p };     // fine
unsafe { *mp = 5 };            // fine: place-expression cannot be wrapped separately
```

Type-aware (knows raw pointers, unsafe fn/method/`FnPtr` calls,
`#[target_feature]` functions, `static mut`, union fields, inline asm) and
place-sensitive (`*p = x`, `&*p`, `(*p).f`, `S += 1` are correctly minimal).

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

## Reuse from any project

Install once (from this repo, a fork, or a checkout):

```bash
cargo install --path tools/unsafe-scope-lint        # or --git <url> --subdir ...
```

Then in any project (CI or locally):

```bash
RUSTC_WRAPPER=unsafe-scope-lint \
RUSTFLAGS="-D unsafe_scope" \
CARGO_TARGET_DIR=target/unsafe-scope \
cargo +nightly check --workspace --all-targets
```

Omit `-D` to see warnings without failing the build. The fixture
(`tests-fixture.rs`) doubles as a self-test: all `BAD` lines fire, all
`GOOD` lines (including the place-expression traps) stay silent.

## Version coupling

`rustc_private` ties the driver to the toolchain it was built against. The
pinned nightly in `rust-toolchain.toml` is the supported combination;
bump it deliberately (rebuild + fixture check) when moving to a newer
nightly.
