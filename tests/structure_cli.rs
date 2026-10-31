mod common;
use common::*;

fn body(parts: &[String]) -> String {
    format!("{}{}", parts.concat(), good_sect_pr())
}

#[test]
fn roman_headings_ok_arabic_flagged() {
    let tmp = tempfile::tempdir().unwrap();
    let ok = body(&[
        heading_para("BAB I", "center"),
        heading_para("BAB IV", "center"),
    ]);
    let (pass, v) = lint_body(tmp.path(), &ok, &[]);
    assert!(pass, "{v}");
    let bad = body(&[
        heading_para("BAB 1", "center"),
        heading_para("BAB IIX", "center"),
    ]);
    let (pass, v) = lint_body(tmp.path(), &bad, &[]);
    assert!(!pass);
    assert_eq!(viol_codes(&v), vec!["HEADING_ERROR", "HEADING_ERROR"]);
}

#[test]
fn sentence_starting_with_prefix_is_not_a_heading() {
    let tmp = tempfile::tempdir().unwrap();
    let b = body(&[good_para("BAB selanjutnya membahas metode")]);
    let (pass, v) = lint_body(tmp.path(), &b, &[]);
    assert!(pass, "{v}");
}

#[test]
fn heading_alignment_checked() {
    let tmp = tempfile::tempdir().unwrap();
    let (pass, v) = lint_body(tmp.path(), &body(&[heading_para("BAB I", "left")]), &[]);
    assert!(!pass);
    assert_eq!(viol_codes(&v), vec!["HEADING_ERROR"]);
}

#[test]
fn captions_numbered_per_chapter() {
    let tmp = tempfile::tempdir().unwrap();
    let ok = body(&[
        heading_para("BAB I", "center"),
        good_para("Gambar 1.1 Arsitektur"),
        good_para("Tabel 1.1 Data"),
        good_para("Gambar 1.2 Alur"),
        heading_para("BAB II", "center"),
        good_para("Gambar 2.1 Model"),
    ]);
    let (pass, v) = lint_body(tmp.path(), &ok, &[]);
    assert!(pass, "{v}");

    let bad = body(&[
        heading_para("BAB I", "center"),
        good_para("Gambar 1-1 Arsitektur"), // wrong separator
        good_para("Gambar 2.2 Alur"),       // wrong chapter
        good_para("Tabel 1.3 Data"),        // skipped sequence
    ]);
    let (pass, v) = lint_body(tmp.path(), &bad, &[]);
    assert!(!pass);
    assert_eq!(viol_codes(&v), vec!["CAPTION_SEQ_ERROR"; 3]);
}

#[test]
fn switching_profile_changes_validation() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    let doc = body(&[
        heading_para("BAB 1", "center"),
        good_para("Gambar 1-1 Arsitektur"),
    ]);
    let (pass, v) = lint_body(d, &doc, &[]);
    assert!(!pass);
    assert_eq!(viol_codes(&v), vec!["HEADING_ERROR", "CAPTION_SEQ_ERROR"]);

    config(d, &["init", "arab"]);
    config(d, &["set", "arab", "structure.chapter_numbering", "arabic"]);
    config(d, &["set", "arab", "structure.caption_separator", "-"]);
    let (pass, v) = lint_body(d, &doc, &["--profile", "arab"]);
    assert!(pass, "{v}");
}
