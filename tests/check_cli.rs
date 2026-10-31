mod common;
use common::*;
use std::process::Command;

fn check(dir: &std::path::Path, body: &str, extra: &[&str]) -> (bool, serde_json::Value) {
    let docx = dir.join("t.docx");
    write_docx(&docx, &document_xml(body));
    let out = Command::new(env!("CARGO_BIN_EXE_docx-lint"))
        .current_dir(dir)
        .arg("check")
        .arg(&docx)
        .arg("--json")
        .args(extra)
        .output()
        .unwrap();
    let json = serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&out.stdout)));
    (out.status.success(), json)
}

fn codes(v: &serde_json::Value) -> Vec<String> {
    v["violations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["code"].as_str().unwrap().to_string())
        .collect()
}

const GOOD_PARA: &str = "<w:p><w:pPr><w:spacing w:line=\"360\" w:lineRule=\"auto\"/></w:pPr>\
<w:r><w:rPr><w:rFonts w:ascii=\"Times New Roman\" w:hAnsi=\"Times New Roman\"/><w:sz w:val=\"24\"/></w:rPr><w:t>Halo</w:t></w:r></w:p>";

#[test]
fn clean_document_passes() {
    let tmp = tempfile::tempdir().unwrap();
    let (ok, v) = check(tmp.path(), &format!("{GOOD_PARA}{}", good_sect_pr()), &[]);
    assert!(ok);
    assert!(codes(&v).is_empty(), "{v}");
}

#[test]
fn margin_error_flagged_and_tolerance_respected() {
    let tmp = tempfile::tempdir().unwrap();
    // left 1440 twips = 2.54cm (bad); top 2268 = 4.0cm, bottom 1701 = 3.0cm, right 1715 ~ 3.02cm (within tolerance)
    let (ok, v) = check(
        tmp.path(),
        &format!("{GOOD_PARA}{}", sect_pr(2268, 1440, 1701, 1715)),
        &[],
    );
    assert!(!ok);
    assert_eq!(codes(&v), vec!["MARGIN_ERROR"]);
}

#[test]
fn font_error_name_and_size() {
    let tmp = tempfile::tempdir().unwrap();
    let bad = "<w:p><w:r><w:rPr><w:rFonts w:ascii=\"Arial\"/><w:sz w:val=\"22\"/></w:rPr><w:t>x</w:t></w:r></w:p>";
    let (ok, v) = check(tmp.path(), &format!("{bad}{}", good_sect_pr()), &[]);
    assert!(!ok);
    assert_eq!(codes(&v), vec!["FONT_ERROR", "FONT_ERROR"]);
}

#[test]
fn spacing_error_flagged() {
    let tmp = tempfile::tempdir().unwrap();
    let bad = "<w:p><w:pPr><w:spacing w:line=\"240\" w:lineRule=\"auto\"/></w:pPr><w:r><w:t>x</w:t></w:r></w:p>";
    let (ok, v) = check(tmp.path(), &format!("{bad}{}", good_sect_pr()), &[]);
    assert!(!ok);
    assert_eq!(codes(&v), vec!["SPACING_ERROR"]);
}

#[test]
fn profile_drives_validation() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    let bin = env!("CARGO_BIN_EXE_docx-lint");
    for args in [
        vec!["config", "init", "p2"],
        vec!["config", "set", "p2", "page.left_cm", "2.54"],
    ] {
        assert!(
            Command::new(bin)
                .current_dir(d)
                .args(args)
                .status()
                .unwrap()
                .success()
        );
    }
    let body = format!("{GOOD_PARA}{}", sect_pr(2268, 1440, 1701, 1701));
    let (_, v) = check(d, &body, &["--profile", "p2"]);
    assert!(codes(&v).is_empty(), "{v}");
    let (_, v) = check(d, &body, &[]);
    assert_eq!(codes(&v), vec!["MARGIN_ERROR"]);
}

#[test]
fn missing_file_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_docx-lint"))
        .current_dir(tmp.path())
        .args(["check", "nope.docx"])
        .output()
        .unwrap();
    assert!(!out.status.success());
}
