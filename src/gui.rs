use crate::config::{ProfileConfig, ProfileStore};
use crate::linter::{self, Violation};
use crate::{fixer, report};
use eframe::egui;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterTab {
    All,
    Layout,
    Structure,
    Academic,
}

impl FilterTab {
    pub const ALL: [FilterTab; 4] = [
        FilterTab::All,
        FilterTab::Layout,
        FilterTab::Structure,
        FilterTab::Academic,
    ];

    pub fn label(self) -> &'static str {
        match self {
            FilterTab::All => "All",
            FilterTab::Layout => "Layout",
            FilterTab::Structure => "Structure",
            FilterTab::Academic => "Academic",
        }
    }

    fn matches(self, code: &str) -> bool {
        match self {
            FilterTab::All => true,
            FilterTab::Layout => ["MARGIN_ERROR", "FONT_ERROR", "SPACING_ERROR"].contains(&code),
            FilterTab::Structure => ["HEADING_ERROR", "CAPTION_SEQ_ERROR"].contains(&code),
            // WHY: Academic is "everything not layout/structure" so new codes never vanish from the UI.
            FilterTab::Academic => !Self::Layout.matches(code) && !Self::Structure.matches(code),
        }
    }
}

/// All GUI state and actions live here (no egui types) so `cargo test` covers them headlessly.
pub struct DocxLintApp {
    store: ProfileStore,
    pub profile: ProfileConfig,
    pub profile_names: Vec<String>,
    pub docx_path: Option<PathBuf>,
    pub violations: Vec<Violation>,
    pub filter: FilterTab,
    pub status: String,
    /// Editable text of `profiles/foreign_terms.txt` (one word per line).
    pub dictionary_text: String,
    /// Name the loaded profile has on disk; a different name in the Name box is a copy or a rename.
    saved_name: String,
}

impl DocxLintApp {
    pub fn new(store: ProfileStore) -> Self {
        let (profile, status) = match store.resolve(None) {
            Ok(p) => (p, "Ready. Open or drop a .docx file.".to_string()),
            Err(e) => (
                ProfileConfig::default_named("default_kampus"),
                format!("Error: {e:#}"),
            ),
        };
        let dictionary_text = store.load_dictionary_text();
        let mut app = DocxLintApp {
            store,
            dictionary_text,
            saved_name: profile.profile_name.clone(),
            profile,
            profile_names: Vec::new(),
            docx_path: None,
            violations: Vec::new(),
            filter: FilterTab::All,
            status,
        };
        app.refresh_profiles();
        app
    }

    pub fn refresh_profiles(&mut self) {
        self.profile_names = self.store.list();
        if !self.profile_names.contains(&self.profile.profile_name) {
            self.profile_names.push(self.profile.profile_name.clone());
            self.profile_names.sort();
        }
    }

    fn error(&mut self, e: anyhow::Error) {
        self.status = format!("Error: {e:#}");
    }

    pub fn load_file(&mut self, path: &Path) {
        self.docx_path = Some(path.to_path_buf());
        self.recheck();
    }

    pub fn recheck(&mut self) {
        let Some(path) = self.docx_path.clone() else {
            return;
        };
        match linter::lint_file(&path, &self.profile) {
            Ok(v) => {
                self.status = format!("{} violation(s) in {}", v.len(), path.display());
                self.violations = v;
            }
            Err(e) => {
                self.violations.clear();
                self.error(e);
            }
        }
    }

    pub fn filtered(&self) -> Vec<&Violation> {
        self.violations
            .iter()
            .filter(|v| self.filter.matches(&v.code))
            .collect()
    }

    /// Re-lints the loaded document (if any) and shows `message`, followed by the lint status.
    fn report_saved(&mut self, message: String) {
        self.recheck();
        self.status = if self.docx_path.is_some() {
            format!("{message} {}", self.status)
        } else {
            message
        };
    }

    pub fn save_profile(&mut self) {
        let name = self.profile.profile_name.clone();
        // WHY: a typed name that matches another profile would silently replace that file.
        if name != self.saved_name && self.store.exists(&name) {
            return self.error(anyhow::anyhow!(
                "profile '{name}' already exists; choose another name"
            ));
        }
        // WHY: the saved profile must be the one the next launch resolves.
        match self
            .store
            .save(&self.profile)
            .and_then(|()| self.store.set_active(&name))
        {
            Ok(()) => {
                self.saved_name = name.clone();
                self.refresh_profiles();
                self.report_saved(format!("Saved profile '{name}'."));
            }
            Err(e) => self.error(e),
        }
    }

    /// Writes `dictionary_text` to `foreign_terms.txt` and swaps the old words for the new ones.
    pub fn save_dictionary(&mut self) {
        if let Err(e) = self.store.save_dictionary(&self.dictionary_text) {
            return self.error(e);
        }
        let mut profile = self.profile.clone();
        let added = std::mem::take(&mut profile.dictionary_added);
        // WHY: drop only the words the old dictionary added, so terms in the profile survive.
        profile
            .academic
            .foreign_terms_must_italicize
            .retain(|t| !added.iter().any(|a| a.eq_ignore_ascii_case(t)));
        self.profile = self.store.with_dictionary(profile);
        self.report_saved("Saved dictionary.".to_string());
    }

    pub fn switch_profile(&mut self, name: &str) {
        match self
            .store
            .load(name)
            .and_then(|p| self.store.set_active(name).map(|()| p))
        {
            Ok(p) => {
                self.saved_name = p.profile_name.clone();
                self.profile = p;
                self.refresh_profiles();
                self.recheck();
                if self.docx_path.is_none() {
                    self.status = format!("Switched to '{name}'.");
                }
            }
            Err(e) => self.error(e),
        }
    }

    pub fn auto_fix(&mut self, out: &Path) {
        let Some(input) = self.docx_path.clone() else {
            return self.error(anyhow::anyhow!("no document loaded"));
        };
        if let Err(e) = self.profile.validate() {
            return self.error(e);
        }
        let result = fixer::fix_file(&input, out, &self.profile)
            .and_then(|()| linter::lint_file(out, &self.profile));
        match result {
            Ok(rem) => {
                self.status = format!(
                    "Saved fixed copy {} ({} remaining)",
                    out.display(),
                    rem.len()
                )
            }
            Err(e) => self.error(e),
        }
    }

    pub fn export_html(&mut self, out: &Path) {
        let Some(input) = self.docx_path.clone() else {
            return self.error(anyhow::anyhow!("no document loaded"));
        };
        if let Err(e) = self.profile.validate() {
            return self.error(e);
        }
        // WHY: the report must describe the file as it is now, never a cached or failed lint.
        match linter::lint_file(&input, &self.profile) {
            Ok(v) => self.violations = v,
            Err(e) => {
                self.violations.clear();
                return self.error(e);
            }
        }
        match report::write_html(&input, out, &self.profile, &self.violations) {
            Ok(()) => self.status = format!("Exported report {}", out.display()),
            Err(e) => self.error(e),
        }
    }

    fn pick_file(&mut self) {
        if let Some(p) = rfd::FileDialog::new()
            .add_filter("Word document", &["docx"])
            .pick_file()
        {
            self.load_file(&p);
        }
    }

    fn pick_fix_target(&mut self) {
        if let Some(p) = rfd::FileDialog::new()
            .add_filter("Word document", &["docx"])
            .set_file_name("fixed.docx")
            .save_file()
        {
            self.auto_fix(&p);
        }
    }

    fn pick_html_target(&mut self) {
        if let Some(p) = rfd::FileDialog::new()
            .add_filter("HTML", &["html"])
            .set_file_name("report.html")
            .save_file()
        {
            self.export_html(&p);
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("Open .docx…").clicked() {
                self.pick_file();
            }
            ui.label("or drop a file here");
            ui.separator();
            ui.label("Profile:");
            let mut chosen = None;
            egui::ComboBox::from_id_salt("profile_switcher")
                .selected_text(self.profile.profile_name.clone())
                .show_ui(ui, |ui| {
                    for n in &self.profile_names {
                        if ui
                            .selectable_label(*n == self.profile.profile_name, n)
                            .clicked()
                        {
                            chosen = Some(n.clone());
                        }
                    }
                });
            // WHY: switching inside the closure would borrow self twice; defer until the UI borrow ends.
            if let Some(n) = chosen {
                self.switch_profile(&n);
            }
        });
    }

    fn settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Profile settings");
        let p = &mut self.profile;
        ui.horizontal(|ui| {
            ui.label("Name");
            ui.text_edit_singleline(&mut p.profile_name);
        });
        ui.separator();
        ui.label("Margins (cm)");
        for (label, val) in [
            ("Top", &mut p.page.top_cm),
            ("Left", &mut p.page.left_cm),
            ("Bottom", &mut p.page.bottom_cm),
            ("Right", &mut p.page.right_cm),
        ] {
            ui.horizontal(|ui| {
                ui.label(label);
                ui.add(egui::DragValue::new(val).speed(0.05).range(0.0..=f64::MAX));
            });
        }
        ui.separator();
        ui.label("Typography");
        ui.horizontal(|ui| {
            ui.label("Font");
            ui.text_edit_singleline(&mut p.typography.font_name);
        });
        ui.horizontal(|ui| {
            ui.label("Size (pt)");
            ui.add(egui::DragValue::new(&mut p.typography.body_size_pt).speed(0.5));
        });
        ui.horizontal(|ui| {
            ui.label("Line spacing");
            ui.add(egui::DragValue::new(&mut p.typography.line_spacing).speed(0.05));
        });
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Chapter numbering");
            let current = if p.structure.chapter_numbering == "arabic" {
                format!("{} 1", p.structure.chapter_prefix)
            } else {
                format!("{} I", p.structure.chapter_prefix)
            };
            egui::ComboBox::from_id_salt("chapter_numbering")
                .selected_text(current)
                .show_ui(ui, |ui| {
                    let prefix = p.structure.chapter_prefix.clone();
                    ui.selectable_value(
                        &mut p.structure.chapter_numbering,
                        "roman".to_string(),
                        format!("{prefix} I"),
                    );
                    ui.selectable_value(
                        &mut p.structure.chapter_numbering,
                        "arabic".to_string(),
                        format!("{prefix} 1"),
                    );
                });
        });
        ui.separator();
        ui.label("Foreign-term dictionary (one word per line)");
        // WHY: only the text box scrolls, so a long dictionary never pushes the Save buttons off screen.
        egui::ScrollArea::vertical()
            .id_salt("dictionary_scroll")
            .max_height(150.0)
            .show(ui, |ui| {
                ui.add(
                    egui::TextEdit::multiline(&mut self.dictionary_text)
                        .desired_rows(6)
                        .desired_width(f32::INFINITY),
                );
            });
        if ui.button("Save dictionary").clicked() {
            self.save_dictionary();
        }
        if ui.button("Save profile").clicked() {
            self.save_profile();
        }
    }

    fn results(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            for tab in FilterTab::ALL {
                ui.selectable_value(&mut self.filter, tab, tab.label());
            }
        });
        ui.horizontal(|ui| {
            let loaded = self.docx_path.is_some();
            if ui
                .add_enabled(loaded, egui::Button::new("Auto-Fix & Save Copy"))
                .clicked()
            {
                self.pick_fix_target();
            }
            if ui
                .add_enabled(loaded, egui::Button::new("Export HTML"))
                .clicked()
            {
                self.pick_html_target();
            }
        });
        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("violations").striped(true).show(ui, |ui| {
                ui.strong("Code");
                ui.strong("Para");
                ui.strong("Message");
                ui.end_row();
                for v in self.filtered() {
                    ui.label(&v.code);
                    ui.label(v.paragraph.to_string());
                    ui.label(&v.message);
                    ui.end_row();
                }
            });
        });
    }
}

impl eframe::App for DocxLintApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let dropped: Vec<PathBuf> = ui.ctx().input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|f| f.path().to_path_buf())
                .collect()
        });
        if let Some(p) = dropped.first() {
            self.load_file(p);
        }
        egui::Panel::top("toolbar").show(ui, |ui| self.toolbar(ui));
        egui::Panel::bottom("status").show(ui, |ui| {
            ui.label(&self.status);
        });
        egui::Panel::left("settings").show(ui, |ui| self.settings(ui));
        egui::CentralPanel::default().show(ui, |ui| self.results(ui));
    }
}

pub fn run() -> eframe::Result {
    eframe::run_native(
        "docx-lint",
        eframe::NativeOptions::default(),
        Box::new(|_cc| {
            Ok(Box::new(DocxLintApp::new(ProfileStore::new(
                crate::config::default_dir(),
            ))))
        }),
    )
}
