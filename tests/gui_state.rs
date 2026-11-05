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

// ---- Phase 5 ----

use docx_lint::config::ProfileConfig;
use eframe::egui;

fn write_clean_doc(path: &std::path::Path) {
    let body = format!("{}{}", good_para("Isi bersih."), good_sect_pr());
    write_docx(path, &document_xml(&body));
}

fn write_bad_doc(path: &std::path::Path) {
    let body = format!("{BAD_PARA}{}", sect_pr(1440, 1440, 1440, 1440));
    write_docx(path, &document_xml(&body));
}

#[test]
fn gui_outputs_cannot_overwrite_input() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    let mut app = app_with_doc(d);
    let input = d.join("in.docx");
    let before = std::fs::read(&input).unwrap();
    std::fs::create_dir(d.join("sub")).unwrap();
    std::fs::hard_link(&input, d.join("link.docx")).unwrap();
    let spellings = [
        input.clone(),
        d.join("./in.docx"),
        d.join("IN.DOCX"),
        d.join("sub/../in.docx"),
        d.join("link.docx"),
    ];
    for out in &spellings {
        app.auto_fix(out);
        assert!(
            app.status.starts_with("Error"),
            "fix {out:?}: {}",
            app.status
        );
        assert_eq!(std::fs::read(&input).unwrap(), before, "fix {out:?}");
        app.export_html(out);
        assert!(
            app.status.starts_with("Error"),
            "html {out:?}: {}",
            app.status
        );
        assert_eq!(std::fs::read(&input).unwrap(), before, "html {out:?}");
    }
}

#[test]
fn export_html_never_reports_a_failed_or_stale_lint() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    let mut app = DocxLintApp::new(ProfileStore::new(d.join("profiles")));
    std::fs::write(d.join("bad.docx"), b"not a zip").unwrap();
    for name in ["bad.docx", "missing.docx"] {
        app.load_file(&d.join(name));
        let report = d.join(format!("{name}.html"));
        app.export_html(&report);
        assert!(app.status.starts_with("Error"), "{name}: {}", app.status);
        assert!(!report.exists(), "{name}");
    }
    write_clean_doc(&d.join("c.docx"));
    app.load_file(&d.join("c.docx"));
    assert!(app.violations.is_empty());
    write_bad_doc(&d.join("c.docx"));
    app.export_html(&d.join("r.html"));
    let html = std::fs::read_to_string(d.join("r.html")).unwrap();
    assert!(html.contains("DRAFT_TOKEN_ERROR"), "{html}");
    assert!(!html.contains("No violations found."));
    assert!(!app.violations.is_empty());
}

#[test]
fn saved_profile_survives_restart() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    let store = ProfileStore::new(d.join("profiles"));
    let mut app = app_with_doc(d);
    app.profile.page.top_cm = 3.3;
    app.save_profile();
    assert!(app.status.contains("Saved profile"), "{}", app.status);
    let restarted = DocxLintApp::new(ProfileStore::new(d.join("profiles")));
    assert_eq!(restarted.profile.page.top_cm, 3.3);
    assert_eq!(store.resolve(None).unwrap().page.top_cm, 3.3);

    app.profile.profile_name = "renamed".into();
    app.profile.page.top_cm = 2.2;
    app.save_profile();
    let resolved = store.resolve(None).unwrap();
    assert_eq!(
        (resolved.profile_name.as_str(), resolved.page.top_cm),
        ("renamed", 2.2)
    );
    let restarted = DocxLintApp::new(ProfileStore::new(d.join("profiles")));
    assert_eq!(restarted.profile.profile_name, "renamed");
    assert_eq!(restarted.profile.page.top_cm, 2.2);
}

#[test]
fn autofix_rejects_values_outside_schema() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    let mut app = app_with_doc(d);
    let good = app.profile.clone();
    type Mutate = Box<dyn Fn(&mut ProfileConfig)>;
    let cases: Vec<(&str, Mutate)> = vec![
        ("size 0", Box::new(|p| p.typography.body_size_pt = 0.0)),
        ("size -5", Box::new(|p| p.typography.body_size_pt = -5.0)),
        (
            "size inf",
            Box::new(|p| p.typography.body_size_pt = f64::INFINITY),
        ),
        (
            "size NaN",
            Box::new(|p| p.typography.body_size_pt = f64::NAN),
        ),
        ("spacing -1", Box::new(|p| p.typography.line_spacing = -1.0)),
        ("spacing 0", Box::new(|p| p.typography.line_spacing = 0.0)),
        ("top -3", Box::new(|p| p.page.top_cm = -3.0)),
        (
            "font empty",
            Box::new(|p| p.typography.font_name = String::new()),
        ),
    ];
    for (label, mutate) in cases {
        app.profile = good.clone();
        mutate(&mut app.profile);
        let stem = label.replace(' ', "_");
        let fixed = d.join(format!("{stem}.docx"));
        let html = d.join(format!("{stem}.html"));
        app.auto_fix(&fixed);
        assert!(
            app.status.starts_with("Error"),
            "fix {label}: {}",
            app.status
        );
        assert!(!fixed.exists(), "fix {label}");
        app.export_html(&html);
        assert!(
            app.status.starts_with("Error"),
            "html {label}: {}",
            app.status
        );
        assert!(!html.exists(), "html {label}");
    }
}

#[test]
fn save_refuses_to_replace_another_profile() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    let store = ProfileStore::new(d.join("profiles"));
    store.init("kampus_b").unwrap();
    store.set_value("kampus_b", "page.top_cm", "1.11").unwrap();
    let file = d.join("profiles/kampus_b.toml");
    let before = std::fs::read(&file).unwrap();

    let mut app = DocxLintApp::new(ProfileStore::new(d.join("profiles")));
    app.profile.profile_name = "kampus_b".into();
    app.save_profile();
    assert!(app.status.starts_with("Error"), "{}", app.status);
    assert_eq!(std::fs::read(&file).unwrap(), before);

    // Saving under the loaded name still works.
    app.switch_profile("kampus_b");
    app.profile.page.top_cm = 2.5;
    app.save_profile();
    assert!(app.status.starts_with("Saved profile"), "{}", app.status);
    assert_eq!(store.load("kampus_b").unwrap().page.top_cm, 2.5);

    // Saving under a new name still works, and so does the next save of it.
    app.profile.profile_name = "kampus_c".into();
    app.save_profile();
    assert!(app.status.starts_with("Saved profile"), "{}", app.status);
    assert!(store.exists("kampus_c"));
    app.save_profile();
    assert!(app.status.starts_with("Saved profile"), "{}", app.status);
}

#[test]
fn dictionary_box_keeps_comments_on_save() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    std::fs::create_dir_all(d.join("profiles")).unwrap();
    let file = d.join("profiles/foreign_terms.txt");
    let text = b"# Istilah asing bab 2\nkernel\n\n# Istilah bab 3\nthread\n";
    std::fs::write(&file, text).unwrap();
    let mut app = DocxLintApp::new(ProfileStore::new(d.join("profiles")));
    assert_eq!(app.dictionary_text.as_bytes(), text);
    app.save_dictionary();
    assert_eq!(std::fs::read(&file).unwrap(), text);
}

fn has_term(app: &DocxLintApp, term: &str) -> bool {
    app.profile
        .academic
        .foreign_terms_must_italicize
        .iter()
        .any(|t| t == term)
}

#[test]
fn dictionary_overlap_term_survives_gui_save() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    let profiles = d.join("profiles");
    let store = ProfileStore::new(&profiles);
    store.init("p").unwrap();
    store
        .set_value("p", "academic.foreign_terms_must_italicize", "framework")
        .unwrap();
    store.set_active("p").unwrap();
    std::fs::write(profiles.join("foreign_terms.txt"), "framework\n").unwrap();
    let toml_has_term = |name: &str| {
        let t = std::fs::read_to_string(profiles.join(format!("{name}.toml"))).unwrap();
        t.contains("\"framework\"")
    };

    let mut app = DocxLintApp::new(ProfileStore::new(&profiles));
    assert!(has_term(&app, "framework"));
    app.dictionary_text.clear();
    app.save_dictionary();
    assert!(has_term(&app, "framework"), "dictionary edit dropped term");
    app.save_profile();
    assert!(toml_has_term("p"), "Save removed the profile's own term");

    app.profile.profile_name = "copy".into();
    app.save_profile();
    assert!(toml_has_term("copy"), "copy lost the profile's own term");
}

#[test]
fn save_dictionary_replaces_only_old_words() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    let profiles = d.join("profiles");
    let store = ProfileStore::new(&profiles);
    store.init("p").unwrap();
    store
        .set_value("p", "academic.foreign_terms_must_italicize", "pipeline")
        .unwrap();
    store.set_active("p").unwrap();
    std::fs::write(profiles.join("foreign_terms.txt"), "alpha\n").unwrap();

    let mut app = DocxLintApp::new(ProfileStore::new(&profiles));
    assert!(has_term(&app, "alpha"));
    app.dictionary_text = "beta".into();
    app.save_dictionary();
    assert!(has_term(&app, "pipeline") && has_term(&app, "beta"));
    assert!(!has_term(&app, "alpha"));

    app.save_profile();
    let saved = std::fs::read_to_string(profiles.join("p.toml")).unwrap();
    assert!(saved.contains("pipeline"), "{saved}");
    assert!(
        !saved.contains("alpha") && !saved.contains("beta"),
        "{saved}"
    );
}

#[test]
fn filter_tabs_partition_every_code() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    let bad = "<w:p><w:pPr><w:spacing w:line=\"240\" w:lineRule=\"auto\"/></w:pPr><w:r>\
<w:rPr><w:rFonts w:ascii=\"Arial\" w:hAnsi=\"Arial\"/><w:sz w:val=\"24\"/></w:rPr>\
<w:t xml:space=\"preserve\">TODO dua  spasi framework (Smith, 2020)</w:t></w:r></w:p>";
    let body = format!(
        "{}{}{bad}{}",
        heading_para("BAB 1", "center"),
        good_para("Gambar 1-1 Salah"),
        sect_pr(1440, 1440, 1440, 1440)
    );
    write_docx(&d.join("in.docx"), &document_xml(&body));
    let mut app = DocxLintApp::new(ProfileStore::new(d.join("profiles")));
    app.load_file(&d.join("in.docx"));

    let tab_codes = |app: &mut DocxLintApp, tab: FilterTab| {
        app.filter = tab;
        let mut codes: Vec<String> = app.filtered().iter().map(|v| v.code.clone()).collect();
        codes.sort();
        codes.dedup();
        codes
    };
    let layout = tab_codes(&mut app, FilterTab::Layout);
    let structure = tab_codes(&mut app, FilterTab::Structure);
    let academic = tab_codes(&mut app, FilterTab::Academic);
    assert_eq!(layout, ["FONT_ERROR", "MARGIN_ERROR", "SPACING_ERROR"]);
    assert_eq!(structure, ["CAPTION_SEQ_ERROR", "HEADING_ERROR"]);
    assert_eq!(
        academic,
        [
            "CITATION_ORPHAN",
            "DOUBLE_SPACE_ERROR",
            "DRAFT_TOKEN_ERROR",
            "FOREIGN_TERM_NOT_ITALIC"
        ]
    );
    let count = |app: &mut DocxLintApp, tab| {
        app.filter = tab;
        app.filtered().len()
    };
    let sum = count(&mut app, FilterTab::Layout)
        + count(&mut app, FilterTab::Structure)
        + count(&mut app, FilterTab::Academic);
    assert_eq!(sum, count(&mut app, FilterTab::All));
    assert_eq!(sum, app.violations.len());
}

/// One text shape from a headless render: its text, bounding rectangle, and clip rectangle.
struct Drawn {
    text: String,
    rect: egui::Rect,
    clip: egui::Rect,
}

fn collect(shape: &egui::Shape, clip: egui::Rect, out: &mut Vec<Drawn>) {
    match shape {
        egui::Shape::Vec(v) => v.iter().for_each(|s| collect(s, clip, out)),
        egui::Shape::Text(t) => out.push(Drawn {
            text: t.galley.text().to_string(),
            rect: egui::Rect::from_min_size(
                t.pos + t.galley.rect.min.to_vec2(),
                t.galley.rect.size(),
            ),
            clip,
        }),
        _ => {}
    }
}

/// Runs `eframe::App::ui` for several frames at a fixed size and returns the last frame's text shapes.
fn render(app: &mut DocxLintApp, width: f32, height: f32) -> Vec<Drawn> {
    let ctx = egui::Context::default();
    let mut frame = eframe::Frame::_new_kittest();
    let mut last = Vec::new();
    for _ in 0..5 {
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(width, height),
            )),
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| eframe::App::ui(app, ui, &mut frame));
        output.textures_delta.clear();
        last.clear();
        for clipped in &output.shapes {
            collect(&clipped.shape, clipped.clip_rect, &mut last);
        }
    }
    last
}

#[test]
fn save_buttons_stay_on_screen_with_long_dictionary() {
    let tmp = tempfile::tempdir().unwrap();
    let mut app = DocxLintApp::new(ProfileStore::new(tmp.path().join("profiles")));
    app.dictionary_text = (0..500).map(|i| format!("word{i}\n")).collect();
    let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0));
    let drawn = render(&mut app, 800.0, 600.0);
    for label in ["Save dictionary", "Save profile"] {
        let shape = drawn
            .iter()
            .find(|d| d.text == label)
            .unwrap_or_else(|| panic!("{label} not drawn"));
        assert!(shape.clip.contains_rect(shape.rect), "{label} clipped");
        assert!(screen.contains_rect(shape.rect), "{label} off screen");
    }
}

#[test]
fn panel_render_keeps_large_margin() {
    let tmp = tempfile::tempdir().unwrap();
    let mut app = DocxLintApp::new(ProfileStore::new(tmp.path().join("profiles")));
    app.profile.page.top_cm = 12.5;
    render(&mut app, 800.0, 600.0);
    assert_eq!(app.profile.page.top_cm, 12.5);
}

#[test]
fn status_does_not_accumulate_without_document() {
    let tmp = tempfile::tempdir().unwrap();
    let mut app = DocxLintApp::new(ProfileStore::new(tmp.path().join("profiles")));
    for _ in 0..3 {
        app.save_profile();
    }
    assert_eq!(
        app.status.matches("Saved profile").count(),
        1,
        "{}",
        app.status
    );
    app.save_dictionary();
    assert!(app.status.starts_with("Saved dictionary."));
    assert!(!app.status.contains("Saved profile"), "{}", app.status);
}

#[test]
fn switch_clears_old_error_and_refreshes_list() {
    let tmp = tempfile::tempdir().unwrap();
    let d = tmp.path();
    let store = ProfileStore::new(d.join("profiles"));
    store.init("kampus_b").unwrap();
    let mut app = DocxLintApp::new(ProfileStore::new(d.join("profiles")));
    assert!(app.profile_names.contains(&"default_kampus".to_string()));
    app.switch_profile("missing");
    assert!(app.status.starts_with("Error"));
    app.switch_profile("kampus_b");
    assert!(!app.status.starts_with("Error"), "{}", app.status);
    assert!(
        app.status.contains("Switched to 'kampus_b'."),
        "{}",
        app.status
    );
    assert!(!app.profile_names.contains(&"default_kampus".to_string()));
}
