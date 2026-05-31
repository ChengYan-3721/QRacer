use anyhow::{Result, anyhow};
use image::DynamicImage;

#[cfg(target_os = "windows")]
pub fn select_screen_region() -> Result<DynamicImage> {
    windows::select_screen_region()
}

#[cfg(target_os = "windows")]
pub fn restore_main_window() {
    windows::restore_main_window();
}

#[cfg(not(target_os = "windows"))]
pub fn select_screen_region() -> Result<DynamicImage> {
    Err(anyhow!("截屏框选目前仅支持 Windows"))
}

#[cfg(not(target_os = "windows"))]
pub fn restore_main_window() {}

#[cfg(target_os = "windows")]
mod windows {
    #![allow(unsafe_op_in_unsafe_fn)]

    use super::*;
    use image::RgbaImage;
    use std::mem::{size_of, zeroed};
    use std::ptr::{null, null_mut};
    use std::time::Duration;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::{
        BI_RGB, BITMAPINFO, BeginPaint, BitBlt, CAPTUREBLT, CreateCompatibleBitmap,
        CreateCompatibleDC, CreatePen, CreateSolidBrush, DIB_RGB_COLORS, DeleteDC, DeleteObject,
        EndPaint, FillRect, GetDC, GetDIBits, GetStockObject, HOLLOW_BRUSH, InvalidateRect,
        PS_SOLID, Rectangle, ReleaseDC, SRCCOPY, SelectObject, SetBkMode, SetTextColor,
        TRANSPARENT, TextOutW, UpdateWindow,
    };
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DestroyWindow,
        DispatchMessageW, FindWindowW, GWLP_USERDATA, GetMessageW, GetSystemMetrics,
        GetWindowLongPtrW, IDC_CROSS, LWA_ALPHA, LoadCursorW, MSG, PostQuitMessage, RegisterClassW,
        SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SW_RESTORE,
        SW_SHOW, SetForegroundWindow, SetLayeredWindowAttributes, SetWindowLongPtrW, ShowWindow,
        TranslateMessage, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_NCCREATE,
        WM_PAINT, WM_RBUTTONDOWN, WNDCLASSW, WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
        WS_POPUP,
    };

    const VK_ESCAPE_CODE: usize = 0x1B;
    const OVERLAY_ALPHA: u8 = 86;

    struct OverlayState {
        origin_x: i32,
        origin_y: i32,
        width: i32,
        height: i32,
        drag_start: Option<POINT>,
        current: POINT,
        selection: Option<RECT>,
        cancelled: bool,
    }

    pub fn select_screen_region() -> Result<DynamicImage> {
        let metrics = screen_metrics()?;
        let selection = unsafe { run_overlay(metrics)? };
        std::thread::sleep(Duration::from_millis(80));
        capture_rect(selection)
    }

    pub fn restore_main_window() {
        unsafe {
            let title = wide("QRacer");
            let hwnd = FindWindowW(null(), title.as_ptr());
            if !hwnd.is_null() {
                ShowWindow(hwnd, SW_RESTORE);
                SetForegroundWindow(hwnd);
            }
        }
    }

    fn screen_metrics() -> Result<RECT> {
        let x = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
        let y = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };
        let w = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
        let h = unsafe { GetSystemMetrics(SM_CYVIRTUALSCREEN) };
        if w <= 0 || h <= 0 {
            return Err(anyhow!("无法获取屏幕尺寸"));
        }
        Ok(RECT {
            left: x,
            top: y,
            right: x + w,
            bottom: y + h,
        })
    }

    unsafe fn run_overlay(metrics: RECT) -> Result<RECT> {
        let class_name = wide("QRacerScreenCaptureOverlay");
        let title = wide("QRacer 截屏");
        let hinstance = GetModuleHandleW(null());
        let wnd_class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(overlay_wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance,
            hIcon: null_mut(),
            hCursor: LoadCursorW(null_mut(), IDC_CROSS),
            hbrBackground: null_mut(),
            lpszMenuName: null(),
            lpszClassName: class_name.as_ptr(),
        };
        RegisterClassW(&wnd_class);

        let mut state = Box::new(OverlayState {
            origin_x: metrics.left,
            origin_y: metrics.top,
            width: metrics.right - metrics.left,
            height: metrics.bottom - metrics.top,
            drag_start: None,
            current: POINT { x: 0, y: 0 },
            selection: None,
            cancelled: false,
        });
        let state_ptr = state.as_mut() as *mut OverlayState;

        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_POPUP,
            metrics.left,
            metrics.top,
            state.width,
            state.height,
            null_mut(),
            null_mut(),
            hinstance,
            state_ptr.cast(),
        );
        if hwnd.is_null() {
            return Err(anyhow!("无法创建截屏遮罩窗口"));
        }

        SetLayeredWindowAttributes(hwnd, 0, OVERLAY_ALPHA, LWA_ALPHA);
        ShowWindow(hwnd, SW_SHOW);
        UpdateWindow(hwnd);
        SetForegroundWindow(hwnd);

        let mut msg: MSG = zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
            if state.cancelled || state.selection.is_some() {
                break;
            }
        }

        DestroyWindow(hwnd);

        if state.cancelled {
            return Err(anyhow!("已取消截屏"));
        }
        state.selection.ok_or_else(|| anyhow!("未选择截屏区域"))
    }

    unsafe extern "system" fn overlay_wnd_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if msg == WM_NCCREATE {
            let create = lparam as *const CREATESTRUCTW;
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, (*create).lpCreateParams as isize);
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }

        let state_ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut OverlayState;
        if state_ptr.is_null() {
            return DefWindowProcW(hwnd, msg, wparam, lparam);
        }
        let state = &mut *state_ptr;

        match msg {
            WM_LBUTTONDOWN => {
                let point = point_from_lparam(lparam);
                state.drag_start = Some(point);
                state.current = point;
                SetCapture(hwnd);
                InvalidateRect(hwnd, null(), 1);
                0
            }
            WM_MOUSEMOVE => {
                if state.drag_start.is_some() {
                    state.current = point_from_lparam(lparam);
                    InvalidateRect(hwnd, null(), 1);
                }
                0
            }
            WM_LBUTTONUP => {
                if let Some(start) = state.drag_start {
                    let end = point_from_lparam(lparam);
                    let rect = normalized_screen_rect(state, start, end);
                    if rect.right - rect.left >= 8 && rect.bottom - rect.top >= 8 {
                        state.selection = Some(rect);
                    } else {
                        state.cancelled = true;
                    }
                } else {
                    state.cancelled = true;
                }
                ReleaseCapture();
                PostQuitMessage(0);
                0
            }
            WM_RBUTTONDOWN => {
                state.cancelled = true;
                ReleaseCapture();
                PostQuitMessage(0);
                0
            }
            WM_KEYDOWN => {
                if wparam == VK_ESCAPE_CODE {
                    state.cancelled = true;
                    ReleaseCapture();
                    PostQuitMessage(0);
                    0
                } else {
                    DefWindowProcW(hwnd, msg, wparam, lparam)
                }
            }
            WM_PAINT => {
                paint_overlay(hwnd, state);
                0
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }

    unsafe fn paint_overlay(hwnd: HWND, state: &OverlayState) {
        let mut ps = zeroed();
        let hdc = BeginPaint(hwnd, &mut ps);
        let full = RECT {
            left: 0,
            top: 0,
            right: state.width,
            bottom: state.height,
        };
        let brush = CreateSolidBrush(rgb(0, 0, 0));
        FillRect(hdc, &full, brush);
        DeleteObject(brush);

        let text = wide("拖拽框选要处理的码，Esc 或右键取消");
        SetBkMode(hdc, TRANSPARENT as i32);
        SetTextColor(hdc, rgb(255, 255, 255));
        TextOutW(hdc, 24, 24, text.as_ptr(), (text.len() - 1) as i32);

        if let Some(start) = state.drag_start {
            let current = state.current;
            let left = start.x.min(current.x);
            let top = start.y.min(current.y);
            let right = start.x.max(current.x);
            let bottom = start.y.max(current.y);
            let pen = CreatePen(PS_SOLID, 2, rgb(0, 180, 255));
            let old_pen = SelectObject(hdc, pen);
            let old_brush = SelectObject(hdc, GetStockObject(HOLLOW_BRUSH));
            Rectangle(hdc, left, top, right, bottom);
            SelectObject(hdc, old_brush);
            SelectObject(hdc, old_pen);
            DeleteObject(pen);
        }

        EndPaint(hwnd, &ps);
    }

    fn capture_rect(rect: RECT) -> Result<DynamicImage> {
        let width = rect.right - rect.left;
        let height = rect.bottom - rect.top;
        if width <= 0 || height <= 0 {
            return Err(anyhow!("截屏区域尺寸无效"));
        }

        unsafe {
            let screen_dc = GetDC(null_mut());
            if screen_dc.is_null() {
                return Err(anyhow!("无法访问屏幕 DC"));
            }
            let mem_dc = CreateCompatibleDC(screen_dc);
            if mem_dc.is_null() {
                ReleaseDC(null_mut(), screen_dc);
                return Err(anyhow!("无法创建截屏 DC"));
            }
            let bitmap = CreateCompatibleBitmap(screen_dc, width, height);
            if bitmap.is_null() {
                DeleteDC(mem_dc);
                ReleaseDC(null_mut(), screen_dc);
                return Err(anyhow!("无法创建截屏位图"));
            }

            let old_bitmap = SelectObject(mem_dc, bitmap);
            let rop = SRCCOPY | CAPTUREBLT;
            let copied = BitBlt(
                mem_dc, 0, 0, width, height, screen_dc, rect.left, rect.top, rop,
            );
            if copied == 0 {
                SelectObject(mem_dc, old_bitmap);
                DeleteObject(bitmap);
                DeleteDC(mem_dc);
                ReleaseDC(null_mut(), screen_dc);
                return Err(anyhow!("复制屏幕像素失败"));
            }

            let mut info: BITMAPINFO = zeroed();
            info.bmiHeader.biSize =
                size_of::<windows_sys::Win32::Graphics::Gdi::BITMAPINFOHEADER>() as u32;
            info.bmiHeader.biWidth = width;
            info.bmiHeader.biHeight = -height;
            info.bmiHeader.biPlanes = 1;
            info.bmiHeader.biBitCount = 32;
            info.bmiHeader.biCompression = BI_RGB;

            let mut data = vec![0_u8; width as usize * height as usize * 4];
            let rows = GetDIBits(
                mem_dc,
                bitmap,
                0,
                height as u32,
                data.as_mut_ptr().cast(),
                &mut info,
                DIB_RGB_COLORS,
            );

            SelectObject(mem_dc, old_bitmap);
            DeleteObject(bitmap);
            DeleteDC(mem_dc);
            ReleaseDC(null_mut(), screen_dc);

            if rows == 0 {
                return Err(anyhow!("读取截屏像素失败"));
            }

            for pixel in data.chunks_exact_mut(4) {
                pixel.swap(0, 2);
                pixel[3] = 255;
            }
            let image = RgbaImage::from_raw(width as u32, height as u32, data)
                .ok_or_else(|| anyhow!("截屏像素数据尺寸异常"))?;
            Ok(DynamicImage::ImageRgba8(image))
        }
    }

    fn point_from_lparam(lparam: LPARAM) -> POINT {
        POINT {
            x: (lparam & 0xffff) as i16 as i32,
            y: ((lparam >> 16) & 0xffff) as i16 as i32,
        }
    }

    fn normalized_screen_rect(state: &OverlayState, a: POINT, b: POINT) -> RECT {
        RECT {
            left: state.origin_x + a.x.min(b.x),
            top: state.origin_y + a.y.min(b.y),
            right: state.origin_x + a.x.max(b.x),
            bottom: state.origin_y + a.y.max(b.y),
        }
    }

    fn rgb(r: u8, g: u8, b: u8) -> u32 {
        r as u32 | ((g as u32) << 8) | ((b as u32) << 16)
    }

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }
}
