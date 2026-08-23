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
}
