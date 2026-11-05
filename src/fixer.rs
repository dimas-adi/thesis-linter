//! Rewrites `word/document.xml` so mechanically fixable violations disappear.
use crate::config::ProfileConfig;
use crate::linter::{MAX_DOCUMENT_XML_BYTES, attr, read_limited};
use crate::structure::{
    Ctx, Ns, TWIPS_PER_CM, contexts, escape_text, find_words, is_heading_style, margin_off,
    parse_document, resolve_ref,
};
use anyhow::{Context, Result, bail};
use quick_xml::events::{BytesStart, BytesText, Event};
use quick_xml::{NsReader, Reader, Writer};
use std::io::Write;
use std::path::Path;

/// Walk state of the rewrite pass: flags, paragraph counter, and the (heading, Ctx) of each open paragraph.
#[derive(Default)]
struct State {
    in_ppr: bool,
    in_rpr: bool,
    in_t: bool,
    prev_space: bool,
    math_depth: usize,
    para: usize,
    ctx: Vec<Ctx>,
    cur: (bool, Ctx),
    saved: Vec<(bool, Ctx)>,
}

impl State {
    fn leave(&mut self, local: &str, ns: Ns) {
        match (ns, local) {
            (Ns::Math, "oMath" | "oMathPara") => {
                self.math_depth = self.math_depth.saturating_sub(1)
            }
            (Ns::Word, "pPr") => self.in_ppr = false,
            (Ns::Word, "rPr") => self.in_rpr = false,
            (Ns::Word, "t") => self.in_t = false,
            // WHY: a text box ends inside its outer paragraph, which gets its own heading and Ctx back.
            (Ns::Word, "p") => {
                self.cur = self.saved.pop().unwrap_or_default();
                self.prev_space = false;
            }
            _ => {}
        }
    }
}

/// Fails when `output` is an existing file that is the same file as `input`, under any spelling.
pub fn ensure_distinct(input: &Path, output: &Path) -> Result<()> {
    // WHY: path text can differ (`./x`, case, `..`, hard link) while naming one file; compare file identity.
    if output.exists() && same_file::is_same_file(input, output).unwrap_or(false) {
        bail!("output must differ from the input file");
    }
    Ok(())
}

/// Copies `input` to `output`, fixing margins, fonts, sizes, line spacing and double spaces.
pub fn fix_file(input: &Path, output: &Path, cfg: &ProfileConfig) -> Result<()> {
    ensure_distinct(input, output)?;
    let file =
        std::fs::File::open(input).with_context(|| format!("opening {}", input.display()))?;
    let mut zin = zip::ZipArchive::new(file).context("not a valid .docx (zip) archive")?;
    // WHY: build beside the output and rename last, so a failed fix never destroys an earlier output.
    let name = output
        .file_name()
        .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let tmp = output.with_file_name(format!(".{name}.{}.tmp", std::process::id()));
    let result = write_fixed(&mut zin, &tmp, cfg).and_then(|()| {
        std::fs::rename(&tmp, output).with_context(|| format!("creating {}", output.display()))
    });
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

fn write_fixed(
    zin: &mut zip::ZipArchive<std::fs::File>,
    tmp: &Path,
    cfg: &ProfileConfig,
) -> Result<()> {
    let mut zout = zip::ZipWriter::new(
        std::fs::File::create(tmp).with_context(|| format!("creating {}", tmp.display()))?,
    );
    let opts = zip::write::SimpleFileOptions::default();
    for i in 0..zin.len() {
        let mut entry = zin.by_index(i)?;
        let name = entry.name()?.into_owned();
        if entry.is_dir() {
            zout.add_directory(name, opts)?;
            continue;
        }
        if name == "word/document.xml" {
            let xml = read_limited(&mut entry, MAX_DOCUMENT_XML_BYTES)?;
            zout.start_file(name, opts)?;
            zout.write_all(fix_xml(&xml, cfg)?.as_bytes())?;
        } else {
            zout.start_file(name, opts)?;
            std::io::copy(&mut entry, &mut zout)?;
        }
    }
    zout.finish()?;
    Ok(())
}

pub fn fix_xml(xml: &str, cfg: &ProfileConfig) -> Result<String> {
    let doc = parse_document(xml)?;
    let mut reader = NsReader::from_str(xml);
    let mut w = Writer::new(Vec::new());
    let mut st = State {
        ctx: contexts(&doc.paras, cfg),
        ..State::default()
    };
    loop {
        let (res, ev) = reader
            .read_resolved_event()
            .context("malformed document.xml")?;
        let ns = Ns::of(&res);
        match ev {
            Event::Eof => break,
            Event::Start(e) => {
                let ne = fix_elem(&e, ns, &mut st, cfg, true);
                w.write_event(Event::Start(ne))?;
            }
            Event::Empty(e) => {
                let ne = fix_elem(&e, ns, &mut st, cfg, false);
                w.write_event(Event::Empty(ne))?;
            }
            Event::End(e) => {
                st.leave(e.local_name().as_ref(), ns);
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
    let mut reader = NsReader::from_str(xml);
    let mut out = String::with_capacity(xml.len());
    let mut copied = 0usize;
    // Open `w:r` elements: (start of the tag, end of the start tag, qualified name).
    let mut open: Vec<(usize, usize, String)> = Vec::new();
    loop {
        let before = reader.buffer_position() as usize;
        let (res, ev) = reader
            .read_resolved_event()
            .context("malformed document.xml")?;
        let word = Ns::of(&res) == Ns::Word;
        match ev {
            Event::Eof => break,
            Event::Start(e) if word && e.local_name().as_ref() == "r" => {
                let name = e.name().as_ref().to_string();
                open.push((before, reader.buffer_position() as usize, name));
            }
            Event::End(e) if word && e.local_name().as_ref() == "r" => {
                let Some((start, tag_end, name)) = open.pop() else {
                    continue;
                };
                let run_end = reader.buffer_position() as usize;
                let pfx = name.strip_suffix('r').unwrap_or("w:");
                // WHY: a run holding a text box is not rewritten; its inner runs end first and were already handled.
                if let Some(new) =
                    split_run(&xml[start..tag_end], &xml[tag_end..before], pfx, terms)?
                {
                    out.push_str(&xml[copied..start]);
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

/// Text a marker element adds to the run text the linter sees (see `parse_document`).
fn marker_text(raw: &str) -> &'static str {
    let name = raw[1..]
        .split(|c: char| c.is_whitespace() || c == '/' || c == '>')
        .next()
        .unwrap_or("");
    match name.rsplit(':').next().unwrap_or("") {
        "tab" => "\t",
        "br" | "cr" => "\n",
        _ => "",
    }
}

/// A piece of a run, tagged with the term span (index) that covers it, if any.
type Cut = (Option<usize>, Piece);

/// Cuts every piece at the term spans; markers inside a span stay with that span.
fn cut_pieces(pieces: Vec<Piece>, joined: &str, spans: &[(usize, usize)]) -> Vec<Cut> {
    let mut cuts: Vec<Cut> = Vec::new();
    let mut at = 0usize;
    for piece in pieces {
        match piece {
            Piece::Raw(raw) => {
                let end = at + marker_text(&raw).len();
                let inside = spans.iter().position(|&(a, b)| {
                    if end == at {
                        a < at && at < b
                    } else {
                        a <= at && end <= b
                    }
                });
                cuts.push((inside, Piece::Raw(raw)));
                at = end;
            }
            Piece::Text(text) => {
                let end = at + text.len();
                if text.is_empty() {
                    let inside = spans.iter().position(|&(a, b)| a < at && at < b);
                    cuts.push((inside, Piece::Text(text)));
                    continue;
                }
                let mut pos = at;
                for (i, &(a, b)) in spans.iter().enumerate() {
                    if b <= pos || a >= end {
                        continue;
                    }
                    let (lo, hi) = (a.max(pos), b.min(end));
                    if lo > pos {
                        cuts.push((None, Piece::Text(joined[pos..lo].to_string())));
                    }
                    cuts.push((Some(i), Piece::Text(joined[lo..hi].to_string())));
                    pos = hi;
                }
                if pos < end {
                    cuts.push((None, Piece::Text(joined[pos..end].to_string())));
                }
                at = end;
            }
        }
    }
    cuts
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
                    text.push(resolve_ref(&g)?);
                }
            }
            _ => {}
        }
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
    // WHY: match on the joined text, exactly as the linter reads the run, so terms across `w:t` are found.
    let joined: String = pieces
        .iter()
        .map(|p| match p {
            Piece::Text(t) => t.as_str(),
            Piece::Raw(r) => marker_text(r),
        })
        .collect();
    let mut hits: Vec<(usize, usize)> = terms.iter().flat_map(|t| find_words(&joined, t)).collect();
    hits.sort();
    // WHY: overlapping terms (e.g. "data" and "database") would double-split; merge them.
    let mut spans: Vec<(usize, usize)> = Vec::new();
    for h in hits {
        match spans.last_mut() {
            Some(last) if h.0 < last.1 => last.1 = last.1.max(h.1),
            _ => spans.push(h),
        }
    }
    if spans.is_empty() {
        return Ok(None);
    }
    let it = italic_rpr(&rpr, pfx);
    let text_el = |s: &str| format!("<{pfx}t xml:space=\"preserve\">{}</{pfx}t>", escape_text(s));
    let cuts = cut_pieces(pieces, &joined, &spans);
    let mut out = String::new();
    let mut i = 0;
    while i < cuts.len() {
        match cuts[i].0 {
            // Outside any term: one run per piece, with the original formatting.
            None => {
                let body = match &cuts[i].1 {
                    Piece::Text(t) => text_el(t),
                    Piece::Raw(r) => r.clone(),
                };
                out.push_str(&format!("{open}{rpr}{body}</{pfx}r>"));
                i += 1;
            }
            // Inside a term: every piece the term covers shares one italic run.
            Some(span) => {
                out.push_str(&format!("{open}{it}"));
                while i < cuts.len() && cuts[i].0 == Some(span) {
                    match &cuts[i].1 {
                        Piece::Text(t) => out.push_str(&text_el(t)),
                        Piece::Raw(r) => out.push_str(r),
                    }
                    i += 1;
                }
                out.push_str(&format!("</{pfx}r>"));
            }
        }
    }
    Ok(Some(out))
}

/// Updates parser state like the linter does and returns the element with corrected attributes.
fn fix_elem(
    e: &BytesStart,
    ns: Ns,
    st: &mut State,
    cfg: &ProfileConfig,
    is_start: bool,
) -> BytesStart<'static> {
    let name = e.local_name();
    let local = name.as_ref().to_string();
    match ns {
        Ns::Math => {
            if is_start && matches!(local.as_str(), "oMath" | "oMathPara") {
                st.math_depth += 1;
            }
            return e.clone().into_owned();
        }
        Ns::Other => return e.clone().into_owned(),
        Ns::Word => {}
    }
    match local.as_str() {
        "p" => {
            st.para += 1;
            st.prev_space = false;
            let ctx = st.ctx.get(st.para - 1).copied().unwrap_or_default();
            if is_start {
                st.saved.push(st.cur);
                st.cur = (false, ctx);
            }
        }
        "pPr" if is_start => st.in_ppr = true,
        "rPr" if is_start => st.in_rpr = true,
        "t" if is_start => st.in_t = true,
        "pStyle" => st.cur.0 = is_heading_style(&attr(e, "val").unwrap_or_default()),
        "tab" | "br" | "cr" => st.prev_space = false,
        _ => {}
    }
    let heading = st.cur.0;
    let applies = match local.as_str() {
        "pgMar" => true,
        "rFonts" | "sz" => st.in_rpr && st.math_depth == 0 && (local == "rFonts" || !heading),
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
            fixed_value(&local, a.key.local_name().as_ref(), &val, cfg, st.cur.1).unwrap_or(val)
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
            // WHY: only touch values the linter would flag.
            margin_off(num?, expected, cfg.page.tolerance_cm)
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
