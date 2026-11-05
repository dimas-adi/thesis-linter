use anyhow::{Context, Result, anyhow, bail};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PageConfig {
    pub top_cm: f64,
    pub left_cm: f64,
    pub bottom_cm: f64,
    pub right_cm: f64,
    pub tolerance_cm: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TypographyConfig {
    pub font_name: String,
    pub body_size_pt: f64,
    pub line_spacing: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StructureConfig {
    pub chapter_prefix: String,
    pub chapter_numbering: String,
    pub heading_alignment: String,
    pub figure_prefix: String,
    pub table_prefix: String,
    pub caption_separator: String,
    pub bibliography_heading: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AcademicConfig {
    pub forbidden_tokens: Vec<String>,
    pub foreign_terms_must_italicize: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProfileConfig {
    pub profile_name: String,
    pub page: PageConfig,
    pub typography: TypographyConfig,
    pub structure: StructureConfig,
    pub academic: AcademicConfig,
    /// Dictionary words that `ProfileStore::with_dictionary` merged into the foreign terms; never saved.
    #[serde(skip)]
    pub dictionary_added: Vec<String>,
}

impl ProfileConfig {
    /// Rejects values outside the Profile Schema, so unusable numbers never reach the fixer.
    pub fn validate(&self) -> Result<()> {
        let finite_at_least = |name: &str, v: f64, min: f64| {
            if v.is_finite() && v >= min {
                Ok(())
            } else {
                bail!("invalid value for '{name}': {v}")
            }
        };
        let p = &self.page;
        for (name, v) in [
            ("page.top_cm", p.top_cm),
            ("page.left_cm", p.left_cm),
            ("page.bottom_cm", p.bottom_cm),
            ("page.right_cm", p.right_cm),
            ("page.tolerance_cm", p.tolerance_cm),
        ] {
            finite_at_least(name, v, 0.0)?;
        }
        let t = &self.typography;
        for (name, v) in [
            ("typography.body_size_pt", t.body_size_pt),
            ("typography.line_spacing", t.line_spacing),
        ] {
            finite_at_least(name, v, 0.0)?;
            if v == 0.0 {
                bail!("invalid value for '{name}': {v}");
            }
        }
        if t.font_name.trim().is_empty() {
            bail!("invalid value for 'typography.font_name': must not be empty");
        }
        let s = &self.structure;
        for (name, v, allowed) in [
            (
                "structure.chapter_numbering",
                &s.chapter_numbering,
                &["roman", "arabic"][..],
            ),
            (
                "structure.heading_alignment",
                &s.heading_alignment,
                &["center", "left"][..],
            ),
            (
                "structure.caption_separator",
                &s.caption_separator,
                &[".", "-"][..],
            ),
        ] {
            if !allowed.contains(&v.as_str()) {
                bail!(
                    "invalid value for '{name}': '{v}' (allowed: {})",
                    allowed.join(", ")
                );
            }
        }
        Ok(())
    }

    pub fn default_named(name: &str) -> Self {
        ProfileConfig {
            profile_name: name.to_string(),
            page: PageConfig {
                top_cm: 4.0,
                left_cm: 4.0,
                bottom_cm: 3.0,
                right_cm: 3.0,
                tolerance_cm: 0.05,
            },
            typography: TypographyConfig {
                font_name: "Times New Roman".into(),
                body_size_pt: 12.0,
                line_spacing: 1.5,
            },
            structure: StructureConfig {
                chapter_prefix: "BAB".into(),
                chapter_numbering: "roman".into(),
                heading_alignment: "center".into(),
                figure_prefix: "Gambar".into(),
                table_prefix: "Tabel".into(),
                caption_separator: ".".into(),
                bibliography_heading: "DAFTAR PUSTAKA".into(),
            },
            academic: AcademicConfig {
                forbidden_tokens: ["TODO", "TBD", "cek lagi", "lorem ipsum"]
                    .map(String::from)
                    .to_vec(),
                foreign_terms_must_italicize: [
                    "framework",
                    "backend",
                    "frontend",
                    "smartphone",
                    "database",
                    "cache",
                ]
                .map(String::from)
                .to_vec(),
            },
            dictionary_added: Vec::new(),
        }
    }
}

/// Reads/writes profile `.toml` files inside one directory.
pub struct ProfileStore {
    dir: PathBuf,
}

impl ProfileStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        ProfileStore { dir: dir.into() }
    }

    fn path(&self, name: &str) -> Result<PathBuf> {
        // WHY: separators, drive colons, and stream or reserved characters could leave the profiles directory.
        let bad = |c: char| c.is_control() || "/\\.:<>\"|?*".contains(c);
        if name.is_empty() || name.contains(bad) {
            bail!("invalid profile name '{name}'");
        }
        Ok(self.dir.join(format!("{name}.toml")))
    }

    fn active_path(&self) -> PathBuf {
        self.dir.join("active_profile.txt")
    }

    pub fn save(&self, cfg: &ProfileConfig) -> Result<()> {
        self.write(&self.path(&cfg.profile_name)?, cfg)
    }

    fn write(&self, path: &Path, cfg: &ProfileConfig) -> Result<()> {
        cfg.validate()?;
        std::fs::create_dir_all(&self.dir)?;
        // WHY: only the words the merge added are the dictionary's; a term the profile already had stays.
        let mut own = cfg.clone();
        own.academic.foreign_terms_must_italicize.retain(|t| {
            !cfg.dictionary_added
                .iter()
                .any(|d| d.eq_ignore_ascii_case(t))
        });
        std::fs::write(path, toml::to_string_pretty(&own)?)
            .with_context(|| format!("writing {}", path.display()))
    }

    /// True when a profile file with this name exists; an invalid name never exists.
    pub fn exists(&self, name: &str) -> bool {
        self.path(name).is_ok_and(|p| p.exists())
    }

    /// Reads and validates the profile file only, without the dictionary.
    fn load_raw(&self, name: &str) -> Result<ProfileConfig> {
        let path = self.path(name)?;
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("profile '{name}' not found at {}", path.display()))?;
        let cfg: ProfileConfig = toml::from_str(&text)?;
        cfg.validate()?;
        Ok(cfg)
    }

    pub fn load(&self, name: &str) -> Result<ProfileConfig> {
        Ok(self.with_dictionary(self.load_raw(name)?))
    }

    /// Merges dictionary words into the foreign terms and records which ones it added.
    pub fn with_dictionary(&self, mut cfg: ProfileConfig) -> ProfileConfig {
        let terms = &mut cfg.academic.foreign_terms_must_italicize;
        let mut added = Vec::new();
        for w in self.load_dictionary() {
            if !terms.iter().any(|t| t.eq_ignore_ascii_case(&w)) {
                terms.push(w.clone());
                added.push(w);
            }
        }
        cfg.dictionary_added = added;
        cfg
    }

    fn dictionary_path(&self) -> PathBuf {
        self.dir.join("foreign_terms.txt")
    }

    /// The whole decoded text of `foreign_terms.txt`, comments and blank lines included.
    pub fn load_dictionary_text(&self) -> String {
        std::fs::read(self.dictionary_path()).map_or_else(|_| String::new(), |b| decode_text(&b))
    }

    /// Words from `foreign_terms.txt` (one per line; blank lines and `#` comments ignored).
    pub fn load_dictionary(&self) -> Vec<String> {
        let text = self.load_dictionary_text();
        text.lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(String::from)
            .collect()
    }

    pub fn save_dictionary(&self, text: &str) -> Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        std::fs::write(self.dictionary_path(), text).context("writing foreign_terms.txt")
    }

    pub fn init(&self, name: &str) -> Result<ProfileConfig> {
        if self.path(name)?.exists() {
            bail!("profile '{name}' already exists");
        }
        let cfg = ProfileConfig::default_named(name);
        self.save(&cfg)?;
        Ok(cfg)
    }

    pub fn set_active(&self, name: &str) -> Result<()> {
        self.load(name)?;
        std::fs::write(self.active_path(), name)?;
        Ok(())
    }

    pub fn active_name(&self) -> Option<String> {
        let s = std::fs::read_to_string(self.active_path()).ok()?;
        let s = s.trim();
        (!s.is_empty()).then(|| s.to_string())
    }

    /// Named profile, else the active one, else built-in defaults.
    pub fn resolve(&self, name: Option<&str>) -> Result<ProfileConfig> {
        match name.map(String::from).or_else(|| self.active_name()) {
            Some(n) => self.load(&n),
            None => Ok(self.with_dictionary(ProfileConfig::default_named("default_kampus"))),
        }
    }

    /// Sets a dotted key (e.g. `page.top_cm`), keeping the value's existing type.
    pub fn set_value(&self, name: &str, key: &str, value: &str) -> Result<()> {
        if key == "profile_name" {
            bail!("'profile_name' cannot be changed; it is the file name");
        }
        let cfg = self.load_raw(name)?;
        let mut root = toml::Value::try_from(&cfg)?;
        let mut cur = &mut root;
        let mut parts = key.split('.').peekable();
        while let Some(part) = parts.next() {
            let table = cur
                .as_table_mut()
                .ok_or_else(|| anyhow!("unknown key '{key}'"))?;
            let slot = table
                .get_mut(part)
                .ok_or_else(|| anyhow!("unknown key '{key}'"))?;
            if parts.peek().is_none() {
                *slot =
                    parse_like(slot, value).with_context(|| format!("bad value for '{key}'"))?;
                break;
            }
            cur = slot;
        }
        let updated: ProfileConfig = root.try_into()?;
        self.write(&self.path(name)?, &updated)
    }

    /// Sorted names of every `<name>.toml` profile in the directory.
    pub fn list(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                (p.extension()? == "toml").then(|| p.file_stem()?.to_str().map(String::from))?
            })
            .collect();
        names.sort();
        names
    }
}

/// Decodes a text file saved as UTF-8 (with or without BOM) or UTF-16 LE with BOM; anything else lossily.
fn decode_text(bytes: &[u8]) -> String {
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        let units: Vec<u16> = rest
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_le_bytes(*c))
            .collect();
        return String::from_utf16_lossy(&units);
    }
    String::from_utf8_lossy(bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes)).into_owned()
}

// WHY: the old value's type decides how the CLI string is parsed (float vs string vs list).
fn parse_like(old: &toml::Value, raw: &str) -> Result<toml::Value> {
    Ok(match old {
        toml::Value::Float(_) => toml::Value::Float(raw.parse()?),
        toml::Value::Integer(_) => toml::Value::Integer(raw.parse()?),
        toml::Value::Boolean(_) => toml::Value::Boolean(raw.parse()?),
        toml::Value::Array(_) => toml::Value::Array(
            raw.split(',')
                .map(|s| toml::Value::String(s.trim().to_string()))
                .collect(),
        ),
        _ => toml::Value::String(raw.to_string()),
    })
}

pub fn default_dir() -> &'static Path {
    Path::new("profiles")
}
