//! Edição de texto no canvas (GD-TXT-01/02): cursor, seleção, digitação,
//! apagar, setas, parágrafos com Enter. Cada letra digitada, cada apagar e
//! cada colar é um passo do histórico; Ctrl+Z desfaz letra por letra sem
//! sair da edição. Texto novo que fica vazio sai sem deixar rastro.

use eframe::egui::{self, Event, Key, Pos2};
use graphdraw_core::edit::text as tedit;
use graphdraw_core::model::{ObjectKind, TextData};
use graphdraw_core::render::text::{TextLayout, layout_text};
use kurbo::{Affine, Point};

use crate::doc::DocState;
use crate::theme;

/// Família para texto novo: Arial se instalada, senão a primeira fonte.
pub fn default_family() -> String {
    static F: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    F.get_or_init(|| {
        let all = graphdraw_core::render::text::font_families();
        if all.iter().any(|f| f == "Arial") {
            "Arial".into()
        } else {
            all.first().cloned().unwrap_or_else(|| "Arial".into())
        }
    })
    .clone()
}

/// Famílias instaladas (lidas uma vez).
pub fn families() -> &'static [String] {
    static F: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    F.get_or_init(graphdraw_core::render::text::font_families)
}

pub struct TextEditing {
    pub id: String,
    /// Cursor e âncora da seleção (índices no texto corrido).
    pub caret: usize,
    pub anchor: usize,
    /// O objeto foi criado nesta sessão (vazio no fim = desfaz a criação).
    created: bool,
    /// Primeiro passo do histórico desta sessão (o da criação, se houve).
    mark: u64,
}

/// Texto do objeto e matriz local → página.
fn text_of(doc: &DocState, id: &str) -> Option<(TextData, Affine)> {
    let r = doc.editor.find(id)?;
    let o = doc.editor.get(&r)?;
    let ObjectKind::Text(t) = &o.kind else {
        return None;
    };
    Some((
        t.clone(),
        doc.editor.parent_affine(&r) * o.transform.affine(),
    ))
}

impl TextEditing {
    /// Começa a editar o texto `id` com o cursor em `caret`. `created` =
    /// o objeto acabou de ser inserido numa transação já aberta, que fecha
    /// aqui como o passo "Criar texto".
    pub fn start(doc: &mut DocState, id: String, caret: usize, created: bool) -> TextEditing {
        let mark = doc.editor.next_step();
        if created {
            doc.editor.commit();
        }
        doc.selection = vec![id.clone()];
        TextEditing {
            id,
            caret,
            anchor: caret,
            created,
            mark,
        }
    }

    /// Encerra a sessão. Texto novo que ficou vazio some junto com todos os
    /// passos da sessão; texto antigo esvaziado sai do documento (um passo).
    pub fn finish(self, doc: &mut DocState) {
        let Some((t, _)) = text_of(doc, &self.id) else {
            // Desfeito até antes da criação: nada a fazer.
            return;
        };
        if !tedit::is_empty(&t) {
            return;
        }
        if self.created {
            doc.editor.discard_since(self.mark);
        } else if let Some(r) = doc.editor.find(&self.id) {
            doc.editor
                .transaction(crate::i18n::tr("Excluir texto", "Delete text"), |ed| {
                    ed.remove_at(&r)
                });
        }
        doc.selection.clear();
    }

    /// Texto corrido atual (base de [`after_history`](Self::after_history)).
    pub fn plain(&self, doc: &DocState) -> String {
        text_of(doc, &self.id).map_or_else(String::new, |(t, _)| {
            graphdraw_core::render::text::plain_text(&t)
        })
    }

    /// Depois de desfazer/refazer: o cursor vai para onde o texto mudou.
    /// `false` se o objeto não existe mais (a edição termina).
    pub fn after_history(&mut self, doc: &DocState, before: &str) -> bool {
        if text_of(doc, &self.id).is_none() {
            return false;
        }
        let b: Vec<char> = before.chars().collect();
        let a: Vec<char> = self.plain(doc).chars().collect();
        let pre = b.iter().zip(&a).take_while(|(x, y)| x == y).count();
        let suf = b
            .iter()
            .rev()
            .zip(a.iter().rev())
            .take(b.len().min(a.len()) - pre)
            .take_while(|(x, y)| x == y)
            .count();
        self.caret = a.len() - suf;
        self.anchor = self.caret;
        true
    }

    /// Cada alteração é um passo próprio do histórico.
    fn edit(&self, doc: &mut DocState, name: String, f: impl FnOnce(&mut TextData)) {
        let Some(r) = doc.editor.find(&self.id) else {
            return;
        };
        let Some(mut o) = doc.editor.get(&r).cloned() else {
            return;
        };
        if let ObjectKind::Text(t) = &mut o.kind {
            let before = t.clone();
            f(t);
            if *t != before {
                doc.editor.transaction(name, |ed| ed.replace_at(&r, o));
            }
        }
    }

    fn range(&self) -> (usize, usize) {
        (self.caret.min(self.anchor), self.caret.max(self.anchor))
    }

    fn insert(&mut self, doc: &mut DocState, s: &str) {
        let (a, b) = self.range();
        let mut at = a;
        let name = match s {
            "\n" => crate::i18n::tr("Nova linha", "New line").to_string(),
            s if s.chars().count() == 1 => {
                format!("{} \u{201c}{s}\u{201d}", crate::i18n::tr("Digitar", "Type"))
            }
            _ => crate::i18n::tr("Inserir texto", "Insert text").to_string(),
        };
        self.edit(doc, name, |t| {
            tedit::delete(t, a, b);
            at = tedit::insert(t, a, s);
        });
        self.caret = at;
        self.anchor = at;
    }

    /// Trata o teclado do quadro. Devolve `false` quando a edição termina (Esc).
    pub fn handle_input(&mut self, ctx: &egui::Context, doc: &mut DocState) -> bool {
        let Some((t, _)) = text_of(doc, &self.id) else {
            return false;
        };
        let n = tedit::len(&t);
        self.caret = self.caret.min(n);
        self.anchor = self.anchor.min(n);
        let layout = layout_text(&t);
        let events = ctx.input(|i| i.events.clone());
        for e in events {
            match e {
                // Letra por letra: cada caractere é um passo de desfazer.
                Event::Text(s) => {
                    for c in s.chars() {
                        self.insert(doc, c.encode_utf8(&mut [0; 4]));
                    }
                }
                Event::Paste(s) => self.insert(doc, &s.replace("\r\n", "\n")),
                Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } => {
                    let shift = modifiers.shift;
                    let n = text_of(doc, &self.id).map_or(0, |(t, _)| tedit::len(&t));
                    let go = |to: usize, s: &mut TextEditing| {
                        s.caret = to.min(n);
                        if !shift {
                            s.anchor = s.caret;
                        }
                    };
                    match key {
                        Key::Escape => return false,
                        Key::Enter => self.insert(doc, "\n"),
                        Key::Backspace => {
                            let (a, b) = self.range();
                            let (a, b) = if a == b {
                                (a.saturating_sub(1), b)
                            } else {
                                (a, b)
                            };
                            self.edit(doc, crate::i18n::tr("Apagar", "Delete").into(), |t| {
                                tedit::delete(t, a, b)
                            });
                            self.caret = a;
                            self.anchor = a;
                        }
                        Key::Delete => {
                            let (a, b) = self.range();
                            let (a, b) = if a == b { (a, (b + 1).min(n)) } else { (a, b) };
                            self.edit(doc, crate::i18n::tr("Apagar", "Delete").into(), |t| {
                                tedit::delete(t, a, b)
                            });
                            self.caret = a;
                            self.anchor = a;
                        }
                        Key::ArrowLeft => {
                            let (a, b) = self.range();
                            let to = if a != b && !shift {
                                a
                            } else {
                                self.caret.saturating_sub(1)
                            };
                            go(to, self);
                        }
                        Key::ArrowRight => {
                            let (a, b) = self.range();
                            let to = if a != b && !shift { b } else { self.caret + 1 };
                            go(to, self);
                        }
                        Key::Home => go(line_edge(&layout, self.caret, false), self),
                        Key::End => go(line_edge(&layout, self.caret, true), self),
                        Key::ArrowUp => go(vertical(&layout, self.caret, -1), self),
                        Key::ArrowDown => go(vertical(&layout, self.caret, 1), self),
                        Key::A if modifiers.command => {
                            self.anchor = 0;
                            self.caret = n;
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        true
    }

    /// Clique dentro do texto em edição: põe o cursor (Shift estende).
    pub fn click(&mut self, doc: &DocState, page_point: Point, extend: bool) {
        if let Some((t, m)) = text_of(doc, &self.id) {
            self.caret = layout_text(&t).index_at(m.inverse() * page_point);
            if !extend {
                self.anchor = self.caret;
            }
        }
    }

    /// Cursor (piscando), seleção e quadro do parágrafo.
    pub fn paint(
        &self,
        painter: &egui::Painter,
        doc: &DocState,
        to_screen: impl Fn(Point) -> Pos2,
        time: f64,
    ) {
        let Some((t, m)) = text_of(doc, &self.id) else {
            return;
        };
        let layout = layout_text(&t);
        let s = |p: Point| to_screen(m * p);
        let (a, b) = self.range();
        if a != b {
            for line in &layout.lines {
                let (lo, hi) = (a.max(line.start), b.min(line.end));
                if lo >= hi && !(a <= line.end && b > line.end) {
                    continue;
                }
                let x = |i: usize| line.carets.iter().find(|c| c.0 == i).map_or(0.0, |c| c.1);
                let (x0, x1) = (x(lo), x(hi.max(lo)));
                let (top, bot) = (
                    line.baseline + line.size * 0.85,
                    line.baseline - line.size * 0.25,
                );
                let quad = vec![
                    s(Point::new(x0, top)),
                    s(Point::new(x1, top)),
                    s(Point::new(x1, bot)),
                    s(Point::new(x0, bot)),
                ];
                painter.add(egui::Shape::convex_polygon(
                    quad,
                    theme::SELECTION_FILL,
                    egui::Stroke::NONE,
                ));
            }
        }
        frame_and_overflow(painter, &t, &layout, &s);
        // Pisca a cada meio segundo.
        if (time * 2.0) as i64 % 2 == 0 {
            let (x, base, size) = layout.caret(self.caret).unwrap_or((
                0.0,
                -tedit::DEFAULT_SIZE * 0.8,
                tedit::DEFAULT_SIZE,
            ));
            painter.line_segment(
                [
                    s(Point::new(x, base - size * 0.25)),
                    s(Point::new(x, base + size * 0.85)),
                ],
                egui::Stroke::new(theme::FOCUS_WIDTH, theme::ACCENT_PRIMARY),
            );
        }
        painter
            .ctx()
            .request_repaint_after(std::time::Duration::from_millis(250));
    }
}

/// Quadro tracejado do texto de parágrafo e o aviso de texto excedente
/// (quadradinho `accent-danger` no canto inferior direito, GD-TXT-02).
pub fn frame_and_overflow(
    painter: &egui::Painter,
    t: &TextData,
    layout: &TextLayout,
    s: &impl Fn(Point) -> Pos2,
) {
    let Some([w, h]) = t.frame else {
        return;
    };
    let corners = [
        s(Point::new(0.0, 0.0)),
        s(Point::new(w, 0.0)),
        s(Point::new(w, -h)),
        s(Point::new(0.0, -h)),
        s(Point::new(0.0, 0.0)),
    ];
    painter.extend(egui::Shape::dashed_line(
        &corners,
        egui::Stroke::new(theme::GUIDE_WIDTH, theme::ACCENT_PRIMARY),
        theme::BLEED_DASH,
        theme::BLEED_GAP,
    ));
    if layout.overflow {
        let c = corners[2];
        let r = egui::Rect::from_center_size(c, egui::Vec2::splat(theme::HANDLE_SIZE * 1.5));
        painter.rect_filled(r, theme::RADIUS_XS, theme::ACCENT_DANGER);
    }
}

fn line_edge(l: &TextLayout, idx: usize, end: bool) -> usize {
    l.lines
        .iter()
        .find(|ln| idx >= ln.start && idx <= ln.end)
        .map_or(idx, |ln| if end { ln.end } else { ln.start })
}

fn vertical(l: &TextLayout, idx: usize, dir: i32) -> usize {
    let Some(i) = l
        .lines
        .iter()
        .position(|ln| idx >= ln.start && idx <= ln.end)
    else {
        return idx;
    };
    let j = i as i32 + dir;
    if j < 0 || j as usize >= l.lines.len() {
        return if dir < 0 {
            0
        } else {
            l.lines.last().map_or(idx, |ln| ln.end)
        };
    }
    let x = l.caret(idx).map_or(0.0, |c| c.0);
    let target = &l.lines[j as usize];
    target
        .carets
        .iter()
        .min_by(|a, b| (a.1 - x).abs().total_cmp(&(b.1 - x).abs()))
        .map_or(target.start, |c| c.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use graphdraw_core::edit::ObjLoc;
    use graphdraw_core::render::text::plain_text;

    fn key(k: Key, shift: bool) -> Event {
        Event::Key {
            key: k,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: if shift {
                egui::Modifiers::SHIFT
            } else {
                egui::Modifiers::NONE
            },
        }
    }

    fn run(
        ctx: &egui::Context,
        ed: &mut TextEditing,
        doc: &mut DocState,
        events: Vec<Event>,
    ) -> bool {
        let mut alive = true;
        let mut out = ctx.run_ui(
            egui::RawInput {
                events,
                ..Default::default()
            },
            |ui| {
                alive = ed.handle_input(ui.ctx(), doc);
            },
        );
        out.textures_delta.clear();
        alive
    }

    fn plain(doc: &DocState, id: &str) -> String {
        text_of(doc, id)
            .map(|(t, _)| plain_text(&t))
            .unwrap_or_default()
    }

    fn new_obj(doc: &mut DocState) -> String {
        let o = tedit::new_text(10_000.0, 100_000.0, None, "Arial", tedit::DEFAULT_SIZE);
        let id = o.id.clone();
        doc.editor.begin("Criar texto");
        doc.editor.insert_object(
            ObjLoc {
                page: 0,
                layer: 0,
                index: 0,
            },
            o,
        );
        id
    }

    /// Digitar, Enter, apagar e selecionar com Shift; cada letra é um passo
    /// do histórico e texto novo vazio não fica no documento.
    #[test]
    fn typing_is_undone_letter_by_letter() {
        let ctx = crate::theme::test_ctx();
        let mut doc = DocState::new_blank();
        let id = new_obj(&mut doc);
        let mut ed = TextEditing::start(&mut doc, id.clone(), 0, true);
        assert!(
            !doc.editor.in_transaction(),
            "nada fica aberto durante a digitação"
        );
        run(
            &ctx,
            &mut ed,
            &mut doc,
            vec![
                Event::Text("Oi".into()),
                key(Key::Enter, false),
                Event::Text("mundo".into()),
            ],
        );
        assert_eq!(plain(&doc, &id), "Oi\nmundo");
        // Criar + O + i + Enter + m,u,n,d,o.
        assert_eq!(doc.editor.undo_len(), 9);
        run(
            &ctx,
            &mut ed,
            &mut doc,
            vec![
                key(Key::Backspace, false),
                key(Key::ArrowLeft, true),
                key(Key::ArrowLeft, true),
            ],
        );
        assert_eq!((ed.caret, ed.anchor), (5, 7));
        run(&ctx, &mut ed, &mut doc, vec![Event::Text("X".into())]);
        assert_eq!(plain(&doc, &id), "Oi\nmuX");
        // Desfazer durante a edição: uma letra/ação por vez, cursor no lugar.
        let before = ed.plain(&doc);
        doc.editor.undo();
        assert!(ed.after_history(&doc, &before));
        assert_eq!(plain(&doc, &id), "Oi\nmund");
        assert_eq!(ed.caret, 7, "cursor depois do trecho devolvido");
        let before = ed.plain(&doc);
        doc.editor.undo();
        assert!(ed.after_history(&doc, &before));
        assert_eq!(plain(&doc, &id), "Oi\nmundo");
        assert_eq!(ed.caret, 8);
        let before = ed.plain(&doc);
        doc.editor.undo();
        assert!(ed.after_history(&doc, &before));
        assert_eq!(plain(&doc, &id), "Oi\nmund");
        let before = ed.plain(&doc);
        doc.editor.redo();
        assert!(ed.after_history(&doc, &before));
        assert_eq!(plain(&doc, &id), "Oi\nmundo");
        // Backspace sem nada para apagar não vira passo.
        let n = doc.editor.undo_len();
        ed.caret = 0;
        ed.anchor = 0;
        run(&ctx, &mut ed, &mut doc, vec![key(Key::Backspace, false)]);
        assert_eq!(doc.editor.undo_len(), n);
        assert!(!run(&ctx, &mut ed, &mut doc, vec![key(Key::Escape, false)]));
        ed.finish(&mut doc);
        assert_eq!(doc.page().layers[0].objects.len(), 1);
        doc.editor.goto(0);
        assert!(doc.page().layers[0].objects.is_empty());
        doc.editor.goto(doc.editor.history().len());
        // Texto novo deixado vazio (mesmo depois de digitar e apagar) não
        // deixa rastro no histórico.
        let steps = doc.editor.undo_len();
        let id = new_obj(&mut doc);
        let mut ed = TextEditing::start(&mut doc, id, 0, true);
        run(
            &ctx,
            &mut ed,
            &mut doc,
            vec![
                Event::Text("ab".into()),
                key(Key::Backspace, false),
                key(Key::Backspace, false),
            ],
        );
        ed.finish(&mut doc);
        assert_eq!(doc.page().layers[0].objects.len(), 1);
        assert_eq!(doc.editor.undo_len(), steps);
        assert!(!doc.editor.can_redo());
        assert!(!doc.editor.in_transaction());
    }
}
