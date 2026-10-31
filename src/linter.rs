use crate::config::ProfileConfig;
use anyhow::{Context, Result};
use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde::Serialize;
use std::io::Read;
use std::path::Path;

const TWIPS_PER_CM: f64 = 567.0;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Violation {
    pub code: String,
    pub message: String,
    /// 1-based paragraph index, or 0 for document-level findings.
    pub paragraph: usize,
}

fn v(code: &str, message: String, paragraph: usize) -> Violation {
    Violation {
        code: code.into(),
        message,
        paragraph,
    }
}

pub fn read_document_xml(path: &Path) -> Result<String> {
    let file = std::fs::File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut zip = zip::ZipArchive::new(file).context("not a valid .docx (zip) archive")?;
    let mut entry = zip
        .by_name("word/document.xml")
        .context("missing word/document.xml")?;
    let mut xml = String::new();
    entry.read_to_string(&mut xml)?;
    Ok(xml)
}

pub fn lint_file(path: &Path, cfg: &ProfileConfig) -> Result<Vec<Violation>> {
    lint_xml(&read_document_xml(path)?, cfg)
}

fn attr(e: &BytesStart, name: &str) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.local_name().as_ref() == name)
        .and_then(|a| {
            a.normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .ok()
                .map(|c| c.into_owned())
        })
}

fn attr_f64(e: &BytesStart, name: &str) -> Option<f64> {
    attr(e, name)?.parse().ok()
}

/// Streams `word/document.xml` once and checks margins, fonts and line spacing.
pub fn lint_xml(xml: &str, cfg: &ProfileConfig) -> Result<Vec<Violation>> {
    let mut out = Vec::new();
    // WHY: quick-xml is pull-based; we keep our own flags to know which parent element we are in.
    let mut reader = Reader::from_str(xml);
    let mut para = 0usize;
    let (mut in_ppr, mut in_rpr) = (false, false);
    let mut heading = false;
    let tol = cfg.page.tolerance_cm;

    loop {
        let ev = reader.read_event().context("malformed document.xml")?;
        let (e, is_start) = match &ev {
            Event::Eof => break,
            Event::Start(e) => (e, true),
            Event::Empty(e) => (e, false),
            Event::End(e) => {
                match e.local_name().as_ref() {
                    "pPr" => in_ppr = false,
                    "rPr" => in_rpr = false,
                    _ => {}
                }
                continue;
            }
            _ => continue,
        };
        match e.local_name().as_ref() {
            "p" => {
                para += 1;
                heading = false;
            }
            "pPr" if is_start => in_ppr = true,
            "rPr" if is_start => in_rpr = true,
            "pStyle" => {
                let s = attr(e, "val").unwrap_or_default().to_lowercase();
                heading = s.starts_with("heading") || s == "title";
            }
            "pgMar" => {
                for (side, expected) in [
                    ("top", cfg.page.top_cm),
                    ("left", cfg.page.left_cm),
                    ("bottom", cfg.page.bottom_cm),
                    ("right", cfg.page.right_cm),
                ] {
                    if let Some(tw) = attr_f64(e, side) {
                        let cm = tw / TWIPS_PER_CM;
                        if (cm - expected).abs() > tol {
                            out.push(v(
                                "MARGIN_ERROR",
                                format!("{side} margin is {cm:.2} cm, expected {expected:.2} cm"),
                                0,
                            ));
                        }
                    }
                }
            }
            "rFonts" if in_rpr => {
                for key in ["ascii", "hAnsi"] {
                    if let Some(f) = attr(e, key)
                        && f != cfg.typography.font_name
                    {
                        out.push(v(
                            "FONT_ERROR",
                            format!("font '{f}', expected '{}'", cfg.typography.font_name),
                            para,
                        ));
                        break;
                    }
                }
            }
            "sz" if in_rpr && !heading => {
                if let Some(half) = attr_f64(e, "val") {
                    let pt = half / 2.0;
                    if (pt - cfg.typography.body_size_pt).abs() > 0.01 {
                        out.push(v(
                            "FONT_ERROR",
                            format!("size {pt} pt, expected {} pt", cfg.typography.body_size_pt),
                            para,
                        ));
                    }
                }
            }
            "spacing" if in_ppr => {
                let auto = attr(e, "lineRule").is_none_or(|r| r == "auto");
                if let (Some(line), true) = (attr_f64(e, "line"), auto) {
                    let ratio = line / 240.0;
                    if (ratio - cfg.typography.line_spacing).abs() > 0.01 {
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
            _ => {}
        }
    }
    Ok(out)
}
