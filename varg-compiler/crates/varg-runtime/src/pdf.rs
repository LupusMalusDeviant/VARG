// Wave 27: PDF Generation Runtime
//
// Native PDF creation using the printpdf crate.
// Provides a simple API for creating documents with sections and text.

use base64::{engine::general_purpose::STANDARD, Engine as _};
use printpdf::*;
use std::io::BufWriter;
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
    let mut d = doc.lock().unwrap();
    d.contents.push(PdfContent {
        kind: ContentKind::Section { heading: heading.to_string() },
        text: body.to_string(),
    });
}

/// Add raw text without heading
pub fn __varg_pdf_add_text(doc: &PdfDocHandle, text: &str) {
    let mut d = doc.lock().unwrap();
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

/// Render the PDF to bytes
fn render_pdf(handle: &PdfHandle) -> Vec<u8> {
    let (doc, page1, layer1) = PdfDocument::new(
        &handle.title,
        Mm(PAGE_WIDTH_MM),
        Mm(PAGE_HEIGHT_MM),
        "Layer 1",
    );

    let font = doc.add_builtin_font(BuiltinFont::Helvetica).unwrap();
    let font_bold = doc.add_builtin_font(BuiltinFont::HelveticaBold).unwrap();

    let usable_width = PAGE_WIDTH_MM - MARGIN_LEFT_MM - MARGIN_RIGHT_MM;
    let _ = usable_width; // used conceptually for word wrap via CHARS_PER_LINE

    let mut current_layer = doc.get_page(page1).get_layer(layer1);
    let mut y_pos = PAGE_HEIGHT_MM - MARGIN_TOP_MM;

    for content in &handle.contents {
        match &content.kind {
            ContentKind::Section { heading } => {
                // Check if we need a new page for heading + at least a few lines
                if y_pos < MARGIN_BOTTOM_MM + LINE_HEIGHT_HEADING + LINE_HEIGHT_BODY * 3.0 {
                    let (new_page, new_layer) = doc.add_page(
                        Mm(PAGE_WIDTH_MM),
                        Mm(PAGE_HEIGHT_MM),
                        "Layer 1",
                    );
                    current_layer = doc.get_page(new_page).get_layer(new_layer);
                    y_pos = PAGE_HEIGHT_MM - MARGIN_TOP_MM;
                }

                // Add spacing before section
                y_pos -= SECTION_SPACING;

                // Draw heading
                current_layer.use_text(heading, HEADING_SIZE, Mm(MARGIN_LEFT_MM), Mm(y_pos), &font_bold);
                y_pos -= LINE_HEIGHT_HEADING;

                // Draw body lines
                let lines = word_wrap(&content.text, CHARS_PER_LINE);
                for line in &lines {
                    if y_pos < MARGIN_BOTTOM_MM {
                        let (new_page, new_layer) = doc.add_page(
                            Mm(PAGE_WIDTH_MM),
                            Mm(PAGE_HEIGHT_MM),
                            "Layer 1",
                        );
                        current_layer = doc.get_page(new_page).get_layer(new_layer);
                        y_pos = PAGE_HEIGHT_MM - MARGIN_TOP_MM;
                    }
                    current_layer.use_text(line, BODY_SIZE, Mm(MARGIN_LEFT_MM), Mm(y_pos), &font);
                    y_pos -= LINE_HEIGHT_BODY;
                }
            }
            ContentKind::Text => {
                let lines = word_wrap(&content.text, CHARS_PER_LINE);
                for line in &lines {
                    if y_pos < MARGIN_BOTTOM_MM {
                        let (new_page, new_layer) = doc.add_page(
                            Mm(PAGE_WIDTH_MM),
                            Mm(PAGE_HEIGHT_MM),
                            "Layer 1",
                        );
                        current_layer = doc.get_page(new_page).get_layer(new_layer);
                        y_pos = PAGE_HEIGHT_MM - MARGIN_TOP_MM;
                    }
                    current_layer.use_text(line, BODY_SIZE, Mm(MARGIN_LEFT_MM), Mm(y_pos), &font);
                    y_pos -= LINE_HEIGHT_BODY;
                }
            }
        }
    }

    let mut buf = BufWriter::new(Vec::new());
    doc.save(&mut buf).unwrap();
    buf.into_inner().unwrap()
}

/// Save the PDF document to a file
pub fn __varg_pdf_save(doc: &PdfDocHandle, path: &str) -> String {
    let handle = doc.lock().unwrap();
    let bytes = render_pdf(&handle);
    match std::fs::write(path, &bytes) {
        Ok(_) => format!("ok:{}", bytes.len()),
        Err(e) => format!("[pdf_save error: {}]", e),
    }
}

/// Get the PDF document as a base64-encoded string
pub fn __varg_pdf_to_base64(doc: &PdfDocHandle) -> String {
    let handle = doc.lock().unwrap();
    let bytes = render_pdf(&handle);
    STANDARD.encode(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pdf_create() {
        let doc = __varg_pdf_create("Test Document");
        let d = doc.lock().unwrap();
        assert_eq!(d.title, "Test Document");
        assert!(d.contents.is_empty());
    }

    #[test]
    fn test_pdf_add_section() {
        let doc = __varg_pdf_create("Test");
        __varg_pdf_add_section(&doc, "Chapter 1", "This is the body text.");
        let d = doc.lock().unwrap();
        assert_eq!(d.contents.len(), 1);
    }

    #[test]
    fn test_pdf_add_text() {
        let doc = __varg_pdf_create("Test");
        __varg_pdf_add_text(&doc, "A paragraph of text.");
        let d = doc.lock().unwrap();
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
