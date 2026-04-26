//! Wave 29: Readline / REPL primitives built on rustyline.
//!
//! Provides line editing (cursor movement, backspace), history navigation
//! (up/down arrows), and persistent history files. This is the baseline
//! primitive for interactive CLI agents like claw_lite.
//!
//! Handle-based API so a REPL can hold onto one editor across many reads
//! and persist history between sessions.

use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::sync::{Arc, Mutex};

pub type ReadlineHandle = Arc<Mutex<DefaultEditor>>;

/// Create a new line editor with history enabled.
pub fn __varg_readline_new() -> Result<ReadlineHandle, String> {
    DefaultEditor::new()
        .map(|ed| Arc::new(Mutex::new(ed)))
        .map_err(|e| e.to_string())
}

/// Prompt the user for a line. Returns the input on success.
/// Errors distinguish EOF (`eof`) and interrupt (`interrupt`) so callers can
/// decide whether to terminate the REPL or just cancel the current line.
pub fn __varg_readline_read(handle: &ReadlineHandle, prompt: &str) -> Result<String, String> {
    let mut editor = handle.lock().map_err(|e| e.to_string())?;
    match editor.readline(prompt) {
        Ok(line) => Ok(line),
        Err(ReadlineError::Eof) => Err("eof".to_string()),
        Err(ReadlineError::Interrupted) => Err("interrupt".to_string()),
        Err(e) => Err(e.to_string()),
    }
}

/// Push a line into the in-memory history ring. Caller decides when (usually
/// after a non-empty successful read, to avoid recording blanks/duplicates).
pub fn __varg_readline_add_history(handle: &ReadlineHandle, line: &str) -> Result<(), String> {
    let mut editor = handle.lock().map_err(|e| e.to_string())?;
    editor
        .add_history_entry(line)
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Load history from a file (typically `$HOME/.claw_history`). Missing file
/// is not an error — the cascade just starts empty.
pub fn __varg_readline_load_history(handle: &ReadlineHandle, path: &str) -> Result<(), String> {
    let mut editor = handle.lock().map_err(|e| e.to_string())?;
    match editor.load_history(path) {
        Ok(()) => Ok(()),
        Err(ReadlineError::Io(ref e)) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

/// Persist history to a file so the next session sees it.
pub fn __varg_readline_save_history(handle: &ReadlineHandle, path: &str) -> Result<(), String> {
    let mut editor = handle.lock().map_err(|e| e.to_string())?;
    editor.save_history(path).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_editor() {
        let h = __varg_readline_new();
        assert!(h.is_ok(), "should create editor: {:?}", h.err());
    }

    #[test]
    fn test_add_history_entry() {
        let h = __varg_readline_new().unwrap();
        let r = __varg_readline_add_history(&h, "some command");
        assert!(r.is_ok(), "should add history: {:?}", r.err());
    }

    #[test]
    fn test_load_history_missing_is_ok() {
        let h = __varg_readline_new().unwrap();
        let r = __varg_readline_load_history(&h, "/nonexistent/varg_wave29_hist");
        assert!(r.is_ok(), "missing history file must be silent: {:?}", r.err());
    }

    #[test]
    fn test_save_and_load_history_roundtrip() {
        let h1 = __varg_readline_new().unwrap();
        __varg_readline_add_history(&h1, "first line").unwrap();
        __varg_readline_add_history(&h1, "second line").unwrap();

        let path = std::env::temp_dir().join("varg_wave29_hist_roundtrip.txt");
        let path_str = path.to_string_lossy().to_string();
        __varg_readline_save_history(&h1, &path_str).unwrap();

        let h2 = __varg_readline_new().unwrap();
        __varg_readline_load_history(&h2, &path_str).unwrap();
        // We can't easily assert the contents through the public API, but a
        // successful round-trip without error is enough to catch regressions.

        let _ = std::fs::remove_file(&path);
    }
}
