use crate::config::ProfileConfig;
use crate::structure::{Document, Layout, contexts, margin_off, parse_document};
use anyhow::{Context, Result};
use quick_xml::events::BytesStart;
use serde::Serialize;
use std::io::Read;
use std::path::Path;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Violation {
    pub code: String,
    pub message: String,
    /// 1-based paragraph index, or 0 for document-level findings.
    pub paragraph: usize,
}

pub(crate) fn v(code: &str, message: String, paragraph: usize) -> Violation {
    Violation {
        code: code.into(),
        message,
        paragraph,
    }
}

/// Largest `word/document.xml` (decompressed) that lint and fix will read.
pub const MAX_DOCUMENT_XML_BYTES: u64 = 64 * 1024 * 1024;

/// Reads UTF-8 text from `r`, failing once it passes `limit` bytes.
pub(crate) fn read_limited(r: &mut impl Read, limit: u64) -> Result<String> {
    let mut bytes = Vec::new();
    r.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        anyhow::bail!("word/document.xml exceeds the {limit} byte limit");
    }
    Ok(String::from_utf8(bytes)?)
}

pub fn read_document_xml(path: &Path) -> Result<String> {
    read_document_xml_limited(path, MAX_DOCUMENT_XML_BYTES)
}

/// Like `read_document_xml`, with an explicit size limit in bytes.
pub fn read_document_xml_limited(path: &Path, limit: u64) -> Result<String> {
    let file = std::fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut zip = zip::ZipArchive::new(file).context("not a valid .docx (zip) archive")?;
    let mut entry = zip
        .by_name("word/document.xml")
        .context("missing word/document.xml")?;
    read_limited(&mut entry, limit)
}

pub fn lint_file(path: &Path, cfg: &ProfileConfig) -> Result<Vec<Violation>> {
    lint_xml(&read_document_xml(path)?, cfg)
}

pub(crate) fn attr(e: &BytesStart, name: &str) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.local_name().as_ref() == name)
        .and_then(|a| {
            a.normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .ok()
                .map(|c| c.into_owned())
        })
}

/// Lints `word/document.xml`: parses it once, then checks the parsed model.
pub fn lint_xml(xml: &str, cfg: &ProfileConfig) -> Result<Vec<Violation>> {
    Ok(lint_document(&parse_document(xml)?, cfg))
}

/// Layout findings in document order, then the structure checks.
pub fn lint_document(doc: &Document, cfg: &ProfileConfig) -> Vec<Violation> {
    let mut out = Vec::new();
    let ctx = contexts(&doc.paras, cfg);
    for fact in &doc.facts {
        let para = fact.para;
        let index = para.checked_sub(1);
        let cur = index.and_then(|i| ctx.get(i)).copied().unwrap_or_default();
        match &fact.layout {
            Layout::Margin { side, twips } => {
                let expected = match *side {
                    "top" => cfg.page.top_cm,
                    "left" => cfg.page.left_cm,
                    "bottom" => cfg.page.bottom_cm,
                    _ => cfg.page.right_cm,
                };
                if margin_off(*twips, expected, cfg.page.tolerance_cm) {
                    let cm = twips / crate::structure::TWIPS_PER_CM;
                    out.push(v(
                        "MARGIN_ERROR",
                        format!("{side} margin is {cm:.2} cm, expected {expected:.2} cm"),
                        0,
                    ));
                }
            }
            Layout::Fonts { ascii, h_ansi } => {
                if let Some(f) = [ascii, h_ansi]
                    .into_iter()
                    .flatten()
                    .find(|f| **f != cfg.typography.font_name)
                {
                    out.push(v(
                        "FONT_ERROR",
                        format!("font '{f}', expected '{}'", cfg.typography.font_name),
                        para,
                    ));
                }
            }
            Layout::Size { half_pt } => {
                let heading = index
                    .and_then(|i| doc.paras.get(i))
                    .is_some_and(|p| p.heading);
                let pt = half_pt / 2.0;
                if !heading && !cur.size_ok(pt, cfg) {
                    out.push(v(
                        "FONT_ERROR",
                        format!("size {pt} pt, expected {} pt", cfg.typography.body_size_pt),
                        para,
                    ));
                }
            }
            Layout::Spacing { line } => {
                let ratio = line / 240.0;
                if !cur.spacing_ok(ratio, cfg) {
                    out.push(v(
                        "SPACING_ERROR",
                        format!(
                            "line spacing {ratio:.2}, expected {}",
                            cfg.typography.line_spacing
                        ),
                        para,
                    ));
                }
            }
        }
    }
    crate::structure::check(&doc.paras, cfg, &mut out);
    out
}
