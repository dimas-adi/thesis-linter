mod common;
use common::*;
use std::process::Command;

const GOOD_PARA: &str = "<w:p><w:pPr><w:spacing w:line=\"360\" w:lineRule=\"auto\"/></w:pPr>\
<w:r><w:rPr><w:rFonts w:ascii=\"Times New Roman\" w:hAnsi=\"Times New Roman\"/><w:sz w:val=\"24\"/></w:rPr><w:t>Halo</w:t></w:r></w:p>";

#[test]
fn clean_document_passes() {
    let tmp = tempfile::tempdir().unwrap();
    let (ok, v) = lint_body(tmp.path(), &format!("{GOOD_PARA}{}", good_sect_pr()), &[]);
    assert!(ok);
    assert!(viol_codes(&v).is_empty(), "{v}");
}

#[test]
fn margin_error_flagged_and_tolerance_respected() {
    let tmp = tempfile::tempdir().unwrap();
    // left 1440 twips = 2.54cm (bad); top 2268 = 4.0cm, bottom 1701 = 3.0cm, right 1715 ~ 3.02cm (within tolerance)
    let (ok, v) = lint_body(
        tmp.path(),
        &format!("{GOOD_PARA}{}", sect_pr(2268, 1440, 1701, 1715)),
        &[],
    );
    assert!(!ok);
    assert_eq!(viol_codes(&v), vec!["MARGIN_ERROR"]);
}

#[test]
fn font_error_name_and_size() {
    let tmp = tempfile::tempdir().unwrap();
    let bad = "<w:p><w:r><w:rPr><w:rFonts w:ascii=\"Arial\"/><w:sz w:val=\"22\"/></w:rPr><w:t>x</w:t></w:r></w:p>";
    let (ok, v) = lint_body(tmp.path(), &format!("{bad}{}", good_sect_pr()), &[]);
    assert!(!ok);
    assert_eq!(viol_codes(&v), vec!["FONT_ERROR", "FONT_ERROR"]);
}

#[test]
fn spacing_error_flagged() {
    let tmp = tempfile::tempdir().unwrap();
    let bad = "<w:p><w:pPr><w:spacing w:line=\"240\" w:lineRule=\"auto\"/></w:pPr><w:r><w:t>x</w:t></w:r></w:p>";
    let (ok, v) = lint_body(tmp.path(), &format!("{bad}{}", good_sect_pr()), &[]);
    assert!(!ok);
    assert_eq!(viol_codes(&v), vec!["SPACING_ERROR"]);
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
    let (_, v) = lint_body(d, &body, &["--profile", "p2"]);
    assert!(viol_codes(&v).is_empty(), "{v}");
    let (_, v) = lint_body(d, &body, &[]);
    assert_eq!(viol_codes(&v), vec!["MARGIN_ERROR"]);
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

#[test]
fn oversized_document_xml_is_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let docx = tmp.path().join("big.docx");
    write_docx(
        &docx,
        &document_xml(&format!("{GOOD_PARA}{}", good_sect_pr())),
    );
    let size = docx_lint::linter::read_document_xml(&docx).unwrap().len() as u64;
    assert!(docx_lint::linter::read_document_xml_limited(&docx, size).is_ok());
    let err = docx_lint::linter::read_document_xml_limited(&docx, size - 1).unwrap_err();
    assert!(err.to_string().contains("limit"), "{err}");
}

#[test]
fn malformed_xml_variants_exit_2() {
    let ns = "xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\"";
    let variants = [
        String::new(),
        "   \n".to_string(),
        "plain text".to_string(),
        format!("<w:document {ns}><w:body><w:p><w:r><w:t>x"),
        format!("<w:document {ns}/><w:document {ns}/>"),
        format!(
            "<w:document {ns}><w:body><w:p><w:r><w:t>&foo;</w:t></w:r></w:p></w:body></w:document>"
        ),
        format!(
            "<w:document {ns}><w:body><w:p><w:r><w:t>&#xFFFFFFFF;</w:t></w:r></w:p></w:body></w:document>"
        ),
    ];
    for (i, xml) in variants.iter().enumerate() {
        for fix in [false, true] {
            let tmp = tempfile::tempdir().unwrap();
            let d = tmp.path();
            write_docx(&d.join("in.docx"), xml);
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_docx-lint"));
            cmd.current_dir(d).args(["check", "in.docx"]);
            if fix {
                cmd.args(["--fix", "out.docx"]);
            }
            let out = cmd.output().unwrap();
            assert_eq!(out.status.code(), Some(2), "variant {i} fix={fix}");
            assert!(
                String::from_utf8_lossy(&out.stderr).contains("error:"),
                "variant {i}"
            );
            assert!(!d.join("out.docx").exists(), "variant {i}");
        }
    }
}

#[test]
fn profiles_dir_without_subcommand_exits_2() {
    let out = Command::new(env!("CARGO_BIN_EXE_docx-lint"))
        .args(["--profiles-dir", "x"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn caption_number_overflow_does_not_panic() {
    let tmp = tempfile::tempdir().unwrap();
    let body = format!(
        "{}{}{}{}{}",
        heading_para("BAB I", "center"),
        good_para("Gambar 1.4294967295 Pertama"),
        good_para("Gambar 1.1 Kedua"),
        good_para("Gambar 1.2 Ketiga"),
        good_sect_pr()
    );
    let docx = tmp.path().join("t.docx");
    write_docx(&docx, &document_xml(&body));
    let out = Command::new(env!("CARGO_BIN_EXE_docx-lint"))
        .current_dir(tmp.path())
        .args(["check", "t.docx"])
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn closed_stdout_does_not_panic() {
    let tmp = tempfile::tempdir().unwrap();
    let docx = tmp.path().join("t.docx");
    write_docx(
        &docx,
        &document_xml(&format!("{GOOD_PARA}{}", good_sect_pr())),
    );
    let mut child = Command::new(env!("CARGO_BIN_EXE_docx-lint"))
        .current_dir(tmp.path())
        .args(["check", "t.docx"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    // WHY: dropping the read end before the child writes makes its stdout a broken pipe.
    drop(child.stdout.take());
    let out = child.wait_with_output().unwrap();
    assert_ne!(
        out.status.code(),
        Some(101),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

fn run_bin(dir: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_docx-lint"))
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn exit_codes_follow_contract() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    write_docx(
        &d.join("ok.docx"),
        &document_xml(&format!("{GOOD_PARA}{}", good_sect_pr())),
    );
    write_docx(
        &d.join("bad.docx"),
        &document_xml(&format!("{GOOD_PARA}{}", sect_pr(1440, 1440, 1440, 1440))),
    );
    assert_eq!(run_bin(d, &["check", "ok.docx"]).status.code(), Some(0));
    assert_eq!(run_bin(d, &["check", "bad.docx"]).status.code(), Some(1));
    assert_eq!(run_bin(d, &["check", "nope.docx"]).status.code(), Some(2));
    assert_eq!(run_bin(d, &["config", "init", "p"]).status.code(), Some(0));
    assert_eq!(
        run_bin(d, &["config", "use", "missing"]).status.code(),
        Some(2)
    );
}

#[test]
fn json_output_has_contract_fields() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    let bad = format!("{GOOD_PARA}{}", sect_pr(1440, 1440, 1440, 1440));
    write_docx(&d.join("bad.docx"), &document_xml(&bad));
    let out = run_bin(d, &["check", "bad.docx", "--json"]);
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(json["file"].is_string() && json["profile"].is_string());
    let first = &json["violations"][0];
    assert!(first["code"].is_string() && first["message"].is_string());
    assert!(first["paragraph"].is_u64());
    assert!(json.get("fixed_file").is_none() && json.get("remaining").is_none());

    let out = run_bin(d, &["check", "bad.docx", "--json", "--fix", "out.docx"]);
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(json["fixed_file"], "out.docx");
    assert!(json["remaining"].is_array());
}

#[test]
fn violation_codes_are_the_documented_set() {
    let tmp = tempfile::tempdir().unwrap();
    let bad = "<w:p><w:pPr><w:spacing w:line=\"240\" w:lineRule=\"auto\"/></w:pPr><w:r>\
<w:rPr><w:rFonts w:ascii=\"Arial\" w:hAnsi=\"Arial\"/><w:sz w:val=\"24\"/></w:rPr>\
<w:t xml:space=\"preserve\">TODO dua  spasi framework (Smith, 2020)</w:t></w:r></w:p>";
    let body = format!(
        "{}{}{bad}{}",
        heading_para("BAB 1", "center"),
        good_para("Gambar 1-1 Salah"),
        sect_pr(1440, 1440, 1440, 1440)
    );
    let (_, v) = lint_body(tmp.path(), &body, &[]);
    let mut found = viol_codes(&v);
    found.sort();
    found.dedup();
    let mut expected = [
        "MARGIN_ERROR",
        "FONT_ERROR",
        "SPACING_ERROR",
        "HEADING_ERROR",
        "CAPTION_SEQ_ERROR",
        "CITATION_ORPHAN",
        "DRAFT_TOKEN_ERROR",
        "DOUBLE_SPACE_ERROR",
        "FOREIGN_TERM_NOT_ITALIC",
    ]
    .map(String::from);
    expected.sort();
    assert_eq!(found, expected);
}

#[test]
fn paragraph_index_counts_every_w_p_in_document_order() {
    let tmp = tempfile::tempdir().unwrap();
    let arial = "<w:p><w:r><w:rPr><w:rFonts w:ascii=\"Arial\" w:hAnsi=\"Arial\"/></w:rPr><w:t>x</w:t></w:r></w:p>";
    let text_box = "<w:p><w:r><w:pict><v:shape id=\"s\"><v:textbox><w:txbxContent><w:p/></w:txbxContent></v:textbox></v:shape></w:pict></w:r></w:p>";
    // 1 empty, 2 Arial, 3 outer of the text box, 4 inside it, 5 Arial.
    let body = format!(
        "<w:p/>{arial}{text_box}{arial}{}",
        sect_pr(1440, 2268, 1701, 1701)
    );
    let (_, v) = lint_body(tmp.path(), &body, &[]);
    assert_eq!(
        viol_pairs(&v, "violations"),
        vec![
            ("FONT_ERROR".to_string(), 2),
            ("FONT_ERROR".to_string(), 5),
            ("MARGIN_ERROR".to_string(), 0)
        ]
    );
}

/// A zip with one stored `word/document.xml`, as raw bytes.
fn zip_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
    use std::io::Write;
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let stored =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for (name, bytes) in entries {
        zip.start_file(*name, stored).unwrap();
        zip.write_all(bytes).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

/// Sets `value` at `offset` from every header that starts with `signature`.
fn patch_headers(bytes: &mut [u8], signature: [u8; 4], offset: usize, value: u16, or: bool) {
    for i in 0..bytes.len().saturating_sub(offset + 2) {
        if bytes[i..i + 4] == signature {
            let old = u16::from_le_bytes([bytes[i + offset], bytes[i + offset + 1]]);
            let new = if or { old | value } else { value };
            bytes[i + offset..i + offset + 2].copy_from_slice(&new.to_le_bytes());
        }
    }
}

#[test]
fn hostile_archives_exit_2_without_panic() {
    let doc = document_xml(&format!("{GOOD_PARA}{}", good_sect_pr()));
    let mut unsupported = zip_bytes(&[("word/document.xml", doc.as_bytes())]);
    patch_headers(&mut unsupported, *b"PK\x03\x04", 8, 12, false);
    patch_headers(&mut unsupported, *b"PK\x01\x02", 10, 12, false);
    let mut encrypted = zip_bytes(&[("word/document.xml", doc.as_bytes())]);
    patch_headers(&mut encrypted, *b"PK\x03\x04", 6, 1, true);
    patch_headers(&mut encrypted, *b"PK\x01\x02", 8, 1, true);
    let cases: Vec<(&str, Vec<u8>)> = vec![
        ("not a zip", b"this is not a zip file".to_vec()),
        ("0 bytes", Vec::new()),
        ("no document.xml", zip_bytes(&[("word/other.xml", b"<a/>")])),
        (
            "non-utf8",
            zip_bytes(&[("word/document.xml", &[0xFF, 0xFE, 0xFA, 0x80])]),
        ),
        ("unsupported method", unsupported),
        ("encrypted", encrypted),
    ];
    for (label, bytes) in cases {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("h.docx"), bytes).unwrap();
        let out = run_bin(tmp.path(), &["check", "h.docx"]);
        assert_eq!(out.status.code(), Some(2), "{label}");
        let err = String::from_utf8_lossy(&out.stderr);
        assert!(
            err.starts_with("error:") && !err.contains("panicked"),
            "{label}: {err}"
        );
    }
}

#[test]
fn spacing_checked_only_for_auto_line_rule() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    let para = |rule: &str| {
        format!(
            "<w:p><w:pPr><w:spacing w:line=\"240\"{rule}/></w:pPr><w:r><w:t>x</w:t></w:r></w:p>"
        )
    };
    let body = format!(
        "{}{}{}{}{}",
        para(" w:lineRule=\"auto\""),
        para(""),
        para(" w:lineRule=\"exact\""),
        para(" w:lineRule=\"atLeast\""),
        good_sect_pr()
    );
    let (_, v) = lint_body(d, &body, &[]);
    assert_eq!(
        viol_pairs(&v, "violations"),
        vec![
            ("SPACING_ERROR".to_string(), 1),
            ("SPACING_ERROR".to_string(), 2)
        ]
    );
    let (json, xml) = fix_body(d, &body);
    assert!(json["remaining"].as_array().unwrap().is_empty());
    assert!(xml.contains("w:line=\"240\" w:lineRule=\"exact\""), "{xml}");
    assert!(
        xml.contains("w:line=\"240\" w:lineRule=\"atLeast\""),
        "{xml}"
    );
    assert_eq!(xml.matches("w:line=\"360\"").count(), 2, "{xml}");
}

#[test]
fn heading_styles_skip_size_but_keep_font_check() {
    let tmp = tempfile::tempdir().unwrap();
    let styled = |style: &str| {
        format!(
            "<w:p><w:pPr><w:pStyle w:val=\"{style}\"/></w:pPr><w:r><w:rPr>\
<w:rFonts w:ascii=\"Arial\" w:hAnsi=\"Arial\"/><w:sz w:val=\"40\"/></w:rPr><w:t>x</w:t></w:r></w:p>"
        )
    };
    let body = format!(
        "{}{}{}{}",
        styled("Heading1"),
        styled("Title"),
        styled("Subtitle"),
        good_sect_pr()
    );
    let (_, v) = lint_body(tmp.path(), &body, &[]);
    let font = |n| ("FONT_ERROR".to_string(), n);
    assert_eq!(
        viol_pairs(&v, "violations"),
        vec![font(1), font(2), font(3), font(3)]
    );
}
