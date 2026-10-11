use std::path::PathBuf;

/// This build script's own arguments: the values are choices of the
/// build protocol, named so (the driver lints build scripts too).
const SYSROOT_QUERY_ARG_COUNT: usize = 2;
const RUSTC_SYSROOT_ARGS: [&str; SYSROOT_QUERY_ARG_COUNT] = ["--print", "sysroot"];
const LINK_SUBDIR: &str = "lib";
const RUSTC_PROGRAM: &str = "rustc";
const SYSROOT_ENV_OVERRIDE: &str = "SERVYI_LINT_SYSROOT";
const SYSROOT_FAILURE_MESSAGE: &str = "failed to determine rustc sysroot";

fn main() {
    let sysroot = rustc_sysroot();
    // Link against this toolchain's rustc_private libraries...
    println!(
        "cargo:rustc-link-search=native={}/{}",
        sysroot.display(),
        LINK_SUBDIR
    );
    // ...and embed the directory as a runtime loader path (rpath), so the
    // binary works as RUSTC_WRAPPER without any LD_LIBRARY_PATH setup.
    println!(
        "cargo:rustc-link-arg=-Wl,-rpath,{}/{}",
        sysroot.display(),
        LINK_SUBDIR
    );
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=SERVYI_LINT_SYSROOT");
}

/// Sysroot of the toolchain compiling THIS binary (not the wrapped one).
fn rustc_sysroot() -> PathBuf {
    std::env::var_os(SYSROOT_ENV_OVERRIDE)
        .map(PathBuf::from)
        .or_else(|| {
            let out = std::process::Command::new(RUSTC_PROGRAM)
                .args(RUSTC_SYSROOT_ARGS)
                .output()
                .ok()?;
            if !out.status.success() {
                return None;
            }
            Some(PathBuf::from(String::from_utf8_lossy(&out.stdout).trim().to_string()))
        })
        .expect(SYSROOT_FAILURE_MESSAGE)
}
