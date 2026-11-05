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

#[test]
fn sentence_starting_with_caption_label_is_not_a_caption() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    let b = body(&[
        heading_para("BAB I", "center"),
        good_para("Gambar 1.1 Arsitektur sistem"),
        good_para("Gambar 1.1 menunjukkan arsitektur sistem."),
        good_para("Gambar 1.2 Alur data"),
    ]);
    assert!(viol_codes(&lint_body(d, &b, &[]).1).is_empty());
}

#[test]
fn sentence_starting_with_chapter_number_is_not_a_heading() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    let b = body(&[
        heading_para("BAB I", "center"),
        good_para("Gambar 1.1 Satu"),
        good_para("BAB II membahas landasan teori."),
        good_para("Gambar 1.2 Dua"),
    ]);
    assert!(viol_codes(&lint_body(d, &b, &[]).1).is_empty());
}

#[test]
fn style_aligned_heading_without_jc_is_not_flagged() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    let styled =
        "<w:p><w:pPr><w:pStyle w:val=\"Heading1\"/></w:pPr><w:r><w:t>BAB I</w:t></w:r></w:p>";
    let plain = "<w:p><w:r><w:t>BAB II</w:t></w:r></w:p>";
    let (_, v) = lint_body(d, &format!("{styled}{}", good_sect_pr()), &[]);
    assert!(viol_codes(&v).is_empty(), "{v}");
    let (_, v) = lint_body(d, &format!("{plain}{}", good_sect_pr()), &[]);
    assert_eq!(viol_codes(&v), vec!["HEADING_ERROR"]);
}

#[test]
fn toc_lines_are_not_chapter_headings() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    let pageref = "<w:p><w:r><w:t>BAB III METODE</w:t></w:r><w:r><w:tab/></w:r>\
<w:r><w:fldChar w:fldCharType=\"begin\"/></w:r><w:r><w:instrText> PAGEREF _Toc1 \\h </w:instrText></w:r>\
<w:r><w:fldChar w:fldCharType=\"end\"/></w:r><w:r><w:t>7</w:t></w:r></w:p>";
    // WHY: a 14 pt line is allowed only in front matter, so a false heading would flag it.
    let cover = "<w:p><w:pPr><w:spacing w:line=\"360\" w:lineRule=\"auto\"/></w:pPr><w:r><w:rPr><w:sz w:val=\"28\"/></w:rPr><w:t>Nama Penulis</w:t></w:r></w:p>";
    let b = body(&[
        good_para("DAFTAR ISI"),
        good_para("BAB I PENDAHULUAN ........ 1"),
        good_para("BAB II TINJAUAN PUSTAKA 5"),
        pageref.to_string(),
        cover.to_string(),
        heading_para("BAB I", "center"),
        good_para("Gambar 1.1 Satu"),
    ]);
    let (_, v) = lint_body(d, &b, &[]);
    assert!(viol_codes(&v).is_empty(), "{v}");
}
