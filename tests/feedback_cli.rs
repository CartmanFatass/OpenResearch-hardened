use std::process::Command;

#[test]
fn disabled_feedback_reports_that_nothing_was_sent() {
    let root = std::env::temp_dir().join(format!("orx-feedback-cli-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_orx"))
        .env("HOME", &root)
        .env("USERPROFILE", &root)
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env("ORX_DATA_DIR", root.join("data"))
        .env("ORX_NO_UPDATE_CHECK", "1")
        .env("ORX_NO_TELEMETRY", "1")
        .args([
            "--no-telemetry",
            "feedback",
            "--kind",
            "bug",
            "--summary",
            "synthetic test",
            "--details",
            "synthetic data only",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Feedback not sent:"), "{stdout}");
    assert!(!stdout.contains("Feedback submitted"), "{stdout}");
    assert!(!root.join("config/openresearch/settings.json").exists());
    std::fs::remove_dir_all(root).unwrap();
}
