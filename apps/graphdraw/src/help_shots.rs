//! The screenshots of the Help: each dialog, options page and docker
//! drawn headlessly in English on a small demo drawing, rasterised in
//! software from egui's meshes and cropped to the window.
//!
//! `web/help_shots.sh` runs `cargo test -p graphdraw help_shots -- --ignored`
//! with `HELP_SHOTS` set to the output folder (`docs/help/img`). The
//! images are named after the Help topic they illustrate (see
//! docs/behavior/help.md).

use crate::app::{App, DockerTab};
use crate::ui::dialogs::{Dialog, OptionsPage};
use egui::epaint::{Color32, Primitive, TextureId, Vertex};
use std::collections::HashMap;

/// A premultiplied RGBA canvas in 0..1.
struct Canvas {
    w: usize,
    h: usize,
    px: Vec<[f32; 4]>,
}

struct Texture {
    w: usize,
    h: usize,
    px: Vec<Color32>,
}

#[derive(Default)]
struct Textures(HashMap<TextureId, Texture>);

impl Textures {
    fn apply(&mut self, delta: &egui::TexturesDelta) {
        for (id, ds) in &delta.set {
            for d in ds.iter() {
                let egui::ImageData::Color(img) = &d.image;
                let [w, h] = img.size;
                match d.pos {
                    None => {
                        self.0.insert(
                            *id,
                            Texture {
                                w,
                                h,
                                px: img.pixels.clone(),
                            },
                        );
                    }
                    Some([x0, y0]) => {
                        if let Some(t) = self.0.get_mut(id) {
                            for y in 0..h {
                                for x in 0..w {
                                    if x0 + x < t.w && y0 + y < t.h {
                                        t.px[(y0 + y) * t.w + x0 + x] = img.pixels[y * w + x];
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn sample(&self, id: TextureId, u: f32, v: f32) -> [f32; 4] {
        let Some(t) = self.0.get(&id) else {
            return [1.0; 4];
        };
        let fx = (u * t.w as f32 - 0.5).clamp(0.0, (t.w - 1) as f32);
        let fy = (v * t.h as f32 - 0.5).clamp(0.0, (t.h - 1) as f32);
        let (x0, y0) = (fx.floor() as usize, fy.floor() as usize);
        let (x1, y1) = ((x0 + 1).min(t.w - 1), (y0 + 1).min(t.h - 1));
        let (ax, ay) = (fx - x0 as f32, fy - y0 as f32);
        let c = |x: usize, y: usize| t.px[y * t.w + x].to_array().map(|b| b as f32 / 255.0);
        let (a, b, cc, d) = (c(x0, y0), c(x1, y0), c(x0, y1), c(x1, y1));
        let mut out = [0.0; 4];
        for i in 0..4 {
            let top = a[i] + (b[i] - a[i]) * ax;
            let bot = cc[i] + (d[i] - cc[i]) * ax;
            out[i] = top + (bot - top) * ay;
        }
        out
    }
}

impl Canvas {
    fn new(w: usize, h: usize) -> Self {
        Canvas {
            w,
            h,
            px: vec![[0.0; 4]; w * h],
        }
    }

    fn triangle(&mut self, tex: &Textures, id: TextureId, v: [&Vertex; 3], clip: egui::Rect) {
        let (p0, p1, p2) = (v[0].pos, v[1].pos, v[2].pos);
        let area = (p1.x - p0.x) * (p2.y - p0.y) - (p2.x - p0.x) * (p1.y - p0.y);
        if area.abs() < 1e-6 {
            return;
        }
        let minx = p0.x.min(p1.x).min(p2.x).max(clip.min.x).max(0.0).floor() as i64;
        let maxx =
            p0.x.max(p1.x)
                .max(p2.x)
                .min(clip.max.x)
                .min(self.w as f32)
                .ceil() as i64;
        let miny = p0.y.min(p1.y).min(p2.y).max(clip.min.y).max(0.0).floor() as i64;
        let maxy =
            p0.y.max(p1.y)
                .max(p2.y)
                .min(clip.max.y)
                .min(self.h as f32)
                .ceil() as i64;
        let col = v.map(|x| x.color.to_array().map(|b| b as f32 / 255.0));
        for y in miny..maxy {
            for x in minx..maxx {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                if px < clip.min.x || px > clip.max.x || py < clip.min.y || py > clip.max.y {
                    continue;
                }
                let w0 = ((p1.x - px) * (p2.y - py) - (p2.x - px) * (p1.y - py)) / area;
                let w1 = ((p2.x - px) * (p0.y - py) - (p0.x - px) * (p2.y - py)) / area;
                let w2 = 1.0 - w0 - w1;
                if w0 < -1e-4 || w1 < -1e-4 || w2 < -1e-4 {
                    continue;
                }
                let u = w0 * v[0].uv.x + w1 * v[1].uv.x + w2 * v[2].uv.x;
                let vv = w0 * v[0].uv.y + w1 * v[1].uv.y + w2 * v[2].uv.y;
                let t = tex.sample(id, u, vv);
                let mut src = [0.0; 4];
                for i in 0..4 {
                    src[i] = (w0 * col[0][i] + w1 * col[1][i] + w2 * col[2][i]) * t[i];
                }
                let d = &mut self.px[y as usize * self.w + x as usize];
                for i in 0..4 {
                    d[i] = src[i] + d[i] * (1.0 - src[3]);
                }
            }
        }
    }

    fn paint(&mut self, tex: &Textures, prims: &[egui::ClippedPrimitive]) {
        for p in prims {
            if let Primitive::Mesh(m) = &p.primitive {
                for tri in m.indices.chunks_exact(3) {
                    let v = [
                        &m.vertices[tri[0] as usize],
                        &m.vertices[tri[1] as usize],
                        &m.vertices[tri[2] as usize],
                    ];
                    self.triangle(tex, m.texture_id, v, p.clip_rect);
                }
            }
        }
    }

    /// The box of the pixels drawn at all (a window and its shadow).
    fn drawn_box(&self) -> Option<(usize, usize, usize, usize)> {
        let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0, 0);
        for y in 0..self.h {
            for x in 0..self.w {
                if self.px[y * self.w + x][3] > 0.02 {
                    x0 = x0.min(x);
                    y0 = y0.min(y);
                    x1 = x1.max(x + 1);
                    y1 = y1.max(y + 1);
                }
            }
        }
        (x1 > x0 && y1 > y0).then_some((x0, y0, x1, y1))
    }

    /// The panel's height without the empty rows at its bottom.
    fn used_height(&self, from: usize) -> usize {
        let row = |y: usize| &self.px[y * self.w..(y + 1) * self.w];
        let last = row(self.h - 1).to_vec();
        let mut y = self.h - 1;
        while y > from
            && row(y)
                .iter()
                .zip(&last)
                .all(|(a, b)| (a[0] - b[0]).abs() < 0.01)
        {
            y -= 1;
        }
        (y + 14).min(self.h)
    }

    fn save(&self, path: &std::path::Path, (x0, y0, x1, y1): (usize, usize, usize, usize)) {
        let mut img = image::RgbaImage::new((x1 - x0) as u32, (y1 - y0) as u32);
        for y in y0..y1 {
            for x in x0..x1 {
                let p = self.px[y * self.w + x];
                let a = p[3];
                let un = |c: f32| {
                    if a > 0.0 {
                        (c / a * 255.0).round().clamp(0.0, 255.0) as u8
                    } else {
                        0
                    }
                };
                img.put_pixel(
                    (x - x0) as u32,
                    (y - y0) as u32,
                    image::Rgba([un(p[0]), un(p[1]), un(p[2]), (a * 255.0).round() as u8]),
                );
            }
        }
        let file = std::fs::File::create(path).unwrap();
        let enc = image::codecs::png::PngEncoder::new_with_quality(
            std::io::BufWriter::new(file),
            image::codecs::png::CompressionType::Best,
            image::codecs::png::FilterType::Adaptive,
        );
        image::ImageEncoder::write_image(
            enc,
            img.as_raw(),
            img.width(),
            img.height(),
            image::ExtendedColorType::Rgba8,
        )
        .unwrap();
    }
}

/// What a screenshot shows.
enum Shot {
    /// A menu command that opens a dialog.
    Command(&'static str),
    /// A dialog set directly (its state built by the app).
    Dialog(fn(&mut App) -> Dialog),
    Options(OptionsPage),
    Docker(DockerTab),
}

const W: usize = 1400;
const H: usize = 1000;

struct Shooter {
    ctx: egui::Context,
    tex: Textures,
    time: f64,
}

impl Shooter {
    fn new() -> Self {
        let ctx = egui::Context::default();
        crate::theme::install_fonts_bundled(&ctx);
        App::setup_context(&ctx);
        Shooter {
            ctx,
            tex: Textures::default(),
            time: 0.0,
        }
    }

    fn frame(&mut self, app: &mut App, mut body: impl FnMut(&mut App, &egui::Context)) -> Canvas {
        self.time += 0.5;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(W as f32, H as f32),
            )),
            time: Some(self.time),
            ..Default::default()
        };
        let mut out = self.ctx.run_ui(input, |ui| {
            let ctx = ui.ctx().clone();
            crate::ui::apply_theme(&ctx, &crate::settings::effective_theme(&app.settings));
            crate::ui::apply_shadows(&ctx, app.settings.ui_shadows);
            body(app, &ctx);
        });
        self.tex.apply(&out.textures_delta);
        let shapes = std::mem::take(&mut out.shapes);
        let prims = self.ctx.tessellate(shapes, out.pixels_per_point);
        let mut c = Canvas::new(W, H);
        c.paint(&self.tex, &prims);
        for id in &out.textures_delta.free {
            self.tex.0.remove(id);
        }
        out.textures_delta.clear();
        c
    }
}

/// A drawing to show: a filled rectangle, an ellipse, a line of text
/// and a small generated bitmap, the rectangle selected.
fn demo_app() -> App {
    use graphdraw_core::geometry::{Point, Rect};
    use graphdraw_core::{Color, Fill, ShapeKind};
    let mut app = App::headless();
    crate::i18n::set_language("en");
    let rect = app.new_shape(ShapeKind::Rect {
        rect: Rect::new(30.0, 40.0, 110.0, 100.0),
        radius: 0.0,
        corners: None,
    });
    let _ = app.new_shape(ShapeKind::Ellipse {
        rect: Rect::new(120.0, 50.0, 180.0, 110.0),
        arc: None,
    });
    let _ = app.new_shape(ShapeKind::Text {
        spans: vec![graphdraw_core::TextSpan::new(
            "Sample text",
            "Selawik",
            24.0,
        )],
        origin: Point::new(30.0, 150.0),
        frame: None,
        align: Default::default(),
        para: Default::default(),
        on_path: None,
    });
    if let Some(id) = rect {
        app.select(vec![id]);
        app.apply_fill(Fill::Solid(Color::rgb8(70, 130, 220)));
    }
    app
}

fn demo_bitmap(app: &mut App) {
    let img = image::RgbaImage::from_fn(96, 64, |x, y| {
        image::Rgba([(x * 2 + 40) as u8, (y * 3 + 60) as u8, 180, 255])
    });
    if let Some(id) = app.place_image(img, Some("Demo".into())) {
        app.select(vec![id]);
    }
}

/// The screenshots: file name (the topic, `/` and `#` as `-`), what to
/// show and whether a bitmap is selected first.
fn shots() -> Vec<(String, Shot, bool)> {
    use OptionsPage as P;
    let mut v: Vec<(String, Shot, bool)> = Vec::new();
    let name = |t: &str| t.replace(['/', '#'], "-");
    let cmd = [
        ("menu.file.new", false),
        ("menu.file.new_from_template", false),
        ("menu.file.save_as", false),
        ("menu.file.document_properties", false),
        ("menu.file.export", false),
        ("menu.file.print", false),
        ("menu.file.print_merge_create", false),
        ("menu.file.preflight", false),
        ("menu.file.publish_to_pdf", false),
        ("menu.file.github_open", false),
        ("menu.file.github_save", false),
        ("menu.edit.paste_special", false),
        ("menu.layout.insert_page", false),
        ("menu.layout.rename_page", false),
        ("menu.layout.go_to_page", false),
        ("menu.layout.page_size", false),
        ("menu.layout.page_number_settings", false),
        ("menu.table.create_new_table", false),
        ("menu.object.create_pattern", false),
        ("menu.object.insert_barcode", false),
        ("menu.bitmaps.convert_to_bitmap", false),
        ("menu.bitmaps.straighten_image", true),
        ("menu.bitmaps.resample", true),
        ("menu.bitmaps.inflate_manual", true),
        ("menu.bitmaps.mode_bw", true),
        ("menu.bitmaps.mode_duotone", true),
        ("menu.bitmaps.mode_paletted", true),
        ("menu.bitmaps.trace_logo", true),
        ("menu.text.tabs", false),
        ("menu.text.columns", false),
        ("menu.text.bullets", false),
        ("menu.text.drop_cap", false),
        ("menu.text.text_statistics", false),
        ("menu.text.spell_check", false),
        ("menu.text.custom_hyphens", false),
        ("menu.text.autocorrect", false),
        ("menu.tools.border_and_grommet", false),
        ("menu.tools.color_management", false),
        ("menu.tools.font_manager", false),
        ("menu.tools.time_tracker", false),
        ("menu.window.palette_editor", false),
        ("menu.help.about", false),
    ];
    for (k, bmp) in cmd {
        v.push((k.to_string(), Shot::Command(k), bmp));
    }
    v.push((
        name("objects/objects-and-layers#rename"),
        Shot::Dialog(|app| match app.selection.first().copied() {
            Some(shape) => Dialog::RenameObject {
                shape,
                name: "Rectangle".into(),
            },
            None => Dialog::None,
        }),
        false,
    ));
    v.push((
        name("lines-shapes/shape-tool#align-nodes"),
        Shot::Dialog(|_| Dialog::NodeAlign {
            horizontal: true,
            vertical: true,
        }),
        false,
    ));
    v.push((
        name("get-started/github#history"),
        Shot::Dialog(|_| Dialog::GithubHistory),
        false,
    ));
    v.push((
        name("text/writing-tools#change-case"),
        Shot::Dialog(|_| Dialog::ChangeCase(crate::ops2::CaseMode::Sentence)),
        false,
    ));
    v.push((
        name("text/writing-tools#encode"),
        Shot::Dialog(|_| Dialog::Encode(Default::default())),
        false,
    ));
    v.push((
        name("objects/shelf#remove"),
        Shot::Dialog(|_| Dialog::ConfirmShelfRemove {
            uid: 1,
            name: "Logo".into(),
            dont_ask: false,
        }),
        false,
    ));
    v.push((
        name("file-formats/publish-pdf#password"),
        Shot::Dialog(|_| {
            Dialog::PdfPassword(crate::ui::pdf_password::PdfPasswordState {
                path: "brochure.pdf".into(),
                import: false,
                password: String::new(),
                wrong: false,
                focused_once: true,
            })
        }),
        false,
    ));
    v.push((
        name("templates-styles/object-styles#document-defaults"),
        Shot::Dialog(|_| {
            Dialog::ChangeDefaults(crate::ui::defaults_dialog::State {
                change: crate::defaults::DefaultChange::Fill(graphdraw_core::Fill::Solid(
                    graphdraw_core::Color::rgb8(70, 130, 220),
                )),
                kinds: [true, false, false],
                dont_ask: false,
            })
        }),
        false,
    ));
    v.push((
        name("effects/bitmap-effects"),
        Shot::Dialog(|app| {
            crate::ui::effect_dialog::choose_effect(app, "gaussian_blur");
            std::mem::replace(&mut app.dialog, Dialog::None)
        }),
        true,
    ));
    v.push((
        name("customize/reset-settings"),
        Shot::Dialog(|_| Dialog::ConfirmResetSettings),
        false,
    ));
    v.push((
        name("get-started/open-and-save#closing"),
        Shot::Dialog(|_| Dialog::ConfirmClose),
        false,
    ));
    v.push((
        name("get-started/browser-version"),
        Shot::Dialog(|_| Dialog::WebNotice),
        false,
    ));
    v.push((
        name("welcome/about-and-updates#updates"),
        Shot::Dialog(|_| Dialog::Updates),
        false,
    ));
    for s in [
        crate::ui::options::OptionsSet::App,
        crate::ui::options::OptionsSet::Customization,
        crate::ui::options::OptionsSet::Tools,
        crate::ui::options::OptionsSet::Global,
        crate::ui::options::OptionsSet::Workspaces,
        crate::ui::options::OptionsSet::Ai,
        crate::ui::options::OptionsSet::Document,
    ] {
        for p in s.pages() {
            v.push((
                name(crate::help::options_topic(*p)),
                Shot::Options(*p),
                false,
            ));
        }
    }
    for t in DockerTab::ALL {
        let _ = P::General;
        v.push((name(crate::help::docker_topic(t)), Shot::Docker(t), false));
    }
    v
}

/// Draw every screenshot into `$HELP_SHOTS` (run by `web/help_shots.sh`).
#[test]
#[ignore]
fn help_shots() {
    let Some(dir) = std::env::var_os("HELP_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut made = Vec::new();
    let mut missed = Vec::new();
    let only: Vec<String> = std::env::var("HELP_SHOTS_ONLY")
        .map(|v| v.split(',').map(str::to_string).collect())
        .unwrap_or_default();
    for (file, shot, bitmap) in shots() {
        if !only.is_empty() && !only.iter().any(|o| file.starts_with(o.as_str())) {
            continue;
        }
        eprintln!("shot {file}");
        let mut sh = Shooter::new();
        let mut app = demo_app();
        if bitmap {
            demo_bitmap(&mut app);
        }
        let mut file = file;
        match &shot {
            Shot::Command(k) => {
                let mut ran = false;
                sh.frame(&mut app, |app, ctx| {
                    ran = crate::ui::menus::run_command(app, ctx, k)
                });
                let topic = crate::help::dialog_topic(&app.dialog, app.options_page);
                if !ran || matches!(app.dialog, Dialog::None) {
                    missed.push(format!("{k}: no dialog"));
                    continue;
                }
                file = topic.replace(['/', '#'], "-");
            }
            Shot::Dialog(f) => app.dialog = f(&mut app),
            Shot::Options(p) => {
                app.options_page = *p;
                app.dialog = Dialog::Options;
            }
            Shot::Docker(t) => {
                app.show_dockers = true;
                if !app.open_dockers.contains(t) {
                    app.open_dockers.push(*t);
                }
                app.docker_tab = *t;
            }
        }
        let docker = matches!(shot, Shot::Docker(_));
        let mut canvas = None;
        for _ in 0..4 {
            canvas = Some(sh.frame(&mut app, |app, ctx| {
                if docker {
                    let rect =
                        egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(340.0, 940.0));
                    let id = egui::Id::new("help_shot_docker");
                    let mut ui = egui::Ui::new(
                        ctx.clone(),
                        id,
                        egui::UiBuilder::new()
                            .max_rect(rect)
                            .layer_id(egui::LayerId::new(egui::Order::Middle, id)),
                    );
                    ui.painter()
                        .rect_filled(rect, 0.0, crate::theme::Tokens::field());
                    crate::ui::dockers::dockers(app, &mut ui);
                } else {
                    crate::ui::dialogs::show(app, ctx);
                }
            }));
            if !docker && matches!(app.dialog, Dialog::None) {
                break;
            }
        }
        let Some(c) = canvas else { continue };
        let area = if docker {
            Some((0, 0, 340, c.used_height(120)))
        } else {
            c.drawn_box()
        };
        let Some(area) = area else {
            missed.push(format!("{file}: nothing drawn"));
            continue;
        };
        let path = dir.join(format!("{file}.png"));
        c.save(&path, area);
        made.push(file);
    }
    let list = made.join("\n") + "\n";
    std::fs::write(dir.join("shots.txt"), list).unwrap();
    eprintln!("{} screenshots; not made: {missed:#?}", made.len());
}

/// The README screenshots: the whole application drawn headlessly with a
/// drawing open and fitted to the window, into `$README_SHOTS`. The
/// drawings are listed in `$README_SHOTS_FILES` as `name=path` pairs
/// separated by `|`; `name+` also selects every object of the page. The
/// interface language is `$README_SHOTS_LANG` (Portuguese by default).
#[test]
#[ignore]
fn readme_shots() {
    let Some(dir) = std::env::var_os("README_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let files = std::env::var("README_SHOTS_FILES").unwrap_or_default();
    let mut sh = Shooter::new();
    for entry in files.split('|').filter(|e| !e.is_empty()) {
        let Some((name, path)) = entry.split_once('=') else {
            continue;
        };
        let (name, select) = match name.strip_suffix('+') {
            Some(n) => (n, true),
            None => (name, false),
        };
        // The system fonts, as the program loads them, so text is drawn.
        graphdraw_text::install();
        let mut app = App::headless();
        crate::i18n::set_language(
            &std::env::var("README_SHOTS_LANG").unwrap_or_else(|_| "pt-BR".into()),
        );
        // Only the drawing shown: the untitled one a headless app starts
        // with is closed (nothing in it to save).
        app.close_document();
        app.open_path(std::path::PathBuf::from(path));
        if select {
            app.select_all();
        }
        // A few frames: the layout settles, then the page is fitted to the
        // canvas it now knows; only the last one is rasterised (slow).
        const FRAMES: usize = 5;
        for n in 0..FRAMES {
            if n == 2 {
                app.zoom_to_page();
            }
            sh.time += 0.5;
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(W as f32, H as f32),
                )),
                time: Some(sh.time),
                ..Default::default()
            };
            let mut out = sh.ctx.run_ui(input, |ui| {
                let ctx = ui.ctx().clone();
                crate::ui::apply_theme(&ctx, &crate::settings::effective_theme(&app.settings));
                crate::ui::root(&mut app, ui);
            });
            sh.tex.apply(&out.textures_delta);
            if n + 1 == FRAMES {
                let shapes = std::mem::take(&mut out.shapes);
                let prims = sh.ctx.tessellate(shapes, out.pixels_per_point);
                let mut canvas = Canvas::new(W, H);
                canvas.paint(&sh.tex, &prims);
                canvas.save(&dir.join(format!("{name}.png")), (0, 0, W, H));
            }
            for id in &out.textures_delta.free {
                sh.tex.0.remove(id);
            }
            out.textures_delta.clear();
        }
    }
}

/// The Edit Fill and Outline Pen windows over a drawing, the whole
/// application drawn headlessly, into `$STYLE_SHOTS`: each window with a
/// change waiting (shown on the selected rectangle), then both together.
#[test]
#[ignore]
fn style_window_shots() {
    use crate::ui::fill_window::Mode;
    let Some(dir) = std::env::var_os("STYLE_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    graphdraw_text::install();
    let mut sh = Shooter::new();
    let mut app = demo_app();
    crate::i18n::set_language(
        &std::env::var("STYLE_SHOTS_LANG").unwrap_or_else(|_| "pt-BR".into()),
    );
    let mut shoot = |app: &mut App, name: &str| {
        for n in 0..6 {
            if n == 2 {
                app.zoom_to_page();
            }
            sh.time += 0.5;
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(W as f32, H as f32),
                )),
                time: Some(sh.time),
                ..Default::default()
            };
            let mut out = sh.ctx.run_ui(input, |ui| {
                let ctx = ui.ctx().clone();
                crate::ui::apply_theme(&ctx, &crate::settings::effective_theme(&app.settings));
                crate::ui::root(app, ui);
            });
            sh.tex.apply(&out.textures_delta);
            if n == 5 {
                let shapes = std::mem::take(&mut out.shapes);
                let prims = sh.ctx.tessellate(shapes, out.pixels_per_point);
                let mut canvas = Canvas::new(W, H);
                canvas.paint(&sh.tex, &prims);
                canvas.save(&dir.join(format!("{name}.png")), (0, 0, W, H));
            }
            for id in &out.textures_delta.free {
                sh.tex.0.remove(id);
            }
            out.textures_delta.clear();
        }
    };
    app.open_fill_window();
    shoot(&mut app, "preenchimento-uniforme");
    if let Some(w) = app.style_windows.fill.as_mut() {
        w.set_mode(Mode::Gradient);
        w.add_stop(0.45, Some(graphdraw_core::Color::rgb8(255, 210, 0)));
    }
    shoot(&mut app, "preenchimento-gradiente");
    app.style_windows.fill = None;
    app.open_outline_window();
    if let Some(w) = app.style_windows.outline.as_mut() {
        w.enabled = true;
        w.draft.width = 2.0;
        w.draft.dash = vec![4.0, 2.0];
        w.draft.color = graphdraw_core::Color::rgb8(200, 30, 30);
        w.mark_changed();
    }
    shoot(&mut app, "contorno");
    // Stage 2: inside and outside, see-through, dashes aligned at the
    // corners, arrowheads scaled and turned.
    for (name, pos) in [
        ("contorno-dentro", graphdraw_core::StrokePosition::Inside),
        ("contorno-fora", graphdraw_core::StrokePosition::Outside),
    ] {
        if let Some(w) = app.style_windows.outline.as_mut() {
            w.draft.width = 4.0;
            w.draft.dash.clear();
            w.draft.position = pos;
            w.draft.opacity = 0.6;
            w.mark_changed();
        }
        shoot(&mut app, name);
    }
    if let Some(w) = app.style_windows.outline.as_mut() {
        w.draft.width = 1.5;
        w.draft.opacity = 1.0;
        w.draft.position = graphdraw_core::StrokePosition::Center;
        w.draft.dash = vec![6.0, 3.0];
        w.draft.dash_corners = graphdraw_core::DashCorners::Align;
        w.draft.miter_limit = 10.0;
        w.draft.start_arrow = graphdraw_core::Arrowhead::Circle;
        w.draft.end_arrow = graphdraw_core::Arrowhead::Arrow;
        w.draft.end_arrow_options = graphdraw_core::ArrowOptions {
            scale: 2.0,
            rotation: 25.0,
            mirror: false,
            offset: 1.0,
        };
        w.mark_changed();
    }
    shoot(&mut app, "contorno-cantos-setas");
    app.open_fill_window();
    if let Some(w) = app.style_windows.fill.as_mut() {
        w.set_mode(Mode::TwoColor);
    }
    shoot(&mut app, "as-duas");
}

/// The New Document window step by step, the whole application drawn
/// headlessly, into `$NEWDOC_SHOTS`: as it opens, a search, a category,
/// lengths typed with units and sums, and the drawing it creates.
#[test]
#[ignore]
fn new_document_window_shots() {
    use crate::new_document_window::Filter;
    use graphdraw_core::page_presets::Category;
    let Some(dir) = std::env::var_os("NEWDOC_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut sh = Shooter::new();
    let mut app = App::headless();
    crate::i18n::set_language(
        &std::env::var("NEWDOC_SHOTS_LANG").unwrap_or_else(|_| "pt-BR".into()),
    );
    app.request_new_document();
    assert!(matches!(app.dialog, Dialog::NewDocument(_)));
    let key = |key: egui::Key, command: bool| egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: if command {
            egui::Modifiers::COMMAND
        } else {
            egui::Modifiers::NONE
        },
    };
    let text = |t: &str| egui::Event::Text(t.to_string());
    let mut run = |app: &mut App, events: Vec<egui::Event>, frames: usize| {
        let mut c = None;
        for n in 0..frames {
            sh.time += 0.5;
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(W as f32, H as f32),
                )),
                time: Some(sh.time),
                events: if n == 0 { events.clone() } else { Vec::new() },
                ..Default::default()
            };
            let mut out = sh.ctx.run_ui(input, |ui| {
                let ctx = ui.ctx().clone();
                crate::ui::apply_theme(&ctx, &crate::settings::effective_theme(&app.settings));
                crate::ui::root(app, ui);
            });
            sh.tex.apply(&out.textures_delta);
            // Rasterising is slow: only the last frame of a step.
            if n + 1 == frames {
                let shapes = std::mem::take(&mut out.shapes);
                let prims = sh.ctx.tessellate(shapes, out.pixels_per_point);
                let mut canvas = Canvas::new(W, H);
                canvas.paint(&sh.tex, &prims);
                c = Some(canvas);
            }
            for id in &out.textures_delta.free {
                sh.tex.0.remove(id);
            }
            out.textures_delta.clear();
        }
        c.unwrap()
    };
    let full = (0, 0, W, H);
    let window = |app: &mut App| match &mut app.dialog {
        Dialog::NewDocument(st) => Some(st.clone()),
        _ => None,
    };
    run(&mut app, vec![], 4).save(&dir.join("01-janela-inicial.png"), full);
    run(&mut app, vec![text("A5")], 3).save(&dir.join("02-busca-a5.png"), full);
    if let Dialog::NewDocument(st) = &mut app.dialog {
        st.search.clear();
        st.filter = Filter::Cat(Category::Social);
    }
    run(&mut app, vec![], 3).save(&dir.join("03-categoria-social.png"), full);
    if let Dialog::NewDocument(st) = &mut app.dialog {
        st.filter = Filter::Custom;
    }
    run(&mut app, vec![], 2);
    // Click the width field, select all and type; then the height.
    let click = |x: f32, y: f32| {
        let pos = egui::pos2(x, y);
        vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            },
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            },
        ]
    };
    run(&mut app, click(940.0, 549.0), 2);
    run(&mut app, vec![key(egui::Key::A, true), text("8.5 in")], 2)
        .save(&dir.join("04-largura-8.5in.png"), full);
    run(&mut app, click(940.0, 577.0), 2);
    run(&mut app, vec![key(egui::Key::A, true), text("100+10 mm")], 2)
        .save(&dir.join("05-altura-100+10mm.png"), full);
    run(&mut app, click(830.0, 470.0), 3).save(&dir.join("06-medidas-aplicadas.png"), full);
    let st = window(&mut app).expect("window open");
    eprintln!("width {} height {}", st.width, st.height);
    assert!((st.width - 215.9).abs() < 1e-9 && (st.height - 110.0).abs() < 1e-9);
    // Create document.
    let (x, y) = (st.rect.right() - 76.0, st.rect.bottom() - 41.0);
    run(&mut app, click(x, y), 4).save(&dir.join("07-documento-criado.png"), full);

    assert!(matches!(app.dialog, Dialog::None));
    let size = app.page_size();
    eprintln!("page {} x {} mm", size.width, size.height);
}

/// The clip frame bar and editing drawn headlessly, the whole application,
/// into `$CLIP_SHOTS` (language `$CLIP_SHOTS_LANG`, Portuguese by default):
/// a clip frame selected with its bar, its contents filling the frame,
/// editing them (dashed frame, level), one level deeper, and the notice
/// for an empty frame.
#[test]
#[ignore]
fn clip_frame_shots() {
    use graphdraw_core::geometry::{Point, Rect};
    use graphdraw_core::{Color, Command, Fill, ShapeKind};
    let Some(dir) = std::env::var_os("CLIP_SHOTS") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut sh = Shooter::new();
    let mut app = App::headless();
    crate::i18n::set_language(
        &std::env::var("CLIP_SHOTS_LANG").unwrap_or_else(|_| "pt-BR".into()),
    );
    let rect = |app: &mut App, r: Rect, c: Color| {
        let id = app
            .new_shape(ShapeKind::Rect {
                rect: r,
                radius: 0.0,
                corners: None,
            })
            .unwrap();
        app.run(Command::SetFill {
            shapes: vec![id],
            fill: Fill::Solid(c),
        });
        id
    };
    let frame = app
        .new_shape(ShapeKind::Ellipse {
            rect: Rect::new(40.0, 40.0, 140.0, 110.0),
            arc: None,
        })
        .unwrap();
    let a = rect(&mut app, Rect::new(30.0, 60.0, 90.0, 120.0), Color::rgb8(70, 130, 220));
    let b = rect(&mut app, Rect::new(80.0, 30.0, 150.0, 80.0), Color::rgb8(230, 120, 40));
    let inner_frame = app
        .new_shape(ShapeKind::Ellipse {
            rect: Rect::new(70.0, 50.0, 110.0, 90.0),
            arc: None,
        })
        .unwrap();
    let c = rect(&mut app, Rect::new(60.0, 60.0, 120.0, 80.0), Color::rgb8(40, 170, 90));
    app.place_inside(vec![c], inner_frame);
    app.place_inside(vec![a, b, inner_frame], frame);
    app.select(vec![frame]);
    app.view.zoom = 3.0;
    let mut run = |app: &mut App, frames: usize| {
        let mut c = None;
        for n in 0..frames {
            sh.time += 0.5;
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(W as f32, H as f32),
                )),
                time: Some(sh.time),
                events: vec![egui::Event::PointerMoved(egui::pos2(700.0, 500.0))],
                ..Default::default()
            };
            let mut out = sh.ctx.run_ui(input, |ui| {
                let ctx = ui.ctx().clone();
                crate::ui::apply_theme(&ctx, &crate::settings::effective_theme(&app.settings));
                crate::ui::root(app, ui);
            });
            sh.tex.apply(&out.textures_delta);
            if n + 1 == frames {
                let shapes = std::mem::take(&mut out.shapes);
                let prims = sh.ctx.tessellate(shapes, out.pixels_per_point);
                let mut canvas = Canvas::new(W, H);
                canvas.paint(&sh.tex, &prims);
                c = Some(canvas);
            }
            for id in &out.textures_delta.free {
                sh.tex.0.remove(id);
            }
            out.textures_delta.clear();
        }
        c.unwrap()
    };
    let full = (0, 0, W, H);
    run(&mut app, 3);
    let page = app.view.to_screen(Point::new(90.0, 75.0));
    let canvas_center = app.canvas_rect.center();
    app.view.origin += canvas_center - page;
    run(&mut app, 4).save(&dir.join("01-moldura-selecionada.png"), full);
    app.fit_clip_contents(crate::clip_frames::ContentFit::Fill);
    run(&mut app, 3).save(&dir.join("02-preencher-moldura.png"), full);
    app.undo();
    app.select(vec![frame]);
    app.edit_clip_frame();
    run(&mut app, 3).save(&dir.join("03-editando-nivel-1.png"), full);
    app.select(vec![inner_frame]);
    app.edit_clip_frame();
    run(&mut app, 3).save(&dir.join("04-editando-nivel-2.png"), full);
    app.finish_all_clip_frame_edits();
    let empty = rect(&mut app, Rect::new(160.0, 40.0, 200.0, 80.0), Color::rgb8(220, 220, 220));
    app.select(vec![empty]);
    app.objects_to_clip_frames();
    app.select(vec![empty]);
    app.extract_clip_contents();
    run(&mut app, 3).save(&dir.join("05-aviso-moldura-vazia.png"), full);
}

/// Every tab of the Print dialog, in Portuguese (the longer labels), on a
/// 1280 x 800 screen: the dialog keeps one size for all of them, so no tab
/// overflows its area, and it stays below the height at which the dialog
/// body starts to scroll. With `$PRINT_SHOTS` set, each tab is also saved
/// there as a PNG.
#[test]
fn print_dialog_tabs_fit_without_scrolling() {
    use crate::ui::dialogs::PrintState;
    const SW: f32 = 1280.0;
    const SH: f32 = 800.0;
    let dir = std::env::var_os("PRINT_SHOTS").map(std::path::PathBuf::from);
    let mut sh = Shooter::new();
    let mut app = demo_app();
    crate::i18n::set_language("pt-BR");
    app.settings.printing.preflight = true;
    let id = egui::Id::new(("chrome_dialog", crate::i18n::tr("dialog.print")));
    // An auto-sized window settles over a few frames.
    const FRAMES: usize = 8;
    let mut sizes = Vec::new();
    for tab in 0..7 {
        app.dialog = Dialog::Print(PrintState {
            tab,
            ..PrintState::default()
        });
        let mut rect = None;
        for n in 0..FRAMES {
            sh.time += 0.5;
            let input = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(SW, SH),
                )),
                time: Some(sh.time),
                ..Default::default()
            };
            let mut out = sh.ctx.run_ui(input, |ui| {
                let ctx = ui.ctx().clone();
                crate::ui::apply_theme(&ctx, &crate::settings::effective_theme(&app.settings));
                crate::ui::dialogs::show(&mut app, &ctx);
            });
            sh.tex.apply(&out.textures_delta);
            if n + 1 == FRAMES {
                rect = sh.ctx.memory(|m| m.area_rect(id));
                if let (Some(dir), Some(r)) = (&dir, rect) {
                    std::fs::create_dir_all(dir).unwrap();
                    let shapes = std::mem::take(&mut out.shapes);
                    let prims = sh.ctx.tessellate(shapes, out.pixels_per_point);
                    let mut c = Canvas::new(SW as usize, SH as usize);
                    c.paint(&sh.tex, &prims);
                    let area = (
                        r.min.x.max(0.0) as usize,
                        r.min.y.max(0.0) as usize,
                        (r.max.x.ceil() as usize).min(SW as usize),
                        (r.max.y.ceil() as usize).min(SH as usize),
                    );
                    c.save(&dir.join(format!("print-tab-{tab}.png")), area);
                }
            }
            for t in &out.textures_delta.free {
                sh.tex.0.remove(t);
            }
            out.textures_delta.clear();
        }
        let r = rect.expect("the Print dialog is drawn");
        sizes.push((tab, r.width().round(), r.height().round()));
    }
    let first = sizes[0];
    for s in &sizes {
        assert_eq!((s.1, s.2), (first.1, first.2), "tab sizes differ: {sizes:?}");
    }
    // The body scrolls from SH - TITLE_BAR - 60 up; the window adds its
    // title bar and frame to the body.
    let title = crate::ui::chrome::TITLE_BAR;
    let scroll_from = SH - title - 60.0;
    assert!(first.2 - title < scroll_from, "{sizes:?}");
    // The whole body shows: the window holds the title bar, the tab row,
    // the tab area and the button row (a window held to a fixed height
    // would cut the buttons off and scroll instead).
    let body = crate::ui::dialogs::PRINT_BODY;
    assert!(first.2 >= title + body.y + 80.0, "{sizes:?}");
    assert!(first.1 >= body.x, "{sizes:?}");
}
