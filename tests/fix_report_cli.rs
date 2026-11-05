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

fn fix_input_bytes(d: &std::path::Path) -> Vec<u8> {
    let body = format!("{BAD_PARA}{}", sect_pr(1440, 1440, 1440, 1440));
    write_docx(&d.join("in.docx"), &document_xml(&body));
    std::fs::read(d.join("in.docx")).unwrap()
}

#[test]
fn fix_rejects_aliased_output_paths() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    let before = fix_input_bytes(d);
    std::fs::create_dir(d.join("sub")).unwrap();
    std::fs::hard_link(d.join("in.docx"), d.join("link.docx")).unwrap();
    let absolute = d.join("in.docx").display().to_string();
    let mut spellings = vec![
        "./in.docx".to_string(),
        "sub/../in.docx".to_string(),
        "link.docx".to_string(),
        absolute,
    ];
    if cfg!(windows) {
        spellings.extend([r".\in.docx".to_string(), "IN.DOCX".to_string()]);
    }
    for out_path in &spellings {
        let out = run(d, &["check", "in.docx", "--fix", out_path]);
        assert_eq!(out.status.code(), Some(2), "{out_path}");
        assert_eq!(
            std::fs::read(d.join("in.docx")).unwrap(),
            before,
            "{out_path}"
        );
    }
}

#[test]
fn failed_fix_keeps_existing_output_and_leaves_no_temp_file() {
    use std::io::Write;
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    let doc = document_xml(&good_para("Halo"));
    let blob = "ABCDEFGHIJKLMNOP";
    let mut zip = zip::ZipWriter::new(std::fs::File::create(d.join("in.docx")).unwrap());
    let stored =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    zip.start_file("word/blob.bin", stored).unwrap();
    zip.write_all(blob.as_bytes()).unwrap();
    zip.start_file("word/document.xml", stored).unwrap();
    zip.write_all(doc.as_bytes()).unwrap();
    zip.finish().unwrap();
    // WHY: flipping a stored byte breaks the CRC of the first entry, which the fixer streams.
    let mut bytes = std::fs::read(d.join("in.docx")).unwrap();
    let at = bytes
        .windows(blob.len())
        .position(|w| w == blob.as_bytes())
        .unwrap();
    bytes[at] ^= 0xff;
    std::fs::write(d.join("in.docx"), &bytes).unwrap();
    std::fs::write(d.join("out.docx"), b"previous output").unwrap();

    let out = run(d, &["check", "in.docx", "--fix", "out.docx"]);
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(
        std::fs::read(d.join("out.docx")).unwrap(),
        b"previous output"
    );
    let mut names: Vec<_> = std::fs::read_dir(d)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, ["in.docx", "out.docx"]);
}

#[test]
fn html_output_cannot_overwrite_input() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    let before = fix_input_bytes(d);
    for out_path in ["in.docx", "./in.docx"] {
        let out = run(d, &["check", "in.docx", "--html", out_path]);
        assert_eq!(out.status.code(), Some(2), "{out_path}");
        assert_eq!(
            std::fs::read(d.join("in.docx")).unwrap(),
            before,
            "{out_path}"
        );
    }
}

#[test]
fn fix_never_modifies_input_bytes() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    let before = fix_input_bytes(d);
    let out = run(d, &["check", "in.docx", "--fix", "out.docx"]);
    assert!(out.status.success());
    assert_eq!(std::fs::read(d.join("in.docx")).unwrap(), before);
    assert_ne!(std::fs::read(d.join("out.docx")).unwrap(), before);
}

fn entries(path: &std::path::Path) -> Vec<(String, bool, Vec<u8>)> {
    let mut zip = zip::ZipArchive::new(std::fs::File::open(path).unwrap()).unwrap();
    (0..zip.len())
        .map(|i| {
            let mut e = zip.by_index(i).unwrap();
            let mut bytes = Vec::new();
            e.read_to_end(&mut bytes).unwrap();
            (e.name().unwrap().into_owned(), e.is_dir(), bytes)
        })
        .collect()
}

#[test]
fn fix_preserves_other_package_entries_byte_for_byte() {
    use std::io::Write;
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    let blob: Vec<u8> = (0..=255u8).cycle().take(4096).collect();
    let doc = document_xml(&format!("{BAD_PARA}{}", sect_pr(1440, 1440, 1440, 1440)));
    let mut zip = zip::ZipWriter::new(std::fs::File::create(d.join("in.docx")).unwrap());
    let opts = zip::write::SimpleFileOptions::default();
    let stored = opts.compression_method(zip::CompressionMethod::Stored);
    zip.start_file("[Content_Types].xml", opts).unwrap();
    zip.write_all(b"<Types/>").unwrap();
    zip.start_file("word/media/blob.bin", stored).unwrap();
    zip.write_all(&blob).unwrap();
    zip.start_file("word/empty.xml", opts).unwrap();
    zip.add_directory("word/media/empty-dir/", opts).unwrap();
    zip.start_file("word/document.xml", opts).unwrap();
    zip.write_all(doc.as_bytes()).unwrap();
    zip.finish().unwrap();

    assert!(
        run(d, &["check", "in.docx", "--fix", "out.docx"])
            .status
            .success()
    );
    let (before, after) = (entries(&d.join("in.docx")), entries(&d.join("out.docx")));
    assert_eq!(
        before.iter().map(|e| (&e.0, e.1)).collect::<Vec<_>>(),
        after.iter().map(|e| (&e.0, e.1)).collect::<Vec<_>>()
    );
    for (b, a) in before.iter().zip(&after) {
        if b.0 == "word/document.xml" {
            assert_ne!(b.2, a.2);
        } else {
            assert_eq!(b.2, a.2, "{}", b.0);
        }
    }
}

#[test]
fn fixer_keeps_cover_and_table_exemptions() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    let sized = |text: &str, half: u32, line: u32| {
        format!(
            "<w:p><w:pPr><w:spacing w:line=\"{line}\" w:lineRule=\"auto\"/></w:pPr>\
<w:r><w:rPr><w:rFonts w:ascii=\"Times New Roman\" w:hAnsi=\"Times New Roman\"/><w:sz w:val=\"{half}\"/></w:rPr>\
<w:t>{text}</w:t></w:r></w:p>"
        )
    };
    let body = format!(
        "{}{}<w:tbl><w:tr><w:tc>{}</w:tc></w:tr></w:tbl>{}{}",
        sized("JUDUL", 32, 360),
        heading_para("BAB I", "center"),
        sized("sel", 20, 240),
        sized("isi", 28, 240),
        good_sect_pr()
    );
    write_docx(&d.join("in.docx"), &document_xml(&body));
    run(d, &["check", "in.docx", "--fix", "out.docx"]);
    let xml = String::from_utf8(
        entries(&d.join("out.docx"))
            .into_iter()
            .find(|e| e.0 == "word/document.xml")
            .unwrap()
            .2,
    )
    .unwrap();
    // Cover 16 pt and table 10 pt / single spacing stay; the body line is corrected.
    assert_eq!(xml.matches("w:sz w:val=\"32\"").count(), 1, "{xml}");
    assert_eq!(xml.matches("w:sz w:val=\"20\"").count(), 1, "{xml}");
    assert_eq!(xml.matches("w:line=\"240\"").count(), 1, "{xml}");
    assert_eq!(xml.matches("w:sz w:val=\"24\"").count(), 1, "{xml}");
    assert!(!xml.contains("w:sz w:val=\"28\""), "{xml}");
}

#[test]
fn html_report_is_self_contained_and_reports_clean_run() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    write_docx(
        &d.join("ok.docx"),
        &document_xml(&format!("{}{}", good_para("Halo"), good_sect_pr())),
    );
    let out = run(d, &["check", "ok.docx", "--html", "r.html"]);
    assert!(out.status.success());
    let html = std::fs::read_to_string(d.join("r.html")).unwrap();
    for banned in ["http", "src=", "href=", "url("] {
        assert!(!html.contains(banned), "{banned} in {html}");
    }
    assert!(html.contains("No violations found."), "{html}");
}
