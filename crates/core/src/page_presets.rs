//! The page format library of the New Document window: print sheets,
//! screens and social media formats, a search over their names and
//! sizes, how many pieces of a size fit on a sheet, and the known format
//! nearest to a size. Sizes are millimetres, in each format's natural
//! orientation.

use crate::document::paper;
use crate::geometry::Size;

/// The group a format belongs to (the window's category filter).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Print,
    Screen,
    Social,
}

/// A page format.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PagePreset {
    /// Stable machine name; the interface translates `paper.<id>`.
    pub id: &'static str,
    /// The name in English (shown when no translation exists).
    pub name: &'static str,
    pub category: Category,
    /// Millimetres, natural orientation.
    pub size: Size,
    /// Recommended resolution (dpi).
    pub dpi: f64,
    /// RGB for screens, CMYK for print.
    pub rgb: bool,
    /// Taken into account when snapping a page size to a known format.
    pub snaps: bool,
    /// Measured in pixels (the interface shows px).
    pub pixels: bool,
}

/// Millimetres per screen pixel (96 dpi).
pub const PX_MM: f64 = 25.4 / 96.0;

const fn print(id: &'static str, name: &'static str, w: f64, h: f64, snaps: bool) -> PagePreset {
    PagePreset {
        id,
        name,
        category: Category::Print,
        size: Size::new(w, h),
        dpi: 300.0,
        rgb: false,
        snaps,
        pixels: false,
    }
}

const fn screen(
    id: &'static str,
    name: &'static str,
    category: Category,
    w: f64,
    h: f64,
) -> PagePreset {
    PagePreset {
        id,
        name,
        category,
        size: Size::new(w * PX_MM, h * PX_MM),
        dpi: 96.0,
        rgb: true,
        snaps: false,
        pixels: true,
    }
}

/// The library, in display order.
pub const PRESETS: &[PagePreset] = &[
    print("a0", "A0", 841.0, 1189.0, true),
    print("a1", "A1", 594.0, 841.0, true),
    print("a2", "A2", 420.0, 594.0, true),
    print("a3", "A3", paper::A3.width, paper::A3.height, true),
    print("a4", "A4", paper::A4.width, paper::A4.height, true),
    print("a5", "A5", 148.0, 210.0, true),
    print("a6", "A6", 105.0, 148.0, true),
    print("b4", "B4", 250.0, 353.0, true),
    print("b5", "B5", 176.0, 250.0, true),
    print(
        "letter",
        "Letter",
        paper::LETTER.width,
        paper::LETTER.height,
        true,
    ),
    print("legal", "Legal", 215.9, 355.6, true),
    print("tabloid", "Tabloid", 279.4, 431.8, false),
    print("business_card", "Business card", 90.0, 50.0, false),
    print("card_85x55", "Card 85 × 55", 85.0, 55.0, false),
    print("envelope_dl", "DL envelope", 220.0, 110.0, false),
    print("banner_1x2", "Banner 1 × 2 m", 1000.0, 2000.0, false),
    screen("full_hd", "Full HD", Category::Screen, 1920.0, 1080.0),
    screen("hd", "HD", Category::Screen, 1280.0, 720.0),
    screen("laptop", "Laptop", Category::Screen, 1366.0, 768.0),
    screen("desktop", "Desktop", Category::Screen, 1440.0, 900.0),
    screen("phone", "Phone", Category::Screen, 390.0, 844.0),
    screen("square_post", "Square post", Category::Social, 1080.0, 1080.0),
    screen("portrait_post", "Portrait post", Category::Social, 1080.0, 1350.0),
    screen("story", "Story / vertical", Category::Social, 1080.0, 1920.0),
    screen("video_cover", "Video cover", Category::Social, 1280.0, 720.0),
];

/// The preset with this id.
pub fn by_id(id: &str) -> Option<&'static PagePreset> {
    PRESETS.iter().find(|p| p.id == id)
}

/// The formats whose name contains every word typed (ignoring case) or
/// whose size in mm or px contains the number typed, with the names as
/// [`PagePreset::name`] gives them.
pub fn search(query: &str) -> Vec<&'static PagePreset> {
    search_named(query, |p| p.name.to_string())
}

/// Like [`search`], with the names as `name` gives them (the interface
/// passes the translated names).
pub fn search_named(query: &str, name: impl Fn(&PagePreset) -> String) -> Vec<&'static PagePreset> {
    let q = query.trim().to_lowercase();
    PRESETS
        .iter()
        .filter(|p| {
            if q.is_empty() {
                return true;
            }
            let unit = if p.pixels { PX_MM } else { 1.0 };
            let (w, h) = (fmt(p.size.width / unit), fmt(p.size.height / unit));
            let text = format!("{} {} {w} {h} {w}x{h}", name(p).to_lowercase(), p.name.to_lowercase());
            q.split_whitespace().all(|word| text.contains(word))
        })
        .collect()
}

fn fmt(v: f64) -> String {
    let s = format!("{v:.1}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// How many `w` × `h` pieces fit on a `sw` × `sh` sheet, all in the same
/// orientation, the better of upright and turned.
pub fn fit_count(sw: f64, sh: f64, w: f64, h: f64) -> usize {
    if !(w > 0.0 && h > 0.0 && sw.is_finite() && sh.is_finite()) {
        return 0;
    }
    let eps = 1e-9;
    let n = |a: f64, b: f64| ((a + eps) / b).floor().clamp(0.0, 1.0e9) as usize;
    let straight = n(sw, w).saturating_mul(n(sh, h));
    let turned = n(sw, h).saturating_mul(n(sh, w));
    straight.max(turned)
}

/// The known format nearest to `w` × `h` (either orientation) when both
/// sides are within `tol` mm, with its size in the orientation asked for.
pub fn snap_format(w: f64, h: f64, tol: f64) -> Option<(&'static PagePreset, f64, f64)> {
    PRESETS
        .iter()
        .filter(|p| p.snaps)
        .flat_map(|p| {
            [
                (p, p.size.width, p.size.height),
                (p, p.size.height, p.size.width),
            ]
        })
        .map(|(p, pw, ph)| (p, pw, ph, (pw - w).abs().max((ph - h).abs())))
        .filter(|(.., d)| *d <= tol)
        .min_by(|a, b| a.3.total_cmp(&b.3))
        .map(|(p, pw, ph, _)| (p, pw, ph))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(id: &str) -> &'static PagePreset {
        by_id(id).expect("preset")
    }

    #[test]
    fn search_by_name_and_size() {
        assert!(search("A5").iter().any(|p| p.id == "a5"));
        assert!(search("1080").iter().all(|p| p.pixels));
        assert!(search("1080").len() >= 3);
        assert!(search("card").iter().any(|p| p.id == "business_card"));
        assert_eq!(search("").len(), PRESETS.len());
        // Translated names are searched too.
        let pt = search_named("cartão", |p| {
            if p.id == "business_card" {
                "Cartão de visita".into()
            } else {
                p.name.into()
            }
        });
        assert_eq!(pt.len(), 1);
        assert!(search("210x297").iter().any(|p| p.id == "a4"));
    }

    /// A5 fits twice on A4; A6 four times on A4 and eight on A3.
    #[test]
    fn fit_count_on_sheets() {
        let (a3, a4, a5, a6) = (named("a3"), named("a4"), named("a5"), named("a6"));
        let f = |s: &PagePreset, p: &PagePreset| {
            fit_count(s.size.width, s.size.height, p.size.width, p.size.height)
        };
        assert_eq!(f(a4, a5), 2);
        assert_eq!(f(a4, a6), 4);
        assert_eq!(f(a3, a6), 8);
        assert_eq!(f(a4, a3), 0);
        assert_eq!(fit_count(100.0, 100.0, 0.0, 10.0), 0);
        assert_eq!(fit_count(f64::NAN, 100.0, 10.0, 10.0), 0);
    }

    /// Near 148 × 210 mm snaps to A5, in both orientations.
    #[test]
    fn snap_to_iso() {
        let (p, w, h) = snap_format(149.5, 209.0, 3.0).expect("A5");
        assert_eq!((p.id, w, h), ("a5", 148.0, 210.0));
        let (p, w, h) = snap_format(211.0, 147.0, 3.0).expect("A5");
        assert_eq!((p.id, w, h), ("a5", 210.0, 148.0));
        assert!(snap_format(160.0, 210.0, 3.0).is_none());
    }

    #[test]
    fn ids_are_unique_and_sizes_positive() {
        for (i, p) in PRESETS.iter().enumerate() {
            assert!(p.size.width > 0.0 && p.size.height > 0.0, "{}", p.id);
            assert!(PRESETS[i + 1..].iter().all(|q| q.id != p.id), "{}", p.id);
        }
        assert_eq!(named("a4").size, paper::A4);
    }
}
