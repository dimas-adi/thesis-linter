mod common;
use common::*;

fn body(parts: &[String]) -> String {
    format!("{}{}", parts.concat(), good_sect_pr())
}

#[test]
fn citation_orphans_flagged() {
    let tmp = tempfile::tempdir().unwrap();
    let b = body(&[
        good_para("Menurut (Smith, 2020) dan (Jones, 2019) hal ini benar."),
        good_para("Lihat juga (Smith, 2020; Lee et al., 2018)."),
        heading_para("DAFTAR PUSTAKA", "center"),
        good_para("Smith, J. (2020). Judul buku. Penerbit."),
        good_para("Lee, K. et al. (2018). Artikel."),
    ]);
    let (pass, v) = lint_body(tmp.path(), &b, &[]);
    assert!(!pass);
    assert_eq!(viol_codes(&v), vec!["CITATION_ORPHAN"]);
    assert!(
        v["violations"][0]["message"]
            .as_str()
            .unwrap()
            .contains("Jones")
    );
}

#[test]
fn citation_without_bibliography_is_orphan_and_plain_parens_ignored() {
    let tmp = tempfile::tempdir().unwrap();
    let b = body(&[good_para(
        "Seperti (Smith, 2020) dan (lihat Gambar 1) serta (2020).",
    )]);
    let (_, v) = lint_body(tmp.path(), &b, &[]);
    assert_eq!(viol_codes(&v), vec!["CITATION_ORPHAN"]);
}

#[test]
fn draft_tokens_flagged_case_insensitively() {
    let tmp = tempfile::tempdir().unwrap();
    let b = body(&[
        good_para("TODO tulis bagian ini"),
        good_para("Perlu Cek Lagi nanti"),
    ]);
    let (pass, v) = lint_body(tmp.path(), &b, &[]);
    assert!(!pass);
    assert_eq!(viol_codes(&v), vec!["DRAFT_TOKEN_ERROR"; 2]);
}

#[test]
fn double_space_within_and_across_runs() {
    let tmp = tempfile::tempdir().unwrap();
    let b = body(&[
        good_para("dua  spasi"),
        run_para(&[("akhir ", false), (" awal", false)]),
        good_para("satu spasi saja"),
    ]);
    let (_, v) = lint_body(tmp.path(), &b, &[]);
    assert_eq!(viol_codes(&v), vec!["DOUBLE_SPACE_ERROR"; 2]);
}

#[test]
fn foreign_terms_need_italic() {
    let tmp = tempfile::tempdir().unwrap();
    let b = body(&[
        run_para(&[
            ("Kami memakai ", false),
            ("framework", false),
            (" modern", false),
        ]),
        run_para(&[("Kami memakai ", false), ("backend", true)]),
        run_para(&[("Banyak frameworks lain", false)]),
    ]);
    let (pass, v) = lint_body(tmp.path(), &b, &[]);
    assert!(!pass);
    assert_eq!(viol_codes(&v), vec!["FOREIGN_TERM_NOT_ITALIC"]);
}

/// Paragraph of plain runs (no run properties), so only the text rules can fire.
fn plain_para(runs: &str) -> String {
    format!("<w:p><w:pPr><w:spacing w:line=\"360\" w:lineRule=\"auto\"/></w:pPr>{runs}</w:p>")
}

fn t(text: &str) -> String {
    format!("<w:t xml:space=\"preserve\">{text}</w:t>")
}

#[test]
fn tab_and_break_shapes_match_double_space_rule() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    let mut paras = Vec::new();
    let mut kept = Vec::new();
    for m in ["<w:tab/>", "<w:br/>", "<w:cr/>"] {
        let (a, b) = (t("a "), t(" b"));
        paras.push(plain_para(&format!("<w:r>{a}{m}{b}</w:r>")));
        paras.push(plain_para(&format!(
            "<w:r>{a}</w:r><w:r>{m}</w:r><w:r>{b}</w:r>"
        )));
        paras.push(plain_para(&format!("<w:r>{a}</w:r><w:r>{m}{b}</w:r>")));
        kept.push(format!("{a}{m}{b}"));
    }
    let b = body(&paras);
    let (_, v) = lint_body(d, &b, &[]);
    assert!(viol_codes(&v).is_empty(), "{v}");
    let (json, xml) = fix_body(d, &b);
    assert!(json["remaining"].as_array().unwrap().is_empty());
    for k in kept {
        assert!(xml.contains(&k), "{k} in {xml}");
    }
}

#[test]
fn tabs_and_breaks_never_create_double_space() {
    let tmp = tempfile::tempdir().unwrap();
    let mut paras = Vec::new();
    for m in ["<w:tab/>", "<w:br/>", "<w:cr/>"] {
        paras.push(plain_para(&format!("<w:r>{}{m}{}</w:r>", t("a "), t("b"))));
        paras.push(plain_para(&format!("<w:r>{}{m}{}</w:r>", t("a"), t(" b"))));
        paras.push(plain_para(&format!(
            "<w:r>{}</w:r><w:r>{m}</w:r><w:r>{}</w:r>",
            t("a "),
            t("b")
        )));
        paras.push(plain_para(&format!(
            "<w:r>{}</w:r><w:r>{m}</w:r><w:r>{}</w:r>",
            t("a"),
            t(" b")
        )));
        paras.push(plain_para(&format!("<w:r>{m}{}</w:r>", t(" b"))));
    }
    let (_, v) = lint_body(tmp.path(), &body(&paras), &[]);
    assert!(viol_codes(&v).is_empty(), "{v}");
}

#[test]
fn many_empty_tab_runs_lint_quickly() {
    let tmp = tempfile::tempdir().unwrap();
    let runs = "<w:r><w:tab/></w:r>".repeat(80_000);
    let start = std::time::Instant::now();
    let (_, v) = lint_body(tmp.path(), &body(&[plain_para(&runs)]), &[]);
    assert!(viol_codes(&v).is_empty(), "{v}");
    assert!(start.elapsed().as_secs() < 5, "{:?}", start.elapsed());
}
