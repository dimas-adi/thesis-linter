//! Rewrites `word/document.xml` so mechanically fixable violations disappear.
use crate::config::ProfileConfig;
use crate::linter::attr;
use anyhow::{Context, Result, bail};
use quick_xml::events::{BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};
use std::io::{Read, Write};
use std::path::Path;

const TWIPS_PER_CM: f64 = 567.0;

#[derive(Default)]
struct State {
    in_ppr: bool,
    in_rpr: bool,
    in_t: bool,
    heading: bool,
    prev_space: bool,
}

/// Copies `input` to `output`, fixing margins, fonts, sizes, line spacing and double spaces.
pub fn fix_file(input: &Path, output: &Path, cfg: &ProfileConfig) -> Result<()> {
    if input == output {
        bail!("--fix output must differ from the input file");
    }
    let file =
        std::fs::File::open(input).with_context(|| format!("opening {}", input.display()))?;
    let mut zin = zip::ZipArchive::new(file).context("not a valid .docx (zip) archive")?;
    let mut zout = zip::ZipWriter::new(
        std::fs::File::create(output).with_context(|| format!("creating {}", output.display()))?,
    );
    let opts = zip::write::SimpleFileOptions::default();
    for i in 0..zin.len() {
        let mut entry = zin.by_index(i)?;
        let name = entry.name()?.into_owned();
        if entry.is_dir() {
            zout.add_directory(name, opts)?;
            continue;
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        if name == "word/document.xml" {
            bytes = fix_xml(&String::from_utf8(bytes)?, cfg)?.into_bytes();
        }
        zout.start_file(name, opts)?;
        zout.write_all(&bytes)?;
    }
    zout.finish()?;
    Ok(())
}

pub fn fix_xml(xml: &str, cfg: &ProfileConfig) -> Result<String> {
    let mut reader = Reader::from_str(xml);
    let mut w = Writer::new(Vec::new());
    let mut st = State::default();
    loop {
        let ev = reader.read_event().context("malformed document.xml")?;
        match ev {
            Event::Eof => break,
            Event::Start(e) => {
                let ne = fix_elem(&e, &mut st, cfg, true);
                w.write_event(Event::Start(ne))?;
            }
            Event::Empty(e) => {
                let ne = fix_elem(&e, &mut st, cfg, false);
                w.write_event(Event::Empty(ne))?;
            }
            Event::End(e) => {
                match e.local_name().as_ref() {
                    "pPr" => st.in_ppr = false,
                    "rPr" => st.in_rpr = false,
                    "t" => st.in_t = false,
                    _ => {}
                }
                w.write_event(Event::End(e))?;
            }
            Event::Text(t) if st.in_t => {
                let mut fixed = String::new();
                for c in t.xml_content(quick_xml::XmlVersion::Implicit1_0).chars() {
                    // WHY: prev_space spans runs, so "a " + " b" also collapses to one space.
                    if !(c == ' ' && st.prev_space) {
                        fixed.push(c);
                    }
                    st.prev_space = c == ' ';
                }
                w.write_event(Event::Text(BytesText::new(&fixed)))?;
            }
            Event::GeneralRef(g) => {
                st.prev_space = false;
                w.write_event(Event::GeneralRef(g))?;
            }
            other => w.write_event(other)?,
        }
    }
    Ok(String::from_utf8(w.into_inner())?)
}

/// Updates parser state like the linter does and returns the element with corrected attributes.
fn fix_elem(
    e: &BytesStart,
    st: &mut State,
    cfg: &ProfileConfig,
    is_start: bool,
) -> BytesStart<'static> {
    let name = e.local_name();
    let local = name.as_ref().to_string();
    match local.as_str() {
        "p" => {
            st.heading = false;
            st.prev_space = false;
        }
        "pPr" if is_start => st.in_ppr = true,
        "rPr" if is_start => st.in_rpr = true,
        "t" if is_start => st.in_t = true,
        "pStyle" => {
            let s = attr(e, "val").unwrap_or_default().to_lowercase();
            st.heading = s.starts_with("heading") || s == "title";
        }
        _ => {}
    }
    let applies = match local.as_str() {
        "pgMar" => true,
        "rFonts" | "sz" => st.in_rpr && (local == "rFonts" || !st.heading),
        "spacing" => st.in_ppr && attr(e, "lineRule").is_none_or(|r| r == "auto"),
        _ => false,
    };
    let full = e.name().as_ref().to_string();
    let mut ne = BytesStart::new(full);
    for a in e.attributes().flatten() {
        let key = a.key.as_ref().to_string();
        let val = a
            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
            .map(|c| c.into_owned())
            .unwrap_or_default();
        let val = if applies {
            fixed_value(&local, a.key.local_name().as_ref(), &val, cfg).unwrap_or(val)
        } else {
            val
        };
        ne.push_attribute((key.as_str(), val.as_str()));
    }
    ne
}

/// New value for one attribute, or None when it already satisfies the profile.
fn fixed_value(elem: &str, key: &str, val: &str, cfg: &ProfileConfig) -> Option<String> {
    let num: Option<f64> = val.parse().ok();
    match (elem, key) {
        ("pgMar", k) => {
            let expected = match k {
                "top" => cfg.page.top_cm,
                "left" => cfg.page.left_cm,
                "bottom" => cfg.page.bottom_cm,
                "right" => cfg.page.right_cm,
                _ => return None,
            };
            // WHY: 1 cm = 567 twips; only touch values the linter would flag.
            ((num? / TWIPS_PER_CM - expected).abs() > cfg.page.tolerance_cm)
                .then(|| format!("{}", (expected * TWIPS_PER_CM).round() as i64))
        }
        ("rFonts", "ascii" | "hAnsi") => {
            (val != cfg.typography.font_name).then(|| cfg.typography.font_name.clone())
        }
        ("sz", "val") => ((num? / 2.0 - cfg.typography.body_size_pt).abs() > 0.01)
            .then(|| format!("{}", (cfg.typography.body_size_pt * 2.0).round() as i64)),
        ("spacing", "line") => ((num? / 240.0 - cfg.typography.line_spacing).abs() > 0.01)
            .then(|| format!("{}", (cfg.typography.line_spacing * 240.0).round() as i64)),
        _ => None,
    }
}
