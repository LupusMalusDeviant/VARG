// Wave 27: PDF Generation Runtime
//
// Native PDF creation using the printpdf crate.
// Provides a simple API for creating documents with sections and text.

use base64::{engine::general_purpose::STANDARD, Engine as _};
use printpdf::*;

use std::sync::{Arc, Mutex};

const PAGE_WIDTH_MM: f32 = 210.0;
const PAGE_HEIGHT_MM: f32 = 297.0;
const MARGIN_LEFT_MM: f32 = 25.0;
const MARGIN_RIGHT_MM: f32 = 25.0;
const MARGIN_TOP_MM: f32 = 25.0;
const MARGIN_BOTTOM_MM: f32 = 25.0;
const HEADING_SIZE: f32 = 18.0;
const BODY_SIZE: f32 = 11.0;
const LINE_HEIGHT_BODY: f32 = 5.0;
const LINE_HEIGHT_HEADING: f32 = 8.0;
const SECTION_SPACING: f32 = 10.0;
const CHARS_PER_LINE: usize = 80;

#[derive(Debug, Clone)]
struct PdfContent {
    kind: ContentKind,
    text: String,
}

#[derive(Debug, Clone)]
enum ContentKind {
    Section { heading: String },
    Text,
}

/// Internal PDF document state
pub struct PdfHandle {
    title: String,
    contents: Vec<PdfContent>,
}

/// Shared, thread-safe PDF handle
pub type PdfDocHandle = Arc<Mutex<PdfHandle>>;

/// Create a new PDF document with a title
pub fn __varg_pdf_create(title: &str) -> PdfDocHandle {
    Arc::new(Mutex::new(PdfHandle {
        title: title.to_string(),
        contents: Vec::new(),
    }))
}

/// Add a section with heading and body text
pub fn __varg_pdf_add_section(doc: &PdfDocHandle, heading: &str, body: &str) {
    let mut d = doc.lock().unwrap_or_else(|e| e.into_inner());
    d.contents.push(PdfContent {
        kind: ContentKind::Section { heading: heading.to_string() },
        text: body.to_string(),
    });
}

/// Add raw text without heading
pub fn __varg_pdf_add_text(doc: &PdfDocHandle, text: &str) {
    let mut d = doc.lock().unwrap_or_else(|e| e.into_inner());
    d.contents.push(PdfContent {
        kind: ContentKind::Text,
        text: text.to_string(),
    });
}

/// Word-wrap text to fit within page width
fn word_wrap(text: &str, max_chars: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.split('\n') {
        if paragraph.is_empty() {
            lines.push(String::new());
            continue;
        }
        let words: Vec<&str> = paragraph.split_whitespace().collect();
        let mut current_line = String::new();
        for word in words {
            if current_line.is_empty() {
                current_line = word.to_string();
            } else if current_line.len() + 1 + word.len() > max_chars {
                lines.push(current_line);
                current_line = word.to_string();
            } else {
                current_line.push(' ');
                current_line.push_str(word);
            }
        }
        if !current_line.is_empty() {
            lines.push(current_line);
        }
    }
    lines
}

/// Render the PDF to bytes.
///
/// printpdf 0.7 built a document of layers and drew through `layer.use_text(...)`. 0.12 has no
/// layers in that sense: a page is a list of operations and the document is a list of pages. The
/// layout is unchanged — same margins, line heights, word wrap and page breaks — only the way a
/// line reaches the page differs.
fn render_pdf(handle: &PdfHandle) -> Vec<u8> {
    let heading_font = PdfFontHandle::Builtin(BuiltinFont::HelveticaBold);
    let body_font = PdfFontHandle::Builtin(BuiltinFont::Helvetica);

    let mut pages: Vec<PdfPage> = Vec::new();
    let mut ops: Vec<Op> = Vec::new();
    let mut y_pos = PAGE_HEIGHT_MM - MARGIN_TOP_MM;

    // One line of text at the current cursor. A text section per line, as the old code emitted
    // one `use_text` per line: the positions are absolute, so nothing depends on line-height
    // state carried between them.
    fn draw(ops: &mut Vec<Op>, text: &str, size: f32, y_mm: f32, font: &PdfFontHandle) {
        ops.push(Op::StartTextSection);
        ops.push(Op::SetFont { font: font.clone(), size: Pt(size) });
        ops.push(Op::SetTextCursor {
            pos: Point { x: Mm(MARGIN_LEFT_MM).into(), y: Mm(y_mm).into() },
        });
        ops.push(Op::ShowText { items: vec![TextItem::Text(text.to_string())] });
        ops.push(Op::EndTextSection);
    }

    for content in &handle.contents {
        match &content.kind {
            ContentKind::Section { heading } => {
                // Enough room for the heading and a few lines under it, or start a new page.
                if y_pos < MARGIN_BOTTOM_MM + LINE_HEIGHT_HEADING + LINE_HEIGHT_BODY * 3.0 {
                    pages.push(PdfPage::new(Mm(PAGE_WIDTH_MM), Mm(PAGE_HEIGHT_MM), std::mem::take(&mut ops)));
                    y_pos = PAGE_HEIGHT_MM - MARGIN_TOP_MM;
                }

                y_pos -= SECTION_SPACING;
                draw(&mut ops, heading, HEADING_SIZE, y_pos, &heading_font);
                y_pos -= LINE_HEIGHT_HEADING;

                for line in &word_wrap(&content.text, CHARS_PER_LINE) {
                    if y_pos < MARGIN_BOTTOM_MM {
                        pages.push(PdfPage::new(Mm(PAGE_WIDTH_MM), Mm(PAGE_HEIGHT_MM), std::mem::take(&mut ops)));
                        y_pos = PAGE_HEIGHT_MM - MARGIN_TOP_MM;
                    }
                    draw(&mut ops, line, BODY_SIZE, y_pos, &body_font);
                    y_pos -= LINE_HEIGHT_BODY;
                }
            }
            ContentKind::Text => {
                for line in &word_wrap(&content.text, CHARS_PER_LINE) {
                    if y_pos < MARGIN_BOTTOM_MM {
                        pages.push(PdfPage::new(Mm(PAGE_WIDTH_MM), Mm(PAGE_HEIGHT_MM), std::mem::take(&mut ops)));
                        y_pos = PAGE_HEIGHT_MM - MARGIN_TOP_MM;
                    }
                    draw(&mut ops, line, BODY_SIZE, y_pos, &body_font);
                    y_pos -= LINE_HEIGHT_BODY;
                }
            }
        }
    }

    // Whatever is left, and at least one page: a document with no pages is not a document.
    if !ops.is_empty() || pages.is_empty() {
        pages.push(PdfPage::new(Mm(PAGE_WIDTH_MM), Mm(PAGE_HEIGHT_MM), ops));
    }

    let mut doc = PdfDocument::new(&handle.title);
    doc.with_pages(pages);
    let mut warnings = Vec::new();
    doc.save(&PdfSaveOptions::default(), &mut warnings)
}

/// Save the PDF document to a file
pub fn __varg_pdf_save(doc: &PdfDocHandle, path: &str) -> String {
    let handle = doc.lock().unwrap_or_else(|e| e.into_inner());
    let bytes = render_pdf(&handle);
    match std::fs::write(path, &bytes) {
        Ok(_) => format!("ok:{}", bytes.len()),
        Err(e) => format!("[pdf_save error: {}]", e),
    }
}

/// Get the PDF document as a base64-encoded string
pub fn __varg_pdf_to_base64(doc: &PdfDocHandle) -> String {
    let handle = doc.lock().unwrap_or_else(|e| e.into_inner());
    let bytes = render_pdf(&handle);
    STANDARD.encode(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A rendered document has to contain the text that went into it.
    ///
    /// Nothing checked this. The golden program asserts that the base64 is longer than 100
    /// characters and the saved file bigger than 100 bytes; a renderer that emitted blank pages
    /// would pass both, and so would one that lost every heading. printpdf writes page content as
    /// a Flate-compressed stream of hex strings, so the text can be recovered and looked for.
    #[test]
    fn rendered_pdf_contains_the_text_it_was_given() {
        use flate2::read::ZlibDecoder;
        use std::io::Read;

        let doc = __varg_pdf_create("Report Title");
        __varg_pdf_add_section(&doc, "First Section", "Body text alpha bravo.");
        __varg_pdf_add_text(&doc, "A paragraph, delta echo.");
        let bytes = {
            let d = doc.lock().unwrap_or_else(|e| e.into_inner());
            render_pdf(&d)
        };

        assert!(bytes.starts_with(b"%PDF-"), "not a PDF at all");
        let page_markers = bytes
            .windows(10)
            .filter(|w| w == b"/Type /Pag" || w == b"/Type/Page")
            .count();
        assert!(page_markers >= 1, "no page object in the output");

        // The file's own bytes, plus the inflation of every stream that inflates. Which of the
        // two holds the text depends on the library's compression settings, and that is not what
        // this test is about.
        let mut recovered = bytes.clone();
        let mut at = 0usize;
        while let Some(s) = find(&bytes[at..], b"stream") {
            let body_start = at + s + b"stream".len();
            let body_start = skip_eol(&bytes, body_start);
            let Some(e) = find(&bytes[body_start..], b"endstream") else { break };
            let body = &bytes[body_start..body_start + e];
            let mut out = Vec::new();
            let _ = ZlibDecoder::new(body).read_to_end(&mut out);
            recovered.extend_from_slice(&out);
            at = body_start + e + b"endstream".len();
        }
        let text = show_text_operands(&recovered);
        for want in ["First Section", "Body text alpha bravo.", "A paragraph, delta echo."] {
            assert!(
                text.contains(want),
                "{:?} is missing from the rendered page; recovered: {:?}",
                want,
                text
            );
        }
    }

    fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
        haystack.windows(needle.len()).position(|w| w == needle)
    }

    fn skip_eol(b: &[u8], mut i: usize) -> usize {
        if b.get(i) == Some(&b'\r') {
            i += 1;
        }
        if b.get(i) == Some(&b'\n') {
            i += 1;
        }
        i
    }

    /// The operands of the text-showing operators: `(literal)` and `<48656C6C6F>` alike.
    fn show_text_operands(content: &[u8]) -> String {
        let mut out = String::new();
        let mut i = 0;
        while i < content.len() {
            match content[i] {
                b'<' => {
                    if let Some(end) = find(&content[i..], b">") {
                        let hex = &content[i + 1..i + end];
                        let mut byte = 0u8;
                        let mut half = false;
                        for c in hex {
                            let Some(v) = (*c as char).to_digit(16) else { continue };
                            if half {
                                out.push((byte << 4 | v as u8) as char);
                                half = false;
                            } else {
                                byte = v as u8;
                                half = true;
                            }
                        }
                        out.push(' ');
                        i += end + 1;
                        continue;
                    }
                }
                b'(' => {
                    // A literal string. Backslash escapes the next byte, including a closing
                    // parenthesis, so it cannot simply be scanned for `)`.
                    let mut j = i + 1;
                    while j < content.len() && content[j] != b')' {
                        if content[j] == b'\\' {
                            j += 1;
                        }
                        out.push(content[j.min(content.len() - 1)] as char);
                        j += 1;
                    }
                    out.push(' ');
                    i = j + 1;
                    continue;
                }
                _ => {}
            }
            i += 1;
        }
        out
    }

    #[test]
    fn test_pdf_create() {
        let doc = __varg_pdf_create("Test Document");
        let d = doc.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(d.title, "Test Document");
        assert!(d.contents.is_empty());
    }

    #[test]
    fn test_pdf_add_section() {
        let doc = __varg_pdf_create("Test");
        __varg_pdf_add_section(&doc, "Chapter 1", "This is the body text.");
        let d = doc.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(d.contents.len(), 1);
    }

    #[test]
    fn test_pdf_add_text() {
        let doc = __varg_pdf_create("Test");
        __varg_pdf_add_text(&doc, "A paragraph of text.");
        let d = doc.lock().unwrap_or_else(|e| e.into_inner());
        assert_eq!(d.contents.len(), 1);
    }

    #[test]
    fn test_pdf_save_to_file() {
        let doc = __varg_pdf_create("Test PDF");
        __varg_pdf_add_section(&doc, "Hello", "World");
        __varg_pdf_add_text(&doc, "Some additional text.");

        let tmp = std::env::temp_dir().join("varg_test_output.pdf");
        let result = __varg_pdf_save(&doc, tmp.to_str().unwrap());
        assert!(result.starts_with("ok:"), "Expected ok, got: {}", result);

        // Verify file exists and has content
        let bytes = std::fs::read(&tmp).unwrap();
        assert!(bytes.len() > 100); // PDF should have reasonable size
        assert_eq!(&bytes[0..5], b"%PDF-"); // PDF magic bytes

        std::fs::remove_file(tmp).ok();
    }

    #[test]
    fn test_pdf_to_base64() {
        let doc = __varg_pdf_create("B64 Test");
        __varg_pdf_add_text(&doc, "Content for base64 encoding.");
        let b64 = __varg_pdf_to_base64(&doc);
        assert!(!b64.is_empty());

        // Decode and verify PDF magic bytes
        let bytes = STANDARD.decode(&b64).unwrap();
        assert_eq!(&bytes[0..5], b"%PDF-");
    }

    #[test]
    fn test_word_wrap() {
        let text = "This is a test of the word wrapping functionality that should break lines properly";
        let lines = word_wrap(text, 30);
        for line in &lines {
            assert!(line.len() <= 35); // Allow slight overflow for long words
        }
        assert!(lines.len() > 1);
    }

    #[test]
    fn test_word_wrap_preserves_newlines() {
        let text = "Line one\n\nLine three";
        let lines = word_wrap(text, 80);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[1], "");
    }

    #[test]
    fn test_pdf_multiple_sections() {
        let doc = __varg_pdf_create("Multi-Section");
        __varg_pdf_add_section(&doc, "Section 1", "First section body.");
        __varg_pdf_add_section(&doc, "Section 2", "Second section body.");
        __varg_pdf_add_section(&doc, "Section 3", "Third section body.");
        __varg_pdf_add_text(&doc, "Final paragraph.");

        let tmp = std::env::temp_dir().join("varg_test_multi.pdf");
        let result = __varg_pdf_save(&doc, tmp.to_str().unwrap());
        assert!(result.starts_with("ok:"));
        std::fs::remove_file(tmp).ok();
    }
}
