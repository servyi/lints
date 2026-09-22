use std::path::PathBuf;

fn main() {
    let sysroot = rustc_sysroot();
    println!("cargo:rustc-link-search=native={}/lib", sysroot.display());
    println!(
        "cargo:rustc-env=USCOPE_SYSROOT_LIB={}/lib",
        sysroot.display()
    );
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=USCOPE_SYSROOT");
}

/// Sysroot of the toolchain compiling THIS binary (not the wrapped one).
fn rustc_sysroot() -> PathBuf {
    std::env::var_os("USCOPE_SYSROOT")
        .map(PathBuf::from)
        .or_else(|| {
            let out = std::process::Command::new("rustc")
                .args(["--print", "sysroot"])
                .output()
                .ok()?;
            if !out.status.success() {
                return None;
            }
            Some(PathBuf::from(String::from_utf8_lossy(&out.stdout).trim().to_string()))
        })
        .map(PathBuf::from)
        .expect("failed to determine rustc sysroot")
}
