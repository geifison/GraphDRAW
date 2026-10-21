//! The system clipboard: what Copy publishes and what Paste reads.
//!
//! On Windows every format goes on the clipboard in one go, so other
//! programs pick the one they understand: the private payload, SVG
//! (registered `image/svg+xml`), the SVG markup as text, PNG (registered
//! `PNG`, with transparency) and a plain bitmap. Other desktops hold one
//! text or one picture (the clipboard library offers no more); the
//! browser build hands text to the page. Tests never touch the real
//! clipboard: they read and write a per-thread stand-in.

use crate::clip_offer::Offer;

/// What a copy puts on the system clipboard.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Publication {
    /// The private payload ([`crate::clip_native`]).
    pub native: Option<Vec<u8>>,
    pub svg: Option<String>,
    /// The SVG markup also as plain text.
    pub svg_as_text: bool,
    pub png: Option<Vec<u8>>,
}

/// Put `p` on the clipboard, replacing what was there. Returns the
/// clipboard's change counter afterwards when the system keeps one.
pub fn publish(p: &Publication) -> Option<u32> {
    #[cfg(test)]
    return fake::publish(p);
    #[cfg(not(test))]
    real::publish(p)
}

/// Every flavour the clipboard holds, read once.
pub fn read() -> Offer {
    #[cfg(test)]
    return fake::read();
    #[cfg(not(test))]
    real::read()
}

/// The clipboard's change counter now, if the system keeps one.
pub fn sequence() -> Option<u32> {
    #[cfg(test)]
    return fake::sequence();
    #[cfg(not(test))]
    real::sequence()
}

/// Whether a paste key was pressed this frame that the windowing layer
/// swallowed: it turns Ctrl+V into a paste event only while the clipboard
/// holds text, so a picture or files copied elsewhere would never reach
/// Paste. `Some(true)` for Ctrl+Shift+V (Paste in View), `Some(false)` for
/// Ctrl+V and Shift+Insert. Each press counts once. Windows only; tests
/// and other systems read no keyboard here.
pub fn swallowed_paste_key(m: egui::Modifiers, focused: bool) -> Option<bool> {
    #[cfg(all(windows, not(test)))]
    {
        use std::sync::atomic::{AtomicBool, Ordering};
        #[link(name = "user32")]
        extern "system" {
            fn GetAsyncKeyState(key: i32) -> i16;
        }
        const VK_V: i32 = 0x56;
        const VK_INSERT: i32 = 0x2D;
        static HELD: AtomicBool = AtomicBool::new(false);
        // SAFETY: GetAsyncKeyState only reads the key state; any key code
        // is valid input.
        let down = |k: i32| unsafe { GetAsyncKeyState(k) } < 0;
        let ctrl_v = m.command && down(VK_V);
        let shift_insert = m.shift && !m.command && down(VK_INSERT);
        let now = focused && (ctrl_v || shift_insert);
        let was = HELD.swap(now, Ordering::Relaxed);
        if now && !was {
            return Some(ctrl_v && m.shift);
        }
        None
    }
    #[cfg(not(all(windows, not(test))))]
    {
        let _ = (m, focused);
        None
    }
}

/// A flavour larger than this is left unread.
#[cfg(windows)]
const MAX_FLAVOUR_BYTES: usize = crate::clip_native::MAX_BYTES;
/// A plain bitmap larger than this is left undecoded.
#[cfg(windows)]
const MAX_BITMAP_BYTES: usize = 128 * 1024 * 1024;

/// `png` as a 24-bit BMP file, transparent parts on white, for programs
/// that only take plain bitmaps.
#[cfg(windows)]
fn png_to_bmp(png: &[u8]) -> Option<Vec<u8>> {
    let img = image::load_from_memory_with_format(png, image::ImageFormat::Png)
        .ok()?
        .to_rgba8();
    let mut rgb = image::RgbImage::new(img.width(), img.height());
    for (dst, src) in rgb.pixels_mut().zip(img.pixels()) {
        let a = u16::from(src[3]);
        let over = |c: u8| ((u16::from(c) * a + 255 * (255 - a)) / 255) as u8;
        *dst = image::Rgb([over(src[0]), over(src[1]), over(src[2])]);
    }
    let mut bmp = Vec::new();
    image::DynamicImage::ImageRgb8(rgb)
        .write_to(&mut std::io::Cursor::new(&mut bmp), image::ImageFormat::Bmp)
        .ok()?;
    Some(bmp)
}

#[cfg(windows)]
mod real {
    use super::{png_to_bmp, Offer, Publication, MAX_BITMAP_BYTES, MAX_FLAVOUR_BYTES};
    use clipboard_win::{formats, options::NoClear, raw};

    const SVG_FORMAT: &str = "image/svg+xml";
    const PNG_FORMAT: &str = "PNG";
    const HTML_FORMAT: &str = "HTML Format";

    fn format(name: &str) -> Option<u32> {
        raw::register_format(name).map(|f| f.get())
    }

    /// The bytes of `format`, if it is there and not oversize.
    fn bytes(format: u32) -> Option<Vec<u8>> {
        if !raw::is_format_avail(format) || raw::size(format)?.get() > MAX_FLAVOUR_BYTES {
            return None;
        }
        let mut out = Vec::new();
        raw::get_vec(format, &mut out).ok()?;
        Some(out)
    }

    /// Text in a registered format, trailing NULs dropped.
    fn text_of(format: u32) -> Option<String> {
        let b = bytes(format)?;
        let t = String::from_utf8_lossy(&b);
        Some(t.trim_end_matches('\0').to_string()).filter(|t| !t.is_empty())
    }

    pub fn publish(p: &Publication) -> Option<u32> {
        let started = web_time::Instant::now();
        {
            let _open = clipboard_win::Clipboard::new_attempts(10).ok()?;
            raw::empty().ok()?;
            if let (Some(n), Some(f)) = (&p.native, format(crate::clip_native::FORMAT_NAME)) {
                let _ = raw::set_without_clear(f, n);
            }
            if let Some(svg) = &p.svg {
                if let Some(f) = format(SVG_FORMAT) {
                    let _ = raw::set_without_clear(f, svg.as_bytes());
                }
                if p.svg_as_text {
                    let _ = raw::set_string_with(svg, NoClear);
                }
            }
            if let Some(png) = &p.png {
                if let Some(f) = format(PNG_FORMAT) {
                    let _ = raw::set_without_clear(f, png);
                }
                if let Some(bmp) = png_to_bmp(png) {
                    let _ = raw::set_bitmap_with(&bmp, NoClear);
                }
            }
        }
        log::debug!("clipboard publish took {:?}", started.elapsed());
        sequence()
    }

    pub fn read() -> Offer {
        let mut o = Offer::default();
        let has_bitmap;
        {
            let Ok(_open) = clipboard_win::Clipboard::new_attempts(10) else {
                return o;
            };
            // Some writers pad formats with NULs.
            o.native = format(crate::clip_native::FORMAT_NAME)
                .and_then(bytes)
                .map(|mut b| {
                    while b.last() == Some(&0) {
                        b.pop();
                    }
                    b
                });
            o.svg = format(SVG_FORMAT).and_then(text_of);
            o.png = format(PNG_FORMAT).and_then(bytes);
            if raw::is_format_avail(formats::CF_HDROP) {
                let _ = raw::get_file_list_path(&mut o.files);
            }
            // Read whole: the fragment offsets in its header come from the
            // other program and are not to be trusted; the picture search
            // scans the text, header included.
            o.html = format(HTML_FORMAT).and_then(text_of);
            if raw::size(formats::CF_UNICODETEXT).is_some_and(|s| s.get() <= MAX_FLAVOUR_BYTES) {
                let mut t = Vec::new();
                if raw::get_string(&mut t).is_ok() {
                    o.text =
                        Some(String::from_utf8_lossy(&t).into_owned()).filter(|t| !t.is_empty());
                }
            }
            // Decoding a plain bitmap allocates all its pixels: only when
            // nothing better is there, and only within the size limit.
            let dib_size = [formats::CF_DIBV5, formats::CF_DIB]
                .into_iter()
                .find_map(|f| raw::size(f).filter(|_| raw::is_format_avail(f)));
            has_bitmap = o.native.is_none()
                && o.svg.is_none()
                && o.files.is_empty()
                && o.png.is_none()
                && dib_size.is_some_and(|s| s.get() <= MAX_BITMAP_BYTES);
            o.other_formats = raw::count_formats().unwrap_or(0) > 0;
        }
        // Plain bitmaps come in several layouts; the clipboard library
        // decodes them. It opens the clipboard itself, hence after closing.
        if has_bitmap {
            o.image = arboard::Clipboard::new()
                .and_then(|mut cb| cb.get_image())
                .ok()
                .and_then(|i| {
                    image::RgbaImage::from_raw(
                        i.width as u32,
                        i.height as u32,
                        i.bytes.into_owned(),
                    )
                });
        }
        o
    }

    pub fn sequence() -> Option<u32> {
        raw::seq_num().map(|n| n.get())
    }
}

#[cfg(all(not(windows), not(target_arch = "wasm32")))]
mod real {
    use super::{Offer, Publication};

    pub fn publish(p: &Publication) -> Option<u32> {
        let Ok(mut cb) = arboard::Clipboard::new() else {
            return None;
        };
        // One flavour only: the SVG text, else the picture.
        match (&p.svg, &p.png) {
            (Some(svg), _) if p.svg_as_text => {
                let _ = cb.set_text(svg.clone());
            }
            (_, Some(png)) => {
                if let Ok(img) = image::load_from_memory(png) {
                    let img = img.to_rgba8();
                    let _ = cb.set_image(arboard::ImageData {
                        width: img.width() as usize,
                        height: img.height() as usize,
                        bytes: img.into_raw().into(),
                    });
                }
            }
            _ => {}
        }
        None
    }

    pub fn read() -> Offer {
        let Ok(mut cb) = arboard::Clipboard::new() else {
            return Offer::default();
        };
        let mut o = Offer {
            text: cb.get_text().ok().filter(|t| !t.is_empty()),
            html: cb.get().html().ok(),
            files: cb.get().file_list().unwrap_or_default(),
            ..Default::default()
        };
        // Decoding a picture allocates all its pixels: only when no
        // vectors or files are there.
        let vectors = o
            .text
            .as_deref()
            .is_some_and(crate::clip_offer::looks_like_svg);
        if !vectors && o.files.is_empty() {
            o.image = cb.get_image().ok().and_then(|i| {
                image::RgbaImage::from_raw(i.width as u32, i.height as u32, i.bytes.into_owned())
            });
        }
        o
    }

    pub fn sequence() -> Option<u32> {
        None
    }
}

#[cfg(target_arch = "wasm32")]
mod real {
    use super::{Offer, Publication};

    pub fn publish(p: &Publication) -> Option<u32> {
        if let Some(svg) = &p.svg {
            crate::web::copy_text(svg);
        }
        None
    }

    /// Browsers hand clipboard contents only to paste events.
    pub fn read() -> Offer {
        Offer::default()
    }

    pub fn sequence() -> Option<u32> {
        None
    }
}

/// The stand-in clipboard of tests, one per test thread.
#[cfg(test)]
pub mod fake {
    use super::{Offer, Publication};
    use std::cell::{Cell, RefCell};

    thread_local! {
        static OFFER: RefCell<Offer> = RefCell::new(Offer::default());
        static PUBLISHED: RefCell<Option<Publication>> = const { RefCell::new(None) };
        static SEQUENCE: Cell<u32> = const { Cell::new(1) };
    }

    fn bump() -> u32 {
        SEQUENCE.with(|s| {
            s.set(s.get() + 1);
            s.get()
        })
    }

    /// Another program puts `o` on the clipboard.
    pub fn set(o: Offer) {
        OFFER.with(|c| *c.borrow_mut() = o);
        bump();
    }

    /// What the last copy published.
    pub fn last_published() -> Option<Publication> {
        PUBLISHED.with(|c| c.borrow().clone())
    }

    pub(super) fn publish(p: &Publication) -> Option<u32> {
        PUBLISHED.with(|c| *c.borrow_mut() = Some(p.clone()));
        OFFER.with(|c| {
            *c.borrow_mut() = Offer {
                native: p.native.clone(),
                svg: p.svg.clone(),
                text: p.svg.clone().filter(|_| p.svg_as_text),
                png: p.png.clone(),
                ..Default::default()
            }
        });
        Some(bump())
    }

    pub(super) fn read() -> Offer {
        OFFER.with(|c| c.borrow().clone())
    }

    pub(super) fn sequence() -> Option<u32> {
        Some(SEQUENCE.with(Cell::get))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stand_in_records_publications_and_serves_reads() {
        let p = Publication {
            native: Some(vec![1, 2]),
            svg: Some("<svg/>".into()),
            svg_as_text: true,
            png: None,
        };
        let seq = publish(&p);
        assert_eq!(fake::last_published(), Some(p));
        assert_eq!(sequence(), seq);
        assert_eq!(read().native, Some(vec![1, 2]));
        assert_eq!(read().text.as_deref(), Some("<svg/>"));
        fake::set(Offer {
            text: Some("hi".into()),
            ..Default::default()
        });
        assert_ne!(sequence(), seq, "another program changed the clipboard");
        assert_eq!(read().text.as_deref(), Some("hi"));
        assert!(read().native.is_none());
    }

    #[cfg(windows)]
    #[test]
    fn bitmaps_for_plain_programs_put_transparency_on_white() {
        let mut png = Vec::new();
        image::DynamicImage::ImageRgba8(image::RgbaImage::new(2, 1))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .expect("png");
        let bmp = png_to_bmp(&png).expect("bmp");
        let back = image::load_from_memory(&bmp)
            .expect("bmp decodes")
            .to_rgb8();
        assert_eq!(back.get_pixel(0, 0), &image::Rgb([255, 255, 255]));
        assert!(png_to_bmp(b"not a png").is_none());
    }

    /// Uses the real clipboard: run alone with
    /// `cargo test -p graphdraw real_clipboard -- --ignored`.
    #[cfg(windows)]
    #[test]
    #[ignore]
    fn real_clipboard_round_trip() {
        let mut png = Vec::new();
        image::DynamicImage::ImageRgba8(image::RgbaImage::new(3, 2))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .expect("png");
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="4" height="4"/>"#;
        let p = Publication {
            native: Some(br#"{"x":1}"#.to_vec()),
            svg: Some(svg.into()),
            svg_as_text: true,
            png: Some(png.clone()),
        };
        let seq = real::publish(&p);
        assert!(seq.is_some());
        assert_eq!(real::sequence(), seq);
        let o = real::read();
        assert_eq!(o.native.as_deref(), Some(&br#"{"x":1}"#[..]));
        assert_eq!(o.svg.as_deref(), Some(svg));
        assert_eq!(o.text.as_deref(), Some(svg));
        assert_eq!(o.png.as_deref(), Some(&png[..]));
    }
}
