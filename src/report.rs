//! Self-contained HTML report for a lint run.
use crate::config::ProfileConfig;
use crate::linter::Violation;
use std::collections::BTreeMap;

fn esc(s: &str) -> String {
    // WHY: messages contain document text (font names etc.), so escape before embedding in HTML.
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

pub fn render_html(file: &str, cfg: &ProfileConfig, violations: &[Violation]) -> String {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for x in violations {
        *counts.entry(x.code.as_str()).or_default() += 1;
    }
    let mut h = String::from(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>docx-lint report</title>\
<style>body{font-family:sans-serif;margin:2rem}table{border-collapse:collapse}\
td,th{border:1px solid #ccc;padding:4px 10px;text-align:left}th{background:#f3f3f3}\
.ok{color:#17803d}.bad{color:#b42318}</style></head><body>",
    );
    h.push_str(&format!(
        "<h1>docx-lint report</h1><p>File: <b>{}</b><br>Profile: <b>{}</b></p>",
        esc(file),
        esc(&cfg.profile_name)
    ));
    if violations.is_empty() {
        h.push_str("<p class=\"ok\">No violations found.</p>");
    } else {
        h.push_str(&format!(
            "<p class=\"bad\">{} violation(s)</p><ul>",
            violations.len()
        ));
        for (code, n) in &counts {
            h.push_str(&format!("<li>{}: {n}</li>", esc(code)));
        }
        h.push_str("</ul><table><tr><th>Code</th><th>Paragraph</th><th>Message</th></tr>");
        for x in violations {
            let para = if x.paragraph == 0 {
                "-".to_string()
            } else {
                x.paragraph.to_string()
            };
            h.push_str(&format!(
                "<tr><td>{}</td><td>{para}</td><td>{}</td></tr>",
                esc(&x.code),
                esc(&x.message)
            ));
        }
        h.push_str("</table>");
    }
    h.push_str("</body></html>");
    h
}
