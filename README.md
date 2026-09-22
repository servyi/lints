# servyi/lints

Custom Rust lints maintained as rustc-driver wrappers. Each directory is one
installable lint; see its README for usage.

| lint | what it does |
|---|---|
| [`unsafe-scope-lint/`](unsafe-scope-lint/README.md) | `unsafe` blocks must wrap only the operations that require unsafe |

## Consuming the latest lints in any CI

Check out this repo in the job and build the lint from source — no install
step, always current (pin with `ref:` for reproducible runs):

```yaml
      - uses: actions/checkout@v4
        with:
          repository: servyi/lints   # add `ref: <sha or tag>` to pin
          path: lints
      - uses: dtolnay/rust-toolchain@nightly-2026-09-21
        with:
          components: rustc-dev
      - run: cargo build --manifest-path lints/unsafe-scope-lint/Cargo.toml
      - run: >
          RUSTC_WRAPPER="$GITHUB_WORKSPACE/lints/unsafe-scope-lint/target/debug/unsafe-scope-lint"
          RUSTFLAGS="-D unsafe_scope"
          CARGO_TARGET_DIR="$GITHUB_WORKSPACE/target/nightly-check"
          cargo check --workspace --all-targets
```
