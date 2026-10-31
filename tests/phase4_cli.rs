mod common;
use common::*;
use docx_lint::config::ProfileStore;
use docx_lint::gui::DocxLintApp;
use std::io::Read;
use std::path::Path;

/// Paragraph with explicit size (half-points) and line spacing (240ths), Times New Roman.
fn sized_para(text: &str, half_pt: u32, line: u32) -> String {
    format!(
        "<w:p><w:pPr><w:spacing w:line=\"{line}\" w:lineRule=\"auto\"/></w:pPr>\
<w:r><w:rPr><w:rFonts w:ascii=\"Times New Roman\" w:hAnsi=\"Times New Roman\"/><w:sz w:val=\"{half_pt}\"/></w:rPr>\
<w:t xml:space=\"preserve\">{text}</w:t></w:r></w:p>"
    )
}

fn table(inner: &str) -> String {
    format!("<w:tbl><w:tr><w:tc>{inner}</w:tc></w:tr></w:tbl>")
}

fn body(parts: &[String]) -> String {
    format!("{}{}", parts.concat(), good_sect_pr())
}

fn codes(dir: &Path, parts: &[String]) -> Vec<String> {
    viol_codes(&lint_body(dir, &body(parts), &[]).1)
}

// T4.1
#[test]
fn draft_tokens_match_whole_words_only() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    // WHY: "METODOLOGI" contains the letters T-O-D-O but is not the word TODO.
    assert!(codes(d, &[good_para("BAB METODOLOGI PENELITIAN dan TBDX")]).is_empty());
    assert_eq!(
        codes(d, &[good_para("Isi (todo) nanti")]),
        vec!["DRAFT_TOKEN_ERROR"]
    );
    assert_eq!(
        codes(d, &[good_para("Perlu cek lagi, TODO: isi")]),
        vec!["DRAFT_TOKEN_ERROR"; 2]
    );
}

// T4.2
#[test]
fn front_matter_allows_cover_sizes_but_body_stays_strict() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    let ok = [
        sized_para("JUDUL SKRIPSI", 32, 360),
        sized_para("Nama", 28, 360),
        heading_para("BAB I", "center"),
        good_para("Isi bab."),
    ];
    assert!(codes(d, &ok).is_empty());

    let bad_front = [
        sized_para("Cover", 26, 360),
        heading_para("BAB I", "center"),
    ];
    assert_eq!(codes(d, &bad_front), vec!["FONT_ERROR"]);

    let bad_body = [heading_para("BAB I", "center"), sized_para("Isi", 28, 360)];
    assert_eq!(codes(d, &bad_body), vec!["FONT_ERROR"]);
}

#[test]
fn table_paragraphs_allow_small_font_and_single_spacing() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    let ok = [
        heading_para("BAB I", "center"),
        table(&format!(
            "{}{}",
            sized_para("sel", 20, 240),
            sized_para("sel", 22, 240)
        )),
        good_para("Setelah tabel."),
    ];
    assert!(codes(d, &ok).is_empty());

    let bad_table = [
        heading_para("BAB I", "center"),
        table(&sized_para("sel", 28, 240)),
    ];
    assert_eq!(codes(d, &bad_table), vec!["FONT_ERROR"]);

    // Exemption ends with the table.
    let bad_after = [
        heading_para("BAB I", "center"),
        table(&sized_para("sel", 20, 240)),
        sized_para("luar", 20, 240),
    ];
    assert_eq!(codes(d, &bad_after), vec!["SPACING_ERROR", "FONT_ERROR"]);
}

// T4.3
#[test]
fn front_matter_captions_skipped_and_numbering_resets_per_chapter() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    let ok = [
        good_para("DAFTAR GAMBAR"),
        good_para("Gambar 1-5 Salah separator di daftar"),
        good_para("Gambar 3.9 Nomor acak di daftar"),
        good_para("DAFTAR TABEL"),
        good_para("Tabel 2.7 Daftar"),
        heading_para("BAB I", "center"),
        good_para("Gambar 1.1 Satu"),
        good_para("Gambar 1.2 Dua"),
        good_para("Tabel 1.1 Satu"),
        heading_para("BAB II", "center"),
        good_para("Gambar 2.1 Reset"),
        good_para("Tabel 2.1 Reset"),
    ];
    assert!(codes(d, &ok).is_empty());

    let no_reset = [
        heading_para("BAB I", "center"),
        good_para("Gambar 1.1 Satu"),
        heading_para("BAB II", "center"),
        good_para("Gambar 2.2 Tidak reset"),
    ];
    assert_eq!(codes(d, &no_reset), vec!["CAPTION_SEQ_ERROR"]);

    let wrong_sep = [
        heading_para("BAB I", "center"),
        good_para("Gambar 1-1 Salah"),
    ];
    assert_eq!(codes(d, &wrong_sep), vec!["CAPTION_SEQ_ERROR"]);
}

// T4.4
#[test]
fn external_dictionary_is_merged_for_cli_and_gui() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    let doc = [good_para("Kami memakai microservice di sini")];
    assert!(codes(d, &doc).is_empty());

    std::fs::create_dir_all(d.join("profiles")).unwrap();
    std::fs::write(
        d.join("profiles/foreign_terms.txt"),
        "# istilah asing\nmicroservice\n\n  Kubernetes  \nframework\n",
    )
    .unwrap();
    assert_eq!(codes(d, &doc), vec!["FOREIGN_TERM_NOT_ITALIC"]);

    let store = ProfileStore::new(d.join("profiles"));
    let terms = store
        .resolve(None)
        .unwrap()
        .academic
        .foreign_terms_must_italicize;
    assert!(terms.contains(&"microservice".to_string()));
    assert!(terms.contains(&"kubernetes".to_string()) || terms.contains(&"Kubernetes".to_string()));
    // Built-in term listed again in the file is not duplicated.
    assert_eq!(
        terms.iter().filter(|t| t.as_str() == "framework").count(),
        1
    );

    // GUI: editing the dictionary box persists to the file and re-lints.
    write_docx(&d.join("g.docx"), &document_xml(&body(&doc)));
    let mut app = DocxLintApp::new(ProfileStore::new(d.join("profiles")));
    app.load_file(&d.join("g.docx"));
    assert_eq!(app.violations.len(), 1);
    app.dictionary_text = "docker\n".into();
    app.save_dictionary();
    assert!(
        std::fs::read_to_string(d.join("profiles/foreign_terms.txt"))
            .unwrap()
            .contains("docker")
    );
    assert!(app.violations.is_empty());

    // Saving the profile must not copy dictionary words into the .toml.
    app.save_profile();
    let toml = std::fs::read_to_string(d.join("profiles/default_kampus.toml")).unwrap();
    assert!(!toml.contains("docker"));
}

// T4.5
#[test]
fn fixer_splits_runs_so_foreign_terms_become_italic() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    let b = body(&[
        run_para(&[(
            "Kami memakai Framework modern &amp; database lokal, bukan frameworks.",
            false,
        )]),
        run_para(&[
            ("Sudah ", false),
            ("backend", true),
            (" dan cache ok", false),
        ]),
    ]);
    write_docx(&d.join("in.docx"), &document_xml(&b));
    let run = |args: &[&str]| {
        std::process::Command::new(env!("CARGO_BIN_EXE_docx-lint"))
            .current_dir(d)
            .args(args)
            .output()
            .unwrap()
    };
    let before = run(&["check", "in.docx"]);
    assert!(String::from_utf8_lossy(&before.stdout).contains("FOREIGN_TERM_NOT_ITALIC"));

    let out = run(&["check", "in.docx", "--fix", "out.docx", "--json"]);
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stdout)
    );
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(json["remaining"].as_array().unwrap().is_empty());
    assert!(run(&["check", "out.docx"]).status.success());

    let mut xml = String::new();
    zip::ZipArchive::new(std::fs::File::open(d.join("out.docx")).unwrap())
        .unwrap()
        .by_name("word/document.xml")
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    // Term runs are isolated, italic, and keep the original font/size.
    assert!(
        xml.contains("<w:i/>") && xml.contains(">Framework</w:t>"),
        "{xml}"
    );
    assert!(xml.contains("&amp;"), "{xml}");
    // Text content is unchanged once runs are concatenated.
    let text: String = xml
        .split("<w:t")
        .skip(1)
        .filter_map(|s| s.split_once('>')?.1.split_once("</w:t>").map(|x| x.0))
        .take(5)
        .collect();
    assert_eq!(
        text,
        "Kami memakai Framework modern &amp; database lokal, bukan frameworks."
    );
}

// T4.6
#[test]
fn fixer_handles_multi_child_runs_and_hyperlinks() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    // WHY: Word emits runs with several <w:t>, <w:tab/> or page-break markers; hyperlinks wrap runs.
    let frag = "<w:p><w:r><w:rPr><w:b/></w:rPr><w:t xml:space=\"preserve\">Pakai data</w:t>\
<w:lastRenderedPageBreak/><w:t xml:space=\"preserve\">base </w:t><w:t>dan cache</w:t></w:r></w:p>";
    let link = "<w:p><w:hyperlink w:anchor=\"x\"><w:r><w:t>lihat database</w:t><w:tab/><w:t>ini</w:t></w:r></w:hyperlink></w:p>";
    let b = body(&[frag.to_string(), link.to_string()]);
    write_docx(&d.join("in.docx"), &document_xml(&b));
    let run = |args: &[&str]| {
        std::process::Command::new(env!("CARGO_BIN_EXE_docx-lint"))
            .current_dir(d)
            .args(args)
            .output()
            .unwrap()
    };
    let out = run(&["check", "in.docx", "--fix", "out.docx", "--json"]);
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    let rem: Vec<_> = json["remaining"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|v| v.to_string().contains("FOREIGN_TERM"))
        .collect();
    assert!(rem.is_empty(), "{rem:?}");
    assert!(run(&["check", "out.docx"]).status.success());
}

#[test]
fn chapter_heading_split_by_line_break_still_resets_captions() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    let br = |t: &str| {
        format!(
            "<w:p><w:pPr><w:jc w:val=\"center\"/></w:pPr><w:r><w:t>{t}</w:t><w:br/><w:t>PENDAHULUAN</w:t></w:r></w:p>"
        )
    };
    let ok = [
        br("BAB I"),
        good_para("Gambar 1.1 Satu"),
        br("BAB II"),
        good_para("Gambar 2.1 Reset"),
    ];
    assert!(codes(d, &ok).is_empty());
}

#[test]
fn auto_numbered_heading1_counts_as_chapter_for_captions() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    // WHY: Word generates "BAB 1" from list numbering, so the chapter text is absent from the XML.
    let h = |t: &str, num: &str| {
        format!(
            "<w:p><w:pPr><w:pStyle w:val=\"Heading1\"/>{num}<w:jc w:val=\"center\"/></w:pPr><w:r><w:t>{t}</w:t></w:r></w:p>"
        )
    };
    let off = "<w:numPr><w:ilvl w:val=\"0\"/><w:numId w:val=\"0\"/></w:numPr>";
    let ok = [
        h("Daftar Gambar", off),
        good_para("Gambar 3.1 Daftar"),
        good_para("Gambar 4.7 Daftar"),
        h("PENDAHULUAN", ""),
        good_para("Gambar 1.1 Satu"),
        h("TINJAUAN", ""),
        good_para("Gambar 2.1 Reset"),
    ];
    assert!(codes(d, &ok).is_empty());
}
