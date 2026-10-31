//! Rewrites `word/document.xml` so mechanically fixable violations disappear.
use crate::config::ProfileConfig;
use crate::linter::attr;
use crate::structure::{Ctx, find_words, para_context};
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
    para: usize,
    ctx: Vec<Ctx>,
    cur: Ctx,
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
    let mut st = State {
        ctx: para_context(xml, cfg)?,
        ..State::default()
    };
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
    split_foreign_runs(&String::from_utf8(w.into_inner())?, cfg)
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Inserts `<w:i/>` into a run's `<w:rPr>` (creating one if absent) in schema order.
fn italic_rpr(rpr: &str, pfx: &str) -> String {
    let tag = format!("<{pfx}i/>");
    if rpr.is_empty() {
        return format!("<{pfx}rPr>{tag}</{pfx}rPr>");
    }
    if let Some(open_end) = rpr
        .find("/>")
        .filter(|_| rpr.ends_with("/>") && !rpr.contains("</"))
    {
        // `<w:rPr/>` with no children.
        return format!("{}>{tag}</{pfx}rPr>", &rpr[..open_end]);
    }
    let mut s = rpr.to_string();
    // WHY: an explicit `<w:i w:val="0"/>` would override ours, so drop it first.
    let key = format!("<{pfx}i");
    let mut from = 0;
    while let Some(off) = s[from..].find(&key) {
        let at = from + off;
        if s[at + key.len()..].starts_with(['/', ' ']) {
            let end = at + s[at..].find("/>").map_or(0, |e| e + 2);
            s.replace_range(at..end.max(at), "");
            break;
        }
        from = at + key.len();
    }
    // WHY: CT_RPr is an ordered sequence; `i` must come after rStyle, rFonts, b and bCs.
    let mut insert = s.find('>').map_or(0, |i| i + 1);
    for name in ["rStyle", "rFonts", "b", "bCs"] {
        let key = format!("<{pfx}{name}");
        let mut from = 0;
        while let Some(off) = s[from..].find(&key) {
            let at = from + off;
            if s[at + key.len()..].starts_with(['/', ' ', '>']) {
                if let Some(e) = s[at..].find("/>") {
                    insert = insert.max(at + e + 2);
                }
                break;
            }
            from = at + key.len();
        }
    }
    s.insert_str(insert, &tag);
    s
}

/// Splits plain runs so every non-italic foreign term sits in its own italic `<w:r>`.
fn split_foreign_runs(xml: &str, cfg: &ProfileConfig) -> Result<String> {
    let terms = &cfg.academic.foreign_terms_must_italicize;
    if terms.iter().all(|t| t.is_empty()) {
        return Ok(xml.to_string());
    }
    let mut reader = Reader::from_str(xml);
    let mut out = String::with_capacity(xml.len());
    let mut copied = 0usize;
    loop {
        let before = reader.buffer_position() as usize;
        let ev = reader.read_event().context("malformed document.xml")?;
        match ev {
            Event::Eof => break,
            Event::Start(e) if e.local_name().as_ref() == "r" => {
                let start_tag_end = reader.buffer_position() as usize;
                let name = e.name().as_ref().to_string();
                let pfx = name.strip_suffix('r').unwrap_or("w:").to_string();
                // WHY: read_to_end skips the whole run so we can slice its raw XML by byte offsets.
                reader.read_to_end(e.name())?;
                let run_end = reader.buffer_position() as usize;
                let inner = &xml[start_tag_end..run_end - format!("</{name}>").len()];
                if let Some(new) = split_run(&xml[before..start_tag_end], inner, &pfx, terms)? {
                    out.push_str(&xml[copied..before]);
                    out.push_str(&new);
                    copied = run_end;
                }
            }
            _ => {}
        }
    }
    out.push_str(&xml[copied..]);
    Ok(out)
}

/// One child of a run body: text from a `<w:t>`, or any other empty element kept verbatim.
enum Piece {
    Text(String),
    Raw(String),
}

/// Replacement XML for one run, or None when the run is untouched.
fn split_run(open: &str, inner: &str, pfx: &str, terms: &[String]) -> Result<Option<String>> {
    // Parse the run body: `rPr`, any number of `t`, and empty markers (tab, br, page break...).
    let mut rd = Reader::from_str(inner);
    let (mut rpr, mut pieces): (String, Vec<Piece>) = (String::new(), Vec::new());
    let (mut in_t, mut depth_rpr) = (false, 0usize);
    let mut rpr_start = 0usize;
    loop {
        let before = rd.buffer_position() as usize;
        match rd.read_event()? {
            Event::Eof => break,
            Event::Start(e) => match e.local_name().as_ref() {
                "rPr" if depth_rpr == 0 => {
                    depth_rpr = 1;
                    rpr_start = before;
                }
                "t" if depth_rpr == 0 => {
                    in_t = true;
                    pieces.push(Piece::Text(String::new()));
                }
                _ if depth_rpr > 0 => depth_rpr += 1,
                _ => return Ok(None),
            },
            Event::Empty(e) => match e.local_name().as_ref() {
                "rPr" if depth_rpr == 0 => {
                    rpr = inner[before..rd.buffer_position() as usize].to_string()
                }
                _ if depth_rpr > 0 => {}
                "t" => {}
                // WHY: keep tabs, breaks and rendered-page-break markers in their original order.
                _ => pieces.push(Piece::Raw(
                    inner[before..rd.buffer_position() as usize].to_string(),
                )),
            },
            Event::End(e) => {
                if in_t && e.local_name().as_ref() == "t" {
                    in_t = false;
                } else if depth_rpr > 0 {
                    depth_rpr -= 1;
                    if depth_rpr == 0 {
                        rpr = inner[rpr_start..rd.buffer_position() as usize].to_string();
                    }
                }
            }
            Event::Text(t) if in_t => {
                if let Some(Piece::Text(text)) = pieces.last_mut() {
                    text.push_str(&t.xml_content(quick_xml::XmlVersion::Implicit1_0));
                }
            }
            Event::GeneralRef(g) if in_t => {
                if let Some(Piece::Text(text)) = pieces.last_mut() {
                    text.push(match g.into_inner().as_ref() {
                        "amp" => '&',
                        "lt" => '<',
                        "gt" => '>',
                        "quot" => '"',
                        "apos" => '\'',
                        _ => return Ok(None),
                    })
                }
            }
            _ => {}
        }
    }
    if open.ends_with("/>") {
        return Ok(None);
    }
    // Already italic runs keep their formatting.
    let italic_key = format!("<{pfx}i");
    let already = rpr.match_indices(&italic_key).any(|(i, _)| {
        let rest = &rpr[i + italic_key.len()..];
        rest.starts_with(['/', ' ']) && {
            let el = &rest[..rest.find("/>").unwrap_or(rest.len())];
            !(el.contains("\"0\"") || el.contains("false"))
        }
    });
    if already {
        return Ok(None);
    }
    let it = italic_rpr(&rpr, pfx);
    let mk = |rp: &str, s: &str| {
        format!(
            "{open}{rp}<{pfx}t xml:space=\"preserve\">{}</{pfx}t></{pfx}r>",
            escape(s)
        )
    };
    let (mut out, mut changed) = (String::new(), false);
    for piece in &pieces {
        let text = match piece {
            Piece::Raw(raw) => {
                out.push_str(&format!("{open}{rpr}{raw}</{pfx}r>"));
                continue;
            }
            Piece::Text(t) => t,
        };
        let mut hits: Vec<(usize, usize)> =
            terms.iter().flat_map(|t| find_words(text, t)).collect();
        hits.sort();
        // WHY: overlapping terms (e.g. "data" and "database") would double-split; merge them.
        let mut spans: Vec<(usize, usize)> = Vec::new();
        for h in hits {
            match spans.last_mut() {
                Some(last) if h.0 < last.1 => last.1 = last.1.max(h.1),
                _ => spans.push(h),
            }
        }
        changed |= !spans.is_empty();
        let mut pos = 0;
        for (a, b) in spans {
            if a > pos {
                out.push_str(&mk(&rpr, &text[pos..a]));
            }
            out.push_str(&mk(&it, &text[a..b]));
            pos = b;
        }
        if pos < text.len() || text.is_empty() {
            out.push_str(&mk(&rpr, &text[pos..]));
        }
    }
    Ok(changed.then_some(out))
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
            st.para += 1;
            st.cur = st.ctx.get(st.para - 1).copied().unwrap_or_default();
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
            fixed_value(&local, a.key.local_name().as_ref(), &val, cfg, st.cur).unwrap_or(val)
        } else {
            val
        };
        ne.push_attribute((key.as_str(), val.as_str()));
    }
    ne
}

/// New value for one attribute, or None when it already satisfies the profile.
fn fixed_value(elem: &str, key: &str, val: &str, cfg: &ProfileConfig, ctx: Ctx) -> Option<String> {
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
        // WHY: reuse the linter's exemption rules so the fixer never "repairs" cover or table text.
        ("sz", "val") => (!ctx.size_ok(num? / 2.0, cfg))
            .then(|| format!("{}", (cfg.typography.body_size_pt * 2.0).round() as i64)),
        ("spacing", "line") => (!ctx.spacing_ok(num? / 240.0, cfg))
            .then(|| format!("{}", (cfg.typography.line_spacing * 240.0).round() as i64)),
        _ => None,
    }
}
