//! Shared text editing utilities for code and text_area widgets
//!
//! Contains word boundary detection, clipboard integration, and
//! other helpers shared between multi-line text editing widgets.

/// Find the start of the previous word from a character position in a line.
/// Words are separated by whitespace and punctuation boundaries.
pub fn word_boundary_left(text: &str, char_pos: usize) -> usize {
    let chars: Vec<char> = text.chars().collect();
    if char_pos == 0 || chars.is_empty() {
        return 0;
    }

    let mut pos = char_pos.min(chars.len());

    // Skip whitespace to the left
    while pos > 0 && chars[pos - 1].is_whitespace() {
        pos -= 1;
    }

    if pos == 0 {
        return 0;
    }

    // Determine the category of the character we landed on
    let is_word = chars[pos - 1].is_alphanumeric() || chars[pos - 1] == '_';

    // Skip characters of the same category
    if is_word {
        while pos > 0 && (chars[pos - 1].is_alphanumeric() || chars[pos - 1] == '_') {
            pos -= 1;
        }
    } else {
        // Punctuation group
        while pos > 0
            && !chars[pos - 1].is_alphanumeric()
            && chars[pos - 1] != '_'
            && !chars[pos - 1].is_whitespace()
        {
            pos -= 1;
        }
    }

    pos
}

/// Find the end of the next word from a character position in a line.
/// Words are separated by whitespace and punctuation boundaries.
pub fn word_boundary_right(text: &str, char_pos: usize) -> usize {
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    if char_pos >= len || chars.is_empty() {
        return len;
    }

    let mut pos = char_pos;

    // Determine the category of the character at pos
    let is_word = chars[pos].is_alphanumeric() || chars[pos] == '_';
    let is_ws = chars[pos].is_whitespace();

    if is_ws {
        // Skip whitespace
        while pos < len && chars[pos].is_whitespace() {
            pos += 1;
        }
    } else if is_word {
        // Skip word characters
        while pos < len && (chars[pos].is_alphanumeric() || chars[pos] == '_') {
            pos += 1;
        }
    } else {
        // Skip punctuation
        while pos < len
            && !chars[pos].is_alphanumeric()
            && chars[pos] != '_'
            && !chars[pos].is_whitespace()
        {
            pos += 1;
        }
    }

    // Also skip trailing whitespace after a word/punct group
    while pos < len && chars[pos].is_whitespace() {
        pos += 1;
    }

    pos
}

/// Find word boundaries around a position (for double-click word selection).
/// Returns (start, end) character positions of the word at `char_pos`.
pub fn word_at_position(text: &str, char_pos: usize) -> (usize, usize) {
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    if len == 0 || char_pos >= len {
        return (char_pos, char_pos);
    }

    let ch = chars[char_pos];
    let is_word = ch.is_alphanumeric() || ch == '_';

    if ch.is_whitespace() {
        // Select the whitespace run
        let mut start = char_pos;
        let mut end = char_pos;
        while start > 0 && chars[start - 1].is_whitespace() {
            start -= 1;
        }
        while end < len && chars[end].is_whitespace() {
            end += 1;
        }
        (start, end)
    } else if is_word {
        let mut start = char_pos;
        let mut end = char_pos;
        while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
            start -= 1;
        }
        while end < len && (chars[end].is_alphanumeric() || chars[end] == '_') {
            end += 1;
        }
        (start, end)
    } else {
        // Punctuation — select the punctuation run
        let mut start = char_pos;
        let mut end = char_pos;
        while start > 0
            && !chars[start - 1].is_alphanumeric()
            && chars[start - 1] != '_'
            && !chars[start - 1].is_whitespace()
        {
            start -= 1;
        }
        while end < len
            && !chars[end].is_alphanumeric()
            && chars[end] != '_'
            && !chars[end].is_whitespace()
        {
            end += 1;
        }
        (start, end)
    }
}

/// Read text from the system clipboard (macOS).
/// Returns None if clipboard is empty or unavailable.
pub fn clipboard_read() -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("pbpaste")
            .output()
            .ok()?;
        if output.status.success() {
            let text = String::from_utf8(output.stdout).ok()?;
            if text.is_empty() {
                None
            } else {
                Some(text)
            }
        } else {
            None
        }
    }
    #[cfg(target_os = "linux")]
    {
        let output = std::process::Command::new("xclip")
            .args(["-selection", "clipboard", "-o"])
            .output()
            .ok()?;
        if output.status.success() {
            let text = String::from_utf8(output.stdout).ok()?;
            if text.is_empty() { None } else { Some(text) }
        } else {
            None
        }
    }
    #[cfg(target_os = "windows")]
    {
        // Windows clipboard requires win32 API; skip for now
        None
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        None
    }
}

/// Write text to the system clipboard (macOS).
/// Returns true on success.
pub fn clipboard_write(text: &str) -> bool {
    #[cfg(target_os = "macos")]
    {
        use std::io::Write;
        let mut child = match std::process::Command::new("pbcopy")
            .stdin(std::process::Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(_) => return false,
        };
        if let Some(ref mut stdin) = child.stdin {
            let _ = stdin.write_all(text.as_bytes());
        }
        child.wait().map(|s| s.success()).unwrap_or(false)
    }
    #[cfg(target_os = "linux")]
    {
        use std::io::Write;
        let mut child = match std::process::Command::new("xclip")
            .args(["-selection", "clipboard"])
            .stdin(std::process::Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(_) => return false,
        };
        if let Some(ref mut stdin) = child.stdin {
            let _ = stdin.write_all(text.as_bytes());
        }
        child.wait().map(|s| s.success()).unwrap_or(false)
    }
    #[cfg(target_os = "windows")]
    {
        let _ = text;
        false
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        let _ = text;
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_word_boundary_left() {
        assert_eq!(word_boundary_left("hello world", 5), 0);
        assert_eq!(word_boundary_left("hello world", 6), 0);
        assert_eq!(word_boundary_left("hello world", 11), 6);
        assert_eq!(word_boundary_left("fn main() {", 3), 0);
        assert_eq!(word_boundary_left("fn main() {", 8), 7); // at ')', skips '(' punct
    }

    #[test]
    fn test_word_boundary_right() {
        assert_eq!(word_boundary_right("hello world", 0), 6);
        assert_eq!(word_boundary_right("hello world", 6), 11);
        assert_eq!(word_boundary_right("fn main() {", 0), 3);
    }

    #[test]
    fn test_word_at_position() {
        assert_eq!(word_at_position("hello world", 2), (0, 5));
        assert_eq!(word_at_position("hello world", 7), (6, 11));
        assert_eq!(word_at_position("hello world", 5), (5, 6)); // space
    }
}
