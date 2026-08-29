use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let commit_hash = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                String::from_utf8(out.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "913014e".to_string());

    let build_date = Command::new("git")
        .args(["log", "-1", "--format=%cs"])
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                String::from_utf8(out.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "2026-08-28".to_string());

    println!("cargo:rustc-env=PIGTREE_COMMIT_HASH={commit_hash}");
    println!("cargo:rustc-env=PIGTREE_BUILD_DATE={build_date}");
}
