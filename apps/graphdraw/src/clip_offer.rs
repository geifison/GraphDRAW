//! What the system clipboard offers and which of it a paste uses.
//!
//! Vectors go first, so a drawing pasted from another program stays
//! editable: the private payload, then SVG, then copied files, then
//! pictures, then SVG markup carried as text, then plain text.

use base64::Engine as _;

/// Every flavour the system clipboard holds, read once per paste.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Offer {
    /// The private payload ([`crate::clip_native`]).
    pub native: Option<Vec<u8>>,
    /// The registered SVG format (`image/svg+xml`).
    pub svg: Option<String>,
    /// Files copied in the system's file manager.
    pub files: Vec<std::path::PathBuf>,
    /// The registered PNG format (keeps transparency).
    pub png: Option<Vec<u8>>,
    /// A plain bitmap, already decoded.
    pub image: Option<image::RgbaImage>,
    /// The HTML fragment format.
    pub html: Option<String>,
    pub text: Option<String>,
    /// Something is on the clipboard, in none of the formats above.
    pub other_formats: bool,
}

/// The flavour a paste uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice {
    Native,
    Svg,
    Files,
    Png,
    Bitmap,
    HtmlImage,
    SvgText,
    Text,
    /// The clipboard holds something no reader understands.
    Unsupported,
    /// The clipboard is empty.
    Nothing,
}

/// The best flavour of `o` (see the module notes for the order).
pub fn choose(o: &Offer) -> Choice {
    match candidates(o).first() {
        Some(c) => *c,
        None if o.other_formats || o.html.is_some() => Choice::Unsupported,
        None => Choice::Nothing,
    }
}

/// Every flavour of `o` a paste can use, best first; a paste falls back
/// to the next when one cannot be read.
pub fn candidates(o: &Offer) -> Vec<Choice> {
    let mut c = Vec::new();
    if o.native.is_some() {
        c.push(Choice::Native);
    }
    if o.svg.is_some() {
        c.push(Choice::Svg);
    }
    if !o.files.is_empty() {
        c.push(Choice::Files);
    }
    if o.png.is_some() {
        c.push(Choice::Png);
    }
    if o.image.is_some() {
        c.push(Choice::Bitmap);
    }
    if o.html.as_deref().and_then(html_image_data).is_some() {
        c.push(Choice::HtmlImage);
    }
    match o.text.as_deref() {
        Some(t) if looks_like_svg(t) => c.extend([Choice::SvgText, Choice::Text]),
        Some(_) => c.push(Choice::Text),
        None => {}
    }
    c
}

/// Text that is an SVG document, optionally after an XML declaration,
/// a doctype or a comment; prose that mentions `<svg>` is not.
pub fn looks_like_svg(t: &str) -> bool {
    let t = t.trim_start();
    (t.starts_with("<svg") || t.starts_with("<?xml") || t.starts_with("<!")) && t.contains("<svg")
}

/// The bytes of the first `<img>` whose `src` is a base64 `data:image/`
/// URL. Remote and file URLs are never fetched.
pub fn html_image_data(html: &str) -> Option<Vec<u8>> {
    // ASCII lowercasing keeps every byte offset, so offsets found in
    // `lower` index `html` too.
    let lower = html.to_ascii_lowercase();
    let mut from = 0;
    while let Some(i) = lower.get(from..)?.find("<img") {
        let start = from + i;
        let end = start + lower.get(start..)?.find('>')?;
        if let Some(data) = img_src(html.get(start..end)?, lower.get(start..end)?) {
            return Some(data);
        }
        from = end;
    }
    None
}

/// The decoded `data:image/` URL in the `src` of one `<img ...` tag.
fn img_src(tag: &str, tag_lower: &str) -> Option<Vec<u8>> {
    // The `src` attribute itself, not `data-src` and the like.
    let s = tag_lower.match_indices("src=").map(|(i, _)| i).find(|i| {
        *i > 0
            && tag_lower
                .as_bytes()
                .get(i - 1)
                .is_some_and(u8::is_ascii_whitespace)
    })?;
    let rest = tag.get(s + 4..)?;
    let quote = rest.chars().next().filter(|q| *q == '"' || *q == '\'')?;
    let body = rest.get(1..)?;
    let url = body.get(..body.find(quote)?)?;
    let (head, b64) = url.split_once(',')?;
    if !head.to_ascii_lowercase().starts_with("data:image/") || !head.ends_with(";base64") {
        return None;
    }
    base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn svg() -> String {
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"/>"#.into()
    }

    #[test]
    fn private_payload_wins_over_everything() {
        let o = Offer {
            native: Some(vec![1]),
            svg: Some(svg()),
            png: Some(vec![2]),
            text: Some("x".into()),
            ..Default::default()
        };
        assert_eq!(choose(&o), Choice::Native);
    }

    #[test]
    fn order_after_the_private_payload() {
        let mut o = Offer {
            svg: Some(svg()),
            files: vec!["a.png".into()],
            png: Some(vec![1]),
            text: Some(svg()),
            ..Default::default()
        };
        assert_eq!(choose(&o), Choice::Svg);
        o.svg = None;
        assert_eq!(choose(&o), Choice::Files);
        o.files.clear();
        assert_eq!(choose(&o), Choice::Png);
        o.png = None;
        o.image = Some(image::RgbaImage::new(1, 1));
        assert_eq!(choose(&o), Choice::Bitmap);
        o.image = None;
        o.html = Some(r#"<img src="data:image/png;base64,iVBORw0KGgo=">"#.into());
        assert_eq!(choose(&o), Choice::HtmlImage);
        o.html = None;
        assert_eq!(choose(&o), Choice::SvgText);
        o.text = Some("hello".into());
        assert_eq!(choose(&o), Choice::Text);
        o.text = None;
        assert_eq!(choose(&o), Choice::Nothing);
    }

    #[test]
    fn unknown_formats_only_are_reported_as_unsupported() {
        let o = Offer {
            other_formats: true,
            ..Default::default()
        };
        assert_eq!(choose(&o), Choice::Unsupported);
        let html_without_image = Offer {
            html: Some("<b>bold</b>".into()),
            ..Default::default()
        };
        assert_eq!(choose(&html_without_image), Choice::Unsupported);
        assert_eq!(choose(&Offer::default()), Choice::Nothing);
    }

    #[test]
    fn html_image_takes_data_urls_only() {
        assert_eq!(
            html_image_data(r#"<p><IMG alt="a" SRC="data:image/png;base64,QUJD"></p>"#),
            Some(b"ABC".to_vec())
        );
        assert_eq!(
            html_image_data(r#"<img src='data:image/jpeg;base64,QUJD'>"#),
            Some(b"ABC".to_vec())
        );
        assert_eq!(
            html_image_data(
                r#"<img src="https://example.com/a.png"><img src="data:image/gif;base64,QUJD">"#
            ),
            Some(b"ABC".to_vec())
        );
        assert_eq!(
            html_image_data(r#"<img src="https://example.com/a.png">"#),
            None
        );
        assert_eq!(
            html_image_data(r#"<img src="file:///C:/secret.png">"#),
            None
        );
        assert_eq!(
            html_image_data(r#"<img src="data:text/html;base64,QUJD">"#),
            None
        );
        assert_eq!(html_image_data("<b>no image</b>"), None);
        assert_eq!(
            html_image_data(
                r#"<img data-src="https://example.com/a.png" src="data:image/png;base64,QUJD">"#
            ),
            Some(b"ABC".to_vec())
        );
        assert_eq!(html_image_data("<img src=\"data:image/png;base64,é"), None);
    }

    #[test]
    fn svg_markup_detection() {
        assert!(looks_like_svg("  <?xml version=\"1.0\"?>\n<svg></svg>"));
        assert!(looks_like_svg(
            "<svg xmlns=\"http://www.w3.org/2000/svg\"/>"
        ));
        assert!(!looks_like_svg("a <svg> inside prose"));
        assert!(!looks_like_svg("<p>html</p>"));
        assert!(!looks_like_svg("plain"));
    }
}
