fn main() {
    if let Ok(n) = std::env::var("GITHUB_RUN_NUMBER") {
        if !n.is_empty() {
            println!("cargo:rustc-env=STUDIO_BUILD={n}");
        }
    }
    if let Ok(sha) = std::env::var("GITHUB_SHA") {
        let short: String = sha.chars().take(7).collect();
        if !short.is_empty() {
            println!("cargo:rustc-env=STUDIO_SHA={short}");
        }
    }
    println!("cargo:rerun-if-env-changed=GITHUB_RUN_NUMBER");
    println!("cargo:rerun-if-env-changed=GITHUB_SHA");
    shadow_gnu_default_manifest();
    tauri_build::build()
}

/// MinGW gcc always links `default-manifest.o` (an RT_MANIFEST). Tauri embeds
/// another. GNU ld cannot merge them. Put an empty object on `-B` so gcc's
/// `if-exists(default-manifest.o)` picks ours instead of MinGW's.
fn shadow_gnu_default_manifest() {
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("gnu") {
        return;
    }
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR")).join("gnu-startfiles");
    let _ = std::fs::create_dir_all(&out);
    let obj = out.join("default-manifest.o");
    let src = out.join("empty-manifest.s");
    if std::fs::write(&src, "    .section .rdata\n    .align 8\n").is_err() {
        return;
    }
    let assembled = std::process::Command::new("gcc")
        .args(["-c", "-o"])
        .arg(&obj)
        .arg(&src)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !assembled || !obj.exists() {
        return;
    }
    let mut prefix = out.display().to_string();
    if !prefix.ends_with('\\') && !prefix.ends_with('/') {
        prefix.push('\\');
    }
    println!("cargo:rustc-link-arg=-B{prefix}");
}
