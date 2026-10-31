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
