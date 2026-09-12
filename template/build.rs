use std::process::Command;

fn output(command: &str, args: &[&str]) -> Option<String> {
    let result = Command::new(command).args(args).output().ok()?;
    result
        .status
        .success()
        .then(|| String::from_utf8_lossy(&result.stdout).trim().to_owned())
}

fn main() {
    let commit = output("git", &["rev-parse", "HEAD"]).unwrap_or_else(|| "unknown".into());
    let git_version = output("git", &["--version"])
        .and_then(|text| text.split_whitespace().nth(2).map(str::to_owned))
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=TEMPLATE_COMMIT={commit}");
    println!("cargo:rustc-env=TEMPLATE_GIT_VERSION={git_version}");
    println!("cargo:rerun-if-changed=../.git/HEAD");
}
