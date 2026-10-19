//! Accessibility settings of the desktop that the interface follows:
//! Windows' text size ("Make text bigger", 100 to 225 %) and its high
//! contrast themes (on or off, and their colours). They are read once at
//! start-up with the system's `reg` tool, so nothing here needs a
//! platform library; elsewhere (and in a browser) the defaults apply.

// The registry readers run on Windows only; elsewhere only the tests use them.
#![cfg_attr(not(windows), allow(dead_code))]

/// High contrast colours, as the desktop names them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContrastColors {
    pub window: [u8; 3],
    pub window_text: [u8; 3],
    pub highlight: [u8; 3],
    pub highlight_text: [u8; 3],
    pub button_face: [u8; 3],
    pub button_text: [u8; 3],
    pub gray_text: [u8; 3],
    pub hot_light: [u8; 3],
}

impl ContrastColors {
    /// The classic black theme: white text, cyan links, a yellow-on-blue
    /// highlight. Used when the high contrast theme is picked by hand or
    /// its colours cannot be read.
    pub const BLACK: ContrastColors = ContrastColors {
        window: [0, 0, 0],
        window_text: [255, 255, 255],
        highlight: [26, 235, 255],
        highlight_text: [0, 0, 0],
        button_face: [0, 0, 0],
        button_text: [255, 255, 255],
        gray_text: [63, 242, 63],
        hot_light: [255, 255, 0],
    };

    /// The window is dark (light text on it).
    pub fn is_dark(&self) -> bool {
        let [r, g, b] = self.window;
        (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32) < 128.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SystemPrefs {
    /// Text size factor, 1.0 to 2.25.
    pub text_scale: f32,
    /// The colours of the high contrast theme in use, if one is on.
    pub high_contrast: Option<ContrastColors>,
}

impl Default for SystemPrefs {
    fn default() -> Self {
        SystemPrefs {
            text_scale: 1.0,
            high_contrast: None,
        }
    }
}

static PREFS: std::sync::OnceLock<SystemPrefs> = std::sync::OnceLock::new();

/// The desktop's settings (read the first time they are asked for).
pub fn get() -> SystemPrefs {
    *PREFS.get_or_init(read)
}

/// A REG_DWORD value from `reg query` output ("    Name    REG_DWORD
/// 0x96").
pub fn parse_dword(output: &str, name: &str) -> Option<u32> {
    output.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        if !parts.next()?.eq_ignore_ascii_case(name) {
            return None;
        }
        if parts.next()? != "REG_DWORD" {
            return None;
        }
        let v = parts.next()?;
        let hex = v.strip_prefix("0x").or_else(|| v.strip_prefix("0X"))?;
        u32::from_str_radix(hex, 16).ok()
    })
}

/// A REG_SZ value from `reg query` output (the rest of the line).
pub fn parse_sz(output: &str, name: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let t = line.trim_start();
        let rest = t.strip_prefix(name)?;
        let rest = rest.trim_start();
        let rest = rest.strip_prefix("REG_SZ")?;
        Some(rest.trim().to_string())
    })
}

/// "R G B" as the desktop stores a colour.
pub fn parse_rgb(v: &str) -> Option<[u8; 3]> {
    let mut it = v.split_whitespace().map(|p| p.parse::<u8>().ok());
    let c = [it.next()??, it.next()??, it.next()??];
    it.next().is_none().then_some(c)
}

/// The text scale factor from `TextScaleFactor` (100 to 225; anything
/// else is 100).
pub fn text_scale_from(output: &str) -> f32 {
    match parse_dword(output, "TextScaleFactor") {
        Some(v) if (100..=225).contains(&v) => v as f32 / 100.0,
        _ => 1.0,
    }
}

/// High contrast is on when bit 1 of the `Flags` value is set (the
/// value is a string of digits).
pub fn high_contrast_on(output: &str) -> bool {
    parse_sz(output, "Flags")
        .and_then(|v| v.trim().parse::<u32>().ok())
        .is_some_and(|f| f & 1 == 1)
}

/// The colours from the `Control Panel\Colors` key, falling back to the
/// classic black theme for any that is missing.
pub fn colors_from(output: &str) -> ContrastColors {
    let d = ContrastColors::BLACK;
    let c = |name: &str, fallback: [u8; 3]| {
        parse_sz(output, name)
            .and_then(|v| parse_rgb(&v))
            .unwrap_or(fallback)
    };
    ContrastColors {
        window: c("Window", d.window),
        window_text: c("WindowText", d.window_text),
        highlight: c("Hilight", d.highlight),
        highlight_text: c("HilightText", d.highlight_text),
        button_face: c("ButtonFace", d.button_face),
        button_text: c("ButtonText", d.button_text),
        gray_text: c("GrayText", d.gray_text),
        hot_light: c("HotTrackingColor", d.hot_light),
    }
}

#[cfg(windows)]
fn reg_query(key: &str) -> Option<String> {
    use std::os::windows::process::CommandExt;
    // No console window flashes up.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let out = std::process::Command::new("reg")
        .args(["query", key])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).to_string())
}

#[cfg(windows)]
fn read() -> SystemPrefs {
    let text_scale = reg_query(r"HKCU\Software\Microsoft\Accessibility")
        .map(|o| text_scale_from(&o))
        .unwrap_or(1.0);
    let high_contrast = reg_query(r"HKCU\Control Panel\Accessibility\HighContrast")
        .filter(|o| high_contrast_on(o))
        .map(|_| {
            reg_query(r"HKCU\Control Panel\Colors")
                .map(|o| colors_from(&o))
                .unwrap_or(ContrastColors::BLACK)
        });
    SystemPrefs {
        text_scale,
        high_contrast,
    }
}

#[cfg(not(windows))]
fn read() -> SystemPrefs {
    SystemPrefs::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_output_is_read_and_nonsense_is_ignored() {
        let acc = "\r\nHKEY_CURRENT_USER\\Software\\Microsoft\\Accessibility\r\n    TextScaleFactor    REG_DWORD    0x96\r\n";
        assert_eq!(text_scale_from(acc), 1.5);
        assert_eq!(
            text_scale_from("    TextScaleFactor    REG_DWORD    0x3e8"),
            1.0
        );
        assert_eq!(text_scale_from(""), 1.0);
        let hc = "    Flags    REG_SZ    127\r\n    High Contrast Scheme    REG_SZ    \r\n";
        assert!(high_contrast_on(hc));
        assert!(!high_contrast_on("    Flags    REG_SZ    126"));
        assert!(!high_contrast_on("    Flags    REG_SZ    x"));
        let colors = "    Window    REG_SZ    0 0 0\r\n    WindowText    REG_SZ    255 255 255\r\n    Hilight    REG_SZ    26 235 255\r\n    HilightText    REG_SZ    0 0 0\r\n    ButtonFace    REG_SZ    10 20 30\r\n    GrayText    REG_SZ    bad\r\n";
        let c = colors_from(colors);
        assert_eq!(c.button_face, [10, 20, 30]);
        assert_eq!(c.window_text, [255, 255, 255]);
        // Missing or unreadable values: the classic black theme's.
        assert_eq!(c.gray_text, ContrastColors::BLACK.gray_text);
        assert_eq!(c.hot_light, ContrastColors::BLACK.hot_light);
        assert!(c.is_dark());
        assert_eq!(parse_rgb("1 2 3 4"), None);
        assert_eq!(parse_rgb("1 2"), None);
        // "Window" does not match "WindowText".
        assert_eq!(
            parse_sz("    WindowText    REG_SZ    1 1 1", "Window"),
            None
        );
    }
}
