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
}

impl ProfileConfig {
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
        // WHY: reject separators so a profile name can never escape the profiles directory.
        if name.is_empty() || name.contains(['/', '\\', '.']) {
            bail!("invalid profile name '{name}'");
        }
        Ok(self.dir.join(format!("{name}.toml")))
    }

    fn active_path(&self) -> PathBuf {
        self.dir.join("active_profile.txt")
    }

    pub fn save(&self, cfg: &ProfileConfig) -> Result<()> {
        let path = self.path(&cfg.profile_name)?;
        std::fs::create_dir_all(&self.dir)?;
        // WHY: dictionary words were merged in on load; writing them back would duplicate the file's content.
        let dict = self.load_dictionary();
        let mut own = cfg.clone();
        own.academic
            .foreign_terms_must_italicize
            .retain(|t| !dict.iter().any(|d| d.eq_ignore_ascii_case(t)));
        std::fs::write(&path, toml::to_string_pretty(&own)?)
            .with_context(|| format!("writing {}", path.display()))
    }

    pub fn load(&self, name: &str) -> Result<ProfileConfig> {
        let path = self.path(name)?;
        let text = std::fs::read_to_string(&path)
            .with_context(|| format!("profile '{name}' not found at {}", path.display()))?;
        Ok(self.with_dictionary(toml::from_str(&text)?))
    }

    fn with_dictionary(&self, mut cfg: ProfileConfig) -> ProfileConfig {
        let terms = &mut cfg.academic.foreign_terms_must_italicize;
        for w in self.load_dictionary() {
            if !terms.iter().any(|t| t.eq_ignore_ascii_case(&w)) {
                terms.push(w);
            }
        }
        cfg
    }

    fn dictionary_path(&self) -> PathBuf {
        self.dir.join("foreign_terms.txt")
    }

    /// Words from `foreign_terms.txt` (one per line; blank lines and `#` comments ignored).
    pub fn load_dictionary(&self) -> Vec<String> {
        let text = std::fs::read_to_string(self.dictionary_path()).unwrap_or_default();
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
        let cfg = self.load(name)?;
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
        self.save(&updated)
    }
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

impl ProfileStore {
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
