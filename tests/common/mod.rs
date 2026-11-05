#![allow(dead_code)]
use std::io::Write;
use std::path::Path;

const W_NS: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

/// Wraps body XML in a `w:document` root.
pub fn document_xml(body: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<w:document xmlns:w=\"{W_NS}\" xmlns:v=\"urn:schemas-microsoft-com:vml\" \nxmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" \nxmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" \nxmlns:mc=\"http://schemas.openxmlformats.org/markup-compatibility/2006\" \nxmlns:wps=\"http://schemas.microsoft.com/office/word/2010/wordprocessingShape\"><w:body>{body}</w:body></w:document>"
    )
}

/// Section properties with margins given in twips.
pub fn sect_pr(top: u32, left: u32, bottom: u32, right: u32) -> String {
    format!(
        "<w:sectPr><w:pgMar w:top=\"{top}\" w:right=\"{right}\" w:bottom=\"{bottom}\" \
w:left=\"{left}\" w:header=\"708\" w:footer=\"708\" w:gutter=\"0\"/></w:sectPr>"
    )
}

/// Margins that match the default profile (4/4/3/3 cm).
pub fn good_sect_pr() -> String {
    sect_pr(2268, 2268, 1701, 1701)
}

/// Packs a `word/document.xml` string into a minimal valid `.docx` archive.
pub fn write_docx(path: &Path, document: &str) {
    let file = std::fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default();
    let parts: [(&str, &str); 3] = [
        (
            "[Content_Types].xml",
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\"><Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/><Default Extension=\"xml\" ContentType=\"application/xml\"/><Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/></Types>",
        ),
        (
            "_rels/.rels",
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?><Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\"><Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"word/document.xml\"/></Relationships>",
        ),
        ("word/document.xml", document),
    ];
    for (name, content) in parts {
        zip.start_file(name, opts).unwrap();
        zip.write_all(content.as_bytes()).unwrap();
    }
    zip.finish().unwrap();
}

/// Body paragraph that satisfies the default profile's font/size/spacing rules.
pub fn good_para(text: &str) -> String {
    format!(
        "<w:p><w:pPr><w:spacing w:line=\"360\" w:lineRule=\"auto\"/></w:pPr>\
<w:r><w:rPr><w:rFonts w:ascii=\"Times New Roman\" w:hAnsi=\"Times New Roman\"/><w:sz w:val=\"24\"/></w:rPr>\
<w:t xml:space=\"preserve\">{text}</w:t></w:r></w:p>"
    )
}

/// Chapter-style heading (heading style, so the body size rule is skipped).
pub fn heading_para(text: &str, jc: &str) -> String {
    format!(
        "<w:p><w:pPr><w:pStyle w:val=\"Heading1\"/><w:jc w:val=\"{jc}\"/></w:pPr>\
<w:r><w:rPr><w:rFonts w:ascii=\"Times New Roman\" w:hAnsi=\"Times New Roman\"/></w:rPr><w:t>{text}</w:t></w:r></w:p>"
    )
}

/// Paragraph made of runs with an explicit italic flag, otherwise default-profile formatting.
pub fn run_para(runs: &[(&str, bool)]) -> String {
    let rs: String = runs
        .iter()
        .map(|(t, it)| {
            format!(
                "<w:r><w:rPr><w:rFonts w:ascii=\"Times New Roman\" w:hAnsi=\"Times New Roman\"/>{}<w:sz w:val=\"24\"/></w:rPr>\
<w:t xml:space=\"preserve\">{t}</w:t></w:r>",
                if *it { "<w:i/>" } else { "" }
            )
        })
        .collect();
    format!("<w:p><w:pPr><w:spacing w:line=\"360\" w:lineRule=\"auto\"/></w:pPr>{rs}</w:p>")
}

/// Writes `body` to `<dir>/t.docx` and runs `docx-lint check --json` on it.
pub fn lint_body(dir: &Path, body: &str, extra: &[&str]) -> (bool, serde_json::Value) {
    let docx = dir.join("t.docx");
    write_docx(&docx, &document_xml(body));
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_docx-lint"))
        .current_dir(dir)
        .arg("check")
        .arg(&docx)
        .arg("--json")
        .args(extra)
        .output()
        .unwrap();
    let json = serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&out.stdout)));
    (out.status.success(), json)
}

pub fn viol_codes(v: &serde_json::Value) -> Vec<String> {
    v["violations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x["code"].as_str().unwrap().to_string())
        .collect()
}

/// Runs `docx-lint config ...` in `dir`.
pub fn config(dir: &Path, args: &[&str]) {
    let st = std::process::Command::new(env!("CARGO_BIN_EXE_docx-lint"))
        .current_dir(dir)
        .arg("config")
        .args(args)
        .status()
        .unwrap();
    assert!(st.success());
}

/// Packs `body` into `<dir>/in.docx`, runs `check --fix out.docx --json`, and returns the JSON
/// result with the fixed `word/document.xml`.
pub fn fix_body(dir: &Path, body: &str) -> (serde_json::Value, String) {
    use std::io::Read;
    write_docx(&dir.join("in.docx"), &document_xml(body));
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_docx-lint"))
        .current_dir(dir)
        .args(["check", "in.docx", "--fix", "out.docx", "--json"])
        .output()
        .unwrap();
    let json = serde_json::from_slice(&out.stdout)
        .unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&out.stderr)));
    let mut xml = String::new();
    zip::ZipArchive::new(std::fs::File::open(dir.join("out.docx")).unwrap())
        .unwrap()
        .by_name("word/document.xml")
        .unwrap()
        .read_to_string(&mut xml)
        .unwrap();
    (json, xml)
}

/// `(code, paragraph)` pairs of a `check --json` result.
pub fn viol_pairs(v: &serde_json::Value, key: &str) -> Vec<(String, u64)> {
    v[key]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| {
            (
                x["code"].as_str().unwrap().to_string(),
                x["paragraph"].as_u64().unwrap(),
            )
        })
        .collect()
}
