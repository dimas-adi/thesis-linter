//! Document model and paragraph-level checks: headings, captions, citations, draft tokens, spacing, italics.
use crate::config::ProfileConfig;
use crate::linter::{Violation, attr, v};
use anyhow::{Context, Result, bail};
use quick_xml::NsReader;
use quick_xml::events::{BytesRef, Event};
use quick_xml::name::ResolveResult;

pub(crate) const TWIPS_PER_CM: f64 = 567.0;

const WORD_NS: [&str; 2] = [
    "http://schemas.openxmlformats.org/wordprocessingml/2006/main",
    "http://purl.oclc.org/ooxml/wordprocessingml/main",
];
const MATH_NS: [&str; 2] = [
    "http://schemas.openxmlformats.org/officeDocument/2006/math",
    "http://purl.oclc.org/ooxml/officeDocument/math",
];

/// Which markup language an element belongs to, by resolved namespace (never by prefix).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ns {
    Word,
    Math,
    Other,
}

impl Ns {
    pub(crate) fn of(res: &ResolveResult) -> Ns {
        match res {
            ResolveResult::Bound(ns) if WORD_NS.contains(&ns.into_inner()) => Ns::Word,
            ResolveResult::Bound(ns) if MATH_NS.contains(&ns.into_inner()) => Ns::Math,
            _ => Ns::Other,
        }
    }
}

/// True for a `pStyle` value that is a heading or title style.
pub(crate) fn is_heading_style(style: &str) -> bool {
    let s = style.to_lowercase();
    s.starts_with("heading") || s == "title"
}

/// True when a margin of `twips` is farther than `tolerance_cm` from `expected_cm`.
pub(crate) fn margin_off(twips: f64, expected_cm: f64, tolerance_cm: f64) -> bool {
    (twips / TWIPS_PER_CM - expected_cm).abs() > tolerance_cm
}

/// Escapes the characters that are special in XML text.
pub(crate) fn escape_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Character for a general entity or character reference; unknown or invalid ones are errors.
pub(crate) fn resolve_ref(g: &BytesRef) -> Result<char> {
    if let Some(c) = g.resolve_char_ref().context("malformed document.xml")? {
        return Ok(c);
    }
    match &**g {
        "amp" => Ok('&'),
        "lt" => Ok('<'),
        "gt" => Ok('>'),
        "quot" => Ok('"'),
        "apos" => Ok('\''),
        other => bail!("malformed document.xml: unknown entity '&{other};'"),
    }
}

pub struct Run {
    pub text: String,
    pub italic: bool,
}

pub struct Para {
    pub jc: Option<String>,
    pub runs: Vec<Run>,
    pub in_table: bool,
    /// Heading1 paragraph whose "BAB n" label comes from automatic list numbering.
    pub auto_chapter: bool,
    /// Heading or Title paragraph style.
    pub heading: bool,
    /// Has a paragraph style, which may supply alignment from `styles.xml`.
    pub styled: bool,
}

impl Para {
    pub fn text(&self) -> String {
        self.runs.iter().map(|r| r.text.as_str()).collect()
    }
}

/// One layout attribute read from the XML, with the paragraph it belongs to (0 = none).
pub enum Layout {
    Margin {
        side: &'static str,
        twips: f64,
    },
    Fonts {
        ascii: Option<String>,
        h_ansi: Option<String>,
    },
    Size {
        half_pt: f64,
    },
    /// `w:spacing` line value, only for `lineRule` auto or absent.
    Spacing {
        line: f64,
    },
}

pub struct Fact {
    pub para: usize,
    pub layout: Layout,
}

/// Everything the checks need from `word/document.xml`, in document order.
pub struct Document {
    /// Paragraphs in start-tag order, so `index + 1` is the reported paragraph number.
    pub paras: Vec<Para>,
    pub facts: Vec<Fact>,
}

fn new_para(in_table: bool) -> Para {
    Para {
        jc: None,
        runs: Vec::new(),
        in_table,
        auto_chapter: false,
        heading: false,
        styled: false,
    }
}

/// An open `w:p` (index into the paragraph list) or `w:r` on the parse stack.
enum Frame {
    Para(usize),
    Run(Run),
}

#[derive(Default)]
struct Parse {
    paras: Vec<Para>,
    facts: Vec<Fact>,
    stack: Vec<Frame>,
    in_ppr: bool,
    in_rpr: bool,
    in_t: bool,
    tbl_depth: usize,
    math_depth: usize,
}

impl Parse {
    /// Paragraph that owns the current position: the innermost open one, else the latest started.
    fn cur_para(&mut self) -> Option<&mut Para> {
        let idx = self.cur_index()?;
        self.paras.get_mut(idx)
    }

    fn cur_index(&self) -> Option<usize> {
        self.stack
            .iter()
            .rev()
            .find_map(|f| match f {
                Frame::Para(i) => Some(*i),
                Frame::Run(_) => None,
            })
            .or(self.paras.len().checked_sub(1))
    }

    fn fact(&mut self, layout: Layout) {
        let para = self.cur_index().map_or(0, |i| i + 1);
        self.facts.push(Fact { para, layout });
    }

    fn push_text(&mut self, text: &str) {
        if let (true, Some(Frame::Run(r))) = (self.in_t, self.stack.last_mut()) {
            r.text.push_str(text);
        }
    }

    fn start_or_empty(&mut self, e: &quick_xml::events::BytesStart, ns: Ns, is_start: bool) {
        let name = e.local_name();
        let local = name.as_ref();
        if ns == Ns::Math {
            if is_start && matches!(local, "oMath" | "oMathPara") {
                self.math_depth += 1;
            }
            return;
        }
        if ns != Ns::Word {
            return;
        }
        match local {
            "tbl" if is_start => self.tbl_depth += 1,
            "p" => {
                self.paras.push(new_para(self.tbl_depth > 0));
                if is_start {
                    self.stack.push(Frame::Para(self.paras.len() - 1));
                }
            }
            "r" if is_start => self.stack.push(Frame::Run(Run {
                text: String::new(),
                italic: false,
            })),
            "pPr" if is_start => self.in_ppr = true,
            "rPr" if is_start => self.in_rpr = true,
            "t" if is_start => self.in_t = true,
            "pStyle" if self.in_ppr => {
                let style = attr(e, "val").unwrap_or_default();
                if let Some(p) = self.cur_para() {
                    p.auto_chapter = style.eq_ignore_ascii_case("heading1");
                    p.heading = is_heading_style(&style);
                    p.styled = true;
                }
            }
            // WHY: numId 0 switches list numbering off ("Daftar Isi" etc.), so it is not a chapter.
            "numId" if self.in_ppr && attr(e, "val").as_deref() == Some("0") => {
                if let Some(p) = self.cur_para() {
                    p.auto_chapter = false;
                }
            }
            // WHY: tab/break become their own characters, so no space rule mistakes them for spaces,
            // and "BAB I<br/>PENDAHULUAN" still splits into words.
            "br" | "tab" | "cr" if !self.in_rpr && !self.in_ppr => {
                if let Some(Frame::Run(r)) = self.stack.last_mut() {
                    r.text.push(if local == "tab" { '\t' } else { '\n' });
                }
            }
            "jc" if self.in_ppr => {
                let jc = attr(e, "val");
                if let Some(p) = self.cur_para() {
                    p.jc = jc;
                }
            }
            "i" if self.in_rpr => {
                if let Some(Frame::Run(r)) = self.stack.last_mut() {
                    r.italic = !matches!(attr(e, "val").as_deref(), Some("0" | "false"));
                }
            }
            "pgMar" => {
                for side in ["top", "left", "bottom", "right"] {
                    if let Some(twips) = attr_f64(e, side) {
                        self.facts.push(Fact {
                            para: 0,
                            layout: Layout::Margin { side, twips },
                        });
                    }
                }
            }
            // WHY: equation runs keep Word's math fonts and sizes, so they are not layout facts.
            "rFonts" if self.in_rpr && self.math_depth == 0 => {
                let layout = Layout::Fonts {
                    ascii: attr(e, "ascii"),
                    h_ansi: attr(e, "hAnsi"),
                };
                self.fact(layout);
            }
            "sz" if self.in_rpr && self.math_depth == 0 => {
                if let Some(half_pt) = attr_f64(e, "val") {
                    self.fact(Layout::Size { half_pt });
                }
            }
            "spacing" if self.in_ppr => {
                let auto = attr(e, "lineRule").is_none_or(|r| r == "auto");
                if let (Some(line), true) = (attr_f64(e, "line"), auto) {
                    self.fact(Layout::Spacing { line });
                }
            }
            _ => {}
        }
    }

    fn end(&mut self, local: &str, ns: Ns) {
        if ns == Ns::Math {
            if matches!(local, "oMath" | "oMathPara") {
                self.math_depth = self.math_depth.saturating_sub(1);
            }
            return;
        }
        if ns != Ns::Word {
            return;
        }
        match local {
            "p" => {
                if matches!(self.stack.last(), Some(Frame::Para(_))) {
                    self.stack.pop();
                }
            }
            "r" => {
                if matches!(self.stack.last(), Some(Frame::Run(_))) {
                    let Some(Frame::Run(run)) = self.stack.pop() else {
                        return;
                    };
                    let owner = self.stack.iter().rev().find_map(|f| match f {
                        Frame::Para(i) => Some(*i),
                        Frame::Run(_) => None,
                    });
                    if let Some(p) = owner.and_then(|i| self.paras.get_mut(i)) {
                        p.runs.push(run);
                    }
                }
            }
            "pPr" => self.in_ppr = false,
            "rPr" => self.in_rpr = false,
            "t" => self.in_t = false,
            "tbl" => self.tbl_depth = self.tbl_depth.saturating_sub(1),
            _ => {}
        }
    }
}

fn attr_f64(e: &quick_xml::events::BytesStart, name: &str) -> Option<f64> {
    attr(e, name)?.parse().ok()
}

/// Parses `word/document.xml` once into paragraphs and layout facts; malformed XML is an error.
pub fn parse_document(xml: &str) -> Result<Document> {
    let mut reader = NsReader::from_str(xml);
    let mut st = Parse::default();
    let (mut depth, mut roots) = (0usize, 0usize);
    loop {
        let (res, ev) = reader
            .read_resolved_event()
            .context("malformed document.xml")?;
        let ns = Ns::of(&res);
        match &ev {
            Event::Eof => break,
            Event::Start(e) | Event::Empty(e) => {
                let is_start = matches!(ev, Event::Start(_));
                if depth == 0 {
                    roots += 1;
                    if roots > 1 {
                        bail!("malformed document.xml: more than one root element");
                    }
                }
                if is_start {
                    depth += 1;
                }
                st.start_or_empty(e, ns, is_start);
            }
            Event::End(e) => {
                depth = depth.saturating_sub(1);
                st.end(e.local_name().as_ref(), ns);
            }
            // WHY: quick-xml delivers entities like &amp; as separate GeneralRef events, not inside Text.
            Event::Text(t) => st.push_text(&t.xml_content(quick_xml::XmlVersion::Implicit1_0)),
            Event::GeneralRef(g) => {
                let c = resolve_ref(g)?;
                st.push_text(c.encode_utf8(&mut [0; 4]));
            }
            _ => {}
        }
    }
    if roots == 0 {
        bail!("malformed document.xml: no root element");
    }
    if depth != 0 {
        bail!("malformed document.xml: unclosed elements");
    }
    Ok(Document {
        paras: st.paras,
        facts: st.facts,
    })
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
fn parse_caption_number(s: &str) -> Option<(u32, char, u32, &str)> {
    let a_end = s.find(|c: char| !c.is_ascii_digit())?;
    let sep = s[a_end..].chars().next().filter(|c| ".-–".contains(*c))?;
    let rest = &s[a_end + sep.len_utf8()..];
    let b_end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    if a_end == 0 || b_end == 0 {
        return None;
    }
    Some((
        s[..a_end].parse().ok()?,
        sep,
        rest[..b_end].parse().ok()?,
        &rest[b_end..],
    ))
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

/// Byte ranges of case-insensitive, whole-word (`\b`-style) occurrences of `word` in `hay`.
pub(crate) fn find_words(hay: &str, word: &str) -> Vec<(usize, usize)> {
    // WHY: lowercase char-by-char (1:1) so matched chars map straight to byte offsets of `hay`.
    let low = |c: char| c.to_lowercase().next().unwrap_or(c);
    let w: Vec<char> = word.chars().map(low).collect();
    let mut out = Vec::new();
    if w.is_empty() {
        return out;
    }
    let (mut prev, mut next_free) = (None::<char>, 0usize);
    for (start, c) in hay.char_indices() {
        if start >= next_free && prev.is_none_or(|p| !p.is_alphanumeric()) {
            let mut rest = hay[start..].chars();
            let mut end = start;
            let hit = w.iter().all(|x| {
                rest.next().is_some_and(|ch| {
                    end += ch.len_utf8();
                    low(ch) == *x
                })
            });
            if hit && rest.next().is_none_or(|n| !n.is_alphanumeric()) {
                out.push((start, end));
                next_free = end;
            }
        }
        prev = Some(c);
    }
    out
}

/// Chapter number token if `text` is a real chapter heading (not a table-of-contents line).
fn chapter_token<'a>(text: &'a str, cfg: &ProfileConfig) -> Option<&'a str> {
    let rest = after_prefix(text.trim_start(), &cfg.structure.chapter_prefix)?;
    let tok = rest
        .split_whitespace()
        .next()
        .filter(|t| is_chapter_token(t))?;
    let last = rest.split_whitespace().last().unwrap_or("");
    // WHY: "BAB I PENDAHULUAN ....... 1" in DAFTAR ISI must not count as the chapter start.
    let toc = rest.contains("...")
        || (rest.split_whitespace().count() > 1 && last.chars().all(|c| c.is_ascii_digit()));
    // WHY: "BAB II membahas landasan teori." is a sentence, not a heading; titles do not start in lowercase.
    let starts_lower = rest[tok.len()..]
        .trim_start()
        .chars()
        .next()
        .is_some_and(char::is_lowercase);
    (!toc && !starts_lower).then_some(tok)
}

/// Index of the first real chapter heading, if the document has one.
/// True when some paragraph carries a typed chapter number such as "BAB I".
fn has_typed_chapters(paras: &[Para], cfg: &ProfileConfig) -> bool {
    paras
        .iter()
        .any(|p| chapter_token(&p.text(), cfg).is_some())
}

/// True when `p` starts a chapter: a typed number, or (only if no chapter is typed) Heading1 auto-numbering.
fn starts_chapter(p: &Para, typed: bool, cfg: &ProfileConfig) -> bool {
    chapter_token(&p.text(), cfg).is_some() || (p.auto_chapter && !typed)
}

fn first_chapter(paras: &[Para], cfg: &ProfileConfig) -> Option<usize> {
    let typed = has_typed_chapters(paras, cfg);
    paras.iter().position(|p| starts_chapter(p, typed, cfg))
}

/// Per-paragraph context used to relax font/spacing rules.
#[derive(Clone, Copy, Default)]
pub struct Ctx {
    pub front: bool,
    pub table: bool,
}

impl Ctx {
    pub fn size_ok(self, pt: f64, cfg: &ProfileConfig) -> bool {
        let mut allowed = vec![cfg.typography.body_size_pt];
        if self.front {
            allowed.extend([12.0, 14.0, 16.0]);
        }
        if self.table {
            allowed.extend([10.0, 11.0]);
        }
        allowed.iter().any(|a| (pt - a).abs() <= 0.01)
    }

    pub fn spacing_ok(self, ratio: f64, cfg: &ProfileConfig) -> bool {
        (ratio - cfg.typography.line_spacing).abs() <= 0.01
            || (self.table && (ratio - 1.0).abs() <= 0.01)
    }
}

/// One `Ctx` per paragraph, in paragraph-number order.
pub fn contexts(paras: &[Para], cfg: &ProfileConfig) -> Vec<Ctx> {
    let first = first_chapter(paras, cfg);
    paras
        .iter()
        .enumerate()
        .map(|(i, p)| Ctx {
            front: first.is_some_and(|f| i < f),
            table: p.in_table,
        })
        .collect()
}

/// Chapter and caption counters carried from one paragraph to the next.
#[derive(Default)]
struct Counters {
    chapter: Option<u32>,
    /// Last caption number seen per kind: figures, then tables.
    seq: [u32; 2],
    auto_chapters: u32,
}

/// Document-wide facts the per-paragraph checks need.
struct Scan<'a> {
    cfg: &'a ProfileConfig,
    first_chapter: Option<usize>,
    typed_chapters: bool,
    bibliography: Option<usize>,
    entries: Vec<String>,
}

impl<'a> Scan<'a> {
    fn new(paras: &[Para], cfg: &'a ProfileConfig) -> Self {
        let wanted = cfg.structure.bibliography_heading.to_lowercase();
        let bibliography = paras
            .iter()
            .position(|p| p.text().trim().to_lowercase() == wanted);
        let entries = bibliography
            .map(|i| {
                paras[i + 1..]
                    .iter()
                    .map(|p| p.text().to_lowercase())
                    .collect()
            })
            .unwrap_or_default();
        Scan {
            cfg,
            first_chapter: first_chapter(paras, cfg),
            typed_chapters: has_typed_chapters(paras, cfg),
            bibliography,
            entries,
        }
    }

    /// HEADING_ERROR: chapter number style and alignment; also advances the chapter counters.
    fn heading(&self, p: &Para, n: usize, c: &mut Counters, out: &mut Vec<Violation>) {
        let s = &self.cfg.structure;
        let text = p.text();
        let trimmed = text.trim_start();
        if let Some(tok) = chapter_token(trimmed, self.cfg) {
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
                let msg = format!(
                    "chapter '{} {tok}' should use {want} numbering",
                    s.chapter_prefix
                );
                out.push(v("HEADING_ERROR", msg, n));
            }
            // WHY: a styled heading without `jc` takes its alignment from styles.xml, which is out of scope.
            let align = match p.jc.as_deref() {
                Some("start") | Some("left") => Some("left"),
                None if p.styled => None,
                None => Some("left"),
                Some(other) => Some(other),
            };
            if let Some(align) = align.filter(|a| *a != s.heading_alignment) {
                let msg = format!(
                    "chapter heading is {align}-aligned, expected {}",
                    s.heading_alignment
                );
                out.push(v("HEADING_ERROR", msg, n));
            }
            c.chapter = tok.parse().ok().or_else(|| from_roman(tok));
            c.seq = [0; 2];
        } else if p.auto_chapter && !self.typed_chapters {
            c.auto_chapters += 1;
            c.chapter = Some(c.auto_chapters);
            c.seq = [0; 2];
        }
    }

    /// CAPTION_SEQ_ERROR: separator, chapter number and running sequence per chapter.
    fn captions(&self, idx: usize, text: &str, c: &mut Counters, out: &mut Vec<Violation>) {
        // WHY: DAFTAR GAMBAR / DAFTAR TABEL entries before BAB I are not real captions.
        if self.first_chapter.is_some_and(|f| idx < f) {
            return;
        }
        let s = &self.cfg.structure;
        let n = idx + 1;
        let trimmed = text.trim_start();
        for (kind, prefix) in [&s.figure_prefix, &s.table_prefix].into_iter().enumerate() {
            let Some((num, sep, k, after)) =
                after_prefix(trimmed, prefix).and_then(parse_caption_number)
            else {
                continue;
            };
            // WHY: "Gambar 1.1 menunjukkan ..." is a body sentence that cites a figure, not a caption.
            if after
                .trim_start()
                .chars()
                .next()
                .is_some_and(char::is_lowercase)
            {
                continue;
            }
            let label = format!("caption '{prefix} {num}{sep}{k}'");
            if sep.to_string() != s.caption_separator {
                let msg = format!("{label} should use separator '{}'", s.caption_separator);
                out.push(v("CAPTION_SEQ_ERROR", msg, n));
            }
            if let Some(ch) = c.chapter.filter(|ch| *ch != num) {
                out.push(v(
                    "CAPTION_SEQ_ERROR",
                    format!("{label} is in chapter {ch}"),
                    n,
                ));
            } else if k != c.seq[kind].saturating_add(1) {
                let msg = format!("{label} should be number {}", c.seq[kind].saturating_add(1));
                out.push(v("CAPTION_SEQ_ERROR", msg, n));
            }
            c.seq[kind] = k;
        }
    }

    /// CITATION_ORPHAN: `(Author, 2020)` citations in the body with no bibliography entry.
    fn citations(&self, idx: usize, text: &str, out: &mut Vec<Violation>) {
        if self.bibliography.is_some_and(|b| idx >= b) {
            return;
        }
        for (author, year) in citations(text) {
            let (a, y) = (author.to_lowercase(), year.to_lowercase());
            if !self
                .entries
                .iter()
                .any(|e| e.contains(&a) && e.contains(&y))
            {
                let msg = format!(
                    "citation '({author}, {year})' has no entry under {}",
                    self.cfg.structure.bibliography_heading
                );
                out.push(v("CITATION_ORPHAN", msg, idx + 1));
            }
        }
    }
}

/// DRAFT_TOKEN_ERROR, DOUBLE_SPACE_ERROR and FOREIGN_TERM_NOT_ITALIC for one paragraph.
fn text_checks(p: &Para, text: &str, n: usize, cfg: &ProfileConfig, out: &mut Vec<Violation>) {
    for tok in &cfg.academic.forbidden_tokens {
        if !find_words(text, tok).is_empty() {
            out.push(v(
                "DRAFT_TOKEN_ERROR",
                format!("draft marker '{tok}' found"),
                n,
            ));
        }
    }
    if text.contains("  ") {
        out.push(v("DOUBLE_SPACE_ERROR", "consecutive spaces".into(), n));
    }
    for term in &cfg.academic.foreign_terms_must_italicize {
        if p.runs
            .iter()
            .any(|r| !r.italic && !find_words(&r.text, term).is_empty())
        {
            let msg = format!("foreign term '{term}' should be italic");
            out.push(v("FOREIGN_TERM_NOT_ITALIC", msg, n));
        }
    }
}

pub fn check(paras: &[Para], cfg: &ProfileConfig, out: &mut Vec<Violation>) {
    let scan = Scan::new(paras, cfg);
    let mut counters = Counters::default();
    for (idx, p) in paras.iter().enumerate() {
        let text = p.text();
        scan.heading(p, idx + 1, &mut counters, out);
        scan.captions(idx, &text, &mut counters, out);
        text_checks(p, &text, idx + 1, cfg, out);
        scan.citations(idx, &text, out);
    }
}
