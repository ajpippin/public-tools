pub fn text() -> String {
    let short: String = env!("TOPIC_COMMIT").chars().take(7).collect();
    format!(
        "topic v{} ({short})",
        env!("TOPIC_GIT_VERSION").trim_start_matches('v')
    )
}
