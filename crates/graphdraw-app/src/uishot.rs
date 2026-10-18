//! Testes: pinta a saída do egui num PNG (rasterizador simples por
//! triângulos), para revisar telas e janelas sem abrir o programa.
//!
//!     GRAPHDRAW_SHOT_DIR=pasta cargo test -p graphdraw-app shot_ -- --ignored

use std::collections::HashMap;

use eframe::egui::{self, Color32, ColorImage, TextureId};

/// Texturas recebidas até aqui (fonte e imagens carregadas).
#[derive(Default)]
pub struct Shot {
    textures: HashMap<TextureId, ColorImage>,
}

impl Shot {
    /// Acumula as texturas de um quadro (chamar em todos os quadros).
    pub fn textures(&mut self, out: &mut egui::FullOutput) {
        for (id, deltas) in &out.textures_delta.set {
            for delta in deltas {
                let egui::ImageData::Color(img) = &delta.image;
                match delta.pos {
                    None => {
                        self.textures.insert(*id, (**img).clone());
                    }
                    Some([x0, y0]) => {
                        if let Some(t) = self.textures.get_mut(id) {
                            for y in 0..img.size[1] {
                                for x in 0..img.size[0] {
                                    t.pixels[(y0 + y) * t.size[0] + x0 + x] =
                                        img.pixels[y * img.size[0] + x];
                                }
                            }
                        }
                    }
                }
            }
        }
        out.textures_delta.clear();
    }

    /// Pinta o quadro e grava o PNG em `GRAPHDRAW_SHOT_DIR/name.png`.
    pub fn save(&mut self, ctx: &egui::Context, mut out: egui::FullOutput, name: &str) {
        self.textures(&mut out);
        let Ok(dir) = std::env::var("GRAPHDRAW_SHOT_DIR") else {
            return;
        };
        let ppp = out.pixels_per_point;
        let screen = ctx.content_rect();
        let (w, h) = (
            (screen.width() * ppp) as usize,
            (screen.height() * ppp) as usize,
        );
        let mut px = vec![[0.12f32, 0.12, 0.13]; w * h];
        let prims = ctx.tessellate(out.shapes, ppp);
        for p in prims {
            let egui::epaint::Primitive::Mesh(mesh) = p.primitive else {
                continue;
            };
            let Some(tex) = self.textures.get(&mesh.texture_id) else {
                continue;
            };
            let clip = p.clip_rect;
            let (cx0, cy0) = (
                (clip.left() * ppp).max(0.0) as i64,
                (clip.top() * ppp).max(0.0) as i64,
            );
            let (cx1, cy1) = (
                ((clip.right() * ppp) as i64).min(w as i64),
                ((clip.bottom() * ppp) as i64).min(h as i64),
            );
            for t in mesh.indices.chunks(3) {
                let v = [
                    &mesh.vertices[t[0] as usize],
                    &mesh.vertices[t[1] as usize],
                    &mesh.vertices[t[2] as usize],
                ];
                let q: Vec<(f32, f32)> = v.iter().map(|v| (v.pos.x * ppp, v.pos.y * ppp)).collect();
                let area =
                    (q[1].0 - q[0].0) * (q[2].1 - q[0].1) - (q[2].0 - q[0].0) * (q[1].1 - q[0].1);
                if area.abs() < 1e-6 {
                    continue;
                }
                let bx0 = (q.iter().map(|p| p.0).fold(f32::MAX, f32::min).floor() as i64).max(cx0);
                let by0 = (q.iter().map(|p| p.1).fold(f32::MAX, f32::min).floor() as i64).max(cy0);
                let bx1 = (q.iter().map(|p| p.0).fold(f32::MIN, f32::max).ceil() as i64).min(cx1);
                let by1 = (q.iter().map(|p| p.1).fold(f32::MIN, f32::max).ceil() as i64).min(cy1);
                for y in by0..by1 {
                    for x in bx0..bx1 {
                        let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
                        let e = |a: (f32, f32), b: (f32, f32)| {
                            (b.0 - a.0) * (fy - a.1) - (fx - a.0) * (b.1 - a.1)
                        };
                        let (w0, w1, w2) = (
                            e(q[1], q[2]) / area,
                            e(q[2], q[0]) / area,
                            e(q[0], q[1]) / area,
                        );
                        if w0 < -1e-4 || w1 < -1e-4 || w2 < -1e-4 {
                            continue;
                        }
                        let lerp = |f: &dyn Fn(&egui::epaint::Vertex) -> f32| {
                            w0 * f(v[0]) + w1 * f(v[1]) + w2 * f(v[2])
                        };
                        let (u, vv) = (lerp(&|v| v.uv.x), lerp(&|v| v.uv.y));
                        let tx = ((u * tex.size[0] as f32) as usize).min(tex.size[0] - 1);
                        let ty = ((vv * tex.size[1] as f32) as usize).min(tex.size[1] - 1);
                        let tc: Color32 = tex.pixels[ty * tex.size[0] + tx];
                        let c = |i: usize| {
                            lerp(&|v| v.color.to_array()[i] as f32 / 255.0)
                                * tc.to_array()[i] as f32
                                / 255.0
                        };
                        let a = c(3);
                        let d = &mut px[y as usize * w + x as usize];
                        for (i, dv) in d.iter_mut().enumerate() {
                            *dv = c(i) + *dv * (1.0 - a);
                        }
                    }
                }
            }
        }
        let bytes: Vec<u8> = px
            .iter()
            .flat_map(|p| p.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8))
            .collect();
        let path = std::path::Path::new(&dir).join(format!("{name}.png"));
        let file = std::fs::File::create(path).expect("png");
        let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w as u32, h as u32);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header()
            .unwrap()
            .write_image_data(&bytes)
            .unwrap();
    }
}
