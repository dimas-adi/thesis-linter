mod common;
use common::*;
use docx_lint::config::ProfileStore;
use docx_lint::gui::{DocxLintApp, FilterTab};

const BAD_PARA: &str = "<w:p><w:pPr><w:spacing w:line=\"240\" w:lineRule=\"auto\"/></w:pPr>\
<w:r><w:rPr><w:rFonts w:ascii=\"Arial\" w:hAnsi=\"Arial\"/><w:sz w:val=\"22\"/></w:rPr>\
<w:t xml:space=\"preserve\">TODO dua  spasi</w:t></w:r></w:p>";

fn app_with_doc(d: &std::path::Path) -> DocxLintApp {
    let body = format!("{BAD_PARA}{}", sect_pr(1440, 1440, 1440, 1440));
    write_docx(&d.join("in.docx"), &document_xml(&body));
    let mut app = DocxLintApp::new(ProfileStore::new(d.join("profiles")));
    app.load_file(&d.join("in.docx"));
    app
}

#[test]
fn load_file_populates_violations_and_status() {
    let tmp = tempfile::tempdir().unwrap();
    let app = app_with_doc(tmp.path());
    assert!(app.docx_path.is_some());
    assert!(!app.violations.is_empty());
    assert!(app.status.contains("violation"));
}

#[test]
fn load_missing_file_sets_error_banner() {
    let tmp = tempfile::tempdir().unwrap();
    let mut app = DocxLintApp::new(ProfileStore::new(tmp.path().join("profiles")));
    app.load_file(&tmp.path().join("nope.docx"));
    assert!(app.violations.is_empty());
    assert!(app.status.starts_with("Error"));
}

#[test]
fn filter_tabs_partition_violations() {
    let tmp = tempfile::tempdir().unwrap();
    let mut app = app_with_doc(tmp.path());
    let all = app.filtered().len();
    assert_eq!(all, app.violations.len());
    app.filter = FilterTab::Layout;
    let layout = app.filtered();
    assert!(!layout.is_empty());
    assert!(layout.iter().all(|v| v.code.ends_with("_ERROR")
        && ["MARGIN_ERROR", "FONT_ERROR", "SPACING_ERROR"].contains(&v.code.as_str())));
    app.filter = FilterTab::Academic;
    assert!(app.filtered().iter().any(|v| v.code == "DRAFT_TOKEN_ERROR"));
}

#[test]
fn save_profile_persists_and_relints() {
    let tmp = tempfile::tempdir().unwrap();
    let mut app = app_with_doc(tmp.path());
    let before = app.violations.len();
    app.profile.page.top_cm = 2.54;
    app.profile.page.left_cm = 2.54;
    app.profile.page.bottom_cm = 2.54;
    app.profile.page.right_cm = 2.54;
    app.save_profile();
    assert!(app.violations.len() < before);
    let saved = ProfileStore::new(tmp.path().join("profiles"))
        .load(&app.profile.profile_name)
        .unwrap();
    assert_eq!(saved.page.top_cm, 2.54);
    assert!(app.profile_names.contains(&app.profile.profile_name));
}

#[test]
fn switch_profile_loads_and_activates() {
    let tmp = tempfile::tempdir().unwrap();
    let mut app = app_with_doc(tmp.path());
    let store = ProfileStore::new(tmp.path().join("profiles"));
    store.init("kampus_b").unwrap();
    store
        .set_value("kampus_b", "structure.chapter_numbering", "arabic")
        .unwrap();
    app.refresh_profiles();
    app.switch_profile("kampus_b");
    assert_eq!(app.profile.structure.chapter_numbering, "arabic");
    assert_eq!(store.active_name().as_deref(), Some("kampus_b"));
}

#[test]
fn autofix_and_export_html_write_files() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    let mut app = app_with_doc(d);
    app.auto_fix(&d.join("fixed.docx"));
    assert!(d.join("fixed.docx").exists());
    assert!(app.status.contains("fixed"));
    app.export_html(&d.join("r.html"));
    let html = std::fs::read_to_string(d.join("r.html")).unwrap();
    assert!(html.contains("<html"));
}

#[test]
fn actions_without_document_report_error() {
    let tmp = tempfile::tempdir().unwrap();
    let mut app = DocxLintApp::new(ProfileStore::new(tmp.path().join("profiles")));
    app.auto_fix(&tmp.path().join("x.docx"));
    assert!(app.status.starts_with("Error"));
    app.export_html(&tmp.path().join("x.html"));
    assert!(app.status.starts_with("Error"));
}
