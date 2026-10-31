use std::process::{Command, Output};

fn run(dir: &std::path::Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_docx-lint"))
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn init_set_use_show_persist_to_profiles_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    assert!(run(d, &["config", "init", "kampus_a"]).status.success());
    assert!(d.join("profiles/kampus_a.toml").exists());

    assert!(
        run(d, &["config", "set", "kampus_a", "page.top_cm", "3.5"])
            .status
            .success()
    );
    assert!(
        run(
            d,
            &[
                "config",
                "set",
                "kampus_a",
                "structure.chapter_numbering",
                "arabic"
            ]
        )
        .status
        .success()
    );
    let toml = std::fs::read_to_string(d.join("profiles/kampus_a.toml")).unwrap();
    assert!(toml.contains("top_cm = 3.5"), "{toml}");
    assert!(toml.contains("chapter_numbering = \"arabic\""), "{toml}");

    assert!(run(d, &["config", "use", "kampus_a"]).status.success());
    let active = std::fs::read_to_string(d.join("profiles/active_profile.txt")).unwrap();
    assert_eq!(active.trim(), "kampus_a");

    let show = run(d, &["config", "show"]);
    assert!(show.status.success());
    assert!(String::from_utf8_lossy(&show.stdout).contains("top_cm = 3.5"));
}

#[test]
fn set_rejects_unknown_key_and_missing_profile_use() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    assert!(run(d, &["config", "init", "p"]).status.success());
    assert!(
        !run(d, &["config", "set", "p", "page.nope", "1"])
            .status
            .success()
    );
    assert!(
        !run(d, &["config", "set", "p", "page.top_cm", "abc"])
            .status
            .success()
    );
    assert!(!run(d, &["config", "use", "missing"]).status.success());
}
