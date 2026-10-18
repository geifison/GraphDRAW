//! Lado COM do gerador de miniaturas: fábrica de classe, o objeto
//! `IThumbnailProvider` + `IInitializeWithStream` e o registro por usuário.

#![allow(non_snake_case)]

use std::cell::RefCell;
use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicIsize, AtomicU32, Ordering};

use windows::Win32::Foundation::{
    CLASS_E_CLASSNOTAVAILABLE, E_FAIL, E_POINTER, ERROR_SUCCESS, HINSTANCE, HMODULE, S_FALSE, S_OK,
};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateDIBSection, DIB_RGB_COLORS, HBITMAP,
};
use windows::Win32::System::Com::{IClassFactory, IClassFactory_Impl, IStream};
use windows::Win32::System::LibraryLoader::GetModuleFileNameW;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_WRITE, REG_OPTION_NON_VOLATILE, REG_SZ, RegCloseKey,
    RegCreateKeyExW, RegDeleteTreeW, RegSetValueExW,
};
use windows::Win32::System::SystemServices::DLL_PROCESS_ATTACH;
use windows::Win32::UI::Shell::PropertiesSystem::{
    IInitializeWithStream, IInitializeWithStream_Impl,
};
use windows::Win32::UI::Shell::{
    IThumbnailProvider, IThumbnailProvider_Impl, SHCNE_ASSOCCHANGED, SHCNF_IDLIST, SHChangeNotify,
    WTS_ALPHATYPE, WTSAT_ARGB,
};
use windows::core::{BOOL, GUID, HRESULT, IUnknown, Interface, PCWSTR, Ref, Result, implement};

/// GUID do gerador de miniaturas (o mesmo de [`crate::CLSID_STR`]).
const CLSID: GUID = GUID::from_u128(0x2BEA79CE_0115_4A17_B4E5_E2993BBF4825);
/// Categoria do shell "gerador de miniaturas" (IThumbnailProvider).
const THUMB_HANDLER: &str = "{e357fccd-a995-4576-b01f-234630154e96}";
/// Maior arquivo lido (a miniatura fica no começo do ZIP mesmo assim).
const MAX_FILE: usize = 512 * 1024 * 1024;

static DLL: AtomicIsize = AtomicIsize::new(0);
static LOCKS: AtomicU32 = AtomicU32::new(0);
static OBJECTS: AtomicU32 = AtomicU32::new(0);

#[implement(IThumbnailProvider, IInitializeWithStream)]
struct Provider {
    data: RefCell<Option<Vec<u8>>>,
}

impl Provider {
    fn new() -> Provider {
        OBJECTS.fetch_add(1, Ordering::SeqCst);
        Provider {
            data: RefCell::new(None),
        }
    }
}

impl Drop for Provider {
    fn drop(&mut self) {
        OBJECTS.fetch_sub(1, Ordering::SeqCst);
    }
}

impl IInitializeWithStream_Impl for Provider_Impl {
    fn Initialize(&self, pstream: Ref<'_, IStream>, _grfmode: u32) -> Result<()> {
        let stream = pstream.ok()?;
        let mut all = Vec::new();
        let mut buf = vec![0u8; 64 * 1024];
        loop {
            let mut read = 0u32;
            let hr = unsafe {
                stream.Read(
                    buf.as_mut_ptr() as *mut c_void,
                    buf.len() as u32,
                    Some(&mut read),
                )
            };
            if hr.is_err() {
                return Err(E_FAIL.into());
            }
            all.extend_from_slice(&buf[..read as usize]);
            if read == 0 || hr == S_FALSE || all.len() > MAX_FILE {
                break;
            }
        }
        *self.data.borrow_mut() = Some(all);
        Ok(())
    }
}

impl IThumbnailProvider_Impl for Provider_Impl {
    fn GetThumbnail(
        &self,
        cx: u32,
        phbmp: *mut HBITMAP,
        pdwalpha: *mut WTS_ALPHATYPE,
    ) -> Result<()> {
        if phbmp.is_null() || pdwalpha.is_null() {
            return Err(E_POINTER.into());
        }
        let data = self.data.borrow();
        let bytes = data.as_deref().ok_or(windows::core::Error::from(E_FAIL))?;
        let made = catch_unwind(AssertUnwindSafe(|| crate::thumbnail_bgra(bytes, cx)))
            .ok()
            .flatten();
        let Some((w, h, bgra)) = made else {
            return Err(E_FAIL.into());
        };
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w as i32,
                // Negativo: linhas de cima para baixo.
                biHeight: -(h as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut c_void = std::ptr::null_mut();
        let bmp = unsafe { CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0)? };
        if bits.is_null() {
            return Err(E_FAIL.into());
        }
        // Alfa pré-multiplicado, como o shell espera em WTSAT_ARGB.
        let dst = unsafe { std::slice::from_raw_parts_mut(bits as *mut u8, bgra.len()) };
        for (d, s) in dst
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .zip(bgra.as_chunks::<4>().0)
        {
            let a = s[3] as u32;
            d[0] = (s[0] as u32 * a / 255) as u8;
            d[1] = (s[1] as u32 * a / 255) as u8;
            d[2] = (s[2] as u32 * a / 255) as u8;
            d[3] = s[3];
        }
        unsafe {
            *phbmp = bmp;
            *pdwalpha = WTSAT_ARGB;
        }
        Ok(())
    }
}

#[implement(IClassFactory)]
struct Factory;

impl IClassFactory_Impl for Factory_Impl {
    fn CreateInstance(
        &self,
        punkouter: Ref<'_, IUnknown>,
        riid: *const GUID,
        ppvobject: *mut *mut c_void,
    ) -> Result<()> {
        if ppvobject.is_null() || riid.is_null() {
            return Err(E_POINTER.into());
        }
        unsafe { *ppvobject = std::ptr::null_mut() };
        if punkouter.is_some() {
            return Err(windows::Win32::Foundation::CLASS_E_NOAGGREGATION.into());
        }
        let unk: IUnknown = Provider::new().into();
        unsafe { unk.query(riid, ppvobject).ok() }
    }

    fn LockServer(&self, flock: BOOL) -> Result<()> {
        if flock.as_bool() {
            LOCKS.fetch_add(1, Ordering::SeqCst);
        } else {
            LOCKS.fetch_sub(1, Ordering::SeqCst);
        }
        Ok(())
    }
}

#[unsafe(no_mangle)]
extern "system" fn DllMain(hinst: HINSTANCE, reason: u32, _reserved: *mut c_void) -> BOOL {
    if reason == DLL_PROCESS_ATTACH {
        DLL.store(hinst.0 as isize, Ordering::SeqCst);
    }
    BOOL(1)
}

#[unsafe(no_mangle)]
extern "system" fn DllGetClassObject(
    rclsid: *const GUID,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    catch_unwind(|| unsafe {
        if rclsid.is_null() || riid.is_null() || ppv.is_null() {
            return E_POINTER;
        }
        *ppv = std::ptr::null_mut();
        if *rclsid != CLSID {
            return CLASS_E_CLASSNOTAVAILABLE;
        }
        let f: IClassFactory = Factory.into();
        f.query(riid, ppv)
    })
    .unwrap_or(E_FAIL)
}

#[unsafe(no_mangle)]
extern "system" fn DllCanUnloadNow() -> HRESULT {
    if LOCKS.load(Ordering::SeqCst) == 0 && OBJECTS.load(Ordering::SeqCst) == 0 {
        S_OK
    } else {
        S_FALSE
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Grava `HKCU\Software\Classes\<path>` com o valor `name` (vazio = padrão).
fn set_value(path: &str, name: &str, value: &str) -> bool {
    let key_path = wide(&format!("Software\\Classes\\{path}"));
    let mut key = HKEY::default();
    unsafe {
        if RegCreateKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(key_path.as_ptr()),
            None,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut key,
            None,
        ) != ERROR_SUCCESS
        {
            return false;
        }
        let v = wide(value);
        let bytes = std::slice::from_raw_parts(v.as_ptr() as *const u8, v.len() * 2);
        let n = (!name.is_empty()).then(|| wide(name));
        let ok = RegSetValueExW(
            key,
            n.as_ref().map_or(PCWSTR::null(), |n| PCWSTR(n.as_ptr())),
            None,
            REG_SZ,
            Some(bytes),
        ) == ERROR_SUCCESS;
        let _ = RegCloseKey(key);
        ok
    }
}

fn dll_path() -> Option<String> {
    let mut buf = vec![0u16; 32_768];
    let n = unsafe {
        GetModuleFileNameW(
            Some(HMODULE(DLL.load(Ordering::SeqCst) as *mut c_void)),
            &mut buf,
        )
    };
    (n > 0).then(|| String::from_utf16_lossy(&buf[..n as usize]))
}

/// Registra para o usuário atual: a classe COM e o `.gdr` usando-a como
/// gerador de miniaturas.
#[unsafe(no_mangle)]
extern "system" fn DllRegisterServer() -> HRESULT {
    let Some(path) = dll_path() else {
        return E_FAIL;
    };
    let clsid = crate::CLSID_STR;
    let ok = set_value(
        &format!("CLSID\\{clsid}"),
        "",
        "GraphDraw — miniaturas .gdr",
    ) && set_value(&format!("CLSID\\{clsid}\\InprocServer32"), "", &path)
        && set_value(
            &format!("CLSID\\{clsid}\\InprocServer32"),
            "ThreadingModel",
            "Apartment",
        )
        && set_value(&format!(".gdr\\ShellEx\\{THUMB_HANDLER}"), "", clsid);
    if !ok {
        return E_FAIL;
    }
    unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None) };
    S_OK
}

/// Tira o registro feito por [`DllRegisterServer`].
#[unsafe(no_mangle)]
extern "system" fn DllUnregisterServer() -> HRESULT {
    let clsid = crate::CLSID_STR;
    for p in [
        format!("Software\\Classes\\.gdr\\ShellEx\\{THUMB_HANDLER}"),
        format!("Software\\Classes\\CLSID\\{clsid}"),
    ] {
        let w = wide(&p);
        unsafe {
            let _ = RegDeleteTreeW(HKEY_CURRENT_USER, PCWSTR(w.as_ptr()));
        }
    }
    unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED, SHCNF_IDLIST, None, None) };
    S_OK
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Graphics::Gdi::{BITMAP, DeleteObject, GetObjectW, HGDIOBJ};
    use windows::Win32::UI::Shell::SHCreateMemStream;

    /// O caminho do Explorador: fábrica → objeto → Initialize(arquivo) →
    /// GetThumbnail(cx) devolve um bitmap 32 bits no tamanho pedido.
    #[test]
    fn com_path_returns_bitmap() {
        let mut d = graphdraw_core::model::Document::new();
        d.data
            .pages
            .push(graphdraw_core::model::Page::new(100_000.0, 50_000.0));
        let bytes = graphdraw_core::gdr::write_bytes(&d, &Default::default()).unwrap();
        unsafe {
            let mut f: *mut c_void = std::ptr::null_mut();
            assert_eq!(DllGetClassObject(&CLSID, &IClassFactory::IID, &mut f), S_OK);
            let factory = IClassFactory::from_raw(f);
            let provider: IThumbnailProvider = factory.CreateInstance(None).unwrap();
            let init: IInitializeWithStream = provider.cast().unwrap();
            let stream = SHCreateMemStream(Some(&bytes)).unwrap();
            init.Initialize(&stream, 0).unwrap();
            let mut bmp = HBITMAP::default();
            let mut alpha = WTS_ALPHATYPE::default();
            provider.GetThumbnail(128, &mut bmp, &mut alpha).unwrap();
            assert_eq!(alpha, WTSAT_ARGB);
            let mut info = BITMAP::default();
            let n = GetObjectW(
                HGDIOBJ(bmp.0),
                std::mem::size_of::<BITMAP>() as i32,
                Some(&mut info as *mut _ as *mut c_void),
            );
            assert!(n > 0);
            assert_eq!(
                (info.bmWidth, info.bmHeight, info.bmBitsPixel),
                (128, 64, 32)
            );
            let _ = DeleteObject(HGDIOBJ(bmp.0));
            // Classe desconhecida: recusa.
            let other = GUID::from_u128(1);
            assert_eq!(
                DllGetClassObject(&other, &IClassFactory::IID, &mut f),
                CLASS_E_CLASSNOTAVAILABLE
            );
        }
    }
}
