# unsafe-scope-lint

Custom rustc lints for the shared Servyi policy, run as one
`RUSTC_WRAPPER` driver:

| lint | default | what it does |
|---|---|---|
| `unsafe_scope` | warn | strict shape of `unsafe` blocks (below) |
| `custom_parser` | warn | hand-parsing primitives only inside sanctioned modules (below) |
| `workspace_lints_table` | warn | the workspace-level Cargo.toml defines no lints |
| `non_test_panic_allow` | warn | `#[allow]`/`#[expect]` of `clippy::panic`/`clippy::unwrap_used` outside test code |
| `unsound_constructor` | warn | `#[servyi::unsound_constructor]` types need `SAFETY` comments at the struct and at every construction site |

## `unsafe_scope`

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

Out of scope on purpose (classic rules live in rustc/clippy — see the
shared flag list in [`action.yml`](../.github/actions/lint-policy/action.yml),
run both tools in CI):

* blocks containing no operation that requires unsafe at all →
  `rustc::unused_unsafe`;
* more than one unsafe operation per block →
  `clippy::multiple_unsafe_ops_per_block`.

The binary is a rustc driver: cargo invokes it in place of rustc via
`RUSTC_WRAPPER` (the same mechanism clippy uses). Its build bakes the
toolchain's library directory as an rpath, so it runs with no environment
setup beyond the toolchain itself.

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

## `workspace_lints_table`

The shared Servyi lint policy is the explicit flag list the CI runs (see
`../.github/actions/lint-policy/action.yml`); lints configured in the
workspace-level `Cargo.toml` are a second source of truth that silently
drifts from it. The lint fires when the workspace-level manifest (the
nearest ancestor with a `[workspace]` table, or the crate's own manifest
when it stands alone) defines `[workspace.lints]` or a `[lints]` table
beyond a plain `workspace = true` inheritance marker:

```text
warning: `Cargo.toml` defines lints; the workspace-level cargo.toml could
         conflict with the actual CI toml, resulting in confusion
```

Only active under cargo (a bare `rustc` invocation has no manifest), and
respects `--cap-lints`, so dependencies are never flagged. The
`lint-policy` action denies it: `-D workspace_lints_table`.

## `non_test_panic_allow`

A panicking test IS the failure signal, so silencing `clippy::panic` /
`clippy::unwrap_used` is a test-only exemption. The lint fires on bare
`#[allow]`/`#[expect]` of those lints in non-test compilations:

```text
warning: `allow(unwrap_used)` silences the panic policy outside test code:
         a panicking test is the failure signal, but production code must
         handle or propagate the case instead; gate exemptions with
         `cfg_attr(test, ...)` or move them into `#[cfg(test)]` code
```

The sanctioned forms never trip it: without `--test`, `cfg_attr(test, ...)`
is not expanded and `#[cfg(test)]` modules are not compiled at all;
integration-test targets compile with `--test`. The `lint-policy` action
denies it: `-D non_test_panic_allow`.

## `unsound_constructor`

Some types can be constructed into aliasing/invalid states from SAFE code
(the construction is where their invariants get established) — the borrow
checker cannot help. Mark such a type:

```rust
#![feature(register_tool)]
#![register_tool(servyi)]

/// A lease over a child of the checkpoint tree.
///
/// SAFETY (constructor): the struct literal aliases the split item's
/// storage; sound only when exactly one handle exists per disjoint
/// split item — construction sites must prove exactly that.
#[servyi::unsound_constructor]
pub struct Handle<'o, T> { .. }

fn lease(child: &'o mut C) -> Handle<'o, C> {
    // SAFETY: `child` is a disjoint split item; this handle is its only
    // lease.
    Handle { .. }
}
```

The lint fires when either SAFETY comment is missing. Accepted styles:
`///`/`//` line comments or `/* */` blocks, either directly above the
literal/struct or — for constructions — inside the literal's braces next
to the fields; the comment must contain `SAFETY`. The diagnostics spell
out what each comment should say (the preconditions at the struct, the
site's argument for them at every construction). Registering the `servyi`
tool needs the (nightly) `register_tool` feature; consumers on the pinned
nightly get it for free. The `lint-policy` action denies it:
`-D unsound_constructor`; the fixture battery covers positive and
negative cases (tests/good/unsound_constructor_ok.rs,
tests/bad/unsound_constructor/).

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

## `custom_parser` (issue #9)

Flags calls to "typical functions used for parsing strings" — the
primitives people reach for when hand-rolling a parser — everywhere
except a sanctioned custom-parser module. Plain `split` is the one
accepted everyday idiom and is deliberately exempt.

Flagged primitives: `split_once`/`rsplit_once`, `split_terminator`,
`split_inclusive`, `splitn`, `split_at` (+ `_checked`, str and
slices), `strip_prefix`/`strip_suffix`, `split_first`/`split_last`
(token consumption), `trim_matches`/`trim_start_matches`/
`trim_end_matches`, `starts_with`/`ends_with`, `trim_start`/
`trim_end`, `find`/`rfind` (index cursors — containment checks
should use `contains`), `match_indices`, `char_indices`,
`Chars::as_str` (the lexer-cursor advance), `is_char_boundary`,
`char::to_digit` (use `str::parse`/`from_str_radix`).

Deliberately NOT flagged (everyday or sanctioned): `split`,
`split_whitespace`, `trim`, `contains`, `str::parse`,
`from_str_radix`, `str::from_utf8` (conversion), `as_bytes` (I/O and
hashing), generic `Iterator::position`/`peek`/`next` (not
string-specific), `chunks`/`windows` (general data processing).

The driver lints ITSELF in CI (the self-apply step) — the lint's own
def-path matcher and header scanner live in
`src/custom_parser.rs`, a sanctioned module carrying this header;
the one argv-suffix probe in `main` carries a reviewed
`#[allow(custom_parser)]`.

The protocol when it fires (and in the diagnostic):

1. first check that no std function or external crate already does
   this parsing job;
2. if hand-rolling is necessary, agree on the approach and design
   with a human supervisor;
3. once agreed, isolate the parser in a submodule headed by

```rust
use ...;

/// WARNING: CUSTOM PARSER — why this parser must be hand-written
```

   with only `use` directives (and crate attributes / blank lines)
   above the explanation. Files carrying that header are exempt from
   the lint wholesale. `#[allow(custom_parser)]` remains the per-site
   escape.
