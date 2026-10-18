//! Impressoras do Windows (GD-PRT-02/10/11): lista, propriedades do driver,
//! papéis, área imprimível e envio das folhas pelo spooler, em faixas.

use graphdraw_core::print::Paper;

/// Papel oferecido pelo driver: código, nome e tamanho (µm).
#[derive(Debug, Clone, PartialEq)]
pub struct PaperKind {
    pub id: i16,
    pub name: String,
    pub size: [f64; 2],
}

/// Folha e resolução de um trabalho: o papel com a área imprimível e os
/// pontos por polegada do dispositivo (X, Y), mais o deslocamento físico
/// (pixels do dispositivo) e a área imprimível em pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DeviceInfo {
    pub paper: Paper,
    pub dpi: [f64; 2],
    pub offset: [i32; 2],
    pub size: [i32; 2],
}

/// Folha a enviar: o app rasteriza a faixa `(x0, y0, largura, altura)` em
/// pixels da resolução `dpi` e devolve RGBA não pré-multiplicado.
pub type Band<'a> = dyn FnMut(usize, [f64; 2], (i64, i64, u32, u32)) -> Option<Vec<u8>> + 'a;

#[cfg(windows)]
mod imp {
    use super::{DeviceInfo, PaperKind};
    use graphdraw_core::print::Paper;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateDCW, DEVMODEW, DIB_RGB_COLORS, DM_IN_BUFFER,
        DM_IN_PROMPT, DM_ORIENTATION, DM_OUT_BUFFER, DM_PAPERSIZE, DeleteDC, GetDeviceCaps, HDC,
        HORZRES, LOGPIXELSX, LOGPIXELSY, PHYSICALHEIGHT, PHYSICALOFFSETX, PHYSICALOFFSETY,
        PHYSICALWIDTH, SRCCOPY, StretchDIBits, VERTRES,
    };
    use windows::Win32::Graphics::Printing::{
        ClosePrinter, DocumentPropertiesW, EnumPrintersW, GetDefaultPrinterW, OpenPrinterW,
        PRINTER_ENUM_CONNECTIONS, PRINTER_ENUM_LOCAL, PRINTER_HANDLE, PRINTER_INFO_4W,
    };
    use windows::Win32::Storage::Xps::{
        AbortDoc, DC_PAPERNAMES, DC_PAPERS, DC_PAPERSIZE, DOCINFOW, DeviceCapabilitiesW, EndDoc,
        EndPage, StartDocW, StartPage,
    };
    use windows::core::{PCWSTR, PWSTR};

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn from_wide(p: *const u16) -> String {
        if p.is_null() {
            return String::new();
        }
        unsafe { PCWSTR(p).to_string().unwrap_or_default() }
    }

    /// Impressoras instaladas (locais e de rede) e a padrão.
    pub fn printers() -> (Vec<String>, Option<String>) {
        let flags = PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS;
        let (mut need, mut count) = (0u32, 0u32);
        unsafe {
            let _ = EnumPrintersW(flags, PCWSTR::null(), 4, None, &mut need, &mut count);
        }
        let mut names = Vec::new();
        if need > 0 {
            // Alinhado para as estruturas PRINTER_INFO_4W.
            let mut buf = vec![0u64; (need as usize).div_ceil(8)];
            let bytes = unsafe {
                std::slice::from_raw_parts_mut(buf.as_mut_ptr() as *mut u8, need as usize)
            };
            if unsafe {
                EnumPrintersW(flags, PCWSTR::null(), 4, Some(bytes), &mut need, &mut count)
            }
            .is_ok()
            {
                let infos = unsafe {
                    std::slice::from_raw_parts(
                        buf.as_ptr() as *const PRINTER_INFO_4W,
                        count as usize,
                    )
                };
                for i in infos {
                    names.push(from_wide(i.pPrinterName.0));
                }
            }
        }
        let mut len = 0u32;
        unsafe {
            let _ = GetDefaultPrinterW(None, &mut len);
        }
        let default = (len > 0)
            .then(|| {
                let mut b = vec![0u16; len as usize];
                unsafe { GetDefaultPrinterW(Some(PWSTR(b.as_mut_ptr())), &mut len) }
                    .as_bool()
                    .then(|| from_wide(b.as_ptr()))
            })
            .flatten();
        (names, default)
    }

    struct Handle(PRINTER_HANDLE);
    impl Drop for Handle {
        fn drop(&mut self) {
            unsafe {
                let _ = ClosePrinter(self.0);
            }
        }
    }

    fn open(name: &str) -> Option<Handle> {
        let w = wide(name);
        let mut h = PRINTER_HANDLE::default();
        unsafe { OpenPrinterW(PCWSTR(w.as_ptr()), &mut h, None) }.ok()?;
        Some(Handle(h))
    }

    /// DEVMODE padrão do driver (buffer com a parte privada do driver).
    pub fn default_devmode(name: &str) -> Option<Vec<u8>> {
        let h = open(name)?;
        let w = wide(name);
        let size = unsafe { DocumentPropertiesW(None, h.0, PCWSTR(w.as_ptr()), None, None, 0) };
        if size <= 0 {
            return None;
        }
        let mut buf = vec![0u8; size as usize];
        let r = unsafe {
            DocumentPropertiesW(
                None,
                h.0,
                PCWSTR(w.as_ptr()),
                Some(buf.as_mut_ptr() as *mut DEVMODEW),
                None,
                DM_OUT_BUFFER.0,
            )
        };
        (r > 0).then_some(buf)
    }

    /// Janela de propriedades do driver; `true` se o usuário confirmou.
    pub fn properties(name: &str, devmode: &mut Vec<u8>, owner: isize) -> bool {
        let Some(h) = open(name) else { return false };
        let w = wide(name);
        let mut out = devmode.clone();
        let r = unsafe {
            DocumentPropertiesW(
                Some(HWND(owner as *mut _)),
                h.0,
                PCWSTR(w.as_ptr()),
                Some(out.as_mut_ptr() as *mut DEVMODEW),
                Some(devmode.as_ptr() as *const DEVMODEW),
                DM_IN_BUFFER.0 | DM_IN_PROMPT.0 | DM_OUT_BUFFER.0,
            )
        };
        if r == 1 {
            *devmode = out;
            true
        } else {
            false
        }
    }

    fn dm(devmode: &[u8]) -> *const DEVMODEW {
        devmode.as_ptr() as *const DEVMODEW
    }

    /// Papéis do driver (código, nome e tamanho).
    pub fn papers(name: &str, devmode: &[u8]) -> Vec<PaperKind> {
        let w = wide(name);
        let dev = PCWSTR(w.as_ptr());
        let n =
            unsafe { DeviceCapabilitiesW(dev, PCWSTR::null(), DC_PAPERS, None, Some(dm(devmode))) };
        if n <= 0 {
            return Vec::new();
        }
        let n = n as usize;
        let mut ids = vec![0u16; n];
        let mut names = vec![0u16; n * 64];
        let mut sizes = vec![0i32; n * 2];
        unsafe {
            DeviceCapabilitiesW(
                dev,
                PCWSTR::null(),
                DC_PAPERS,
                Some(PWSTR(ids.as_mut_ptr())),
                Some(dm(devmode)),
            );
            DeviceCapabilitiesW(
                dev,
                PCWSTR::null(),
                DC_PAPERNAMES,
                Some(PWSTR(names.as_mut_ptr())),
                Some(dm(devmode)),
            );
            DeviceCapabilitiesW(
                dev,
                PCWSTR::null(),
                DC_PAPERSIZE,
                Some(PWSTR(sizes.as_mut_ptr() as *mut u16)),
                Some(dm(devmode)),
            );
        }
        (0..n)
            .map(|i| {
                let raw = &names[i * 64..i * 64 + 64];
                let end = raw.iter().position(|&c| c == 0).unwrap_or(64);
                PaperKind {
                    id: ids[i] as i16,
                    name: String::from_utf16_lossy(&raw[..end]),
                    // Décimos de milímetro → µm.
                    size: [sizes[i * 2] as f64 * 100.0, sizes[i * 2 + 1] as f64 * 100.0],
                }
            })
            .collect()
    }

    fn fields(devmode: &mut [u8]) -> &mut DEVMODEW {
        unsafe { &mut *(devmode.as_mut_ptr() as *mut DEVMODEW) }
    }

    pub fn paper_id(devmode: &[u8]) -> i16 {
        unsafe { (*dm(devmode)).Anonymous1.Anonymous1.dmPaperSize }
    }

    pub fn landscape(devmode: &[u8]) -> bool {
        unsafe { (*dm(devmode)).Anonymous1.Anonymous1.dmOrientation == 2 }
    }

    pub fn set_paper(devmode: &mut [u8], id: i16) {
        let d = fields(devmode);
        d.dmFields |= DM_PAPERSIZE;
        d.Anonymous1.Anonymous1.dmPaperSize = id;
    }

    pub fn set_landscape(devmode: &mut [u8], on: bool) {
        let d = fields(devmode);
        d.dmFields |= DM_ORIENTATION;
        d.Anonymous1.Anonymous1.dmOrientation = if on { 2 } else { 1 };
    }

    struct Dc(HDC);
    impl Drop for Dc {
        fn drop(&mut self) {
            unsafe {
                let _ = DeleteDC(self.0);
            }
        }
    }

    fn create_dc(name: &str, devmode: &[u8]) -> Option<Dc> {
        let w = wide(name);
        let drv = wide("WINSPOOL");
        let hdc = unsafe {
            CreateDCW(
                PCWSTR(drv.as_ptr()),
                PCWSTR(w.as_ptr()),
                PCWSTR::null(),
                Some(dm(devmode)),
            )
        };
        (!hdc.is_invalid()).then_some(Dc(hdc))
    }

    fn info_of(hdc: HDC) -> DeviceInfo {
        let cap = |i| unsafe { GetDeviceCaps(Some(hdc), i) };
        let dpi = [cap(LOGPIXELSX).max(1) as f64, cap(LOGPIXELSY).max(1) as f64];
        let um = |px: i32, d: f64| px as f64 / d * 25_400.0;
        let (pw, ph) = (
            um(cap(PHYSICALWIDTH), dpi[0]),
            um(cap(PHYSICALHEIGHT), dpi[1]),
        );
        let (ox, oy) = (cap(PHYSICALOFFSETX), cap(PHYSICALOFFSETY));
        let (hw, vh) = (cap(HORZRES), cap(VERTRES));
        let x0 = um(ox, dpi[0]);
        let y1 = ph - um(oy, dpi[1]);
        DeviceInfo {
            paper: Paper {
                width: pw,
                height: ph,
                printable: kurbo::Rect::new(x0, y1 - um(vh, dpi[1]), x0 + um(hw, dpi[0]), y1),
            },
            dpi,
            offset: [ox, oy],
            size: [hw, vh],
        }
    }

    pub fn device(name: &str, devmode: &[u8]) -> Option<DeviceInfo> {
        let dc = create_dc(name, devmode)?;
        Some(info_of(dc.0))
    }

    /// Envia `sheets` folhas; `band` rasteriza cada faixa. `progress(i)` é
    /// chamado antes de cada folha e devolve `false` para cancelar.
    #[allow(clippy::too_many_arguments)]
    pub fn print(
        name: &str,
        devmode: &[u8],
        job: &str,
        output: Option<&str>,
        sheets: usize,
        max_dpi: f64,
        band: &mut super::Band,
        progress: &mut dyn FnMut(usize) -> bool,
    ) -> Result<(), String> {
        let dc = create_dc(name, devmode).ok_or("não foi possível abrir a impressora")?;
        let info = info_of(dc.0);
        let title = wide(job);
        let file = output.map(wide);
        let di = DOCINFOW {
            cbSize: std::mem::size_of::<DOCINFOW>() as i32,
            lpszDocName: PCWSTR(title.as_ptr()),
            lpszOutput: file.as_ref().map_or(PCWSTR::null(), |f| PCWSTR(f.as_ptr())),
            ..Default::default()
        };
        if unsafe { StartDocW(dc.0, &di) } <= 0 {
            return Err("a impressora recusou o trabalho".into());
        }
        // Resolução da rasterização (até `max_dpi`); o GDI amplia o resto.
        let rd = [info.dpi[0].min(max_dpi), info.dpi[1].min(max_dpi)];
        let (kx, ky) = (rd[0] / info.dpi[0], rd[1] / info.dpi[1]);
        let x0 = (info.offset[0] as f64 * kx).round() as i64;
        let y0 = (info.offset[1] as f64 * ky).round() as i64;
        let w = ((info.size[0] as f64 * kx).round() as u32).max(1);
        let h = ((info.size[1] as f64 * ky).round() as u32).max(1);
        let rows = (16_000_000 / w).clamp(16, h.max(16));
        for i in 0..sheets {
            if !progress(i) {
                unsafe { AbortDoc(dc.0) };
                return Err("cancelado".into());
            }
            if unsafe { StartPage(dc.0) } <= 0 {
                unsafe { AbortDoc(dc.0) };
                return Err("falha ao começar a folha".into());
            }
            let mut y = 0u32;
            while y < h {
                let bh = rows.min(h - y);
                let Some(rgba) = band(i, rd, (x0, y0 + y as i64, w, bh)) else {
                    unsafe { AbortDoc(dc.0) };
                    return Err("falha ao rasterizar a folha".into());
                };
                let mut bgra = rgba;
                for p in bgra.as_chunks_mut::<4>().0 {
                    p.swap(0, 2);
                }
                let bmi = BITMAPINFO {
                    bmiHeader: BITMAPINFOHEADER {
                        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                        biWidth: w as i32,
                        // Negativo: linhas de cima para baixo.
                        biHeight: -(bh as i32),
                        biPlanes: 1,
                        biBitCount: 32,
                        biCompression: BI_RGB.0,
                        ..Default::default()
                    },
                    ..Default::default()
                };
                let dy0 = (y as f64 / ky).round() as i32;
                let dy1 = ((y + bh) as f64 / ky).round() as i32;
                unsafe {
                    StretchDIBits(
                        dc.0,
                        0,
                        dy0,
                        info.size[0],
                        dy1 - dy0,
                        0,
                        0,
                        w as i32,
                        bh as i32,
                        Some(bgra.as_ptr() as *const _),
                        &bmi,
                        DIB_RGB_COLORS,
                        SRCCOPY,
                    );
                }
                y += bh;
            }
            if unsafe { EndPage(dc.0) } <= 0 {
                unsafe { AbortDoc(dc.0) };
                return Err("falha ao terminar a folha".into());
            }
        }
        unsafe { EndDoc(dc.0) };
        Ok(())
    }
}

#[cfg(not(windows))]
mod imp {
    use super::{DeviceInfo, PaperKind};
    pub fn printers() -> (Vec<String>, Option<String>) {
        (Vec::new(), None)
    }
    pub fn default_devmode(_: &str) -> Option<Vec<u8>> {
        None
    }
    pub fn properties(_: &str, _: &mut Vec<u8>, _: isize) -> bool {
        false
    }
    pub fn papers(_: &str, _: &[u8]) -> Vec<PaperKind> {
        Vec::new()
    }
    pub fn paper_id(_: &[u8]) -> i16 {
        0
    }
    pub fn landscape(_: &[u8]) -> bool {
        false
    }
    pub fn set_paper(_: &mut [u8], _: i16) {}
    pub fn set_landscape(_: &mut [u8], _: bool) {}
    pub fn device(_: &str, _: &[u8]) -> Option<DeviceInfo> {
        None
    }
    #[allow(clippy::too_many_arguments)]
    pub fn print(
        _: &str,
        _: &[u8],
        _: &str,
        _: Option<&str>,
        _: usize,
        _: f64,
        _: &mut super::Band,
        _: &mut dyn FnMut(usize) -> bool,
    ) -> Result<(), String> {
        Err("impressão só no Windows".into())
    }
}

pub use imp::*;

#[cfg(test)]
mod tests {
    use super::*;

    /// Envio de verdade pelo GDI, na impressora virtual do Windows, para um
    /// arquivo (GRAPHDRAW_PRINT_OUT). Sem a impressora, não faz nada.
    #[test]
    #[ignore]
    fn print_to_virtual_printer() {
        let Ok(out) = std::env::var("GRAPHDRAW_PRINT_OUT") else {
            return;
        };
        let (names, _) = printers();
        let Some(name) = names.into_iter().find(|n| n.contains("Print to PDF")) else {
            return;
        };
        let dm = default_devmode(&name).expect("devmode");
        let info = device(&name, &dm).expect("dispositivo");
        assert!(
            info.paper.width > 100_000.0 && info.dpi[0] >= 300.0,
            "{info:?}"
        );
        let mut bands = 0;
        let mut band = |_i: usize, _dpi: [f64; 2], px: (i64, i64, u32, u32)| {
            bands += 1;
            // Faixas alternadas de vermelho e azul.
            let c = if bands % 2 == 0 {
                [255, 0, 0, 255]
            } else {
                [0, 0, 255, 255]
            };
            Some(c.repeat((px.2 * px.3) as usize))
        };
        print(
            &name,
            &dm,
            "GraphDraw teste",
            Some(&out),
            2,
            300.0,
            &mut band,
            &mut |_| true,
        )
        .expect("impressão");
        assert!(bands >= 2);
        assert!(
            std::fs::metadata(&out)
                .map(|m| m.len() > 1000)
                .unwrap_or(false),
            "arquivo gerado"
        );
    }
}
