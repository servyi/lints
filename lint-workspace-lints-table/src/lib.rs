//! `workspace_lints_table`: the workspace-level Cargo.toml defines no
//! lint tables — the shared Servyi policy is the explicit flag list
//! the CI runs (servyi/lints lint-policy action); a manifest table is
//! a second place to configure lints that silently drifts from it.

#![feature(rustc_private)]
#![allow(internal_features)]
extern crate rustc_driver;
extern crate rustc_errors;
extern crate rustc_lint;
extern crate rustc_span;

use rustc_lint::{LateContext, LateLintPass, LintContext, declare_lint, impl_lint_pass};

declare_lint! {
    /// Checks that the workspace-level Cargo.toml defines no lint tables.
    /// The shared Servyi policy is the explicit flag list the CI runs
    /// (servyi/lints lint-policy.yml); a manifest table is a second place
    /// to configure lints that silently drifts from it.
    pub WORKSPACE_LINTS_TABLE,
    Warn,
    "the workspace-level Cargo.toml defines lints"
}

pub fn register(store: &mut rustc_lint::LintStore) {
    store.register_lints(&[WORKSPACE_LINTS_TABLE]);
    store.register_late_lint_pass(Box::new(|_| Box::new(Pass)));
}

pub struct Pass;

impl_lint_pass!(Pass => [WORKSPACE_LINTS_TABLE]);

// -----------------------------------------------------------------------------
// Manifest hygiene: the workspace-level Cargo.toml must define no lints.

/// Walk up from `manifest_dir` to the workspace-level Cargo.toml the way
/// cargo does: skip directories without a manifest, stop at the nearest
/// ancestor manifest with a `[workspace]` table (the workspace root), or
/// at an ancestor package manifest without one (a nested package — the
/// crate stands alone, its own manifest is the root). Return that
/// manifest when it defines lints (`[workspace.lints]` or a `[lints]`
/// table beyond a plain `workspace = true` inheritance marker).
fn conflicting_workspace_manifest_from(
    manifest_dir: &std::path::Path,
) -> Option<std::path::PathBuf> {
    let mut own: Option<(std::path::PathBuf, toml::Table)> = None;
    let mut dir = Some(manifest_dir);
    while let Some(d) = dir {
        let manifest = d.join("Cargo.toml");
        if let Ok(src) = std::fs::read_to_string(&manifest)
            && let Ok(cfg) = src.parse::<toml::Table>()
        {
            if cfg.contains_key("workspace") {
                return defines_lints(&cfg).then_some(manifest);
            }
            if d == manifest_dir {
                own = Some((manifest, cfg));
            } else {
                break; // nested package without [workspace]
            }
        }
        dir = d.parent();
    }
    own.and_then(|(manifest, cfg)| defines_lints(&cfg).then_some(manifest))
}

fn defines_lints(cfg: &toml::Table) -> bool {
    let workspace_lints = cfg
        .get("workspace")
        .and_then(|w| w.get("lints"))
        .and_then(|l| l.as_table())
        .is_some_and(|t| !t.is_empty());
    let package_lints = cfg
        .get("lints")
        .and_then(|l| l.as_table())
        .is_some_and(|t| !(t.len() == 1 && t.contains_key("workspace")));
    workspace_lints || package_lints
}

fn conflicting_workspace_manifest() -> Option<std::path::PathBuf> {
    // Only cargo sets this; plain rustc invocations have no manifest.
    let manifest_dir = std::env::var_os("CARGO_MANIFEST_DIR")?;
    conflicting_workspace_manifest_from(std::path::Path::new(&manifest_dir))
}

impl<'tcx> LateLintPass<'tcx> for Pass {
    fn check_crate(&mut self, cx: &LateContext<'tcx>) {
        // Skip where the lint is not active (`--cap-lints allow` for
        // dependencies, `#[allow]`'d scopes).
        if !servyi_lint_core::is_active(cx, WORKSPACE_LINTS_TABLE) {
            return;
        }
        if let Some(manifest) = conflicting_workspace_manifest() {
            cx.opt_span_lint(
                WORKSPACE_LINTS_TABLE,
                None::<rustc_span::Span>,
                rustc_errors::DiagDecorator(|diag| {
                    diag.note(format!(
                        "`{}` defines lints; the workspace-level cargo.toml could conflict \
                         with the actual CI toml, resulting in confusion",
                        manifest.display()
                    ));
                }),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::conflicting_workspace_manifest_from;
    use std::path::{Path, PathBuf};

    fn scratch(name: &str, files: &[(&str, &str)]) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "lint-workspace-lints-table-test-{}-{}",
            name,
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        for (rel, content) in files {
            let p = dir.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, content).unwrap();
        }
        dir
    }

    fn member(dir: &Path) -> PathBuf {
        dir.join("crates/foo")
    }

    #[test]
    fn clean_workspace_is_silent() {
        let dir = scratch(
            "clean",
            &[
                ("Cargo.toml", "[workspace]\nmembers = [\"crates/foo\"]\n"),
                ("crates/foo/Cargo.toml", "[package]\nname = \"foo\"\n"),
            ],
        );
        assert!(conflicting_workspace_manifest_from(&member(&dir)).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn workspace_lints_table_is_flagged() {
        let dir = scratch(
            "ws-lints",
            &[
                (
                    "Cargo.toml",
                    "[workspace]\nmembers = [\"crates/foo\"]\n\n[workspace.lints.rust]\nunused = \"deny\"\n",
                ),
                ("crates/foo/Cargo.toml", "[package]\nname = \"foo\"\n"),
            ],
        );
        let hit = conflicting_workspace_manifest_from(&member(&dir)).unwrap();
        assert_eq!(hit, dir.join("Cargo.toml"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn package_lints_in_root_are_flagged() {
        let dir = scratch(
            "pkg-lints",
            &[
                (
                    "Cargo.toml",
                    "[workspace]\nmembers = [\"crates/foo\"]\n\n[lints.rust]\nunused = \"deny\"\n",
                ),
                ("crates/foo/Cargo.toml", "[package]\nname = \"foo\"\n"),
            ],
        );
        assert!(conflicting_workspace_manifest_from(&member(&dir)).is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn single_crate_root_is_checked() {
        let dir = scratch(
            "single",
            &[
                ("src/main.rs", "fn main() {}\n"),
                (
                    "Cargo.toml",
                    "[package]\nname = \"foo\"\n\n[lints.clippy]\nunwrap_used = \"deny\"\n",
                ),
            ],
        );
        let hit = conflicting_workspace_manifest_from(&dir).unwrap();
        assert_eq!(hit, dir.join("Cargo.toml"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn pure_inheritance_marker_is_not_a_definition() {
        // A root manifest whose [lints] is only the inheritance marker is
        // meaningless but harmless; only real definitions are flagged.
        let dir = scratch(
            "marker",
            &[
                ("Cargo.toml", "[package]\nname = \"foo\"\n\n[lints]\nworkspace = true\n"),
                ("src/main.rs", "fn main() {}\n"),
            ],
        );
        assert!(conflicting_workspace_manifest_from(&dir).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
