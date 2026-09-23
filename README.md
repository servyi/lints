# servyi/lints

Custom Rust lints maintained as rustc-driver wrappers. Each directory is one
installable lint; see its README for usage.

| lint | tool | what it does |
|---|---|---|
| [`unsafe-scope-lint/`](unsafe-scope-lint/README.md) | this repo (RUSTC_WRAPPER driver) | `unsafe` blocks may contain only reads of existing bindings plus a single operation that requires unsafe |

## Shared lint list (classic rules)

Classic rules are NOT re-implemented here — projects run **clippy + this
driver** in every CI. [`example-workspace-lints.toml`](example-workspace-lints.toml)
is the reusable list of clippy/rustc lints to deny everywhere; today:

| lint | tool |
|---|---|
| `clippy::all` | clippy |
| `clippy::multiple_unsafe_ops_per_block` | clippy |
| `rustc::unused_unsafe` | rustc |
| `rustc::unsafe_op_in_unsafe_fn` | rustc |
| `rustc::unused_results` / `rustc::unused_must_use` | rustc |
| `rustc::rust_2018_idioms` (warn) | rustc |
| `rustdoc::broken_intra_doc_links` | rustdoc |
| `clippy::undocumented_unsafe_blocks` | clippy |
| `clippy::unwrap_used` | clippy |
| `clippy::dbg_macro` / `clippy::todo` / `clippy::unimplemented` | clippy |

Copy the `[workspace.lints]` table into your workspace manifest and set
`[lints] workspace = true` per crate; `cargo clippy` enforces it.

## Consuming the latest lints in any CI

Check out this repo in the job and build the lint from source — no install
step, always current (pin with `ref:` for reproducible runs):

```yaml
      - uses: actions/checkout@v4
        with:
          repository: servyi/lints   # add `ref: <sha or tag>` to pin
          path: lints
      - uses: dtolnay/rust-toolchain@master
        with:
          toolchain: nightly-2026-09-21
          components: rustc-dev, clippy
      - run: cargo build --manifest-path lints/unsafe-scope-lint/Cargo.toml
      # classic rules: clippy with the shared list
      - run: cp lints/example-workspace-lints.toml ./workspace-lints.toml
      - run: >
          CARGO_TARGET_DIR="$GITHUB_WORKSPACE/target/clippy-check"
          cargo clippy --workspace --all-targets -- -D warnings
      # custom rule: the unsafe-scope driver
      - run: >
          RUSTC_WRAPPER="$GITHUB_WORKSPACE/lints/unsafe-scope-lint/target/debug/unsafe-scope-lint"
          RUSTFLAGS="-D unsafe_scope"
          CARGO_TARGET_DIR="$GITHUB_WORKSPACE/target/nightly-check"
          cargo check --workspace --all-targets
```
