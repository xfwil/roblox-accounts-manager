//! Windows-only window thumbnail capture for the bulk-login grid.
//!
//! Uses `PrintWindow` with `PW_RENDERFULLCONTENT`, the documented way to
//! capture WebView2 / Chromium windows — plain BitBlt returns black for
//! GPU-composited content.
//!
//! ponytail: PrintWindow(PW_RENDERFULLCONTENT) is the most reliable method but
//! can still return black on some GPU/driver configs. If that happens, drop
//! thumbnails and surface a "Show window" button instead (the windows are
//! already visible for manual captcha solving, so nothing is lost).

#![cfg(windows)]

use std::ffi::c_void;
use std::io::BufWriter;

use base64::Engine;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits,
    ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP, HGDIOBJ,
};
use windows::Win32::Storage::Xps::{PrintWindow, PRINT_WINDOW_FLAGS};
use windows::Win32::UI::WindowsAndMessaging::{GetClientRect, PW_RENDERFULLCONTENT};

/// Capture a window by raw HWND into a downscaled PNG `data:` URL.
/// Returns None if the window is gone, zero-sized, or capture failed.
pub fn capture_window_png_base64(hwnd_raw: isize, max_width: u32) -> Option<String> {
    unsafe {
        let hwnd = HWND(hwnd_raw as *mut c_void);

        let mut rect = RECT::default();
        if GetClientRect(hwnd, &mut rect).is_err() {
            return None;
        }
        let w = (rect.right - rect.left).max(0) as u32;
        let h = (rect.bottom - rect.top).max(0) as u32;
        if w == 0 || h == 0 {
            return None;
        }

        let screen = GetDC(Some(hwnd));
        if screen.is_invalid() {
            return None;
        }
        let mem = CreateCompatibleDC(Some(screen));
        let bmp = CreateCompatibleBitmap(screen, w as i32, h as i32);
        let old = SelectObject(mem, HGDIOBJ(bmp.0));

        // PW_RENDERFULLCONTENT (2) captures WebView2's GPU content.
        let ok = PrintWindow(hwnd, mem, PRINT_WINDOW_FLAGS(PW_RENDERFULLCONTENT)).as_bool();

        let rgba = if ok {
            read_bgra(mem, bmp, w, h).map(|bgra| bgra_to_rgba(&bgra))
        } else {
            None
        };

        // Cleanup GDI resources.
        SelectObject(mem, old);
        let _ = DeleteObject(HGDIOBJ(bmp.0));
        let _ = DeleteDC(mem);
        ReleaseDC(Some(hwnd), screen);

        let rgba = rgba?;
        let (tw, th, thumb) = downscale(&rgba, w, h, max_width);
        encode_png_base64(&thumb, tw, th)
    }
}

/// Read 32-bit top-down BGRA pixels out of the memory bitmap.
unsafe fn read_bgra(mem: windows::Win32::Graphics::Gdi::HDC, bmp: HBITMAP, w: u32, h: u32) -> Option<Vec<u8>> {
    let mut bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w as i32,
            biHeight: -(h as i32), // negative = top-down
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut buf = vec![0u8; (w * h * 4) as usize];
    let scanned = GetDIBits(
        mem,
        bmp,
        0,
        h,
        Some(buf.as_mut_ptr() as *mut c_void),
        &mut bmi,
        DIB_RGB_COLORS,
    );
    if scanned == 0 {
        None
    } else {
        Some(buf)
    }
}

fn bgra_to_rgba(bgra: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; bgra.len()];
    for (i, px) in bgra.chunks_exact(4).enumerate() {
        let o = i * 4;
        out[o] = px[2];     // R
        out[o + 1] = px[1]; // G
        out[o + 2] = px[0]; // B
        out[o + 3] = 255;   // PrintWindow leaves alpha 0; force opaque
    }
    out
}

/// Nearest-neighbour downscale to `max_width` (keeps aspect; never upscales).
fn downscale(rgba: &[u8], w: u32, h: u32, max_width: u32) -> (u32, u32, Vec<u8>) {
    if w <= max_width {
        return (w, h, rgba.to_vec());
    }
    let tw = max_width;
    let th = ((h as f32) * (tw as f32 / w as f32)).round().max(1.0) as u32;
    let mut out = vec![0u8; (tw * th * 4) as usize];
    for y in 0..th {
        let sy = (y as u64 * h as u64 / th as u64) as u32;
        for x in 0..tw {
            let sx = (x as u64 * w as u64 / tw as u64) as u32;
            let si = ((sy * w + sx) * 4) as usize;
            let di = ((y * tw + x) * 4) as usize;
            out[di..di + 4].copy_from_slice(&rgba[si..si + 4]);
        }
    }
    (tw, th, out)
}

fn encode_png_base64(rgba: &[u8], w: u32, h: u32) -> Option<String> {
    let mut png = Vec::new();
    {
        let mut enc = png::Encoder::new(BufWriter::new(&mut png), w, h);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().ok()?;
        writer.write_image_data(rgba).ok()?;
    }
    let b64 = base64::engine::general_purpose::STANDARD.encode(&png);
    Some(format!("data:image/png;base64,{}", b64))
}

#[cfg(test)]
mod tests {
    use super::{bgra_to_rgba, downscale};

    #[test]
    fn bgra_to_rgba_swaps_channels_and_forces_opaque() {
        let bgra = vec![10, 20, 30, 0, 40, 50, 60, 0];
        assert_eq!(bgra_to_rgba(&bgra), vec![30, 20, 10, 255, 60, 50, 40, 255]);
    }

    #[test]
    fn downscale_halves_width_keeps_aspect_and_buffer_size() {
        let (w, h) = (4u32, 2u32);
        let rgba = vec![0u8; (w * h * 4) as usize];
        let (tw, th, out) = downscale(&rgba, w, h, 2);
        assert_eq!((tw, th), (2, 1));
        assert_eq!(out.len() as u32, tw * th * 4);
    }

    #[test]
    fn downscale_never_upscales() {
        let rgba = vec![1u8; 2 * 2 * 4];
        let (tw, th, out) = downscale(&rgba, 2, 2, 10);
        assert_eq!((tw, th), (2, 2));
        assert_eq!(out, rgba);
    }
}
