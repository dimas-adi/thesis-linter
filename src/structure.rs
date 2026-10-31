//! Paragraph-level checks: headings, captions, citations, draft tokens, spacing, italics.
use crate::config::ProfileConfig;
use crate::linter::{Violation, attr, v};
use anyhow::{Context, Result};
use quick_xml::Reader;
use quick_xml::events::Event;

pub struct Run {
    pub text: String,
    pub italic: bool,
}

pub struct Para {
    pub jc: Option<String>,
    pub runs: Vec<Run>,
}

impl Para {
    pub fn text(&self) -> String {
        self.runs.iter().map(|r| r.text.as_str()).collect()
    }
}

/// Collects paragraphs (text, runs, italics, alignment) in document order.
pub fn parse_paragraphs(xml: &str) -> Result<Vec<Para>> {
    let mut reader = Reader::from_str(xml);
    let mut paras = Vec::new();
    let (mut cur, mut run): (Option<Para>, Option<Run>) = (None, None);
    let (mut in_ppr, mut in_rpr, mut in_t) = (false, false, false);
    loop {
        let ev = reader.read_event().context("malformed document.xml")?;
        let (e, is_start) = match &ev {
            Event::Eof => break,
            Event::Start(e) => (e, true),
            Event::Empty(e) => (e, false),
            // WHY: quick-xml delivers entities like &amp; as separate GeneralRef events, not inside Text.
            Event::Text(t) => {
                if let (true, Some(r)) = (in_t, run.as_mut()) {
                    r.text
                        .push_str(&t.xml_content(quick_xml::XmlVersion::Implicit1_0));
                }
                continue;
            }
            Event::GeneralRef(g) => {
                if let (true, Some(r)) = (in_t, run.as_mut()) {
                    r.text.push(match g.clone().into_inner().as_ref() {
                        "amp" => '&',
                        "lt" => '<',
                        "gt" => '>',
                        "quot" => '"',
                        "apos" => '\'',
                        _ => '?',
                    });
                }
                continue;
            }
            Event::End(e) => {
                match e.local_name().as_ref() {
                    "p" => paras.extend(cur.take()),
                    "r" => {
                        if let (Some(r), Some(p)) = (run.take(), cur.as_mut()) {
                            p.runs.push(r);
                        }
                    }
                    "pPr" => in_ppr = false,
                    "rPr" => in_rpr = false,
                    "t" => in_t = false,
                    _ => {}
                }
                continue;
            }
            _ => continue,
        };
        match e.local_name().as_ref() {
            "p" => {
                let p = Para {
                    jc: None,
                    runs: Vec::new(),
                };
                if is_start {
                    cur = Some(p);
                } else {
                    // Keep numbering identical to the streaming linter, which counts <w:p/> too.
                    paras.push(p);
                }
            }
            "r" if is_start => {
                run = Some(Run {
                    text: String::new(),
                    italic: false,
                })
            }
            "pPr" if is_start => in_ppr = true,
            "rPr" if is_start => in_rpr = true,
            "t" if is_start => in_t = true,
            "jc" if in_ppr => {
                if let Some(p) = cur.as_mut() {
                    p.jc = attr(e, "val");
                }
            }
            "i" if in_rpr => {
                if let Some(r) = run.as_mut() {
                    r.italic = !matches!(attr(e, "val").as_deref(), Some("0" | "false"));
                }
            }
            _ => {}
        }
    }
    Ok(paras)
}

fn to_roman(mut n: u32) -> String {
    const T: [(u32, &str); 13] = [
        (1000, "M"),
        (900, "CM"),
        (500, "D"),
        (400, "CD"),
        (100, "C"),
        (90, "XC"),
        (50, "L"),
        (40, "XL"),
        (10, "X"),
        (9, "IX"),
        (5, "V"),
        (4, "IV"),
        (1, "I"),
    ];
    let mut s = String::new();
    for (val, sym) in T {
        while n >= val {
            s.push_str(sym);
            n -= val;
        }
    }
    s
}

/// Parses a roman numeral (any case); returns None for non-canonical forms like "IIX".
fn from_roman(s: &str) -> Option<u32> {
    let up = s.to_uppercase();
    let val = |c| match c {
        'I' => Some(1),
        'V' => Some(5),
        'X' => Some(10),
        'L' => Some(50),
        'C' => Some(100),
        'D' => Some(500),
        'M' => Some(1000),
        _ => None,
    };
    let digits: Vec<u32> = up.chars().map(val).collect::<Option<_>>()?;
    let mut total = 0;
    for (i, d) in digits.iter().enumerate() {
        match digits.get(i + 1) {
            Some(next) if next > d => total -= *d as i64,
            _ => total += *d as i64,
        }
    }
    let total = u32::try_from(total).ok().filter(|t| *t > 0)?;
    // WHY: round-tripping rejects malformed numerals that the subtraction rule alone would accept.
    (to_roman(total) == up).then_some(total)
}

/// If `text` starts with `prefix` + whitespace, returns the text after that whitespace.
fn after_prefix<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    if prefix.is_empty() {
        return None;
    }
    let rest = text.strip_prefix(prefix)?;
    let trimmed = rest.trim_start();
    (trimmed.len() < rest.len()).then_some(trimmed)
}

/// Parses `<digits><sep><digits>` at the start of `s`.
fn parse_caption_number(s: &str) -> Option<(u32, char, u32)> {
    let a_end = s.find(|c: char| !c.is_ascii_digit())?;
    let sep = s[a_end..].chars().next().filter(|c| ".-–".contains(*c))?;
    let rest = &s[a_end + sep.len_utf8()..];
    let b_end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    if a_end == 0 || b_end == 0 {
        return None;
    }
    Some((s[..a_end].parse().ok()?, sep, rest[..b_end].parse().ok()?))
}

fn is_chapter_token(tok: &str) -> bool {
    !tok.is_empty()
        && (tok.chars().all(|c| c.is_ascii_digit())
            || tok.chars().all(|c| "IVXLCDMivxlcdm".contains(c)))
}

/// Finds `(Author, 2020)` style citations; returns (author, year) pairs.
fn citations(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('(') {
        let Some(close) = rest[open..].find(')') else {
            break;
        };
        for part in rest[open + 1..open + close].split(';') {
            out.extend(parse_citation(part.trim()));
        }
        rest = &rest[open + close + 1..];
    }
    out
}

fn parse_citation(part: &str) -> Option<(String, String)> {
    // Year is 4 digits with an optional trailing a-z suffix.
    let (head, year) = [0usize, 1]
        .iter()
        .filter_map(|suffix| {
            let end = part.len().checked_sub(*suffix)?;
            let start = end.checked_sub(4)?;
            let y = part.get(start..end)?;
            let ok = y.chars().all(|c| c.is_ascii_digit())
                && (y.starts_with('1') || y.starts_with('2'))
                && (*suffix == 0 || part[end..].chars().all(|c| c.is_ascii_lowercase()));
            ok.then(|| (&part[..start], &part[start..]))
        })
        .next()?;
    let head = head.trim_end().strip_suffix(',')?;
    let author = head
        .split_whitespace()
        .next()?
        .trim_end_matches(['.', ','])
        .to_string();
    let first = author.chars().next()?;
    let valid = first.is_uppercase()
        && author
            .chars()
            .all(|c| c.is_alphabetic() || c == '-' || c == '\'')
        && head.split_whitespace().count() <= 8;
    valid.then(|| (author, year.to_string()))
}

fn contains_word(hay: &str, word: &str) -> bool {
    hay.match_indices(word).any(|(i, _)| {
        let before = hay[..i].chars().next_back();
        let after = hay[i + word.len()..].chars().next();
        !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(char::is_alphanumeric)
    })
}

pub fn check(xml: &str, cfg: &ProfileConfig, out: &mut Vec<Violation>) -> Result<()> {
    let paras = parse_paragraphs(xml)?;
    let s = &cfg.structure;
    let bib = paras
        .iter()
        .position(|p| p.text().trim().to_lowercase() == s.bibliography_heading.to_lowercase());
    let entries: Vec<String> = bib
        .map(|i| {
            paras[i + 1..]
                .iter()
                .map(|p| p.text().to_lowercase())
                .collect()
        })
        .unwrap_or_default();

    let mut chapter: Option<u32> = None;
    let mut seq = [0u32; 2];
    let captions = [&s.figure_prefix, &s.table_prefix];

    for (idx, p) in paras.iter().enumerate() {
        let n = idx + 1;
        let text = p.text();
        let trimmed = text.trim_start();
        let in_body = bib.is_none_or(|b| idx < b);

        // HEADING_ERROR: chapter number style and alignment.
        if let Some(rest) = after_prefix(trimmed, &s.chapter_prefix) {
            let tok = rest.split_whitespace().next().unwrap_or("");
            if is_chapter_token(tok) {
                let arabic = s.chapter_numbering == "arabic";
                let valid = if arabic {
                    tok.chars().all(|c| c.is_ascii_digit())
                } else {
                    from_roman(tok).is_some_and(|_| tok.chars().all(|c| c.is_uppercase()))
                };
                if !valid {
                    let want = if arabic {
                        "arabic (BAB 1)"
                    } else {
                        "roman (BAB I)"
                    };
                    out.push(v(
                        "HEADING_ERROR",
                        format!(
                            "chapter '{} {tok}' should use {want} numbering",
                            s.chapter_prefix
                        ),
                        n,
                    ));
                }
                let align = match p.jc.as_deref() {
                    Some("start") | Some("left") | None => "left",
                    Some(other) => other,
                };
                if align != s.heading_alignment {
                    out.push(v(
                        "HEADING_ERROR",
                        format!(
                            "chapter heading is {align}-aligned, expected {}",
                            s.heading_alignment
                        ),
                        n,
                    ));
                }
                chapter = tok.parse().ok().or_else(|| from_roman(tok));
                seq = [0; 2];
            }
        }

        // CAPTION_SEQ_ERROR: separator, chapter number and running sequence per chapter.
        for (kind, prefix) in captions.iter().enumerate() {
            let Some((c, sep, k)) = after_prefix(trimmed, prefix).and_then(parse_caption_number)
            else {
                continue;
            };
            if sep.to_string() != s.caption_separator {
                out.push(v(
                    "CAPTION_SEQ_ERROR",
                    format!(
                        "caption '{prefix} {c}{sep}{k}' should use separator '{}'",
                        s.caption_separator
                    ),
                    n,
                ));
            }
            if let Some(ch) = chapter.filter(|ch| *ch != c) {
                out.push(v(
                    "CAPTION_SEQ_ERROR",
                    format!("caption '{prefix} {c}{sep}{k}' is in chapter {ch}"),
                    n,
                ));
            } else if k != seq[kind] + 1 {
                out.push(v(
                    "CAPTION_SEQ_ERROR",
                    format!(
                        "caption '{prefix} {c}{sep}{k}' should be number {}",
                        seq[kind] + 1
                    ),
                    n,
                ));
            }
            seq[kind] = k;
        }

        // DRAFT_TOKEN_ERROR
        let lower = text.to_lowercase();
        for tok in &cfg.academic.forbidden_tokens {
            if !tok.is_empty() && lower.contains(&tok.to_lowercase()) {
                out.push(v(
                    "DRAFT_TOKEN_ERROR",
                    format!("draft marker '{tok}' found"),
                    n,
                ));
            }
        }

        // DOUBLE_SPACE_ERROR
        if text.contains("  ") {
            out.push(v("DOUBLE_SPACE_ERROR", "consecutive spaces".into(), n));
        }

        // FOREIGN_TERM_NOT_ITALIC
        for term in &cfg.academic.foreign_terms_must_italicize {
            let t = term.to_lowercase();
            if !t.is_empty()
                && p.runs
                    .iter()
                    .any(|r| !r.italic && contains_word(&r.text.to_lowercase(), &t))
            {
                out.push(v(
                    "FOREIGN_TERM_NOT_ITALIC",
                    format!("foreign term '{term}' should be italic"),
                    n,
                ));
            }
        }

        // CITATION_ORPHAN
        if in_body {
            for (author, year) in citations(&text) {
                let (a, y) = (author.to_lowercase(), year.to_lowercase());
                if !entries.iter().any(|e| e.contains(&a) && e.contains(&y)) {
                    out.push(v(
                        "CITATION_ORPHAN",
                        format!(
                            "citation '({author}, {year})' has no entry under {}",
                            s.bibliography_heading
                        ),
                        n,
                    ));
                }
            }
        }
    }
    Ok(())
}
