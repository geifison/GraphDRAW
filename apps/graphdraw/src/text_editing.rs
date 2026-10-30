//! In-place text editing: caret, selection, typing, deleting and
//! character formatting of the text object being edited with the Text
//! tool. Positions are character indices of the concatenated span text
//! (`graphdraw_core::spans_text`); geometry comes from the text layout's
//! line boxes.

use crate::app::App;
use graphdraw_core::document::text_outline::TextRequest;
use graphdraw_core::geometry::{Affine, Point, Rect};
use graphdraw_core::{
    spans_apply, spans_char_count, spans_delete, spans_insert, spans_text, Command, ShapeId,
    ShapeKind, TextSpan,
};
use graphdraw_text::TextLayout;

/// The text object being edited and where the caret is.
#[derive(Debug, Clone)]
pub struct TextEdit {
    pub shape: ShapeId,
    /// Caret position: a character index of the concatenated spans.
    pub caret: usize,
    /// The other end of the selection; equals `caret` when nothing is
    /// selected.
    pub anchor: usize,
    /// Horizontal position remembered across Up/Down moves (layout space).
    pub goal_x: Option<f64>,
}

impl TextEdit {
    pub fn at_end(shape: ShapeId, len: usize) -> Self {
        TextEdit {
            shape,
            caret: len,
            anchor: len,
            goal_x: None,
        }
    }

    /// The selected range, ordered.
    pub fn selection(&self) -> (usize, usize) {
        (self.caret.min(self.anchor), self.caret.max(self.anchor))
    }

    pub fn has_selection(&self) -> bool {
        self.caret != self.anchor
    }
}

/// Caret and selection move requests from the keyboard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaretMove {
    Left,
    Right,
    WordLeft,
    WordRight,
    LineStart,
    LineEnd,
    Up,
    Down,
    TextStart,
    TextEnd,
}

impl App {
    /// The spans of the text being edited.
    pub fn edit_spans(&self) -> Option<Vec<TextSpan>> {
        let te = self.text_edit.as_ref()?;
        match self
            .doc()
            .find_shape(te.shape)
            .map(|s| crate::clip_frames::text_view(s.clone()).kind)
        {
            Some(ShapeKind::Text { spans, .. }) => Some(spans),
            _ => None,
        }
    }

    /// The text being edited, concatenated.
    pub fn edit_text(&self) -> String {
        self.edit_spans()
            .map(|s| spans_text(&s))
            .unwrap_or_default()
    }

    /// Layout of a text object and the transform from layout space to the
    /// page (the shape's transform after the origin or frame shift).
    pub fn text_layout_of(&self, id: ShapeId) -> Option<(TextLayout, Affine)> {
        let s = crate::clip_frames::text_view(self.doc().find_shape(id)?.clone());
        layout_of_text_shape(&s, Affine::IDENTITY)
    }

    /// Replace the spans of the text being edited, as one undo step.
    pub fn set_edit_spans(&mut self, spans: Vec<TextSpan>) {
        let Some(te) = self.text_edit.clone() else {
            return;
        };
        let Some(s) = self
            .doc()
            .find_shape(te.shape)
            .cloned()
            .map(crate::clip_frames::text_view)
        else {
            return;
        };
        let ShapeKind::Text {
            origin,
            frame,
            align,
            para,
            on_path,
            ..
        } = s.kind
        else {
            return;
        };
        let mut para = para;
        para.hyphenate = self.text_hyphenation;
        let kind = ShapeKind::Text {
            spans,
            origin,
            frame,
            align,
            para,
            on_path,
        };
        // Every keystroke is an undo step of its own: Ctrl+Z takes the
        // typing back letter by letter (GD-UND-01).
        let _ = self.engine.run_with_label(
            &Command::SetShapeKind {
                shape: te.shape,
                kind,
            },
            "Edit Text",
        );
        if self.is_linked_frame(te.shape) {
            self.reflow_chain(te.shape);
        }
        self.sync_text_defaults_from(te.shape);
    }

    /// After an undo or redo while typing: the caret goes where the text
    /// changed (`before` is the text before the step). A text whose
    /// creation was undone is no longer edited.
    pub fn text_after_history(&mut self, before: &str) {
        let Some(id) = self.text_edit.as_ref().map(|t| t.shape) else {
            return;
        };
        if self.doc().find_shape(id).is_none() {
            self.text_edit = None;
            return;
        }
        let b: Vec<char> = before.chars().collect();
        let a: Vec<char> = self.edit_text().chars().collect();
        let pre = b.iter().zip(&a).take_while(|(x, y)| x == y).count();
        let suf = b
            .iter()
            .rev()
            .zip(a.iter().rev())
            .take(b.len().min(a.len()) - pre)
            .take_while(|(x, y)| x == y)
            .count();
        self.set_caret(a.len() - suf, false);
        if let Some(te) = self.text_edit.as_mut() {
            te.goal_x = None;
        }
    }

    /// Place the caret at a character index (a click); Shift extends.
    pub fn text_set_caret_at(&mut self, idx: usize, extend: bool) {
        self.set_caret(idx, extend);
        if let Some(te) = self.text_edit.as_mut() {
            te.goal_x = None;
        }
        if let Some(id) = self.text_edit.as_ref().map(|t| t.shape) {
            self.sync_text_defaults_from(id);
        }
    }

    /// Keyboard input while a text is being edited.
    pub fn text_keyboard(&mut self, ctx: &egui::Context) {
        use egui::{Event, Key};
        let events = ctx.input(|i| i.events.clone());
        for ev in &events {
            // An input method composing text owns the keys until it
            // commits (Enter, Backspace and the arrows edit the
            // composition, not the text).
            if !self.ime_preedit.is_empty() && matches!(ev, Event::Key { .. }) {
                continue;
            }
            match ev {
                Event::Ime(ime) => {
                    if let Some(commit) = self.ime_event(ime) {
                        self.text_insert(&commit);
                    }
                }
                Event::Text(t) => self.text_insert(t),
                Event::Paste(t) => self.text_insert(t),
                Event::Copy => {
                    let sel = self.text_selected();
                    if !sel.is_empty() {
                        ctx.copy_text(sel);
                    }
                }
                Event::Cut => {
                    let sel = self.text_selected();
                    if !sel.is_empty() {
                        ctx.copy_text(sel);
                        self.text_delete_backward();
                    }
                }
                Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => {
                    let shift = modifiers.shift;
                    let ctrl = modifiers.command;
                    match key {
                        // Letter by letter, without leaving the text.
                        Key::Z if ctrl && shift => self.redo(),
                        Key::Z if ctrl => self.undo(),
                        Key::Num8 if ctrl => self.step_text_size(true),
                        Key::Num2 if ctrl => self.step_text_size(false),
                        Key::Backspace => self.text_delete_backward(),
                        Key::Delete => self.text_delete_forward(),
                        Key::Enter if !shift => self.text_insert("\n"),
                        Key::Enter => self.text_insert("\u{2028}"),
                        Key::Tab => self.text_insert("\t"),
                        Key::ArrowLeft if ctrl => self.text_move(CaretMove::WordLeft, shift),
                        Key::ArrowRight if ctrl => self.text_move(CaretMove::WordRight, shift),
                        Key::ArrowLeft => self.text_move(CaretMove::Left, shift),
                        Key::ArrowRight => self.text_move(CaretMove::Right, shift),
                        Key::ArrowUp => self.text_move(CaretMove::Up, shift),
                        Key::ArrowDown => self.text_move(CaretMove::Down, shift),
                        Key::Home if ctrl => self.text_move(CaretMove::TextStart, shift),
                        Key::End if ctrl => self.text_move(CaretMove::TextEnd, shift),
                        Key::Home => self.text_move(CaretMove::LineStart, shift),
                        Key::End => self.text_move(CaretMove::LineEnd, shift),
                        Key::A if ctrl => self.text_select_all(),
                        Key::B if ctrl => {
                            let on = !self.text_bold;
                            self.text_bold = on;
                            self.text_apply_style(move |s| s.bold = on);
                        }
                        Key::I if ctrl => {
                            let on = !self.text_italic;
                            self.text_italic = on;
                            self.text_apply_style(move |s| s.italic = on);
                        }
                        Key::U if ctrl => {
                            let on = !self.text_underline;
                            self.text_underline = on;
                            self.text_apply_style(move |s| s.underline = on);
                        }
                        Key::Escape => {
                            self.finish_text();
                            self.set_tool(crate::tools::Tool::Pick);
                            return;
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    /// Follow an input method: its composition is kept to show at the
    /// caret; the committed text is returned to be inserted.
    pub fn ime_event(&mut self, ime: &egui::ImeEvent) -> Option<String> {
        match ime {
            egui::ImeEvent::Preedit { text, .. } => {
                self.ime_preedit = text.clone();
                None
            }
            egui::ImeEvent::Commit(text) => {
                self.ime_preedit.clear();
                (!text.is_empty()).then(|| text.clone())
            }
            _ => None,
        }
    }

    fn set_caret(&mut self, caret: usize, extend: bool) {
        let len = spans_char_count(&self.edit_spans().unwrap_or_default());
        if let Some(te) = self.text_edit.as_mut() {
            te.caret = caret.min(len);
            if !extend {
                te.anchor = te.caret;
            }
        }
    }

    /// Type `s` at the caret, replacing the selection. Autocorrect acts
    /// on the word just finished when the caret is at the end.
    pub fn text_insert(&mut self, s: &str) {
        let Some(te) = self.text_edit.clone() else {
            return;
        };
        let Some(mut spans) = self.edit_spans() else {
            return;
        };
        let (a, b) = te.selection();
        spans_delete(&mut spans, a, b);
        spans_insert(&mut spans, a, s);
        let mut caret = a + s.chars().count();
        if self.settings.autocorrect.enabled {
            let typed: String = spans_text(&spans).chars().take(caret).collect();
            let style = crate::autocorrect::QuoteStyle::for_language(&crate::i18n::language());
            if let Some(fixed) =
                crate::autocorrect::on_typed(&typed, &self.settings.autocorrect, style)
            {
                // Replace from the first differing character.
                let common = typed
                    .chars()
                    .zip(fixed.chars())
                    .take_while(|(x, y)| x == y)
                    .count();
                let tail: String = fixed.chars().skip(common).collect();
                spans_delete(&mut spans, common, caret);
                spans_insert(&mut spans, common, &tail);
                caret = common + tail.chars().count();
            }
        }
        self.set_edit_spans(spans);
        self.set_caret(caret, false);
        if let Some(te) = self.text_edit.as_mut() {
            te.goal_x = None;
        }
    }

    /// Backspace: delete the selection or the character before the caret.
    pub fn text_delete_backward(&mut self) {
        let Some(te) = self.text_edit.clone() else {
            return;
        };
        let Some(mut spans) = self.edit_spans() else {
            return;
        };
        let (a, b) = if te.has_selection() {
            te.selection()
        } else if te.caret > 0 {
            (te.caret - 1, te.caret)
        } else {
            return;
        };
        spans_delete(&mut spans, a, b);
        self.set_edit_spans(spans);
        self.set_caret(a, false);
    }

    /// Delete: delete the selection or the character after the caret.
    pub fn text_delete_forward(&mut self) {
        let Some(te) = self.text_edit.clone() else {
            return;
        };
        let Some(mut spans) = self.edit_spans() else {
            return;
        };
        let len = spans_char_count(&spans);
        let (a, b) = if te.has_selection() {
            te.selection()
        } else if te.caret < len {
            (te.caret, te.caret + 1)
        } else {
            return;
        };
        spans_delete(&mut spans, a, b);
        self.set_edit_spans(spans);
        self.set_caret(a, false);
    }

    /// Move the caret; `extend` keeps the anchor (Shift).
    pub fn text_move(&mut self, m: CaretMove, extend: bool) {
        let Some(te) = self.text_edit.clone() else {
            return;
        };
        let text: Vec<char> = self.edit_text().chars().collect();
        let len = text.len();
        let (a, b) = te.selection();
        let mut goal_x = None;
        let caret = match m {
            CaretMove::Left => {
                if te.has_selection() && !extend {
                    a
                } else {
                    te.caret.saturating_sub(1)
                }
            }
            CaretMove::Right => {
                if te.has_selection() && !extend {
                    b
                } else {
                    (te.caret + 1).min(len)
                }
            }
            CaretMove::WordLeft => word_start_before(&text, te.caret),
            CaretMove::WordRight => word_end_after(&text, te.caret),
            CaretMove::TextStart => 0,
            CaretMove::TextEnd => len,
            CaretMove::LineStart | CaretMove::LineEnd => match self.text_layout_of(te.shape) {
                Some((layout, _)) => {
                    let line = layout.line_of(te.caret).and_then(|i| layout.lines.get(i));
                    match (line, m) {
                        (Some(l), CaretMove::LineStart) => l.start_char,
                        (Some(l), _) => l.end_char,
                        (None, CaretMove::LineStart) => 0,
                        (None, _) => len,
                    }
                }
                None => {
                    if m == CaretMove::LineStart {
                        0
                    } else {
                        len
                    }
                }
            },
            CaretMove::Up | CaretMove::Down => match self.text_layout_of(te.shape) {
                Some((layout, _)) => {
                    let Some(li) = layout.line_of(te.caret) else {
                        return;
                    };
                    let x = te
                        .goal_x
                        .or_else(|| layout.caret(te.caret).map(|c| c.0))
                        .unwrap_or(0.0);
                    goal_x = Some(x);
                    // The line above or below: parts of one line split
                    // around a wrap object share a baseline and count as
                    // one.
                    let here = layout.lines.get(li).map_or(0.0, |l| l.baseline);
                    let target = if m == CaretMove::Up {
                        layout
                            .lines
                            .get(..li)
                            .unwrap_or(&[])
                            .iter()
                            .rposition(|l| l.baseline > here + 1e-6)
                    } else {
                        layout
                            .lines
                            .iter()
                            .enumerate()
                            .skip(li + 1)
                            .find(|(_, l)| l.baseline < here - 1e-6)
                            .map(|(i, _)| i)
                    };
                    match target.and_then(|t| layout.lines.get(t)) {
                        Some(l) => layout
                            .hit_char(Point::new(x, l.baseline))
                            .unwrap_or(l.start_char),
                        None if m == CaretMove::Up => 0,
                        None => len,
                    }
                }
                None => return,
            },
        };
        self.set_caret(caret, extend);
        if let Some(te) = self.text_edit.as_mut() {
            te.goal_x = goal_x;
        }
    }

    pub fn text_select_all(&mut self) {
        let len = spans_char_count(&self.edit_spans().unwrap_or_default());
        if let Some(te) = self.text_edit.as_mut() {
            te.anchor = 0;
            te.caret = len;
        }
    }

    /// Select the word around the character index `idx` (double-click).
    pub fn text_select_word_at(&mut self, idx: usize) {
        let text: Vec<char> = self.edit_text().chars().collect();
        let idx = idx.min(text.len());
        let is_word = |c: char| c.is_alphanumeric() || c == '_' || c == '\'';
        let mut a = idx;
        while a > 0 && text.get(a - 1).is_some_and(|c| is_word(*c)) {
            a -= 1;
        }
        let mut b = idx;
        while text.get(b).is_some_and(|c| is_word(*c)) {
            b += 1;
        }
        if a == b {
            // Not on a word: select the single character.
            b = (idx + 1).min(text.len());
        }
        if let Some(te) = self.text_edit.as_mut() {
            te.anchor = a;
            te.caret = b;
        }
    }

    /// Character index under the page point `p` in the text being edited.
    pub fn text_hit_char(&self, p: Point) -> Option<usize> {
        let te = self.text_edit.as_ref()?;
        let (layout, to_page) = self.text_layout_of(te.shape)?;
        let local = to_page.inverse() * p;
        layout.hit_char(local)
    }

    /// Apply a style change to the selected characters, or to the whole
    /// text when nothing is selected.
    pub fn text_apply_style(&mut self, f: impl Fn(&mut TextSpan)) {
        let Some(te) = self.text_edit.clone() else {
            return;
        };
        let Some(mut spans) = self.edit_spans() else {
            return;
        };
        let (a, b) = if te.has_selection() {
            te.selection()
        } else {
            (0, spans_char_count(&spans))
        };
        if a == b {
            for sp in spans.iter_mut() {
                f(sp);
            }
        } else {
            spans_apply(&mut spans, a, b, f);
        }
        self.set_edit_spans(spans);
    }

    /// The selected text (empty without a selection).
    pub fn text_selected(&self) -> String {
        let Some(te) = self.text_edit.as_ref() else {
            return String::new();
        };
        let (a, b) = te.selection();
        self.edit_text().chars().skip(a).take(b - a).collect()
    }

    /// Screen-independent caret and selection geometry on the page: the
    /// caret segment and one rectangle per selected line.
    pub fn text_caret_geometry(&self) -> Option<(Point, Point, Vec<[Point; 4]>)> {
        let te = self.text_edit.as_ref()?;
        let (layout, to_page) = self.text_layout_of(te.shape)?;
        let size_pt = self
            .edit_spans()?
            .first()
            .map(|s| s.size_pt)
            .unwrap_or(12.0);
        caret_geometry_sized(&layout, &to_page, te.caret, te.selection(), size_pt)
    }
}

/// The page-space box of artistic text `s`'s lines from the font's
/// ascent to its descent, whatever letters they hold ("ac" is as tall as
/// "Tg"), so its selection box does not jump while it is typed. `None`
/// for other shapes and text without lines.
pub fn text_line_box(s: &graphdraw_core::Shape) -> Option<Rect> {
    let ShapeKind::Text {
        frame: None,
        on_path: None,
        ..
    } = &s.kind
    else {
        return None;
    };
    let (layout, to_page) = layout_of_text_shape(s, Affine::IDENTITY)?;
    layout
        .lines
        .iter()
        .flat_map(|line| {
            let x0 = line.edges.first().copied().unwrap_or(0.0);
            let x1 = line.edges.last().copied().unwrap_or(x0);
            let (y0, y1) = (line.baseline - line.descent, line.baseline + line.ascent);
            [
                Point::new(x0, y0),
                Point::new(x1, y0),
                Point::new(x1, y1),
                Point::new(x0, y1),
            ]
        })
        .map(|p| to_page * p)
        .fold(None, |r: Option<Rect>, p| {
            Some(r.map_or(Rect::from_points(p, p), |r| r.union_pt(p)))
        })
}

/// Layout of a text shape (any `ShapeKind::Text`, placed under `parent`)
/// and the transform from layout space to the page.
pub fn layout_of_text_shape(
    s: &graphdraw_core::Shape,
    parent: Affine,
) -> Option<(TextLayout, Affine)> {
    let ShapeKind::Text {
        spans,
        origin,
        frame,
        align,
        para,
        on_path,
    } = &s.kind
    else {
        return None;
    };
    let layout = graphdraw_text::fonts().layout(&TextRequest {
        spans,
        frame: *frame,
        align: *align,
        para,
        on_path: on_path.as_ref(),
    });
    let shift = match frame {
        Some(f) if on_path.is_none() => Affine::translate((origin.x, origin.y + f.height)),
        _ => Affine::translate(origin.to_vec2()),
    };
    Some((layout, parent * s.transform * shift))
}

/// Caret segment (top, bottom) and one quad per selected line, on the page.
pub fn caret_geometry(
    layout: &TextLayout,
    to_page: &Affine,
    caret: usize,
    selection: (usize, usize),
) -> Option<(Point, Point, Vec<[Point; 4]>)> {
    caret_geometry_sized(layout, to_page, caret, selection, 12.0)
}

fn caret_geometry_sized(
    layout: &TextLayout,
    to_page: &Affine,
    caret: usize,
    (a, b): (usize, usize),
    size_pt: f64,
) -> Option<(Point, Point, Vec<[Point; 4]>)> {
    if !layout.path_carets.is_empty() {
        return path_caret_geometry(layout, to_page, caret, (a, b));
    }
    let (x, base, asc, desc) = layout.caret(caret).unwrap_or_else(|| {
        // Empty text: a caret of the text size at the origin.
        let size_mm = size_pt * 25.4 / 72.0;
        (0.0, 0.0, size_mm * 0.8, size_mm * 0.2)
    });
    let top = *to_page * Point::new(x, base + asc);
    let bottom = *to_page * Point::new(x, base - desc);
    let mut quads = Vec::new();
    if a != b {
        for line in &layout.lines {
            let s = a.max(line.start_char);
            let e = b.min(line.end_char);
            if s > e || (s == e && !(a < line.start_char && b > line.end_char)) {
                continue;
            }
            let x0 = line.edges.get(s - line.start_char).copied().unwrap_or(0.0);
            let x1 = line.edges.get(e - line.start_char).copied().unwrap_or(x0);
            let r = Rect::new(
                x0,
                line.baseline - line.descent,
                x1.max(x0 + 0.3),
                line.baseline + line.ascent,
            );
            quads.push([
                *to_page * Point::new(r.x0, r.y0),
                *to_page * Point::new(r.x1, r.y0),
                *to_page * Point::new(r.x1, r.y1),
                *to_page * Point::new(r.x0, r.y1),
            ]);
        }
    }
    Some((top, bottom, quads))
}

/// Caret and selection of a text on a path: they follow the letters
/// along the curve (one quad per selected character).
fn path_caret_geometry(
    layout: &TextLayout,
    to_page: &Affine,
    caret: usize,
    (a, b): (usize, usize),
) -> Option<(Point, Point, Vec<[Point; 4]>)> {
    let (top, bottom) = layout.path_caret(caret)?;
    let mut quads = Vec::new();
    for line in &layout.lines {
        for c in a.max(line.start_char)..b.min(line.end_char) {
            if let (Some((t0, b0)), Some((t1, b1))) =
                (layout.path_caret(c), layout.path_caret(c + 1))
            {
                quads.push([*to_page * b0, *to_page * b1, *to_page * t1, *to_page * t0]);
            }
        }
    }
    Some((*to_page * top, *to_page * bottom, quads))
}

fn word_start_before(text: &[char], from: usize) -> usize {
    let mut i = from.min(text.len());
    while i > 0 && text[i - 1].is_whitespace() {
        i -= 1;
    }
    while i > 0 && !text[i - 1].is_whitespace() {
        i -= 1;
    }
    i
}

fn word_end_after(text: &[char], from: usize) -> usize {
    let mut i = from.min(text.len());
    while i < text.len() && !text[i].is_whitespace() {
        i += 1;
    }
    while i < text.len() && text[i].is_whitespace() {
        i += 1;
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fonts_available() -> bool {
        !graphdraw_text::fonts().families().is_empty()
    }

    #[test]
    fn typing_moves_the_caret_and_edits_at_it() {
        let mut app = App::headless();
        app.settings.autocorrect.enabled = false;
        app.start_text(Point::new(10.0, 10.0), None);
        app.text_insert("Hello world");
        assert_eq!(app.edit_text(), "Hello world");
        assert_eq!(app.text_edit.as_ref().map(|t| t.caret), Some(11));
        // Shift+Left five times selects "world"; typing replaces it.
        for _ in 0..5 {
            app.text_move(CaretMove::Left, true);
        }
        assert_eq!(app.text_selected(), "world");
        app.text_insert("there");
        assert_eq!(app.edit_text(), "Hello there");
        // Home, then Delete removes the first character; End, Backspace the last.
        app.text_move(CaretMove::LineStart, false);
        app.text_delete_forward();
        app.text_move(CaretMove::LineEnd, false);
        app.text_delete_backward();
        assert_eq!(app.edit_text(), "ello ther");
        // Word moves.
        app.text_move(CaretMove::WordLeft, false);
        assert_eq!(app.text_edit.as_ref().map(|t| t.caret), Some(5));
        app.text_move(CaretMove::WordLeft, false);
        assert_eq!(app.text_edit.as_ref().map(|t| t.caret), Some(0));
        app.text_move(CaretMove::WordRight, false);
        assert_eq!(app.text_edit.as_ref().map(|t| t.caret), Some(5));
        // Typing at the caret inserts in the middle.
        app.text_insert("X");
        assert_eq!(app.edit_text(), "ello Xther");
        // Undo takes the typing back one keystroke at a time, still
        // editing, with the caret where the text changed (GD-UND-01).
        app.undo();
        assert_eq!(app.edit_text(), "ello ther");
        assert!(app.text_edit.is_some());
        assert_eq!(app.text_edit.as_ref().map(|t| t.caret), Some(5));
        app.undo();
        assert_eq!(app.edit_text(), "ello there");
        app.redo();
        assert_eq!(app.edit_text(), "ello ther");
        app.finish_text();
        assert_eq!(app.engine.undo_label(), Some("Edit Text"));
    }

    #[test]
    fn a_double_click_on_a_text_edits_it_with_any_tool() {
        use crate::tools::Tool;
        let mut app = App::headless();
        app.settings.autocorrect.enabled = false;
        app.start_text(Point::new(10.0, 10.0), None);
        app.text_insert("Hello");
        let id = app.text_edit.as_ref().map(|t| t.shape).expect("text");
        app.finish_text();
        let at = app.doc().find_shape(id).expect("shape").bounds().center();
        for tool in [Tool::Rectangle, Tool::Zoom, Tool::Shape, Tool::Bezier] {
            app.set_tool(tool);
            assert!(app.double_click_text(at), "{tool:?}");
            assert_eq!(app.tool, Tool::Text, "{tool:?}");
            assert_eq!(app.text_edit.as_ref().map(|t| t.shape), Some(id));
            app.finish_text();
        }
        // The eraser keeps its own double click; off the text nothing opens.
        app.set_tool(Tool::Eraser);
        assert!(!app.double_click_text(at));
        app.set_tool(Tool::Rectangle);
        assert!(!app.double_click_text(Point::new(500.0, 500.0)));
        assert!(app.text_edit.is_none());
    }

    #[test]
    fn undoing_the_text_creation_ends_the_editing() {
        let mut app = App::headless();
        app.settings.autocorrect.enabled = false;
        app.start_text(Point::new(10.0, 10.0), None);
        for c in ["a", "b"] {
            app.text_insert(c);
        }
        app.undo();
        assert_eq!(app.edit_text(), "a");
        app.undo();
        app.undo();
        assert!(app.text_edit.is_none(), "the text is gone");
    }

    #[test]
    fn selection_formatting_splits_spans() {
        let mut app = App::headless();
        app.settings.autocorrect.enabled = false;
        app.start_text(Point::new(0.0, 0.0), None);
        app.text_insert("abcdef");
        app.text_move(CaretMove::LineStart, false);
        app.text_move(CaretMove::Right, false);
        app.text_move(CaretMove::Right, true);
        app.text_move(CaretMove::Right, true);
        assert_eq!(app.text_selected(), "bc");
        app.text_apply_style(|s| s.bold = true);
        let spans = app.edit_spans().unwrap();
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[1].text, "bc");
        assert!(spans[1].bold && !spans[0].bold && !spans[2].bold);
        // Deleting the bold run merges the plain neighbours back.
        app.text_delete_backward();
        let spans = app.edit_spans().unwrap();
        assert_eq!(spans.len(), 1);
        assert_eq!(spans[0].text, "adef");
        // Select all and the double-click word selection.
        app.text_select_all();
        assert_eq!(app.text_selected(), "adef");
        app.text_select_word_at(2);
        assert_eq!(app.text_selected(), "adef");
    }

    #[test]
    fn the_caret_of_a_text_on_a_path_follows_the_path() {
        if !fonts_available() {
            return;
        }
        let mut app = App::headless();
        app.settings.autocorrect.enabled = false;
        // A path going down the page: the letters, the caret and the
        // selection run down it, not across.
        let mut down = graphdraw_core::geometry::BezPath::new();
        down.move_to((50.0, 10.0));
        down.line_to((50.0, 190.0));
        let path = app
            .new_shape(ShapeKind::Path {
                path: down,
                closed: false,
            })
            .expect("path");
        app.selection.clear();
        app.begin_fit_text_to_path();
        app.fit_text_to_picked_path(path);
        app.text_insert("Hello");
        let (t5, b5, _) = app.text_caret_geometry().expect("caret");
        app.text_move(CaretMove::LineStart, false);
        let (t0, b0, _) = app.text_caret_geometry().expect("caret");
        // Each caret lies across the path (horizontal on the page) and
        // typing moved it along the path (vertically).
        assert!((t0.y - b0.y).abs() < 1e-6 && (t5.y - b5.y).abs() < 1e-6);
        assert!((t0.x - b0.x).abs() > 1.0);
        assert!((t5.y - t0.y).abs() > 5.0 && (t5.x - t0.x).abs() < 1e-6);
        // A click at the end caret finds the end again.
        assert_eq!(app.text_hit_char(t5.midpoint(b5)), Some(5));
        // Selecting the word gives one quad per letter, down the path.
        app.text_move(CaretMove::LineEnd, true);
        let (_, _, quads) = app.text_caret_geometry().expect("caret");
        assert_eq!(quads.len(), 5);
    }

    #[test]
    fn caret_geometry_and_hits_follow_the_layout() {
        if !fonts_available() {
            return;
        }
        let mut app = App::headless();
        app.settings.autocorrect.enabled = false;
        app.start_text(Point::new(20.0, 30.0), None);
        app.text_insert("ab\ncd");
        let (top, bottom, quads) = app.text_caret_geometry().unwrap();
        assert!(top.y > bottom.y);
        assert!(quads.is_empty());
        // The caret after "cd" is on the second line, below the origin.
        assert!(top.y < 30.0);
        // Up moves to the first line keeping the x; Down comes back.
        app.text_move(CaretMove::Up, false);
        assert_eq!(app.text_edit.as_ref().map(|t| t.caret), Some(2));
        app.text_move(CaretMove::Down, false);
        assert_eq!(app.text_edit.as_ref().map(|t| t.caret), Some(5));
        // Clicking left of the first character puts the caret at 0.
        let hit = app.text_hit_char(Point::new(19.0, 30.5));
        assert_eq!(hit, Some(0));
        // A selection across both lines gives two quads.
        app.text_select_all();
        let (_, _, quads) = app.text_caret_geometry().unwrap();
        assert_eq!(quads.len(), 2);
    }
}
