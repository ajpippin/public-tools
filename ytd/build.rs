use std::path::Path;
use std::process::Command;

fn output(args: &[&str]) -> Option<String> {
    let result = Command::new("git").args(args).output().ok()?;
    result
        .status
        .success()
        .then(|| String::from_utf8_lossy(&result.stdout).trim().to_owned())
        .filter(|text| !text.is_empty())
}

fn main() {
    let tag = output(&["describe", "--tags", "--exact-match"]).unwrap_or_else(|| "dev".into());
    let sha = output(&["rev-parse", "--short=7", "HEAD"]).unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=YTD_GIT_TAG={tag}");
    println!("cargo:rustc-env=YTD_GIT_SHA={sha}");
    println!("cargo:rerun-if-changed=build.rs");
    if let Some(git_dir) = output(&["rev-parse", "--absolute-git-dir"]) {
        println!("cargo:rerun-if-changed={git_dir}/HEAD");
        println!("cargo:rerun-if-changed={git_dir}/refs");
        // Only watched when present: Cargo treats a missing path as changed.
        let packed_refs = format!("{git_dir}/packed-refs");
        if Path::new(&packed_refs).exists() {
            println!("cargo:rerun-if-changed={packed_refs}");
        }
    }
}
