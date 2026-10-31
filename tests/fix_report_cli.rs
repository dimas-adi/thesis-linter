mod common;
use common::*;
use std::io::Read;
use std::process::Command;

const BAD_PARA: &str = "<w:p><w:pPr><w:spacing w:line=\"240\" w:lineRule=\"auto\"/></w:pPr>\
<w:r><w:rPr><w:rFonts w:ascii=\"Arial\" w:hAnsi=\"Arial\"/><w:sz w:val=\"22\"/></w:rPr>\
<w:t xml:space=\"preserve\">dua  spasi &amp; </w:t></w:r><w:r><w:t xml:space=\"preserve\"> lanjut</w:t></w:r></w:p>";

fn run(dir: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_docx-lint"))
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn fix_clears_fixable_violations_and_keeps_package_parts() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    let body = format!("{BAD_PARA}{}", sect_pr(1440, 1440, 1440, 1440));
    write_docx(&d.join("in.docx"), &document_xml(&body));

    let out = run(d, &["check", "in.docx", "--fix", "out.docx", "--json"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(!viol_codes(&json).is_empty());
    assert!(json["remaining"].as_array().unwrap().is_empty());

    // The fixed file lints clean on its own.
    assert!(run(d, &["check", "out.docx"]).status.success());

    let mut zip = zip::ZipArchive::new(std::fs::File::open(d.join("out.docx")).unwrap()).unwrap();
    assert!(zip.by_name("[Content_Types].xml").is_ok());
    let mut xml = String::new();
    zip.by_name("word/document.xml")
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    assert!(xml.contains("dua spasi &amp; "), "{xml}");
}

#[test]
fn fix_reports_remaining_unfixable() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    let body = format!("{}{}", good_para("TODO isi"), good_sect_pr());
    write_docx(&d.join("in.docx"), &document_xml(&body));
    let out = run(d, &["check", "in.docx", "--fix", "out.docx", "--json"]);
    assert!(!out.status.success());
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(json["remaining"][0]["code"], "DRAFT_TOKEN_ERROR");
}

#[test]
fn html_report_lists_and_escapes_violations() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    let bad =
        "<w:p><w:r><w:rPr><w:rFonts w:ascii=\"A&amp;B&lt;i&gt;\"/></w:rPr><w:t>x</w:t></w:r></w:p>";
    write_docx(
        &d.join("in.docx"),
        &document_xml(&format!("{bad}{}", good_sect_pr())),
    );
    let out = run(d, &["check", "in.docx", "--html", "r.html"]);
    assert!(!out.status.success());
    let html = std::fs::read_to_string(d.join("r.html")).unwrap();
    assert!(html.contains("<table") && html.contains("FONT_ERROR"));
    assert!(html.contains("A&amp;B&lt;i&gt;"), "{html}");
    assert!(!html.contains("<i>"));
}
