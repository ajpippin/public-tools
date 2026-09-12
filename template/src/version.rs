pub fn text() -> String {
    version_text(env!("TEMPLATE_GIT_VERSION"), env!("TEMPLATE_COMMIT"))
}

fn version_text(git_version: &str, commit: &str) -> String {
    let short: String = commit.chars().take(7).collect();
    format!(
        "template v{} ({short})",
        git_version.trim_start_matches('v')
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_version() {
        assert_eq!(
            version_text("2.50.1", "c92b9a4123456789"),
            "template v2.50.1 (c92b9a4)"
        );
    }
}
