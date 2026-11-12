//! Extracts the real icon of an executable using the Windows Shell.

use eframe::egui::ColorImage;
use windows::core::PCWSTR;
use windows::Win32::Graphics::Gdi::{
    DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC, BITMAP, BITMAPINFO, BITMAPINFOHEADER,
    BI_RGB, DIB_RGB_COLORS, HGDIOBJ,
};
use windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL;
use windows::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON};
use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, ICONINFO};

pub fn load_exe_icon(path: &str) -> Option<ColorImage> {
    unsafe {
        let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
        let mut file_info = SHFILEINFOW::default();
        let result = SHGetFileInfoW(
            PCWSTR(wide.as_ptr()),
            FILE_ATTRIBUTE_NORMAL,
            Some(&mut file_info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_ICON | SHGFI_LARGEICON,
        );
        if result == 0 || file_info.hIcon.is_invalid() {
            return None;
        }

        let mut icon_info = ICONINFO::default();
        if GetIconInfo(file_info.hIcon, &mut icon_info).is_err() {
            let _ = DestroyIcon(file_info.hIcon);
            return None;
        }

        let mut image = None;
        if !icon_info.hbmColor.is_invalid() {
            let mut bitmap = BITMAP::default();
            let copied = GetObjectW(
                HGDIOBJ(icon_info.hbmColor.0),
                std::mem::size_of::<BITMAP>() as i32,
                Some(&mut bitmap as *mut _ as *mut _),
            );
            if copied != 0 && bitmap.bmWidth > 0 && bitmap.bmHeight > 0 {
                let width = bitmap.bmWidth;
                let height = bitmap.bmHeight;
                let mut info = BITMAPINFO::default();
                info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
                info.bmiHeader.biWidth = width;
                info.bmiHeader.biHeight = -height;
                info.bmiHeader.biPlanes = 1;
                info.bmiHeader.biBitCount = 32;
                info.bmiHeader.biCompression = BI_RGB.0;

                let mut pixels = vec![0u8; width as usize * height as usize * 4];
                let dc = GetDC(None);
                let lines = GetDIBits(
                    dc,
                    icon_info.hbmColor,
                    0,
                    height as u32,
                    Some(pixels.as_mut_ptr() as *mut _),
                    &mut info,
                    DIB_RGB_COLORS,
                );
                ReleaseDC(None, dc);

                if lines != 0 {
                    let mut has_alpha = false;
                    for pixel in pixels.as_chunks_mut::<4>().0 {
                        pixel.swap(0, 2);
                        if pixel[3] != 0 {
                            has_alpha = true;
                        }
                    }
                    if !has_alpha {
                        for pixel in pixels.as_chunks_mut::<4>().0 {
                            pixel[3] = 255;
                        }
                    }
                    image = Some(ColorImage::from_rgba_unmultiplied(
                        [width as usize, height as usize],
                        &pixels,
                    ));
                }
            }
        }

        if !icon_info.hbmColor.is_invalid() {
            let _ = DeleteObject(HGDIOBJ(icon_info.hbmColor.0));
        }
        if !icon_info.hbmMask.is_invalid() {
            let _ = DeleteObject(HGDIOBJ(icon_info.hbmMask.0));
        }
        let _ = DestroyIcon(file_info.hIcon);
        image
    }
}
