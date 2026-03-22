//! Code block widget with syntax highlighting
//!
//! Two modes of operation:
//!
//! - **Read-only** via `code("content")` — lightweight display widget
//! - **Editable** via `code_editor(&state)` — full editor with Stateful incremental updates
//!
//! # Example
//!
//! ```ignore
//! use blinc_layout::prelude::*;
//! use blinc_layout::syntax::{SyntaxConfig, RustHighlighter};
//!
//! // Read-only code block
//! code(r#"fn main() { println!("Hello"); }"#)
//!     .syntax(SyntaxConfig::new(RustHighlighter::new()))
//!     .line_numbers(true)
//!     .font_size(14.0)
//!
//! // Editable code block
//! let state = code_editor_state("let x = 42;");
//! code_editor(&state)
//!     .syntax(SyntaxConfig::new(RustHighlighter::new()))
//!     .line_numbers(true)
//!     .on_change(|new_content| {
//!         println!("Content changed: {}", new_content);
//!     })
//! ```

use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex};

use blinc_core::{Brush, Color, CornerRadius, Rect};
use blinc_theme::{ColorToken, ThemeState};

use crate::canvas::canvas;
use crate::div::{div, Div, ElementBuilder, ElementTypeId};
use crate::element::RenderProps;
use crate::styled_text::StyledText;
use crate::syntax::{SyntaxConfig, SyntaxHighlighter, TokenHit};
use crate::text::text;
use crate::tree::{LayoutNodeId, LayoutTree};
use crate::widgets::cursor::{cursor_state, CursorAnimation, SharedCursorState};
use crate::widgets::scroll::{Scroll, ScrollDirection, ScrollPhysics, SharedScrollPhysics};
use crate::widgets::text_area::TextPosition;
use crate::widgets::text_edit;
use crate::widgets::text_input::{
    decrement_focus_count, increment_focus_count, request_continuous_redraw_pub,
};

// ============================================================================
// Configuration
// ============================================================================

/// Code block configuration
#[derive(Clone)]
pub struct CodeConfig {
    /// Font size in pixels
    pub font_size: f32,
    /// Line height multiplier
    pub line_height: f32,
    /// Show line numbers in gutter
    pub line_numbers: bool,
    /// Gutter width (for line numbers)
    pub gutter_width: f32,
    /// Padding inside the code block
    pub padding: f32,
    /// Corner radius
    pub corner_radius: f32,
    /// Background color
    pub bg_color: Color,
    /// Text color (default, when no syntax highlighting)
    pub text_color: Color,
    /// Line number color
    pub line_number_color: Color,
    /// Cursor color (when editable)
    pub cursor_color: Color,
    /// Selection color (when editable)
    pub selection_color: Color,
    /// Gutter background color
    pub gutter_bg_color: Color,
    /// Gutter separator color
    pub gutter_separator_color: Color,
}

impl Default for CodeConfig {
    fn default() -> Self {
        let theme = ThemeState::get();
        Self {
            font_size: 13.0,
            line_height: 1.5,
            line_numbers: false,
            gutter_width: 48.0,
            padding: 16.0,
            corner_radius: 8.0,
            bg_color: theme.color(ColorToken::Surface),
            text_color: theme.color(ColorToken::TextPrimary),
            line_number_color: theme.color(ColorToken::TextTertiary),
            cursor_color: theme.color(ColorToken::Accent),
            selection_color: theme.color(ColorToken::Selection),
            gutter_bg_color: theme.color(ColorToken::SurfaceOverlay),
            gutter_separator_color: theme.color(ColorToken::Border),
        }
    }
}

// ============================================================================
// Shared Editor State
// ============================================================================

/// Callback type for content changes
type OnChangeCallback = Arc<dyn Fn(&str) + Send + Sync>;

/// Internal state for code editing
#[derive(Clone)]
pub struct CodeEditorData {
    /// Lines of text
    pub lines: Vec<String>,
    /// Cursor position
    pub cursor: TextPosition,
    /// Selection start (if selecting)
    pub selection_start: Option<TextPosition>,
    /// Whether currently focused
    pub focused: bool,
    /// Canvas-based cursor state
    pub cursor_state: SharedCursorState,
    /// On-change callback
    pub on_change: Option<OnChangeCallback>,
    /// Syntax highlighter
    pub highlighter: Option<Arc<dyn SyntaxHighlighter>>,
    /// Configuration snapshot (updated from builder)
    pub config: CodeConfig,
    /// Undo stack
    pub undo_stack: Vec<UndoEntry>,
    /// Redo stack
    pub redo_stack: Vec<UndoEntry>,
    /// Cached monospace character width for current font_size.
    /// Avoids calling measure_text() for every cursor/selection calculation.
    mono_char_width: f32,
    /// Font size that mono_char_width was computed for
    mono_char_width_font_size: f32,
    /// Cached per-line syntax highlight results. Indexed by line number.
    /// Cleared when content changes on that line.
    highlight_cache: Vec<Option<crate::styled_text::StyledLine>>,
    /// Drag selection anchor (mouse-down position, set to selection_start on drag)
    pub drag_anchor: Option<TextPosition>,
    /// Scroll physics for vertical scrolling
    pub scroll_physics: SharedScrollPhysics,
    /// Cached viewport height (content area minus padding)
    pub viewport_height: f32,
}

/// Undo/redo entry
#[derive(Debug, Clone)]
pub struct UndoEntry {
    pub lines: Vec<String>,
    pub cursor: TextPosition,
    pub selection_start: Option<TextPosition>,
}

/// Shared code editor state (thread-safe, persists across rebuilds)
pub type SharedCodeEditorState = Arc<Mutex<CodeEditorData>>;

/// Create a new shared code editor state
pub fn code_editor_state(content: impl Into<String>) -> SharedCodeEditorState {
    let content = content.into();
    let lines: Vec<String> = if content.is_empty() {
        vec![String::new()]
    } else {
        content.lines().map(|s| s.to_string()).collect()
    };

    let num_lines = lines.len();
    Arc::new(Mutex::new(CodeEditorData {
        lines,
        cursor: TextPosition::default(),
        selection_start: None,
        focused: false,
        cursor_state: cursor_state(),
        on_change: None,
        highlighter: None,
        config: CodeConfig::default(),
        undo_stack: Vec::new(),
        redo_stack: Vec::new(),
        mono_char_width: 0.0,
        mono_char_width_font_size: 0.0,
        highlight_cache: vec![None; num_lines],
        drag_anchor: None,
        scroll_physics: Arc::new(Mutex::new(ScrollPhysics::default())),
        viewport_height: 0.0,
    }))
}

impl CodeEditorData {
    /// Get full text content
    pub fn value(&self) -> String {
        self.lines.join("\n")
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.lines.len() == 1 && self.lines[0].is_empty()
    }

    /// Measure text width using monospace font options (matches rendered text).
    pub fn measure_mono(&self, text: &str) -> f32 {
        let opts = crate::text_measure::TextLayoutOptions {
            generic_font: crate::div::GenericFont::Monospace,
            ..crate::text_measure::TextLayoutOptions::new()
        };
        crate::text_measure::measure_text_with_options(text, self.config.font_size, &opts).width
    }

    /// Get the cached monospace character width, recomputing if font size changed.
    pub fn char_width(&mut self) -> f32 {
        let fs = self.config.font_size;
        if (self.mono_char_width_font_size - fs).abs() > 0.01 || self.mono_char_width <= 0.0 {
            self.mono_char_width = self.measure_mono("M");
            self.mono_char_width_font_size = fs;
        }
        self.mono_char_width
    }

    /// Measure the pixel width of `char_count` monospace characters.
    pub fn measure_chars(&mut self, char_count: usize) -> f32 {
        self.char_width() * char_count as f32
    }

    /// Invalidate highlight cache for a specific line
    fn invalidate_highlight_line(&mut self, line: usize) {
        if line < self.highlight_cache.len() {
            self.highlight_cache[line] = None;
        }
    }

    /// Resize highlight cache to match line count
    fn sync_highlight_cache(&mut self) {
        self.highlight_cache.resize(self.lines.len(), None);
    }

    /// Invalidate entire highlight cache
    fn invalidate_all_highlights(&mut self) {
        for slot in &mut self.highlight_cache {
            *slot = None;
        }
        self.sync_highlight_cache();
    }

    /// Save current state to undo stack
    pub fn push_undo(&mut self) {
        self.undo_stack.push(UndoEntry {
            lines: self.lines.clone(),
            cursor: self.cursor,
            selection_start: self.selection_start,
        });
        self.redo_stack.clear();
        // Cap undo history
        if self.undo_stack.len() > 200 {
            self.undo_stack.remove(0);
        }
    }

    /// Undo last change
    pub fn undo(&mut self) -> bool {
        if let Some(entry) = self.undo_stack.pop() {
            self.redo_stack.push(UndoEntry {
                lines: self.lines.clone(),
                cursor: self.cursor,
                selection_start: self.selection_start,
            });
            self.lines = entry.lines;
            self.cursor = entry.cursor;
            self.selection_start = entry.selection_start;
            self.invalidate_all_highlights();
            true
        } else {
            false
        }
    }

    /// Redo last undone change
    pub fn redo(&mut self) -> bool {
        if let Some(entry) = self.redo_stack.pop() {
            self.undo_stack.push(UndoEntry {
                lines: self.lines.clone(),
                cursor: self.cursor,
                selection_start: self.selection_start,
            });
            self.lines = entry.lines;
            self.cursor = entry.cursor;
            self.selection_start = entry.selection_start;
            self.invalidate_all_highlights();
            true
        } else {
            false
        }
    }

    /// Insert text at cursor position
    pub fn insert(&mut self, text: &str) {
        self.push_undo();
        self.delete_selection();
        let start_line = self.cursor.line;
        for ch in text.chars() {
            if ch == '\n' {
                self.insert_newline();
            } else {
                self.insert_char(ch);
            }
        }
        // Invalidate from start_line to current cursor line
        for l in start_line..=self.cursor.line {
            self.invalidate_highlight_line(l);
        }
        self.sync_highlight_cache();
    }

    fn insert_char(&mut self, ch: char) {
        if self.cursor.line < self.lines.len() {
            let line = &mut self.lines[self.cursor.line];
            let byte_pos = char_to_byte_pos(line, self.cursor.column);
            line.insert(byte_pos, ch);
            self.cursor.column += 1;
        }
    }

    fn insert_newline(&mut self) {
        if self.cursor.line < self.lines.len() {
            let current_line = &self.lines[self.cursor.line];
            let byte_pos = char_to_byte_pos(current_line, self.cursor.column);
            let rest = current_line[byte_pos..].to_string();
            self.lines[self.cursor.line] = current_line[..byte_pos].to_string();
            self.cursor.line += 1;
            self.cursor.column = 0;
            self.lines.insert(self.cursor.line, rest);
        }
    }

    /// Delete backward (backspace)
    pub fn delete_backward(&mut self) {
        if self.delete_selection() {
            return;
        }
        self.push_undo();
        let affected_line = self.cursor.line;
        if self.cursor.column > 0 {
            let line = &mut self.lines[self.cursor.line];
            let byte_pos = char_to_byte_pos(line, self.cursor.column - 1);
            let next_byte = char_to_byte_pos(line, self.cursor.column);
            line.replace_range(byte_pos..next_byte, "");
            self.cursor.column -= 1;
        } else if self.cursor.line > 0 {
            let current_line = self.lines.remove(self.cursor.line);
            self.cursor.line -= 1;
            self.cursor.column = self.lines[self.cursor.line].chars().count();
            self.lines[self.cursor.line].push_str(&current_line);
        }
        self.invalidate_highlight_line(self.cursor.line);
        self.sync_highlight_cache();
    }

    /// Delete forward (delete key)
    pub fn delete_forward(&mut self) {
        if self.delete_selection() {
            return;
        }
        self.push_undo();
        if self.cursor.line < self.lines.len() {
            let line_len = self.lines[self.cursor.line].chars().count();
            if self.cursor.column < line_len {
                let line = &mut self.lines[self.cursor.line];
                let byte_pos = char_to_byte_pos(line, self.cursor.column);
                let next_byte = char_to_byte_pos(line, self.cursor.column + 1);
                line.replace_range(byte_pos..next_byte, "");
            } else if self.cursor.line + 1 < self.lines.len() {
                let next_line = self.lines.remove(self.cursor.line + 1);
                self.lines[self.cursor.line].push_str(&next_line);
            }
        }
        self.invalidate_highlight_line(self.cursor.line);
        self.sync_highlight_cache();
    }

    /// Delete selection. Returns true if a selection was deleted.
    pub fn delete_selection(&mut self) -> bool {
        if let Some(sel_start) = self.selection_start.take() {
            let (start, end) = order_positions(sel_start, self.cursor);
            if start == end {
                return false;
            }
            self.push_undo();

            if start.line == end.line {
                let line = &mut self.lines[start.line];
                let start_byte = char_to_byte_pos(line, start.column);
                let end_byte = char_to_byte_pos(line, end.column);
                line.replace_range(start_byte..end_byte, "");
            } else {
                let start_byte = char_to_byte_pos(&self.lines[start.line], start.column);
                let end_byte = char_to_byte_pos(&self.lines[end.line], end.column);
                let end_remainder = self.lines[end.line][end_byte..].to_string();
                self.lines[start.line] = self.lines[start.line][..start_byte].to_string();
                self.lines[start.line].push_str(&end_remainder);
                self.lines.drain((start.line + 1)..=end.line);
            }

            self.cursor = start;
            self.selection_start = None;
            self.invalidate_all_highlights();
            true
        } else {
            false
        }
    }

    /// Get selected text
    pub fn selected_text(&self) -> Option<String> {
        let sel_start = self.selection_start?;
        let (start, end) = order_positions(sel_start, self.cursor);
        if start == end {
            return None;
        }

        if start.line == end.line {
            let line = &self.lines[start.line];
            let s = char_to_byte_pos(line, start.column);
            let e = char_to_byte_pos(line, end.column);
            Some(line[s..e].to_string())
        } else {
            let mut result = String::new();
            for line_idx in start.line..=end.line {
                let line = &self.lines[line_idx];
                if line_idx == start.line {
                    let s = char_to_byte_pos(line, start.column);
                    result.push_str(&line[s..]);
                } else if line_idx == end.line {
                    result.push('\n');
                    let e = char_to_byte_pos(line, end.column);
                    result.push_str(&line[..e]);
                } else {
                    result.push('\n');
                    result.push_str(line);
                }
            }
            Some(result)
        }
    }

    /// Select all text
    pub fn select_all(&mut self) {
        self.selection_start = Some(TextPosition::new(0, 0));
        let last_line = self.lines.len().saturating_sub(1);
        let last_col = self.lines.last().map(|l| l.chars().count()).unwrap_or(0);
        self.cursor = TextPosition::new(last_line, last_col);
    }

    // ========================================================================
    // Cursor movement
    // ========================================================================

    pub fn move_left(&mut self, select: bool) {
        if select && self.selection_start.is_none() {
            self.selection_start = Some(self.cursor);
        } else if !select {
            // Collapse selection to start
            if let Some(sel) = self.selection_start.take() {
                let (start, _) = order_positions(sel, self.cursor);
                self.cursor = start;
                return;
            }
        }
        if self.cursor.column > 0 {
            self.cursor.column -= 1;
        } else if self.cursor.line > 0 {
            self.cursor.line -= 1;
            self.cursor.column = self.lines[self.cursor.line].chars().count();
        }
    }

    pub fn move_right(&mut self, select: bool) {
        if select && self.selection_start.is_none() {
            self.selection_start = Some(self.cursor);
        } else if !select {
            if let Some(sel) = self.selection_start.take() {
                let (_, end) = order_positions(sel, self.cursor);
                self.cursor = end;
                return;
            }
        }
        if self.cursor.line < self.lines.len() {
            let line_len = self.lines[self.cursor.line].chars().count();
            if self.cursor.column < line_len {
                self.cursor.column += 1;
            } else if self.cursor.line + 1 < self.lines.len() {
                self.cursor.line += 1;
                self.cursor.column = 0;
            }
        }
    }

    pub fn move_up(&mut self, select: bool) {
        if select && self.selection_start.is_none() {
            self.selection_start = Some(self.cursor);
        } else if !select {
            self.selection_start = None;
        }
        if self.cursor.line > 0 {
            self.cursor.line -= 1;
            let line_len = self.lines[self.cursor.line].chars().count();
            self.cursor.column = self.cursor.column.min(line_len);
        }
    }

    pub fn move_down(&mut self, select: bool) {
        if select && self.selection_start.is_none() {
            self.selection_start = Some(self.cursor);
        } else if !select {
            self.selection_start = None;
        }
        if self.cursor.line + 1 < self.lines.len() {
            self.cursor.line += 1;
            let line_len = self.lines[self.cursor.line].chars().count();
            self.cursor.column = self.cursor.column.min(line_len);
        }
    }

    pub fn move_to_line_start(&mut self, select: bool) {
        if select && self.selection_start.is_none() {
            self.selection_start = Some(self.cursor);
        } else if !select {
            self.selection_start = None;
        }
        self.cursor.column = 0;
    }

    pub fn move_to_line_end(&mut self, select: bool) {
        if select && self.selection_start.is_none() {
            self.selection_start = Some(self.cursor);
        } else if !select {
            self.selection_start = None;
        }
        if self.cursor.line < self.lines.len() {
            self.cursor.column = self.lines[self.cursor.line].chars().count();
        }
    }

    pub fn move_word_left(&mut self, select: bool) {
        if select && self.selection_start.is_none() {
            self.selection_start = Some(self.cursor);
        } else if !select {
            self.selection_start = None;
        }
        if self.cursor.column == 0 && self.cursor.line > 0 {
            self.cursor.line -= 1;
            self.cursor.column = self.lines[self.cursor.line].chars().count();
        } else if self.cursor.line < self.lines.len() {
            self.cursor.column =
                text_edit::word_boundary_left(&self.lines[self.cursor.line], self.cursor.column);
        }
    }

    pub fn move_word_right(&mut self, select: bool) {
        if select && self.selection_start.is_none() {
            self.selection_start = Some(self.cursor);
        } else if !select {
            self.selection_start = None;
        }
        if self.cursor.line < self.lines.len() {
            let line_len = self.lines[self.cursor.line].chars().count();
            if self.cursor.column >= line_len && self.cursor.line + 1 < self.lines.len() {
                self.cursor.line += 1;
                self.cursor.column = 0;
            } else {
                self.cursor.column = text_edit::word_boundary_right(
                    &self.lines[self.cursor.line],
                    self.cursor.column,
                );
            }
        }
    }

    /// Position cursor from click coordinates
    pub fn cursor_from_click(&mut self, x: f32, y: f32) {
        let line_height = self.config.font_size * self.config.line_height;
        let line_idx = ((y / line_height).floor() as usize).min(self.lines.len().saturating_sub(1));

        let line = &self.lines[line_idx];
        let char_count = line.chars().count();
        let mut best_col = char_count;
        for col in 0..=char_count {
            let text_before: String = line.chars().take(col).collect();
            let w = self.measure_mono(&text_before);
            if w >= x {
                if col > 0 {
                    let prev: String = line.chars().take(col - 1).collect();
                    let prev_w = self.measure_mono(&prev);
                    best_col = if (x - prev_w).abs() < (x - w).abs() {
                        col - 1
                    } else {
                        col
                    };
                } else {
                    best_col = 0;
                }
                break;
            }
        }

        self.cursor = TextPosition::new(line_idx, best_col);
        self.selection_start = None;
    }

    /// Total content height in pixels
    pub fn content_height(&self) -> f32 {
        let line_height = self.config.font_size * self.config.line_height;
        self.lines.len() as f32 * line_height
    }

    /// Ensure cursor is within the visible scroll viewport
    pub fn ensure_cursor_visible(&mut self) {
        let line_height = self.config.font_size * self.config.line_height;
        if self.viewport_height <= 0.0 {
            return;
        }

        let cursor_y = self.cursor.line as f32 * line_height;
        let cursor_bottom = cursor_y + line_height;

        let mut physics = self.scroll_physics.lock().unwrap();
        let current_offset = -physics.offset_y;

        let mut new_offset = current_offset;
        if cursor_y < current_offset {
            new_offset = cursor_y;
        }
        if cursor_bottom > current_offset + self.viewport_height {
            new_offset = cursor_bottom - self.viewport_height;
        }

        let max_scroll = (self.content_height() - self.viewport_height).max(0.0);
        new_offset = new_offset.clamp(0.0, max_scroll);
        physics.offset_y = -new_offset;
    }

    /// Reset cursor blink
    pub fn reset_cursor_blink(&self) {
        if let Ok(mut cs) = self.cursor_state.lock() {
            cs.reset_blink();
        }
    }

    /// Get styled content with syntax highlighting (uses per-line cache)
    fn get_styled_content(&mut self) -> StyledText {
        self.sync_highlight_cache();

        if let Some(ref highlighter) = self.highlighter {
            let mut styled_lines = Vec::with_capacity(self.lines.len());
            for (i, line) in self.lines.iter().enumerate() {
                if let Some(ref cached) = self.highlight_cache[i] {
                    styled_lines.push(cached.clone());
                } else {
                    // Highlight single line
                    let line_styled = highlighter.highlight(line);
                    let styled_line = line_styled.lines.into_iter().next().unwrap_or_else(|| {
                        crate::styled_text::StyledLine {
                            text: line.clone(),
                            spans: Vec::new(),
                        }
                    });
                    self.highlight_cache[i] = Some(styled_line.clone());
                    styled_lines.push(styled_line);
                }
            }
            StyledText {
                lines: styled_lines,
            }
        } else {
            StyledText::plain(&self.value(), self.config.text_color)
        }
    }
}

/// Convert character position to byte position
fn char_to_byte_pos(s: &str, char_pos: usize) -> usize {
    s.char_indices()
        .nth(char_pos)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}

/// Order two positions (earlier, later)
fn order_positions(a: TextPosition, b: TextPosition) -> (TextPosition, TextPosition) {
    if a.line < b.line || (a.line == b.line && a.column <= b.column) {
        (a, b)
    } else {
        (b, a)
    }
}

// ============================================================================
// Read-only Code Widget
// ============================================================================

/// Read-only code block widget
///
/// For editable code, use `code_editor()` instead.
pub struct Code {
    inner: Div,
    content: String,
    config: CodeConfig,
    highlighter: Option<Arc<dyn SyntaxHighlighter>>,
}

impl Deref for Code {
    type Target = Div;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for Code {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl Code {
    pub fn new(content: impl Into<String>) -> Self {
        let content = content.into();
        let config = CodeConfig::default();
        let mut code = Self {
            inner: Div::new(),
            content,
            config,
            highlighter: None,
        };
        code.rebuild_inner();
        code
    }

    fn rebuild_inner(&mut self) {
        self.inner = self.create_visual_structure();
    }

    pub fn line_numbers(mut self, enabled: bool) -> Self {
        self.config.line_numbers = enabled;
        self.rebuild_inner();
        self
    }

    pub fn syntax(mut self, config: SyntaxConfig) -> Self {
        let bg_color = config.highlighter().background_color();
        let text_color = config.highlighter().default_color();
        let line_number_color = config.highlighter().line_number_color();
        self.highlighter = Some(config.into_arc());
        self.config.bg_color = bg_color;
        self.config.text_color = text_color;
        self.config.line_number_color = line_number_color;
        self.rebuild_inner();
        self
    }

    pub fn font_size(mut self, size: f32) -> Self {
        self.config.font_size = size;
        self.rebuild_inner();
        self
    }

    pub fn line_height(mut self, multiplier: f32) -> Self {
        self.config.line_height = multiplier;
        self.rebuild_inner();
        self
    }

    pub fn padding(mut self, padding: f32) -> Self {
        self.config.padding = padding;
        self.rebuild_inner();
        self
    }

    pub fn code_bg(mut self, color: Color) -> Self {
        self.config.bg_color = color;
        self
    }

    pub fn text_color(mut self, color: Color) -> Self {
        self.config.text_color = color;
        self
    }

    fn get_styled_content(&self) -> StyledText {
        if let Some(ref highlighter) = self.highlighter {
            highlighter.highlight(&self.content)
        } else {
            StyledText::plain(&self.content, self.config.text_color)
        }
    }

    fn create_visual_structure(&self) -> Div {
        let styled = self.get_styled_content();
        let line_height_px = self.config.font_size * self.config.line_height;
        let num_lines = styled.line_count().max(1);

        let mut container = div()
            .flex_row()
            .bg(self.config.bg_color)
            .rounded(self.config.corner_radius)
            .overflow_clip();

        if self.config.line_numbers {
            container = container.child(build_gutter(num_lines, line_height_px, &self.config));
        }

        let mut code_area = div()
            .flex_col()
            .flex_grow()
            .padding_x_px(self.config.padding)
            .padding_y_px(self.config.padding);

        for styled_line in &styled.lines {
            code_area =
                code_area.child(build_styled_line(styled_line, &self.config, line_height_px));
        }

        container.child(code_area)
    }

    // Shadowed Div methods
    pub fn w(mut self, px: f32) -> Self {
        self.inner = std::mem::take(&mut self.inner).w(px);
        self
    }
    pub fn h(mut self, px: f32) -> Self {
        self.inner = std::mem::take(&mut self.inner).h(px);
        self
    }
    pub fn w_full(mut self) -> Self {
        self.inner = std::mem::take(&mut self.inner).w_full();
        self
    }
    pub fn rounded(mut self, radius: f32) -> Self {
        self.config.corner_radius = radius;
        self
    }
    pub fn border(mut self, width: f32, color: Color) -> Self {
        self.inner = std::mem::take(&mut self.inner).border(width, color);
        self
    }
    pub fn m(mut self, value: f32) -> Self {
        self.inner = std::mem::take(&mut self.inner).m(value);
        self
    }
    pub fn mt(mut self, value: f32) -> Self {
        self.inner = std::mem::take(&mut self.inner).mt(value);
        self
    }
    pub fn mb(mut self, value: f32) -> Self {
        self.inner = std::mem::take(&mut self.inner).mb(value);
        self
    }
}

impl ElementBuilder for Code {
    fn build(&self, tree: &mut LayoutTree) -> LayoutNodeId {
        self.inner.build(tree)
    }
    fn render_props(&self) -> RenderProps {
        self.inner.render_props()
    }
    fn children_builders(&self) -> &[Box<dyn ElementBuilder>] {
        self.inner.children_builders()
    }
    fn element_type_id(&self) -> ElementTypeId {
        ElementTypeId::Div
    }
    fn semantic_type_name(&self) -> Option<&'static str> {
        Some("code")
    }
    fn event_handlers(&self) -> Option<&crate::event_handler::EventHandlers> {
        ElementBuilder::event_handlers(&self.inner)
    }
    fn layout_style(&self) -> Option<&taffy::Style> {
        self.inner.layout_style()
    }
}

// ============================================================================
// Editable Code Editor Widget (Stateful, incremental updates)
// ============================================================================

/// Editable code editor widget using Stateful for incremental updates
pub struct CodeEditor {
    inner: crate::stateful::Stateful<crate::stateful::TextFieldState>,
    state: SharedCodeEditorState,
}

impl CodeEditor {
    pub fn new(state: &SharedCodeEditorState) -> Self {
        use crate::stateful::{
            refresh_stateful, SharedState, StateTransitions, Stateful, StatefulInner,
            TextFieldState,
        };
        use blinc_core::events::event_types;

        let shared_state: SharedState<TextFieldState> =
            Arc::new(Mutex::new(StatefulInner::new(TextFieldState::Idle)));

        let data_for_click = Arc::clone(state);
        let data_for_drag = Arc::clone(state);
        let data_for_key = Arc::clone(state);
        let data_for_text = Arc::clone(state);
        let shared_for_click = Arc::clone(&shared_state);
        let shared_for_drag = Arc::clone(&shared_state);
        let shared_for_key = Arc::clone(&shared_state);
        let shared_for_text = Arc::clone(&shared_state);

        let mut inner = Stateful::with_shared_state(Arc::clone(&shared_state))
            .on_mouse_down(move |ctx| {
                let click_x = ctx.local_x;
                let click_y = ctx.local_y;

                {
                    let mut d = match data_for_click.lock() {
                        Ok(d) => d,
                        Err(_) => return,
                    };

                    // Focus via FSM
                    {
                        let mut shared = shared_for_click.lock().unwrap();
                        if !shared.state.is_focused() {
                            if let Some(new_state) = shared
                                .state
                                .on_event(event_types::POINTER_DOWN)
                                .or_else(|| shared.state.on_event(event_types::FOCUS))
                            {
                                shared.state = new_state;
                                shared.needs_visual_update = true;
                            }
                        }
                    }

                    if !d.focused {
                        d.focused = true;
                        increment_focus_count();
                        request_continuous_redraw_pub();
                    }

                    // Account for gutter, padding, and scroll in click coordinates
                    let gutter_offset = if d.config.line_numbers {
                        d.config.gutter_width
                    } else {
                        0.0
                    };
                    let adjusted_x = (click_x - gutter_offset - d.config.padding).max(0.0);
                    // Add scroll offset: local_y is viewport-relative, we need content-relative
                    let scroll_offset = d.scroll_physics.lock().map(|p| -p.offset_y).unwrap_or(0.0);
                    let adjusted_y = (click_y - d.config.padding + scroll_offset).max(0.0);
                    d.cursor_from_click(adjusted_x, adjusted_y);
                    // Save click position as drag selection anchor
                    // Will be cleared by shift-arrow if no drag occurs
                    d.drag_anchor = Some(d.cursor);
                    d.reset_cursor_blink();
                }

                refresh_stateful(&shared_for_click);
            })
            .on_event(event_types::DRAG, move |ctx| {
                let mut d = match data_for_drag.lock() {
                    Ok(d) => d,
                    Err(_) => return,
                };
                if !d.focused {
                    return;
                }

                let gutter_offset = if d.config.line_numbers {
                    d.config.gutter_width
                } else {
                    0.0
                };
                let adjusted_x = (ctx.local_x - gutter_offset - d.config.padding).max(0.0);
                let scroll_offset = d.scroll_physics.lock().map(|p| -p.offset_y).unwrap_or(0.0);
                let adjusted_y = (ctx.local_y - d.config.padding + scroll_offset).max(0.0);

                // Move cursor to drag position (selection_start stays at mouse-down position)
                let line_height = d.config.font_size * d.config.line_height;
                let line_idx = ((adjusted_y / line_height).floor() as usize)
                    .min(d.lines.len().saturating_sub(1));
                let line = d.lines[line_idx].clone();
                let char_count = line.chars().count();
                let mut best_col = char_count;
                for col in 0..=char_count {
                    let text_before: String = line.chars().take(col).collect();
                    let w = d.measure_mono(&text_before);
                    if w >= adjusted_x {
                        if col > 0 {
                            let prev: String = line.chars().take(col - 1).collect();
                            let prev_w = d.measure_mono(&prev);
                            best_col = if (adjusted_x - prev_w).abs() < (adjusted_x - w).abs() {
                                col - 1
                            } else {
                                col
                            };
                        } else {
                            best_col = 0;
                        }
                        break;
                    }
                }
                d.cursor = TextPosition::new(line_idx, best_col);

                // Set selection from drag anchor to current cursor
                if let Some(anchor) = d.drag_anchor {
                    if anchor != d.cursor {
                        d.selection_start = Some(anchor);
                    } else {
                        d.selection_start = None;
                    }
                }

                drop(d);
                crate::stateful::refresh_stateful(&shared_for_drag);
            })
            .on_event(event_types::TEXT_INPUT, move |ctx| {
                let needs_refresh = {
                    let mut d = match data_for_text.lock() {
                        Ok(d) => d,
                        Err(_) => return,
                    };
                    if !d.focused {
                        return;
                    }
                    // Skip control characters and modifier combos (Cmd+C/V/Z etc.)
                    if ctx.meta || ctx.ctrl {
                        return;
                    }
                    if let Some(c) = ctx.key_char {
                        if c.is_control() && c != '\t' {
                            return;
                        }
                        d.insert(&c.to_string());
                        d.reset_cursor_blink();
                        d.ensure_cursor_visible();
                        if let Some(ref cb) = d.on_change {
                            cb(&d.value());
                        }
                        true
                    } else {
                        false
                    }
                };
                if needs_refresh {
                    refresh_stateful(&shared_for_text);
                }
            })
            .on_key_down(move |ctx| {
                let needs_refresh = {
                    let mut d = match data_for_key.lock() {
                        Ok(d) => d,
                        Err(_) => return,
                    };
                    if !d.focused {
                        return;
                    }

                    let mut cursor_changed = true;
                    let mut text_changed = false;
                    let mut needs_visual_refresh = false;

                    // Platform modifier: Cmd on macOS, Ctrl elsewhere
                    let mod_key = ctx.meta || ctx.ctrl;

                    match ctx.key_code {
                        8 => {
                            d.delete_backward();
                            text_changed = true;
                        }
                        127 => {
                            d.delete_forward();
                            text_changed = true;
                        }
                        13 => {
                            d.insert("\n");
                            text_changed = true;
                        }
                        37 => {
                            // Left
                            if mod_key {
                                d.move_word_left(ctx.shift);
                            } else {
                                d.move_left(ctx.shift);
                            }
                        }
                        39 => {
                            // Right
                            if mod_key {
                                d.move_word_right(ctx.shift);
                            } else {
                                d.move_right(ctx.shift);
                            }
                        }
                        38 => d.move_up(ctx.shift),
                        40 => d.move_down(ctx.shift),
                        36 => d.move_to_line_start(ctx.shift),
                        35 => d.move_to_line_end(ctx.shift),
                        9 => {
                            d.insert("    ");
                            text_changed = true;
                        }
                        _ => {
                            // Check for Cmd+key combos
                            if mod_key {
                                match ctx.key_code {
                                    // A = Select All (visual refresh only, no scroll)
                                    65 => {
                                        d.select_all();
                                        cursor_changed = false;
                                        needs_visual_refresh = true;
                                    }
                                    // C = Copy
                                    67 => {
                                        if let Some(selected) = d.selected_text() {
                                            text_edit::clipboard_write(&selected);
                                        }
                                        cursor_changed = false;
                                    }
                                    // X = Cut
                                    88 => {
                                        if let Some(selected) = d.selected_text() {
                                            text_edit::clipboard_write(&selected);
                                            d.delete_selection();
                                            text_changed = true;
                                        }
                                    }
                                    // V = Paste
                                    86 => {
                                        if let Some(clip) = text_edit::clipboard_read() {
                                            d.insert(&clip);
                                            text_changed = true;
                                        }
                                    }
                                    // Z = Undo / Shift+Z = Redo
                                    90 => {
                                        if ctx.shift {
                                            d.redo();
                                        } else {
                                            d.undo();
                                        }
                                        text_changed = true;
                                    }
                                    _ => {
                                        cursor_changed = false;
                                    }
                                }
                            } else {
                                cursor_changed = false;
                            }
                        }
                    }

                    if cursor_changed {
                        d.reset_cursor_blink();
                        d.ensure_cursor_visible();
                    }
                    if text_changed {
                        if let Some(ref cb) = d.on_change {
                            cb(&d.value());
                        }
                    }
                    cursor_changed || text_changed || needs_visual_refresh
                };
                if needs_refresh {
                    refresh_stateful(&shared_for_key);
                }
            })
            .on_blur(move |_ctx| {
                // Blur handled by FSM transition
            })
            .overflow_clip()
            .cursor_text();

        // Register the state callback that builds visual content
        {
            let data_for_callback = Arc::clone(state);
            let mut shared = shared_state.lock().unwrap();
            shared.state_callback = Some(Arc::new(
                move |visual: &crate::stateful::TextFieldState, container: &mut Div| {
                    let mut data = data_for_callback.lock().unwrap();
                    let cw = data.char_width();
                    let content = build_editor_content(&mut data, visual.is_focused(), cw);
                    container.set_child(content);

                    // Apply visual styling
                    container.set_bg(data.config.bg_color);
                    container.set_rounded(data.config.corner_radius);
                    container.set_overflow_clip(true);
                },
            ));
            shared.needs_visual_update = true;
        }

        inner.ensure_state_handlers_registered();

        Self {
            inner,
            state: Arc::clone(state),
        }
    }

    // Builder methods that update shared state config
    pub fn line_numbers(self, enabled: bool) -> Self {
        self.state.lock().unwrap().config.line_numbers = enabled;
        self
    }

    pub fn syntax(self, syntax_config: SyntaxConfig) -> Self {
        let bg = syntax_config.highlighter().background_color();
        let text_col = syntax_config.highlighter().default_color();
        let ln = syntax_config.highlighter().line_number_color();
        let hl = syntax_config.into_arc();
        {
            let mut d = self.state.lock().unwrap();
            d.highlighter = Some(hl);
            d.config.bg_color = bg;
            d.config.text_color = text_col;
            d.config.line_number_color = ln;
        }
        self
    }

    pub fn font_size(self, size: f32) -> Self {
        self.state.lock().unwrap().config.font_size = size;
        self
    }

    pub fn line_height(self, multiplier: f32) -> Self {
        self.state.lock().unwrap().config.line_height = multiplier;
        self
    }

    pub fn padding(self, padding: f32) -> Self {
        self.state.lock().unwrap().config.padding = padding;
        self
    }

    pub fn on_change<F: Fn(&str) + Send + Sync + 'static>(self, callback: F) -> Self {
        self.state.lock().unwrap().on_change = Some(Arc::new(callback));
        self
    }

    pub fn code_bg(self, color: Color) -> Self {
        self.state.lock().unwrap().config.bg_color = color;
        self
    }

    pub fn text_color(self, color: Color) -> Self {
        self.state.lock().unwrap().config.text_color = color;
        self
    }

    // Shadowed Div methods
    pub fn w(mut self, px: f32) -> Self {
        self.inner = self.inner.w(px);
        self
    }
    pub fn h(mut self, px: f32) -> Self {
        self.inner = self.inner.h(px);
        // Update viewport height for scroll calculations
        let (vh, physics) = {
            let mut d = self.state.lock().unwrap();
            let vh = (px - d.config.padding * 2.0).max(0.0);
            d.viewport_height = vh;
            (vh, Arc::clone(&d.scroll_physics))
        };
        if let Ok(mut p) = physics.lock() {
            p.viewport_height = vh;
        }
        self
    }
    pub fn w_full(mut self) -> Self {
        self.inner = self.inner.w_full();
        self
    }
    pub fn border(mut self, width: f32, color: Color) -> Self {
        self.inner = self.inner.border(width, color);
        self
    }
    pub fn rounded(self, radius: f32) -> Self {
        self.state.lock().unwrap().config.corner_radius = radius;
        self
    }
    pub fn m(mut self, value: f32) -> Self {
        self.inner = self.inner.m(value);
        self
    }
    pub fn mt(mut self, value: f32) -> Self {
        self.inner = self.inner.mt(value);
        self
    }
    pub fn mb(mut self, value: f32) -> Self {
        self.inner = self.inner.mb(value);
        self
    }
}

impl ElementBuilder for CodeEditor {
    fn build(&self, tree: &mut LayoutTree) -> LayoutNodeId {
        self.inner.build(tree)
    }
    fn render_props(&self) -> RenderProps {
        self.inner.render_props()
    }
    fn children_builders(&self) -> &[Box<dyn ElementBuilder>] {
        self.inner.children_builders()
    }
    fn element_type_id(&self) -> ElementTypeId {
        ElementTypeId::Div
    }
    fn semantic_type_name(&self) -> Option<&'static str> {
        Some("code-editor")
    }
    fn event_handlers(&self) -> Option<&crate::event_handler::EventHandlers> {
        ElementBuilder::event_handlers(&self.inner)
    }
    fn layout_style(&self) -> Option<&taffy::Style> {
        self.inner.layout_style()
    }
}

// ============================================================================
// Shared Visual Building Helpers
// ============================================================================

fn build_gutter(num_lines: usize, line_height_px: f32, config: &CodeConfig) -> Div {
    let mut line_numbers_col = div()
        .flex_col()
        .padding_y_px(config.padding)
        .padding_x_px(8.0);

    for line_num in 1..=num_lines {
        line_numbers_col = line_numbers_col.child(
            div()
                .h(line_height_px)
                .flex_row()
                .justify_end()
                .items_center()
                .child(
                    text(format!("{}", line_num))
                        .size(config.font_size)
                        .color(config.line_number_color)
                        .text_right(),
                ),
        );
    }

    div()
        .flex_row()
        .bg(config.gutter_bg_color)
        .w(config.gutter_width)
        .child(line_numbers_col.flex_grow())
        .child(div().w(1.0).h_full().bg(config.gutter_separator_color))
}

fn build_styled_line(
    styled_line: &crate::styled_text::StyledLine,
    config: &CodeConfig,
    line_height_px: f32,
) -> Div {
    let mut line_div = div().h(line_height_px).flex_row().items_center();

    if styled_line.spans.is_empty() {
        line_div = line_div.child(text(" ").size(config.font_size).color(config.text_color));
    } else {
        for span in &styled_line.spans {
            let span_text = &styled_line.text[span.start..span.end];
            let mut txt = text(span_text)
                .size(config.font_size)
                .color(span.color)
                .no_wrap();
            if span.bold {
                txt = txt.bold();
            }
            txt = txt.monospace();
            line_div = line_div.child(txt);
        }
    }

    line_div
}

/// Build the visual content for the editable code editor
fn build_editor_content(data: &mut CodeEditorData, is_focused: bool, char_width: f32) -> Div {
    let styled = data.get_styled_content();
    let config = &data.config;
    let line_height_px = config.font_size * config.line_height;
    let num_lines = styled.line_count().max(1);

    let mut container = div().flex_row().w_full().h_full().overflow_clip();

    if config.line_numbers {
        container = container.child(build_gutter(num_lines, line_height_px, config));
    }

    let pad = config.padding;
    let mut code_area = div().flex_col().flex_grow().relative();

    // Selection highlights (behind text)
    if let Some(sel_start) = data.selection_start {
        let (start, end) = order_positions(sel_start, data.cursor);
        if start != end {
            let sel_color = config.selection_color;
            for line_idx in start.line..=end.line {
                if line_idx >= data.lines.len() {
                    break;
                }
                let line_text = &data.lines[line_idx];
                let line_char_count = line_text.chars().count();
                let col_start = if line_idx == start.line {
                    start.column
                } else {
                    0
                };
                let col_end = if line_idx == end.line {
                    end.column
                } else {
                    line_char_count
                };

                let x_start = if col_start > 0 {
                    let before: String = line_text.chars().take(col_start).collect();
                    data.measure_mono(&before)
                } else {
                    0.0
                };
                let x_end = if col_end > 0 {
                    let before: String = line_text.chars().take(col_end).collect();
                    data.measure_mono(&before)
                } else {
                    0.0
                };
                let width = if col_end == line_char_count && line_idx != end.line {
                    (x_end - x_start) + config.font_size * 0.5
                } else {
                    x_end - x_start
                };

                if width > 0.0 {
                    let sel_top = line_idx as f32 * line_height_px + pad;
                    code_area = code_area.child(
                        div()
                            .absolute()
                            .left(x_start + pad)
                            .top(sel_top)
                            .w(width)
                            .h(line_height_px)
                            .bg(sel_color)
                            .rounded(2.0),
                    );
                }
            }
        }
    }

    // Text lines in a padded wrapper
    let mut text_wrapper = div().flex_col().padding_x_px(pad).padding_y_px(pad);
    for styled_line in &styled.lines {
        text_wrapper = text_wrapper.child(build_styled_line(styled_line, config, line_height_px));
    }
    code_area = code_area.child(text_wrapper);

    // Cursor (offset by padding to align with text)
    if is_focused {
        let cursor_height = config.font_size * 1.2;
        let cursor_line = data.cursor.line;
        let cursor_col = data.cursor.column;

        let cursor_x = if cursor_col > 0 && cursor_line < data.lines.len() {
            let text_before: String = data.lines[cursor_line].chars().take(cursor_col).collect();
            data.measure_mono(&text_before) + pad
        } else {
            pad
        };

        let cursor_top =
            (cursor_line as f32 * line_height_px) + (line_height_px - cursor_height) / 2.0 + pad;

        let cursor_state_clone = Arc::clone(&data.cursor_state);
        let cursor_color = config.cursor_color;

        {
            if let Ok(mut cs) = cursor_state_clone.lock() {
                cs.visible = true;
                cs.color = cursor_color;
                cs.x = cursor_x;
                cs.animation = CursorAnimation::SmoothFade;
            }
        }

        let cursor_state_for_canvas = Arc::clone(&data.cursor_state);
        let cursor_canvas = canvas(
            move |ctx: &mut dyn blinc_core::DrawContext, bounds: crate::canvas::CanvasBounds| {
                let cs = cursor_state_for_canvas.lock().unwrap();
                if !cs.visible {
                    return;
                }
                let opacity = cs.current_opacity();
                if opacity < 0.01 {
                    return;
                }
                let color = Color::rgba(
                    cursor_color.r,
                    cursor_color.g,
                    cursor_color.b,
                    cursor_color.a * opacity,
                );
                ctx.fill_rect(
                    Rect::new(0.0, 0.0, bounds.width, bounds.height),
                    CornerRadius::default(),
                    Brush::Solid(color),
                );
            },
        )
        .absolute()
        .top(cursor_top)
        .left(cursor_x)
        .w(2.0)
        .h(cursor_height);

        code_area = code_area.child(cursor_canvas);
    }

    // Wrap code_area in a Scroll container for vertical scrolling
    let scrollable = Scroll::with_physics(Arc::clone(&data.scroll_physics))
        .direction(ScrollDirection::Vertical)
        .no_bounce()
        .flex_grow()
        .child(code_area);

    container.child(scrollable)
}

// ============================================================================
// Convenience Constructors
// ============================================================================

/// Create a read-only code block
pub fn code(content: impl Into<String>) -> Code {
    Code::new(content)
}

/// Create a preformatted text block (alias for code)
pub fn pre(content: impl Into<String>) -> Code {
    Code::new(content)
}

/// Create an editable code editor widget
pub fn code_editor(state: &SharedCodeEditorState) -> CodeEditor {
    CodeEditor::new(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Once;

    static THEME_INIT: Once = Once::new();

    fn ensure_theme_initialized() {
        THEME_INIT.call_once(ThemeState::init_default);
    }

    #[test]
    fn test_code_creation() {
        ensure_theme_initialized();
        let c = code("fn main() {}");
        assert!(!c.config.line_numbers);
    }

    #[test]
    fn test_code_builder() {
        ensure_theme_initialized();
        let c = code("let x = 42;")
            .line_numbers(true)
            .font_size(14.0)
            .rounded(12.0);

        assert!(c.config.line_numbers);
        assert_eq!(c.config.font_size, 14.0);
        assert_eq!(c.config.corner_radius, 12.0);
    }

    #[test]
    fn test_editor_state_insert() {
        ensure_theme_initialized();
        let state = code_editor_state("hello");
        {
            let mut d = state.lock().unwrap();
            d.cursor = TextPosition::new(0, 5);
            d.insert(" world");
            assert_eq!(d.value(), "hello world");
        }
    }

    #[test]
    fn test_editor_state_newline() {
        ensure_theme_initialized();
        let state = code_editor_state("hello world");
        {
            let mut d = state.lock().unwrap();
            d.cursor = TextPosition::new(0, 5);
            d.insert("\n");
            assert_eq!(d.lines.len(), 2);
            assert_eq!(d.lines[0], "hello");
            assert_eq!(d.lines[1], " world");
        }
    }

    #[test]
    fn test_editor_state_undo_redo() {
        ensure_theme_initialized();
        let state = code_editor_state("hello");
        {
            let mut d = state.lock().unwrap();
            d.cursor = TextPosition::new(0, 5);
            d.insert(" world");
            assert_eq!(d.value(), "hello world");
            d.undo();
            assert_eq!(d.value(), "hello");
            d.redo();
            assert_eq!(d.value(), "hello world");
        }
    }

    #[test]
    fn test_editor_state_select_all() {
        ensure_theme_initialized();
        let state = code_editor_state("line1\nline2");
        {
            let mut d = state.lock().unwrap();
            d.select_all();
            assert_eq!(d.selected_text(), Some("line1\nline2".to_string()));
        }
    }

    #[test]
    fn test_editor_state_word_nav() {
        ensure_theme_initialized();
        let state = code_editor_state("hello world");
        {
            let mut d = state.lock().unwrap();
            d.cursor = TextPosition::new(0, 0);
            d.move_word_right(false);
            assert_eq!(d.cursor.column, 6); // after "hello "
            d.move_word_left(false);
            assert_eq!(d.cursor.column, 0);
        }
    }
}
