//! Seletor de cor da janela de preenchimento: aba Modelos (roda de matiz
//! com o quadrado de saturação e brilho, canais do modelo e hexadecimal) e
//! aba Paletas (escolha da paleta, busca e grade de amostras que também se
//! arrastam até a rampa do gradiente), mais o botão do conta-gotas.

use std::sync::{Arc, Mutex};

use eframe::egui::{self, Color32, Pos2, Vec2};
use graphdraw_core::model::Color;
use graphdraw_core::model::color::{hsb_to_rgb, rgb_to_hsb};

use crate::i18n::tr;
use crate::icons;
use crate::theme;
use crate::widgets::{self, to_color32};

/// Paletas oferecidas: nome e cores (nome, cor).
pub type PaletteList = Vec<(String, Vec<(String, Color)>)>;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Tab {
    #[default]
    Models,
    Palettes,
}

/// Estado do seletor (aba, paleta escolhida e busca).
#[derive(Debug, Clone, Default)]
pub struct ColorPicker {
    tab: Tab,
    palette: usize,
    search: String,
}

/// O que o seletor devolveu neste quadro.
#[derive(Debug, Clone, Default)]
pub struct PickOut {
    pub color: Option<Color>,
    /// O conta-gotas foi pedido.
    pub eyedrop: bool,
}

/// Valores da cor no modelo dela, para a dica (ex.: "C 100 M 50 Y 0 K 0").
pub fn color_text(c: &Color) -> String {
    let names: &[&str] = match c.model.as_str() {
        "cmyk" => &["C", "M", "Y", "K"],
        "rgb" => &["R", "G", "B"],
        "hsb" => &["H", "S", "B"],
        "hls" => &["H", "L", "S"],
        "lab" => &["L", "a", "b"],
        "gray" => &["K"],
        _ => &[],
    };
    let scale = |i: usize| match c.model.as_str() {
        "rgb" => 255.0,
        "hsb" | "hls" if i == 0 => 360.0,
        "lab" => 1.0,
        _ => 100.0,
    };
    let vals: Vec<String> = names
        .iter()
        .enumerate()
        .map(|(i, n)| {
            let mut v = c.values.get(i).copied().unwrap_or(0.0);
            if c.model == "gray" {
                v = 1.0 - v;
            }
            format!("{n} {}", (v * scale(i)).round())
        })
        .collect();
    if vals.is_empty() {
        c.to_hex()
    } else {
        format!("{} · {}", vals.join(" "), c.to_hex())
    }
}

impl ColorPicker {
    /// Desenha o seletor para `color`; `id` distingue seletores na mesma janela.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        id: &str,
        color: &Color,
        palettes: &PaletteList,
    ) -> PickOut {
        let mut out = PickOut::default();
        ui.horizontal(|ui| {
            for (t, label) in [
                (Tab::Models, tr("Modelos", "Models")),
                (Tab::Palettes, tr("Paletas", "Palettes")),
            ] {
                if ui.selectable_label(self.tab == t, label).clicked() {
                    self.tab = t;
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if icons::button(
                    ui,
                    "tool-eyedropper",
                    tr(
                        "Conta-gotas: clique em qualquer ponto da janela do programa",
                        "Eyedropper: click anywhere in the program window",
                    ),
                    false,
                    true,
                )
                .clicked()
                {
                    out.eyedrop = true;
                }
            });
        });
        match self.tab {
            Tab::Models => {
                let mut cur = color.clone();
                if let Some(c) = hue_wheel(ui, ui.make_persistent_id(("roda", id)), &cur) {
                    cur = c.clone();
                    out.color = Some(c);
                }
                if let Some(c) = widgets::color_channels(ui, &cur) {
                    out.color = Some(c);
                }
            }
            Tab::Palettes => {
                if let Some(c) = self.palettes_tab(ui, id, palettes) {
                    out.color = Some(c);
                }
            }
        }
        out
    }

    fn palettes_tab(
        &mut self,
        ui: &mut egui::Ui,
        id: &str,
        palettes: &PaletteList,
    ) -> Option<Color> {
        let mut out = None;
        if palettes.is_empty() {
            ui.weak(tr("Nenhuma paleta", "No palettes"));
            return None;
        }
        self.palette = self.palette.min(palettes.len() - 1);
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt(("paleta-seletor", id))
                .selected_text(&palettes[self.palette].0)
                .width(theme::FONT_COMBO_WIDTH)
                .show_ui(ui, |ui| {
                    for (i, (name, _)) in palettes.iter().enumerate() {
                        ui.selectable_value(&mut self.palette, i, name);
                    }
                });
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text(tr("Buscar nome ou código", "Search name or code"))
                    .desired_width(theme::FONT_STYLE_WIDTH),
            );
        });
        let q = self.search.trim().to_lowercase();
        let colors: Vec<&(String, Color)> = palettes[self.palette]
            .1
            .iter()
            .filter(|(n, c)| {
                q.is_empty()
                    || n.to_lowercase().contains(&q)
                    || c.to_hex().to_lowercase().contains(&q)
            })
            .collect();
        egui::ScrollArea::vertical()
            .id_salt(("paleta-grade", id))
            .max_height(theme::PICKER_GRID_HEIGHT)
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    for (name, c) in &colors {
                        let tip = if name.is_empty() {
                            color_text(c)
                        } else {
                            format!("{name}\n{}", color_text(c))
                        };
                        let r =
                            crate::paletteui::swatch_drag(ui, c, theme::SWATCH).on_hover_text(tip);
                        if r.clicked() {
                            out = Some(c.clone());
                        }
                    }
                });
                if colors.is_empty() {
                    ui.weak(tr("Nada encontrado", "Nothing found"));
                }
            });
        out
    }
}

/// Roda de matiz com o quadrado de saturação (x) e brilho (y) dentro. O
/// matiz fica guardado em `id` para não se perder nos cinzas. Devolve a cor
/// no modelo de `color`.
pub fn hue_wheel(ui: &mut egui::Ui, id: egui::Id, color: &Color) -> Option<Color> {
    let size = theme::HUE_WHEEL;
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::click_and_drag());
    let [h0, s, b] = rgb_to_hsb(color.to_srgb());
    let stored: Option<f64> = ui.data(|d| d.get_temp(id));
    // Sem saturação o matiz é indefinido: vale o último usado.
    let h = if s < 1e-6 || b < 1e-6 {
        stored.unwrap_or(h0)
    } else {
        h0
    };
    let c = rect.center();
    let outer = size / 2.0;
    let inner = outer - theme::HUE_RING;
    let half = (inner - theme::SPACE_200) / std::f32::consts::SQRT_2;
    let sq = egui::Rect::from_center_size(c, Vec2::splat(half * 2.0));
    let rgb32 = |h: f64, s: f64, v: f64| {
        let [r, g, bb] = hsb_to_rgb(h, s, v);
        let q = |x: f64| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
        Color32::from_rgb(q(r), q(g), q(bb))
    };
    // Anel: matiz crescendo no sentido anti-horário a partir da direita.
    let mut mesh = egui::Mesh::default();
    let n = 96;
    let at = |a: f32, r: f32| c + Vec2::new(a.cos() * r, -a.sin() * r);
    for k in 0..n {
        let (a0, a1) = (
            k as f32 / n as f32 * std::f32::consts::TAU,
            (k + 1) as f32 / n as f32 * std::f32::consts::TAU,
        );
        let (c0, c1) = (
            rgb32(k as f64 / n as f64, 1.0, 1.0),
            rgb32((k + 1) as f64 / n as f64, 1.0, 1.0),
        );
        let i = mesh.vertices.len() as u32;
        mesh.colored_vertex(at(a0, inner), c0);
        mesh.colored_vertex(at(a0, outer), c0);
        mesh.colored_vertex(at(a1, outer), c1);
        mesh.colored_vertex(at(a1, inner), c1);
        mesh.add_triangle(i, i + 1, i + 2);
        mesh.add_triangle(i, i + 2, i + 3);
    }
    // Quadrado: saturação na horizontal, brilho de cima (1) para baixo (0).
    let g = 24;
    let base = mesh.vertices.len() as u32;
    for y in 0..=g {
        for x in 0..=g {
            let (fx, fy) = (x as f32 / g as f32, y as f32 / g as f32);
            let p = Pos2::new(sq.left() + fx * sq.width(), sq.top() + fy * sq.height());
            mesh.colored_vertex(p, rgb32(h, fx as f64, 1.0 - fy as f64));
        }
    }
    for y in 0..g {
        for x in 0..g {
            let i = base + (y * (g + 1) + x) as u32;
            let j = i + (g + 1) as u32;
            mesh.add_triangle(i, i + 1, j + 1);
            mesh.add_triangle(i, j + 1, j);
        }
    }
    let p = ui.painter();
    p.add(egui::Shape::mesh(mesh));
    let pal = theme::palette(ui);
    let mark = |p: &egui::Painter, at: Pos2| {
        p.circle_stroke(
            at,
            theme::HANDLE_SIZE / 2.0 + 1.0,
            egui::Stroke::new(theme::FOCUS_WIDTH, Color32::BLACK),
        );
        p.circle_stroke(
            at,
            theme::HANDLE_SIZE / 2.0,
            egui::Stroke::new(theme::BORDER_WIDTH, Color32::WHITE),
        );
    };
    let ha = (h * std::f64::consts::TAU) as f32;
    mark(p, at(ha, (inner + outer) / 2.0));
    mark(
        p,
        Pos2::new(
            sq.left() + s as f32 * sq.width(),
            sq.top() + (1.0 - b as f32) * sq.height(),
        ),
    );
    p.rect_stroke(
        sq,
        theme::RADIUS_NONE,
        egui::Stroke::new(theme::BORDER_WIDTH, pal.fg_muted),
        egui::StrokeKind::Outside,
    );
    // Arrasto: o alvo (anel ou quadrado) é decidido no toque.
    let target_id = id.with("alvo");
    let pointer = resp.interact_pointer_pos();
    if (resp.drag_started() || resp.clicked())
        && let Some(q) = ui.input(|i| i.pointer.press_origin()).or(pointer)
    {
        let d = (q - c).length();
        let t: u8 = if sq.expand(theme::SPACE_200).contains(q) {
            2
        } else if d >= inner - theme::SPACE_200 && d <= outer + theme::SPACE_200 {
            1
        } else {
            0
        };
        ui.data_mut(|m| m.insert_temp(target_id, t));
    }
    let target: u8 = ui.data(|m| m.get_temp(target_id)).unwrap_or(0);
    if (resp.dragged() || resp.clicked())
        && let Some(q) = pointer
    {
        let (nh, ns, nb) = match target {
            1 => {
                let v = q - c;
                let a = (-v.y).atan2(v.x) as f64;
                (
                    a.rem_euclid(std::f64::consts::TAU) / std::f64::consts::TAU,
                    s,
                    b,
                )
            }
            2 => (
                h,
                ((q.x - sq.left()) / sq.width()).clamp(0.0, 1.0) as f64,
                (1.0 - (q.y - sq.top()) / sq.height()).clamp(0.0, 1.0) as f64,
            ),
            _ => return None,
        };
        ui.data_mut(|m| m.insert_temp(id, nh));
        let [r, g, bb] = hsb_to_rgb(nh, ns, nb);
        let rgb = Color::rgb(r, g, bb);
        return Some(if color.model == "rgb" {
            rgb
        } else {
            widgets::convert_model(&rgb, &color.model)
        });
    }
    if resp.drag_stopped() {
        ui.data_mut(|m| m.remove::<u8>(target_id));
    }
    None
}

/// Botão de amostra da cor (abre o seletor no balão): devolve a resposta.
pub fn color_button(ui: &mut egui::Ui, c: &Color, open: bool) -> egui::Response {
    let side = theme::BUTTON_COMPACT;
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(side), egui::Sense::click());
    let p = ui.painter();
    p.rect_filled(rect.shrink(3.0), theme::RADIUS_XS, to_color32(c));
    let stroke = if open || resp.hovered() {
        egui::Stroke::new(theme::FOCUS_WIDTH, theme::ACCENT_PRIMARY)
    } else {
        theme::border_default(ui)
    };
    p.rect_stroke(
        rect.shrink(1.0),
        theme::RADIUS_XS,
        stroke,
        egui::StrokeKind::Inside,
    );
    resp
}

/// Conta-gotas da janela: esconde a janela, fotografa a tela, mostra a foto
/// congelada com a lupa e espera o clique.
enum Eye {
    Off,
    Hide,
    Wait(Arc<Mutex<Option<Arc<egui::ColorImage>>>>, u32),
    Pick(egui::TextureHandle, Arc<egui::ColorImage>),
}

/// Conta-gotas compartilhado pelas janelas de preenchimento e contorno.
pub struct Eyedropper {
    state: Eye,
}

impl Default for Eyedropper {
    fn default() -> Self {
        Eyedropper { state: Eye::Off }
    }
}

impl Eyedropper {
    /// Começa: a janela deve sumir enquanto [`Self::active`].
    pub fn start(&mut self) {
        self.state = Eye::Hide;
    }

    pub fn active(&self) -> bool {
        !matches!(self.state, Eye::Off)
    }

    /// Roda um quadro do conta-gotas; devolve a cor quando escolhida.
    pub fn run(&mut self, ctx: &egui::Context) -> Option<Color> {
        match &mut self.state {
            Eye::Off => {}
            Eye::Hide => {
                let slot: Arc<Mutex<Option<Arc<egui::ColorImage>>>> = Arc::new(Mutex::new(None));
                let (s, c) = (slot.clone(), ctx.clone());
                ctx.request_screenshot(move |img| {
                    if let Ok(mut v) = s.lock() {
                        *v = Some(img);
                    }
                    c.request_repaint();
                });
                self.state = Eye::Wait(slot, 0);
                ctx.request_repaint();
            }
            Eye::Wait(slot, frames) => {
                let img = slot.lock().ok().and_then(|mut v| v.take());
                if let Some(img) = img {
                    let tex = ctx.load_texture(
                        "conta-gotas-tela",
                        (*img).clone(),
                        egui::TextureOptions::NEAREST,
                    );
                    self.state = Eye::Pick(tex, img);
                } else if *frames > 120 {
                    self.state = Eye::Off;
                } else {
                    *frames += 1;
                    ctx.request_repaint();
                }
            }
            Eye::Pick(tex, img) => {
                let screen = ctx.content_rect();
                let (tex, img) = (tex.clone(), img.clone());
                let mut picked = None;
                let mut cancel = ctx.input(|i| i.key_pressed(egui::Key::Escape));
                egui::Area::new(egui::Id::unique("conta-gotas-janela"))
                    .order(egui::Order::Foreground)
                    .fixed_pos(screen.min)
                    .show(ctx, |ui| {
                        let (r, resp) = ui.allocate_exact_size(screen.size(), egui::Sense::click());
                        let p = ui.painter();
                        p.image(
                            tex.id(),
                            r,
                            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                            Color32::WHITE,
                        );
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
                        let (sx, sy) = (
                            img.size[0] as f32 / r.width(),
                            img.size[1] as f32 / r.height(),
                        );
                        let px = |q: egui::Pos2| {
                            let x = ((q.x - r.left()) * sx) as i64;
                            let y = ((q.y - r.top()) * sy) as i64;
                            (
                                x.clamp(0, img.size[0] as i64 - 1) as usize,
                                y.clamp(0, img.size[1] as i64 - 1) as usize,
                            )
                        };
                        if let Some(q) = resp.hover_pos() {
                            // Lupa: 11 × 11 pixels ampliados, o do meio marcado.
                            let (cx, cy) = px(q);
                            let cell = theme::LOUPE_CELL;
                            let n = 11i64;
                            let origin = q + Vec2::new(theme::SPACE_600, theme::SPACE_600);
                            let lens =
                                egui::Rect::from_min_size(origin, Vec2::splat(cell * n as f32));
                            p.rect_filled(lens.expand(2.0), theme::RADIUS_XS, Color32::BLACK);
                            for dy in 0..n {
                                for dx in 0..n {
                                    let x = (cx as i64 + dx - n / 2)
                                        .clamp(0, img.size[0] as i64 - 1)
                                        as usize;
                                    let y = (cy as i64 + dy - n / 2)
                                        .clamp(0, img.size[1] as i64 - 1)
                                        as usize;
                                    let c = img.pixels[y * img.size[0] + x];
                                    let cr = egui::Rect::from_min_size(
                                        origin + Vec2::new(dx as f32 * cell, dy as f32 * cell),
                                        Vec2::splat(cell),
                                    );
                                    p.rect_filled(cr, 0.0, c);
                                }
                            }
                            let mid = egui::Rect::from_min_size(
                                origin + Vec2::splat(cell * (n / 2) as f32),
                                Vec2::splat(cell),
                            );
                            p.rect_stroke(
                                mid,
                                0.0,
                                egui::Stroke::new(theme::FOCUS_WIDTH, Color32::WHITE),
                                egui::StrokeKind::Outside,
                            );
                            let c = img.pixels[cy * img.size[0] + cx];
                            p.text(
                                lens.left_bottom() + Vec2::new(0.0, theme::SPACE_200),
                                egui::Align2::LEFT_TOP,
                                format!("#{:02X}{:02X}{:02X}", c.r(), c.g(), c.b()),
                                theme::font_micro(),
                                Color32::WHITE,
                            );
                        }
                        if resp.clicked()
                            && let Some(q) = resp.interact_pointer_pos()
                        {
                            let (x, y) = px(q);
                            let c = img.pixels[y * img.size[0] + x];
                            picked = Some(Color::rgb(
                                c.r() as f64 / 255.0,
                                c.g() as f64 / 255.0,
                                c.b() as f64 / 255.0,
                            ));
                        }
                        if resp.secondary_clicked() {
                            cancel = true;
                        }
                    });
                if picked.is_some() || cancel {
                    self.state = Eye::Off;
                }
                return picked;
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_text_by_model() {
        assert_eq!(
            color_text(&Color::cmyk(1.0, 0.5, 0.0, 0.0))
                .split(" · ")
                .next(),
            Some("C 100 M 50 Y 0 K 0")
        );
        assert!(color_text(&Color::rgb(1.0, 0.0, 0.0)).starts_with("R 255 G 0 B 0"));
    }

    /// O seletor desenha as duas abas; a busca filtra a grade.
    #[test]
    fn picker_draws_and_filters() {
        let ctx = crate::theme::test_ctx();
        let pals: PaletteList = vec![(
            "Teste".into(),
            vec![
                ("Vermelho".into(), Color::rgb(1.0, 0.0, 0.0)),
                ("Azul".into(), Color::rgb(0.0, 0.0, 1.0)),
            ],
        )];
        let mut p = ColorPicker::default();
        for tab in [Tab::Models, Tab::Palettes] {
            p.tab = tab;
            p.search = "azu".into();
            let mut out = ctx.run_ui(Default::default(), |ui| {
                egui::CentralPanel::default().show(ui, |ui| {
                    let o = p.show(ui, "t", &Color::cmyk(0.0, 0.0, 0.0, 1.0), &pals);
                    assert!(o.color.is_none() && !o.eyedrop);
                });
            });
            out.textures_delta.clear();
        }
    }
}
