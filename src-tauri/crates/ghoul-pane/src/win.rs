// SPDX-License-Identifier: GPL-3.0-or-later

#![cfg(windows)]

use std::sync::atomic::{AtomicIsize, Ordering};

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, EndPaint, FillRect, HBRUSH, HDC, PAINTSTRUCT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GetClientRect, IsWindow, MoveWindow,
    RegisterClassExW, SetWindowPos, CS_HREDRAW, CS_VREDRAW, HMENU, HWND_TOP, SWP_NOACTIVATE,
    SWP_NOMOVE, SWP_NOSIZE, WINDOW_EX_STYLE, WM_DESTROY, WM_ERASEBKGND, WM_PAINT, WNDCLASSEXW,
    WS_CHILD, WS_CLIPSIBLINGS, WS_VISIBLE,
};

use crate::{Error, Rect};

const CLASS: PCWSTR = w!("GhoulVideoPane");
static BRUSH: AtomicIsize = AtomicIsize::new(0);

fn brush() -> HBRUSH {
    let cur = BRUSH.load(Ordering::Relaxed);
    if cur != 0 {
        return HBRUSH(cur as *mut core::ffi::c_void);
    }
    let created = unsafe { CreateSolidBrush(COLORREF(0)) };
    BRUSH.store(created.0 as isize, Ordering::Relaxed);
    created
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_ERASEBKGND => {
            let hdc = HDC(wp.0 as *mut core::ffi::c_void);
            let mut rc = RECT::default();
            let _ = GetClientRect(hwnd, &mut rc);
            let _ = FillRect(hdc, &rc, brush());
            LRESULT(1)
        }
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            let mut rc = RECT::default();
            let _ = GetClientRect(hwnd, &mut rc);
            let _ = FillRect(hdc, &rc, brush());
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_DESTROY => LRESULT(0),
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

fn register_class() -> Result<(), Error> {
    let inst = unsafe { GetModuleHandleW(None) }.map_err(|_| Error::Failed("pane class"))?;
    let wc = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(wnd_proc),
        hInstance: inst.into(),
        hbrBackground: brush(),
        lpszClassName: CLASS,
        ..Default::default()
    };
    unsafe { RegisterClassExW(&wc) };
    Ok(())
}

pub struct Inner {
    hwnd: HWND,
}

impl Inner {
    pub fn create(parent: usize) -> Result<Self, Error> {
        if parent == 0 {
            return Err(Error::Failed("parent hwnd"));
        }
        register_class()?;
        let parent_hwnd = HWND(parent as *mut core::ffi::c_void);
        if !unsafe { IsWindow(parent_hwnd).as_bool() } {
            return Err(Error::Failed("parent hwnd"));
        }
        let inst = unsafe { GetModuleHandleW(None) }.map_err(|_| Error::Failed("pane class"))?;
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                CLASS,
                w!(""),
                WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
                0,
                0,
                16,
                16,
                parent_hwnd,
                HMENU::default(),
                inst,
                None,
            )
        }
        .map_err(|_| Error::Failed("create pane"))?;
        if hwnd.0.is_null() {
            return Err(Error::Failed("create pane"));
        }
        let _ = unsafe {
            SetWindowPos(
                hwnd,
                HWND_TOP,
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
            )
        };
        Ok(Self { hwnd })
    }

    pub fn hwnd(&self) -> usize {
        self.hwnd.0 as usize
    }

    pub fn move_to(&self, rect: Rect) {
        if rect.is_empty() || self.hwnd.0.is_null() {
            return;
        }
        let _ = unsafe { MoveWindow(self.hwnd, rect.x, rect.y, rect.w, rect.h, true) };
        let _ = unsafe {
            SetWindowPos(
                self.hwnd,
                HWND_TOP,
                0,
                0,
                0,
                0,
                SWP_NOACTIVATE | SWP_NOMOVE | SWP_NOSIZE,
            )
        };
    }

    pub fn destroy(&mut self) {
        if !self.hwnd.0.is_null() {
            let _ = unsafe { DestroyWindow(self.hwnd) };
            self.hwnd = HWND(std::ptr::null_mut());
        }
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        self.destroy();
    }
}
