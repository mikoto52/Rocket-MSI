use std::{env, path::PathBuf, process::Command};
fn main() {
    println!("cargo:rerun-if-changed=src/action_runtime.rs");
    println!("cargo:rerun-if-changed=src/action_protocol.rs");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    println!("cargo:rerun-if-changed=assets/installer/app-icon.ico");
    tauri_winres::WindowsResource::new()
        .set_icon("assets/installer/app-icon.ico")
        .set("ProductName", "Rocket MSI")
        .set("FileDescription", "Rocket MSI Builder CLI")
        .compile_for(&["emb-msi"])
        .expect("compile CLI icon resource");
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    for (arch, target) in [
        ("x86", "i686-pc-windows-msvc"),
        ("x64", "x86_64-pc-windows-msvc"),
    ] {
        let status = Command::new(env::var_os("RUSTC").unwrap())
            .args([
                "--edition=2024",
                "--crate-type=cdylib",
                "--crate-name=emb_actions",
                "--target",
                target,
                "-C",
                "opt-level=2",
                "-C",
                "target-feature=+crt-static",
                "-C",
                "panic=abort",
                "src/action_runtime.rs",
                "-o",
            ])
            .arg(out.join(format!("emb-actions-{arch}.dll")))
            .status()
            .expect("run rustc for MSI action runtime");
        assert!(
            status.success(),
            "Cannot build {target} action runtime. Install both Rust MSVC targets and x86/x64 Visual C++ build tools."
        );
    }
}
