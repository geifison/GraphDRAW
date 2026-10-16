//! Barra de páginas em abas, acima da barra de status (doc 21, GD-PGT-*).
//!
//! `[Grade] [Lista] [◄] Pág. [n] / N [►] | [0] Mestra | [1] Capa | … | [+]`
//!
//! A seleção de abas guarda IDs de página, não índices, para continuar certa
//! depois de reordenar, desfazer ou excluir.

use std::collections::HashMap;

use eframe::egui::{self, Color32, Pos2, Rect, Sense, Vec2};
use graphdraw_core::edit::history::MASTER_KEY;
use graphdraw_core::edit::{MASTER, pages};
use graphdraw_core::export::ExportFormat;
use graphdraw_core::model::{Color, Page};

use crate::doc::DocState;
use crate::i18n::tr;
use crate::icons;
use crate::widgets::{fmt_mm, length_field};

/// Pedido que a barra devolve para a janela principal.
pub enum PageAction {
    /// A página mudou: ajustar a vista.
    Switched,
    Export(ExportFormat, Vec<usize>),
}

struct Rename {
    id: String,
    text: String,
    focus: bool,
}

struct TabDrag {
    /// Páginas arrastadas, na ordem do documento.
    ids: Vec<String>,
    /// Distância do ponteiro à borda esquerda da aba agarrada.
    grab: f32,
}

struct Settings {
    id: String,
    width: f64,
    height: f64,
    own_bleed: bool,
    bleed: f64,
}

#[derive(Default)]
pub struct PageBar {
    sel: Vec<String>,
    anchor: Option<String>,
    rename: Option<Rename>,
    drag: Option<TabDrag>,
    jump_focus: bool,
    search: String,
    pub grid: bool,
    confirm: Option<Vec<String>>,
    settings: Option<Settings>,
    thumbs: HashMap<String, (u64, egui::TextureHandle)>,
    last_active: Option<usize>,
}

/// Etiquetas de cor (GD-PGT-20). Cores próprias, em RGB só para a tela.
fn tag_colors() -> [(&'static str, Color); 6] {
    [
        (tr("Verde", "Green"), Color::rgb(0.18, 0.65, 0.3)),
        (tr("Amarelo", "Yellow"), Color::rgb(0.95, 0.77, 0.1)),
        (tr("Laranja", "Orange"), Color::rgb(0.95, 0.5, 0.1)),
        (tr("Vermelho", "Red"), Color::rgb(0.85, 0.2, 0.2)),
        (tr("Azul", "Blue"), Color::rgb(0.15, 0.45, 0.9)),
        (tr("Roxo", "Purple"), Color::rgb(0.55, 0.3, 0.8)),
    ]
}

fn index_of(doc: &DocState, id: &str) -> Option<usize> {
    doc.doc().data.pages.iter().position(|p| p.id == id)
}

fn label(page: &Page, i: usize) -> String {
    format!("[{}] {}", i + 1, pages::display_name(page, i))
}

impl PageBar {
    /// IDs das abas selecionadas que ainda existem; sem seleção, a ativa.
    fn selected(&self, doc: &DocState) -> Vec<usize> {
        let mut v: Vec<usize> = self.sel.iter().filter_map(|id| index_of(doc, id)).collect();
        if v.is_empty() && !doc.is_master() {
            v.push(doc.page);
        }
        v.sort();
        v
    }

    fn go(&mut self, doc: &mut DocState, index: usize, out: &mut Vec<PageAction>) {
        if index != doc.page {
            doc.set_page(index);
            out.push(PageAction::Switched);
        }
        self.sel.clear();
        self.anchor = None;
    }

    fn thumb(&mut self, ctx: &egui::Context, doc: &DocState, index: usize) -> egui::TextureHandle {
        let id = if index == MASTER {
            MASTER_KEY.to_string()
        } else {
            doc.doc().data.pages[index].id.clone()
        };
        let rev = doc.editor.revision();
        if let Some((r, t)) = self.thumbs.get(&id)
            && *r == rev
        {
            return t.clone();
        }
        let img = doc.thumbnail(index, 160.0);
        let tex = ctx.load_texture(format!("miniatura-{id}"), img, egui::TextureOptions::LINEAR);
        self.thumbs.insert(id, (rev, tex.clone()));
        tex
    }

    /// Pede a exclusão: página vazia sai na hora; com objetos, confirma (GD-PGT-15).
    fn request_delete(&mut self, doc: &mut DocState, ids: Vec<String>) {
        let objects: usize = ids
            .iter()
            .filter_map(|id| index_of(doc, id))
            .map(|i| doc.doc().data.pages[i].object_count())
            .sum();
        if objects == 0 {
            self.delete_now(doc, &ids);
        } else {
            self.confirm = Some(ids);
        }
    }

    fn delete_now(&mut self, doc: &mut DocState, ids: &[String]) {
        let idx: Vec<usize> = ids.iter().filter_map(|id| index_of(doc, id)).collect();
        pages::delete(&mut doc.editor, &idx);
        self.sel.clear();
        doc.sync();
        doc.prune_selection();
    }

    /// Atalhos da barra (GD-PGT-04, 06, 07, 11).
    pub fn shortcuts(&mut self, ctx: &egui::Context, doc: &mut DocState) -> Vec<PageAction> {
        let mut out = Vec::new();
        let n = doc.doc().data.pages.len();
        let typing = ctx.egui_wants_keyboard_input();
        let cmd = egui::Modifiers::COMMAND;
        if ctx.input_mut(|i| i.consume_key(cmd, egui::Key::J)) {
            self.jump_focus = true;
        }
        if ctx.input_mut(|i| i.consume_key(cmd | egui::Modifiers::SHIFT, egui::Key::N)) {
            let after = if doc.is_master() { n - 1 } else { doc.page };
            let i = pages::add_after(&mut doc.editor, after);
            self.go(doc, i, &mut out);
        }
        if typing {
            return out;
        }
        let pressed = |k| ctx.input(|i| i.key_pressed(k) && i.modifiers.is_none());
        let cur = if doc.is_master() {
            None
        } else {
            Some(doc.page)
        };
        let target = if pressed(egui::Key::PageDown) {
            Some(cur.map_or(0, |c| (c + 1).min(n - 1)))
        } else if pressed(egui::Key::PageUp) {
            match cur {
                Some(0) => Some(MASTER),
                Some(c) => Some(c - 1),
                None => None,
            }
        } else if pressed(egui::Key::Home) {
            Some(0)
        } else if pressed(egui::Key::End) {
            Some(n - 1)
        } else {
            None
        };
        if let Some(t) = target {
            self.go(doc, t, &mut out);
        }
        if pressed(egui::Key::F2) && !doc.is_master() {
            let p = &doc.doc().data.pages[doc.page];
            self.rename = Some(Rename {
                id: p.id.clone(),
                text: pages::display_name(p, doc.page),
                focus: true,
            });
        }
        out
    }

    pub fn show(&mut self, ui: &mut egui::Ui, doc: &mut DocState) -> Vec<PageAction> {
        let mut out = Vec::new();
        let n = doc.doc().data.pages.len();
        self.sel
            .retain(|id| doc.doc().data.pages.iter().any(|p| &p.id == id));
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            if icons::button(
                ui,
                "page-grid",
                tr("Visão geral das páginas", "Page overview"),
                self.grid,
                true,
            )
            .clicked()
            {
                self.grid = !self.grid;
            }
            self.list_button(ui, doc, &mut out);
            let cur = if doc.is_master() {
                None
            } else {
                Some(doc.page)
            };
            if icons::button(
                ui,
                "page-prev",
                tr("Página anterior (PgUp)", "Previous page (PgUp)"),
                false,
                cur.is_some_and(|c| c > 0),
            )
            .clicked()
            {
                let c = cur.unwrap();
                self.go(doc, c - 1, &mut out);
            }
            self.jump_field(ui, doc, &mut out);
            if icons::button(
                ui,
                "page-next",
                tr("Próxima página (PgDn)", "Next page (PgDn)"),
                false,
                cur.is_none_or(|c| c + 1 < n),
            )
            .clicked()
            {
                let t = cur.map_or(0, |c| c + 1);
                self.go(doc, t, &mut out);
            }
            ui.separator();
            // Abas ocupam o espaço até o botão "+".
            let width = (ui.available_width() - 40.0).max(80.0);
            ui.allocate_ui(Vec2::new(width, 30.0), |ui| {
                egui::ScrollArea::horizontal()
                    .id_salt("abas-paginas")
                    .max_width(width)
                    .scroll_bar_visibility(
                        egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded,
                    )
                    .show(ui, |ui| self.tabs(ui, doc, &mut out));
            });
            if icons::button(
                ui,
                "page-add",
                tr(
                    "Nova página após a atual (Ctrl+Shift+N)",
                    "New page after the current one (Ctrl+Shift+N)",
                ),
                false,
                true,
            )
            .clicked()
            {
                let after = cur.unwrap_or(n - 1);
                let i = pages::add_after(&mut doc.editor, after);
                self.go(doc, i, &mut out);
            }
        });
        self.windows(ui.ctx(), doc, &mut out);
        out
    }

    fn list_button(&mut self, ui: &mut egui::Ui, doc: &mut DocState, out: &mut Vec<PageAction>) {
        let resp = icons::button(
            ui,
            "page-list",
            tr("Procurar página pelo nome", "Find page by name"),
            false,
            true,
        );
        let mut jump = None;
        egui::Popup::from_toggle_button_response(&resp)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                ui.set_width(260.0);
                let r = ui.add(
                    egui::TextEdit::singleline(&mut self.search)
                        .hint_text(tr("Nome ou número…", "Name or number…")),
                );
                r.request_focus();
                let q = self.search.trim().to_lowercase();
                egui::ScrollArea::vertical()
                    .max_height(320.0)
                    .show(ui, |ui| {
                        if (q.is_empty() || "mestra master 0".contains(&q))
                            && ui
                                .selectable_label(doc.is_master(), tr("[0] Mestra", "[0] Master"))
                                .clicked()
                        {
                            jump = Some(MASTER);
                        }
                        for (i, p) in doc.doc().data.pages.iter().enumerate() {
                            let l = label(p, i);
                            if (q.is_empty()
                                || l.to_lowercase().contains(&q)
                                || q == (i + 1).to_string())
                                && ui.selectable_label(i == doc.page, l).clicked()
                            {
                                jump = Some(i);
                            }
                        }
                    });
                if jump.is_some() {
                    ui.close();
                }
            });
        if let Some(j) = jump {
            self.search.clear();
            self.go(doc, j, out);
        }
    }

    fn jump_field(&mut self, ui: &mut egui::Ui, doc: &mut DocState, out: &mut Vec<PageAction>) {
        let n = doc.doc().data.pages.len();
        ui.label(tr("Pág.", "Page"));
        let id = ui.scope_id().with("pular-pagina");
        let shown = if doc.is_master() {
            "0".to_string()
        } else {
            (doc.page + 1).to_string()
        };
        let mut text: String = ui
            .data(|d| d.get_temp::<String>(id))
            .unwrap_or_else(|| shown.clone());
        let r = ui
            .add(
                egui::TextEdit::singleline(&mut text)
                    .id(id.with("campo"))
                    .desired_width(32.0)
                    .align(egui::Align2::CENTER_CENTER),
            )
            .on_hover_text(tr(
                "Digite o número e Enter (Ctrl+J)",
                "Type the number and press Enter (Ctrl+J)",
            ));
        if self.jump_focus {
            r.request_focus();
            ui.data_mut(|d| d.insert_temp(id, String::new()));
            self.jump_focus = false;
        } else if r.has_focus() {
            ui.data_mut(|d| d.insert_temp(id, text.clone()));
        } else {
            ui.data_mut(|d| d.remove::<String>(id));
        }
        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            if let Ok(k) = text.trim().parse::<usize>() {
                let t = if k == 0 { MASTER } else { k.min(n) - 1 };
                self.go(doc, t, out);
            }
            ui.data_mut(|d| d.remove::<String>(id));
        }
        ui.label(format!("/ {n}"));
    }

    fn tabs(&mut self, ui: &mut egui::Ui, doc: &mut DocState, out: &mut Vec<PageAction>) {
        let dy = ui.input(|i| i.smooth_scroll_delta.y);
        if dy != 0.0 && ui.rect_contains_pointer(ui.clip_rect()) {
            ui.scroll_with_delta(Vec2::new(dy, 0.0));
        }
        let n = doc.doc().data.pages.len();
        let dirty = doc.editor.dirty_pages();
        let font = egui::TextStyle::Button.resolve(ui.style());
        let text_color = ui.visuals().text_color();
        let h = 26.0;
        let gap = 3.0;
        let pad = 10.0;
        let lock_w = 16.0;

        // Mestra (fixa no começo, não arrasta).
        let master_text = tr("[0] Mestra", "[0] Master");
        let mg = ui
            .painter()
            .layout_no_wrap(master_text.into(), font.clone(), text_color);
        let master_w = mg.size().x + 2.0 * pad;

        // Ordem exibida: durante um arrasto, as abas arrastadas entram onde
        // o ponteiro está e as vizinhas se afastam (renumeração ao vivo).
        let ids: Vec<String> = doc.doc().data.pages.iter().map(|p| p.id.clone()).collect();
        let pointer = ui.input(|i| i.pointer.interact_pos());
        let origin = ui.cursor().min;
        let base_x = origin.x + master_w + gap;
        let widths: HashMap<String, f32> = doc
            .doc()
            .data
            .pages
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let g = ui.painter().layout_no_wrap(
                    format!("{}*", label(p, i)),
                    font.clone(),
                    text_color,
                );
                let extra = if p.locked { lock_w } else { 0.0 };
                (p.id.clone(), g.size().x + 2.0 * pad + extra)
            })
            .collect();
        let mut order = ids.clone();
        let mut insert_at = None;
        if let (Some(d), Some(p)) = (&self.drag, pointer) {
            let rest: Vec<String> = ids
                .iter()
                .filter(|id| !d.ids.contains(id))
                .cloned()
                .collect();
            let mut x = base_x;
            let mut k = rest.len();
            let left = p.x - d.grab;
            for (j, id) in rest.iter().enumerate() {
                let w = widths[id];
                if left < x + w / 2.0 {
                    k = j;
                    break;
                }
                x += w + gap;
            }
            insert_at = Some(k);
            order = rest;
            for (m, id) in d.ids.iter().enumerate() {
                order.insert(k + m, id.clone());
            }
        }
        let total: f32 = master_w + order.iter().map(|id| widths[id] + gap).sum::<f32>() + gap;
        let (strip, _) = ui.allocate_exact_size(Vec2::new(total, h), Sense::hover());

        // Aba da mestra.
        let mrect =
            Rect::from_min_size(Pos2::new(strip.min.x, strip.min.y), Vec2::new(master_w, h));
        let mresp = ui
            .interact(mrect, ui.scope_id().with("aba-mestra"), Sense::click())
            .on_hover_text(tr(
                "Página mestra: o que estiver nela aparece como fundo em todas as páginas",
                "Master page: its content shows as background on every page",
            ));
        let empty_master = doc
            .doc()
            .data
            .master_page
            .layers
            .iter()
            .all(|l| l.objects.is_empty());
        paint_tab(
            ui,
            mrect,
            master_text,
            doc.is_master(),
            false,
            None,
            false,
            empty_master,
        );
        if mresp.clicked() {
            self.go(doc, MASTER, out);
        }
        if !doc.is_master() {
            let _ = mresp.on_hover_ui(|ui| {
                let t = self.thumb(ui.ctx(), doc, MASTER);
                ui.add(egui::Image::new(&t).max_size(Vec2::splat(160.0)));
            });
        }

        let mut x = base_x;
        let mut drop = None;
        let alt = ui.input(|i| i.modifiers.alt);
        let active_changed = self.last_active != Some(doc.page);
        self.last_active = Some(doc.page);
        for (pos, id) in order.iter().enumerate() {
            let Some(i) = index_of(doc, id) else { continue };
            let w = widths[id];
            let dragging = self.drag.as_ref().is_some_and(|d| d.ids.contains(id));
            let anim = ui.ctx().animate_value_with_time(
                ui.scope_id().with(("aba-x", id)),
                x - strip.min.x,
                0.12,
            );
            let mut rect =
                Rect::from_min_size(Pos2::new(strip.min.x + anim, strip.min.y), Vec2::new(w, h));
            if dragging
                && let (Some(d), Some(p)) = (&self.drag, pointer)
                && d.ids.first() == Some(id)
            {
                rect = Rect::from_min_size(Pos2::new(p.x - d.grab, strip.min.y), Vec2::new(w, h));
            }
            x += w + gap;
            // Só os campos usados (não clonar a página, que pode ser enorme).
            let (name, locked, tag) = {
                let p = &doc.doc().data.pages[i];
                (
                    pages::display_name(p, i),
                    p.locked,
                    p.tag_color.as_ref().map(crate::widgets::to_color32),
                )
            };
            let text = format!(
                "[{}] {}{}",
                pos + 1,
                name,
                if dirty.contains(id) { "*" } else { "" }
            );
            let active = !doc.is_master() && i == doc.page;
            let selected = self.sel.contains(id);
            let resp = ui.interact(
                rect,
                ui.scope_id().with(("aba", id)),
                Sense::click_and_drag(),
            );
            if active && active_changed {
                ui.scroll_to_rect(rect, None);
            }

            // Renomear no lugar (GD-PGT-04).
            if self.rename.as_ref().is_some_and(|r| &r.id == id) {
                let r = self.rename.as_mut().unwrap();
                let te = ui.put(
                    rect.shrink(2.0),
                    egui::TextEdit::singleline(&mut r.text)
                        .id(ui.scope_id().with(("renomear", id))),
                );
                if r.focus {
                    te.request_focus();
                    r.focus = false;
                }
                let esc = ui.input(|i| i.key_pressed(egui::Key::Escape));
                if esc {
                    self.rename = None;
                } else if te.lost_focus() {
                    let name = r.text.trim().to_string();
                    self.rename = None;
                    if !name.is_empty() && name != pages::display_name(&doc.doc().data.pages[i], i)
                    {
                        doc.editor
                            .transaction(tr("Renomear página", "Rename page"), |ed| {
                                ed.edit_page(i, |p| p.name = Some(name))
                            });
                    }
                }
                continue;
            }

            let alpha_dragging = dragging && self.drag.is_some();
            paint_tab(
                ui,
                rect,
                &text,
                active,
                selected,
                tag,
                locked,
                alpha_dragging,
            );

            if resp.drag_started() {
                let grabbed = if selected || active {
                    let mut s: Vec<String> = self.sel.clone();
                    if !s.contains(id) {
                        s.push(id.clone());
                    }
                    s.sort_by_key(|s| index_of(doc, s));
                    s
                } else {
                    vec![id.clone()]
                };
                // A aba agarrada vai na frente do bloco, para seguir o ponteiro.
                let mut ids2 = vec![id.clone()];
                ids2.extend(grabbed.into_iter().filter(|s| s != id));
                let grab = resp
                    .interact_pointer_pos()
                    .map_or(0.0, |p| p.x - rect.min.x);
                self.drag = Some(TabDrag { ids: ids2, grab });
            }
            if resp.drag_stopped() && self.drag.is_some() {
                drop = Some(alt);
            }
            if resp.clicked() {
                let m = ui.input(|i| i.modifiers);
                if m.command {
                    if let Some(k) = self.sel.iter().position(|s| s == id) {
                        self.sel.remove(k);
                    } else {
                        if self.sel.is_empty() && !doc.is_master() {
                            self.sel.push(doc.doc().data.pages[doc.page].id.clone());
                        }
                        self.sel.push(id.clone());
                    }
                    self.anchor = Some(id.clone());
                } else if m.shift {
                    let a = self
                        .anchor
                        .as_ref()
                        .and_then(|a| index_of(doc, a))
                        .unwrap_or(if doc.is_master() { 0 } else { doc.page });
                    let (lo, hi) = (a.min(i), a.max(i));
                    self.sel = (lo..=hi).map(|k| ids[k].clone()).collect();
                } else {
                    self.go(doc, i, out);
                    self.anchor = Some(id.clone());
                }
            }
            if resp.double_clicked() {
                self.rename = Some(Rename {
                    id: id.clone(),
                    text: name.clone(),
                    focus: true,
                });
            }
            if resp.middle_clicked() && n > 1 {
                self.request_delete(doc, vec![id.clone()]);
            }
            if !active && self.drag.is_none() {
                let _ = resp.clone().on_hover_ui(|ui| {
                    let t = self.thumb(ui.ctx(), doc, i);
                    ui.add(egui::Image::new(&t).max_size(Vec2::splat(160.0)));
                    ui.label(label(&doc.doc().data.pages[i], i));
                });
            }
            resp.context_menu(|ui| self.context_menu(ui, doc, id, i, out));
        }
        if self.drag.is_some() && !ui.input(|i| i.pointer.any_down()) && drop.is_none() {
            // Soltou fora de qualquer aba.
            drop = Some(alt);
        }
        if let (Some(dup), Some(d), Some(k)) = (drop, self.drag.take(), insert_at) {
            let rest: Vec<usize> = (0..n).filter(|i| !d.ids.contains(&ids[*i])).collect();
            let to = rest.get(k).copied().unwrap_or(n);
            let mut idx: Vec<usize> = d.ids.iter().filter_map(|id| index_of(doc, id)).collect();
            idx.sort();
            if dup {
                // Alt + arrastar: cópias na posição solta (GD-PGT-13).
                let at = if k < rest.len() { rest[k] } else { n };
                let mut first = None;
                doc.editor
                    .transaction(tr("Duplicar página", "Duplicate page"), |ed| {
                        for (m, src) in idx.iter().enumerate() {
                            let src = if *src >= at { src + m } else { *src };
                            let i = pages::duplicate(ed, src, at + m);
                            first.get_or_insert(i);
                        }
                    });
                if let Some(f) = first {
                    self.go(doc, f, out);
                }
            } else {
                let active_id = (!doc.is_master()).then(|| ids[doc.page].clone());
                pages::move_block(&mut doc.editor, &idx, to);
                doc.sync();
                if let Some(a) = active_id.and_then(|a| index_of(doc, &a)) {
                    doc.page = a;
                    doc.last_page = a;
                }
            }
        } else if drop.is_some() {
            self.drag = None;
        }
        if self.drag.is_some() {
            ui.ctx().request_repaint();
        }
        // Delete com o ponteiro sobre a barra exclui as abas selecionadas.
        if ui.rect_contains_pointer(ui.clip_rect())
            && ui.input(|i| i.key_pressed(egui::Key::Delete))
            && n > 1
        {
            let ids: Vec<String> = self
                .selected(doc)
                .into_iter()
                .map(|i| doc.doc().data.pages[i].id.clone())
                .collect();
            self.request_delete(doc, ids);
        }
    }

    fn context_menu(
        &mut self,
        ui: &mut egui::Ui,
        doc: &mut DocState,
        id: &str,
        i: usize,
        out: &mut Vec<PageAction>,
    ) {
        if !self.sel.iter().any(|s| s == id) {
            self.sel = vec![id.to_string()];
        }
        let targets = self.selected(doc);
        let many = targets.len() > 1;
        let n = doc.doc().data.pages.len();
        // Sem os objetos: a página pode ser grande e o menu redesenha a cada quadro.
        let page = doc.doc().data.pages[i].props_only();
        if ui.button(tr("Configurar página…", "Page setup…")).clicked() {
            self.settings = Some(Settings {
                id: page.id.clone(),
                width: page.width,
                height: page.height,
                own_bleed: page.bleed.is_some(),
                bleed: page.bleed.unwrap_or(doc.doc().data.settings.bleed),
            });
            ui.close();
        }
        if ui.button(tr("Renomear  (F2)", "Rename  (F2)")).clicked() {
            self.rename = Some(Rename {
                id: id.to_string(),
                text: pages::display_name(&page, i),
                focus: true,
            });
            ui.close();
        }
        if ui.button(tr("Duplicar página", "Duplicate page")).clicked() {
            let k = pages::duplicate(&mut doc.editor, i, i + 1);
            self.go(doc, k, out);
            ui.close();
        }
        ui.separator();
        let lock_label = if page.locked {
            tr("Desbloquear página", "Unlock page")
        } else {
            tr("Bloquear página", "Lock page")
        };
        if ui.button(lock_label).clicked() {
            let v = !page.locked;
            doc.editor.transaction(lock_label, |ed| {
                for &t in &targets {
                    ed.edit_page(t, |p| p.locked = v);
                }
            });
            doc.prune_selection();
            ui.close();
        }
        let hide = page.hide_master;
        if ui
            .checkbox(
                &mut hide.clone(),
                tr("Ocultar a mestra nesta página", "Hide master on this page"),
            )
            .clicked()
        {
            doc.editor
                .transaction(tr("Mestra na página", "Master on page"), |ed| {
                    for &t in &targets {
                        ed.edit_page(t, |p| p.hide_master = !hide);
                    }
                });
            ui.close();
        }
        ui.menu_button(tr("Etiqueta de cor", "Color tag"), |ui| {
            let mut pick: Option<Option<Color>> = None;
            if ui.button(tr("Nenhuma", "None")).clicked() {
                pick = Some(None);
            }
            for (name, c) in tag_colors() {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
                    ui.painter()
                        .rect_filled(r, 2.0, crate::widgets::to_color32(&c));
                    if ui.button(name).clicked() {
                        pick = Some(Some(c.clone()));
                    }
                });
            }
            if let Some(c) = pick {
                doc.editor
                    .transaction(tr("Etiqueta da página", "Page tag"), |ed| {
                        for &t in &targets {
                            ed.edit_page(t, |p| p.tag_color = c.clone());
                        }
                    });
                ui.close();
            }
        });
        ui.separator();
        ui.menu_button(tr("Exportar esta página", "Export this page"), |ui| {
            for (name, f) in [
                ("PDF…", ExportFormat::Pdf),
                ("SVG…", ExportFormat::Svg),
                ("PNG…", ExportFormat::Png),
            ] {
                if ui.button(name).clicked() {
                    out.push(PageAction::Export(f, vec![i]));
                    ui.close();
                }
            }
        });
        if many
            && ui
                .button(format!(
                    "{} ({})",
                    tr("Exportar seleção em PDF…", "Export selection as PDF…"),
                    targets.len()
                ))
                .clicked()
        {
            out.push(PageAction::Export(ExportFormat::Pdf, targets.clone()));
            ui.close();
        }
        ui.separator();
        let del = if many {
            format!(
                "{} {}",
                tr("Excluir páginas:", "Delete pages:"),
                targets.len()
            )
        } else {
            tr("Excluir página", "Delete page").to_string()
        };
        if ui.add_enabled(n > 1, egui::Button::new(del)).clicked() {
            let ids = targets
                .iter()
                .map(|t| doc.doc().data.pages[*t].id.clone())
                .collect();
            self.request_delete(doc, ids);
            ui.close();
        }
    }

    fn windows(&mut self, ctx: &egui::Context, doc: &mut DocState, out: &mut Vec<PageAction>) {
        // Confirmação de exclusão (GD-PGT-15).
        if let Some(ids) = self.confirm.clone() {
            let idx: Vec<usize> = ids.iter().filter_map(|id| index_of(doc, id)).collect();
            let objects: usize = idx
                .iter()
                .map(|i| doc.doc().data.pages[*i].object_count())
                .sum();
            let msg = if idx.len() == 1 {
                format!(
                    "{} {} {} {} {}",
                    tr("Excluir a página", "Delete page"),
                    idx[0] + 1,
                    tr("com", "with"),
                    objects,
                    tr("objetos?", "objects?")
                )
            } else {
                format!(
                    "{} {} {} {} {}",
                    tr("Excluir", "Delete"),
                    idx.len(),
                    tr("páginas com", "pages with"),
                    objects,
                    tr("objetos?", "objects?")
                )
            };
            let mut close = None;
            egui::Modal::new(egui::Id::unique("confirmar-excluir-pagina")).show(ctx, |ui| {
                ui.set_width(380.0);
                ui.label(msg);
                ui.weak(tr(
                    "Pode ser desfeito com Ctrl+Z.",
                    "This can be undone with Ctrl+Z.",
                ));
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if ui.button(tr("Excluir", "Delete")).clicked() {
                        close = Some(true);
                    }
                    if ui.button(tr("Cancelar", "Cancel")).clicked()
                        || ui.input(|i| i.key_pressed(egui::Key::Escape))
                    {
                        close = Some(false);
                    }
                });
            });
            match close {
                Some(true) => {
                    self.confirm = None;
                    self.delete_now(doc, &ids);
                    out.push(PageAction::Switched);
                }
                Some(false) => self.confirm = None,
                None => {}
            }
        }

        // Configuração da página (GD-PGT-16).
        if let Some(s) = &mut self.settings {
            let mut close = None;
            let doc_bleed = doc.doc().data.settings.bleed;
            egui::Modal::new(egui::Id::unique("configurar-pagina")).show(ctx, |ui| {
                ui.set_width(380.0);
                ui.heading(tr("Configurar página", "Page setup"));
                ui.separator();
                egui::Grid::new("cfg-pagina").num_columns(2).show(ui, |ui| {
                    ui.label(tr("Largura", "Width"));
                    ui.horizontal(|ui| {
                        if let Some(v) = length_field(ui, "cfg-l", s.width, None) {
                            s.width = v.max(1000.0);
                        }
                    });
                    ui.end_row();
                    ui.label(tr("Altura", "Height"));
                    ui.horizontal(|ui| {
                        if let Some(v) = length_field(ui, "cfg-a", s.height, None) {
                            s.height = v.max(1000.0);
                        }
                    });
                    ui.end_row();
                    ui.label(tr("Orientação", "Orientation"));
                    ui.horizontal(|ui| {
                        let portrait = s.height >= s.width;
                        if ui
                            .selectable_label(portrait, tr("Retrato", "Portrait"))
                            .clicked()
                            && !portrait
                        {
                            std::mem::swap(&mut s.width, &mut s.height);
                        }
                        if ui
                            .selectable_label(!portrait, tr("Paisagem", "Landscape"))
                            .clicked()
                            && portrait
                        {
                            std::mem::swap(&mut s.width, &mut s.height);
                        }
                    });
                    ui.end_row();
                    ui.label(tr("Sangria", "Bleed"));
                    ui.vertical(|ui| {
                        ui.checkbox(
                            &mut s.own_bleed,
                            tr("Própria desta página", "Own for this page"),
                        );
                        if s.own_bleed {
                            ui.horizontal(|ui| {
                                if let Some(v) = length_field(ui, "cfg-s", s.bleed, None) {
                                    s.bleed = v.max(0.0);
                                }
                            });
                        } else {
                            ui.weak(format!(
                                "{} {} mm",
                                tr("A do documento:", "Document's:"),
                                fmt_mm(doc_bleed)
                            ));
                        }
                    });
                    ui.end_row();
                });
                ui.horizontal_wrapped(|ui| {
                    for (name, w, h) in [
                        ("A3", 297_000.0, 420_000.0),
                        ("A4", 210_000.0, 297_000.0),
                        ("A5", 148_000.0, 210_000.0),
                        (tr("Carta", "Letter"), 215_900.0, 279_400.0),
                        (tr("Ofício", "Legal"), 215_900.0, 355_600.0),
                        ("1080 × 1080 px", 285_750.0, 285_750.0),
                    ] {
                        if ui.small_button(name).clicked() {
                            let land = s.width > s.height && w != h;
                            (s.width, s.height) = if land { (h, w) } else { (w, h) };
                        }
                    }
                });
                ui.weak(tr(
                    "Os campos aceitam unidades: 1080px, 8.5in, 297.",
                    "Fields accept units: 1080px, 8.5in, 297.",
                ));
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(tr("OK", "OK")).clicked() {
                        close = Some(true);
                    }
                    if ui.button(tr("Cancelar", "Cancel")).clicked()
                        || ui.input(|i| i.key_pressed(egui::Key::Escape))
                    {
                        close = Some(false);
                    }
                });
            });
            match close {
                Some(true) => {
                    if let Some(i) = index_of(doc, &s.id) {
                        let (w, h) = (s.width, s.height);
                        let bleed = s.own_bleed.then_some(s.bleed);
                        doc.editor
                            .transaction(tr("Configurar página", "Page setup"), |ed| {
                                ed.edit_page(i, |p| {
                                    p.width = w;
                                    p.height = h;
                                    p.bleed = bleed;
                                })
                            });
                        out.push(PageAction::Switched);
                    }
                    self.settings = None;
                }
                Some(false) => self.settings = None,
                None => {}
            }
        }

        // Visão geral (GD-PGT-09).
        if self.grid {
            let mut open = true;
            let mut jump = None;
            let screen = ctx.content_rect();
            egui::Window::new(tr("Visão geral das páginas", "Page overview"))
                .open(&mut open)
                .default_size(Vec2::new(screen.width() * 0.6, screen.height() * 0.6))
                .max_size(screen.size() * 0.9)
                .collapsible(false)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.horizontal_wrapped(|ui| {
                            let n = doc.doc().data.pages.len();
                            for i in std::iter::once(MASTER).chain(0..n) {
                                let t = self.thumb(ctx, doc, i);
                                let (name, active) = if i == MASTER {
                                    (tr("[0] Mestra", "[0] Master").to_string(), doc.is_master())
                                } else {
                                    (label(&doc.doc().data.pages[i], i), i == doc.page)
                                };
                                let r = ui
                                    .vertical(|ui| {
                                        ui.set_width(170.0);
                                        let img = egui::Image::new(&t)
                                            .max_size(Vec2::splat(160.0))
                                            .sense(Sense::click());
                                        let r = ui.add(img);
                                        if active {
                                            ui.painter().rect_stroke(
                                                r.rect.expand(2.0),
                                                2.0,
                                                egui::Stroke::new(
                                                    2.0,
                                                    ui.visuals().selection.bg_fill,
                                                ),
                                                egui::StrokeKind::Outside,
                                            );
                                        }
                                        ui.label(name);
                                        r
                                    })
                                    .inner;
                                if r.clicked() {
                                    jump = Some(i);
                                }
                            }
                        });
                    });
                });
            if let Some(j) = jump {
                self.go(doc, j, out);
                open = false;
            }
            self.grid = open;
        }
    }
}

/// Desenha uma aba: fundo, faixa da etiqueta de cor, texto e cadeado.
#[allow(clippy::too_many_arguments)]
fn paint_tab(
    ui: &egui::Ui,
    rect: Rect,
    text: &str,
    active: bool,
    selected: bool,
    tag: Option<Color32>,
    locked: bool,
    faded: bool,
) {
    let v = ui.visuals();
    let mut fill = if active {
        v.selection.bg_fill
    } else if selected {
        v.selection.bg_fill.gamma_multiply(0.45)
    } else {
        v.widgets.inactive.weak_bg_fill
    };
    let mut color = if active {
        v.selection.stroke.color
    } else {
        v.text_color()
    };
    if faded {
        fill = fill.gamma_multiply(0.5);
        color = color.gamma_multiply(0.5);
    }
    let p = ui.painter();
    p.rect_filled(rect, 4.0, fill);
    if let Some(t) = tag {
        let strip = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 3.0));
        p.rect_filled(strip, 2.0, t);
    }
    let mut tx = rect.min.x + 10.0;
    if locked {
        let r = Rect::from_center_size(Pos2::new(tx + 6.0, rect.center().y), Vec2::splat(13.0));
        egui::Image::new(icons::get("lock-on"))
            .tint(color)
            .paint_at(ui, r);
        tx += 16.0;
    }
    p.text(
        Pos2::new(tx, rect.center().y),
        egui::Align2::LEFT_CENTER,
        text,
        egui::TextStyle::Button.resolve(ui.style()),
        color,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(ctx: &egui::Context, bar: &mut PageBar, doc: &mut DocState) -> Rect {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1366.0, 768.0))),
            ..Default::default()
        };
        let mut used = Rect::NOTHING;
        let mut out = ctx.run_ui(input, |ui| {
            egui::Panel::bottom("paginas").show(ui, |ui| {
                doc.sync();
                bar.show(ui, doc);
                used = ui.min_rect();
            });
        });
        out.textures_delta.clear();
        used
    }

    #[test]
    fn bar_draws_pages_master_and_overview() {
        let ctx = egui::Context::default();
        let mut doc = DocState::new_blank();
        pages::add_after(&mut doc.editor, 0);
        pages::add_after(&mut doc.editor, 1);
        let mut bar = PageBar::default();
        for _ in 0..3 {
            let r = frame(&ctx, &mut bar, &mut doc);
            assert!(r.height() < 60.0, "barra alta demais: {r:?}");
        }
        bar.grid = true;
        frame(&ctx, &mut bar, &mut doc);
        frame(&ctx, &mut bar, &mut doc);
        assert!(bar.grid, "visão geral continua aberta");
        doc.set_page(MASTER);
        frame(&ctx, &mut bar, &mut doc);
        assert!(doc.is_master());
        assert_eq!(
            doc.page_size(),
            (210_000.0, 297_000.0),
            "mestra usa o tamanho da última página"
        );
    }
}
