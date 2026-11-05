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

#[test]
fn profile_name_with_colon_stays_in_profiles_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    for name in ["C:evil", "ab:stream"] {
        assert_eq!(run(d, &["config", "init", name]).status.code(), Some(2));
    }
    assert!(!d.join("evil.toml").exists());
    assert!(!d.join("profiles/ab").exists());
    assert!(!d.join("ab").exists());
}

#[test]
fn config_set_rejects_values_outside_schema() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    assert!(run(d, &["config", "init", "p"]).status.success());
    let file = d.join("profiles/p.toml");
    let before = std::fs::read(&file).unwrap();
    let bad = [
        ("page.top_cm", "inf"),
        ("page.top_cm", "NaN"),
        ("page.tolerance_cm", "-1"),
        ("typography.body_size_pt", "0"),
        ("typography.line_spacing", "inf"),
        ("structure.chapter_numbering", "Arabic"),
        ("structure.heading_alignment", "tengah"),
        ("structure.caption_separator", "abc"),
    ];
    for (key, value) in bad {
        let out = run(d, &["config", "set", "p", key, value]);
        assert_eq!(out.status.code(), Some(2), "{key}={value}");
        assert_eq!(std::fs::read(&file).unwrap(), before, "{key}={value}");
    }
    assert!(
        run(
            d,
            &["config", "set", "p", "structure.caption_separator", "-"]
        )
        .status
        .success()
    );
}

#[test]
fn config_set_writes_the_named_profile_file() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    assert!(run(d, &["config", "init", "other"]).status.success());
    let file = d.join("profiles/other.toml");
    // The name inside the file differs from the file name.
    let text = std::fs::read_to_string(&file)
        .unwrap()
        .replace("profile_name = \"other\"", "profile_name = \"yy\"");
    std::fs::write(&file, text).unwrap();
    assert!(
        run(d, &["config", "set", "other", "page.top_cm", "1.25"])
            .status
            .success()
    );
    assert!(
        std::fs::read_to_string(&file)
            .unwrap()
            .contains("top_cm = 1.25")
    );
    assert!(!d.join("profiles/yy.toml").exists());
    let out = run(d, &["config", "set", "other", "profile_name", "x"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(!d.join("profiles/x.toml").exists());
}

#[test]
fn config_init_refuses_existing_profile() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    assert!(run(d, &["config", "init", "p"]).status.success());
    assert!(
        run(d, &["config", "set", "p", "page.top_cm", "2.5"])
            .status
            .success()
    );
    let before = std::fs::read(d.join("profiles/p.toml")).unwrap();
    let out = run(d, &["config", "init", "p"]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(std::fs::read(d.join("profiles/p.toml")).unwrap(), before);
}

#[test]
fn invalid_profile_names_are_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    for name in ["", "a/b", "a\\b", "a.b", ".."] {
        let out = run(d, &["config", "init", name]);
        assert_eq!(out.status.code(), Some(2), "{name:?}");
    }
    let created: Vec<_> = std::fs::read_dir(d).unwrap().collect();
    assert!(created.is_empty(), "{created:?}");
}

const SCHEMA: &str = r#"
profile_name = "default_kampus"

[page]
top_cm = 4.0
left_cm = 4.0
bottom_cm = 3.0
right_cm = 3.0
tolerance_cm = 0.05

[typography]
font_name = "Times New Roman"
body_size_pt = 12.0
line_spacing = 1.5

[structure]
chapter_prefix = "BAB"
chapter_numbering = "roman"     # "roman" (BAB I) | "arabic" (BAB 1)
heading_alignment = "center"    # "center" | "left"
figure_prefix = "Gambar"
table_prefix = "Tabel"
caption_separator = "."         # "." (Gambar 1.1) | "-" (Gambar 1-1)
bibliography_heading = "DAFTAR PUSTAKA"

[academic]
forbidden_tokens = ["TODO", "TBD", "cek lagi", "lorem ipsum"]
foreign_terms_must_italicize = ["framework", "backend", "frontend", "smartphone", "database", "cache"]
"#;

#[test]
fn schema_profile_loads_with_same_values() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    std::fs::create_dir_all(d.join("profiles")).unwrap();
    std::fs::write(d.join("profiles/default_kampus.toml"), SCHEMA).unwrap();
    let shown = run(d, &["config", "show", "default_kampus"]);
    assert!(shown.status.success());
    let got: toml::Value = toml::from_str(&String::from_utf8(shown.stdout).unwrap()).unwrap();
    let want: toml::Value = toml::from_str(SCHEMA).unwrap();
    assert_eq!(got, want);

    // An integer where the schema shows a float still loads, with the same value.
    std::fs::write(
        d.join("profiles/default_kampus.toml"),
        SCHEMA.replace("top_cm = 4.0", "top_cm = 4"),
    )
    .unwrap();
    let shown = run(d, &["config", "show", "default_kampus"]);
    assert!(shown.status.success());
    let got: toml::Value = toml::from_str(&String::from_utf8(shown.stdout).unwrap()).unwrap();
    assert_eq!(got["page"]["top_cm"].as_float(), Some(4.0));
}

#[test]
fn profiles_dir_applies_to_every_subcommand() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    let doc = format!(
        "{}{}",
        "<w:p><w:r><w:t>Halo</w:t></w:r></w:p>",
        "<w:sectPr><w:pgMar w:top=\"2268\" w:right=\"1701\" w:bottom=\"1701\" w:left=\"2268\"/></w:sectPr>"
    );
    let xml = format!(
        "<w:document xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"><w:body>{doc}</w:body></w:document>"
    );
    {
        use std::io::Write;
        let mut zip = zip::ZipWriter::new(std::fs::File::create(d.join("t.docx")).unwrap());
        zip.start_file(
            "word/document.xml",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        zip.write_all(xml.as_bytes()).unwrap();
        zip.finish().unwrap();
    }
    let steps: [&[&str]; 9] = [
        &["--profiles-dir", "pd", "config", "init", "a"],
        &["config", "init", "b", "--profiles-dir", "pd"],
        &[
            "--profiles-dir",
            "pd",
            "config",
            "set",
            "a",
            "page.top_cm",
            "3.5",
        ],
        &[
            "config",
            "set",
            "b",
            "page.top_cm",
            "3.6",
            "--profiles-dir",
            "pd",
        ],
        &["--profiles-dir", "pd", "config", "use", "a"],
        &["config", "use", "b", "--profiles-dir", "pd"],
        &["--profiles-dir", "pd", "config", "show", "a"],
        &["config", "show", "--profiles-dir", "pd"],
        &["check", "t.docx", "--profile", "a", "--profiles-dir", "pd"],
    ];
    for args in steps {
        let out = run(d, args);
        // The last step lints a document with the wrong margins for profile `a`, so exit 1 is fine.
        assert!(matches!(out.status.code(), Some(0 | 1)), "{args:?}");
    }
    let check = run(
        d,
        &["--profiles-dir", "pd", "check", "t.docx", "--profile", "b"],
    );
    assert!(matches!(check.status.code(), Some(0 | 1)));
    assert!(d.join("pd/a.toml").exists() && d.join("pd/b.toml").exists());
    assert!(
        std::fs::read_to_string(d.join("pd/active_profile.txt"))
            .unwrap()
            .contains('b')
    );
    assert!(!d.join("profiles").exists());
}

#[test]
fn config_set_rejects_empty_font_name() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    assert!(run(d, &["config", "init", "p"]).status.success());
    let file = d.join("profiles/p.toml");
    let before = std::fs::read(&file).unwrap();
    for value in ["", "   "] {
        let out = run(d, &["config", "set", "p", "typography.font_name", value]);
        assert_eq!(out.status.code(), Some(2), "{value:?}");
        assert_eq!(std::fs::read(&file).unwrap(), before, "{value:?}");
    }
}
