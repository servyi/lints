# servyi/lints

Custom Rust lints maintained as rustc-driver wrappers. Each directory is one
installable lint; see its README for usage.

| lint | tool | what it does |
|---|---|---|
| [`unsafe-scope-lint/`](unsafe-scope-lint/README.md) | this repo (RUSTC_WRAPPER driver) | `unsafe_scope`: `unsafe` blocks may contain only reads of existing bindings plus a single operation that requires unsafe |
| | | `workspace_lints_table`: the workspace-level Cargo.toml must define no lints (they would drift from the CI flag list) |

## Shared lint policy (classic rules)

Classic rules are NOT re-implemented here — projects run **clippy + this
driver** in every CI. The policy lives as the explicit flag list in
[`.github/actions/lint-policy/action.yml`](.github/actions/lint-policy/action.yml)
(the single source of truth); today:

| lint | tool |
|---|---|
| `clippy::all` | clippy |
| `clippy::multiple_unsafe_ops_per_block` | clippy |
| `rustc::unused_unsafe` | rustc |
| `rustc::unsafe_op_in_unsafe_fn` | rustc |
| `rustc::unused_results` / `rustc::unused_must_use` | rustc |
| `rustc::non_ascii_idents` | rustc |
| `rustc::rust_2018_idioms` (warn) | rustc |
| `rustdoc::broken_intra_doc_links` | rustdoc |
| `clippy::undocumented_unsafe_blocks` | clippy |
| `clippy::unwrap_used` / `clippy::panic` / `clippy::missing_panics_doc` | clippy |
| `clippy::dbg_macro` / `clippy::todo` / `clippy::unimplemented` | clippy |

For the same enforcement in a local plain `cargo clippy`, copy the
equivalent `[workspace.lints]` table into your workspace manifest and set
`[lints] workspace = true` per crate. CI does not depend on that — it
enforces the flags above directly.

## Mutex poisoning policy

When `.lock()` returns `PoisonError`, pick a handling **by argument** —
never a bare `unwrap()`/`expect("poisoned")`:

1. `expect("<why poisoning is impossible>")` — only pure, non-panicking
   operations run under the lock, so nothing can ever poison it.
2. `unwrap_or_else(|e| e.into_inner())` **with a comment arguing the
   state is still valid** — read-only sections, or single atomic-op
   transitions that cannot be left half-done.
3. Propagate the error — operating on possibly-broken state is worse
   than failing the operation.
4. Log the incident and restart/rebuild the service state — when the
   guarded state is recoverable (re-derivable from scratch), a poisoned
   lock is a bug report plus a rebuild, not a crash.

Test mocks are the canonical case for (2): the panicking test already
fails the run; a readable queue lets the real failure report itself.

## Consuming the policy in any CI

One step inside your own job (pin `@main` to a sha or tag for
reproducible runs):

```yaml
jobs:
  lint-policy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      # native dependencies your project needs, exactly like for your
      # own build/test jobs
      # - run: sudo apt-get install -y pkg-config libfuse3-dev
      - uses: servyi/lints/.github/actions/lint-policy@main
```

It runs clippy under the shared flag list, the `unsafe_scope` driver
(built from source — no install step), and rustdoc link checks — in
your job's environment, with your native deps.

To inline it instead, check out this repo in the job and build the lint
from source:

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
      # classic rules: clippy with the shared flag list (see
      # .github/actions/lint-policy/action.yml in this repo)
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
