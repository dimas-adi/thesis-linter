#![allow(dead_code)]
use std::io::Write;
use std::path::Path;

const W_NS: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";

/// Wraps body XML in a `w:document` root.
pub fn document_xml(body: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\
<w:document xmlns:w=\"{W_NS}\"><w:body>{body}</w:body></w:document>"
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
