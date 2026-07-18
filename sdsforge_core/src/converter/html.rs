/// HTML generation for SDS data in JIS Z 7253 16-section layout.
use crate::error::SdsError;
use crate::language::Language;
use crate::schema::SdsRoot;

use super::generator::{lang_index, section_name, DOCUMENT_TITLE, SECTION_KEYS};

/// Renamed to [`render_html`]; kept as a thin compat wrapper during the deprecation window.
#[deprecated(note = "renamed to render_html — \"generate\" is now reserved for the CAS/composition SDS-authoring feature")]
pub fn generate_html(sds: &SdsRoot, lang: Language) -> Result<String, SdsError> {
    render_html(sds, lang)
}

/// Generate an HTML document from an [`SdsRoot`] in the given language.
///
/// The output is a self-contained UTF-8 HTML5 document with inline CSS
/// including `@media print` styles suitable for printing to PDF.
pub fn render_html(sds: &SdsRoot, lang: Language) -> Result<String, SdsError> {
    let json = serde_json::to_value(sds)
        .map_err(|e| SdsError::Extract(format!("SDS serialize error: {e}")))?;

    let lang_idx = lang_index(lang);
    let title = DOCUMENT_TITLE[lang_idx];

    let product_name = json
        .pointer("/Identification/TradeProductIdentity/TradeNameJP")
        .or_else(|| json.pointer("/Identification/TradeProductIdentity/TradeNameEN"))
        .and_then(|v| v.as_str())
        .unwrap_or(title);

    let mut html = String::with_capacity(65_536);
    html.push_str(&format!(
        r#"<!DOCTYPE html>
<html lang="{lang_attr}">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>{page_title}</title>
<style>
  body {{
    font-family: "Noto Sans JP", "Hiragino Sans", "Meiryo", sans-serif;
    font-size: 10pt;
    color: #1a1a1a;
    max-width: 1000px;
    margin: 0 auto;
    padding: 1em 2em;
  }}
  h1 {{
    font-size: 16pt;
    border-bottom: 2px solid #2c5f8a;
    padding-bottom: 0.3em;
    color: #2c5f8a;
  }}
  h2 {{
    font-size: 12pt;
    background: #2c5f8a;
    color: white;
    padding: 0.2em 0.6em;
    margin-top: 1.2em;
    page-break-after: avoid;
  }}
  table {{
    border-collapse: collapse;
    width: 100%;
    margin: 0.4em 0;
    font-size: 9pt;
  }}
  th, td {{
    border: 1px solid #bbb;
    padding: 0.3em 0.5em;
    vertical-align: top;
  }}
  th {{
    background: #e8eef5;
    font-weight: bold;
    white-space: nowrap;
    width: 30%;
  }}
  .kv-key {{
    background: #f4f6f9;
    font-weight: bold;
    width: 35%;
  }}
  ul {{
    margin: 0.2em 0;
    padding-left: 1.5em;
  }}
  @media print {{
    body {{ font-size: 9pt; padding: 0; }}
    h2 {{ page-break-after: avoid; }}
    table, tr, td, th {{ page-break-inside: avoid; }}
  }}
</style>
</head>
<body>
<h1>{title} — {product_name}</h1>
"#,
        lang_attr = lang_attr(lang),
        page_title = html_escape(title),
        title = html_escape(title),
        product_name = html_escape(product_name),
    ));

    let obj = match &json {
        serde_json::Value::Object(o) => o,
        _ => return Err(SdsError::Extract("SDS JSON is not an object".into())),
    };

    for (i, key) in SECTION_KEYS.iter().enumerate() {
        let section_label = section_name(i, lang);
        html.push_str(&format!(
            "<h2>{}. {}</h2>\n",
            i + 1,
            html_escape(section_label)
        ));
        if let Some(val) = obj.get(*key) {
            html.push_str(&render_value_html(val));
        } else {
            html.push_str("<p><em>(not extracted)</em></p>\n");
        }
    }

    html.push_str("</body>\n</html>\n");
    Ok(html)
}

fn lang_attr(lang: Language) -> &'static str {
    match lang {
        Language::Japanese => "ja",
        Language::English => "en",
        Language::ChineseSimplified => "zh-Hans",
        Language::ChineseTraditional => "zh-Hant",
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn render_value_html(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::Null => String::new(),
        serde_json::Value::Bool(b) => format!("<span>{b}</span>\n"),
        serde_json::Value::Number(n) => format!("<span>{n}</span>\n"),
        serde_json::Value::String(s) if s.is_empty() => String::new(),
        serde_json::Value::String(s) => format!("<span>{}</span>\n", html_escape(s)),

        serde_json::Value::Array(arr) => {
            if arr.is_empty() {
                return String::new();
            }
            // If array contains objects, render as a multi-column table
            if arr.iter().all(|v| v.is_object()) {
                let keys: Vec<&str> = arr
                    .first()
                    .and_then(|v| v.as_object())
                    .map(|o| o.keys().map(|k| k.as_str()).collect())
                    .unwrap_or_default();
                if keys.is_empty() {
                    return String::new();
                }
                let mut out = String::from("<table>\n<thead><tr>");
                for k in &keys {
                    out.push_str(&format!("<th>{}</th>", html_escape(k)));
                }
                out.push_str("</tr></thead>\n<tbody>\n");
                for item in arr {
                    if let Some(obj) = item.as_object() {
                        out.push_str("<tr>");
                        for k in &keys {
                            let cell = obj.get(*k).map(value_to_text).unwrap_or_default();
                            out.push_str(&format!("<td>{}</td>", html_escape(&cell)));
                        }
                        out.push_str("</tr>\n");
                    }
                }
                out.push_str("</tbody></table>\n");
                return out;
            }
            // Scalar array → unordered list
            let mut out = String::from("<ul>\n");
            for item in arr {
                let t = value_to_text(item);
                if !t.is_empty() {
                    out.push_str(&format!("<li>{}</li>\n", html_escape(&t)));
                }
            }
            out.push_str("</ul>\n");
            out
        }

        serde_json::Value::Object(obj) => {
            if obj.is_empty() {
                return String::new();
            }
            // All object depths rendered as a 2-column key/value table.
            // This ensures harumi compatibility (no <dl>/<dt>/<dd>).
            let mut out = String::from("<table>\n<tbody>\n");
            for (k, v) in obj {
                if v.is_null() {
                    continue;
                }
                if let serde_json::Value::String(s) = v {
                    if s.is_empty() {
                        continue;
                    }
                }
                let child_html = render_value_html(v);
                if child_html.trim().is_empty() {
                    continue;
                }
                out.push_str(&format!(
                    "<tr><th class=\"kv-key\">{}</th><td>{}</td></tr>\n",
                    html_escape(k),
                    child_html.trim()
                ));
            }
            out.push_str("</tbody></table>\n");
            out
        }
    }
}

fn value_to_text(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Array(arr) => arr
            .iter()
            .map(value_to_text)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(", "),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::SdsRoot;

    #[test]
    fn render_html_empty_sds_produces_valid_html() {
        let sds = SdsRoot::default();
        let html = render_html(&sds, Language::Japanese).unwrap();
        assert!(html.starts_with("<!DOCTYPE html>"));
        assert!(html.contains("安全データシート"));
        assert!(html.contains("</html>"));
    }

    #[test]
    fn render_html_english() {
        let sds = SdsRoot::default();
        let html = render_html(&sds, Language::English).unwrap();
        assert!(html.contains("Safety Data Sheet"));
        assert!(html.contains("lang=\"en\""));
    }

    /// `generate` is reserved for the future CAS/composition SDS-authoring
    /// workflow; the deprecated re-export here must still only ever mean
    /// "render an existing SdsRoot", never build one from scratch.
    #[test]
    #[allow(deprecated)]
    fn deprecated_generate_html_delegates_to_render_html() {
        let sds = SdsRoot::default();
        assert_eq!(
            generate_html(&sds, Language::Japanese).unwrap(),
            render_html(&sds, Language::Japanese).unwrap(),
        );
    }

    #[test]
    fn html_escape_special_chars() {
        assert_eq!(html_escape("a < b & c > d"), "a &lt; b &amp; c &gt; d");
        assert_eq!(html_escape("\"quoted\""), "&quot;quoted&quot;");
    }
}
