// SPDX-License-Identifier: GPL-3.0-or-later

//! Native video pane the playback engine draws into.
//!
//! Windows: a `WS_CHILD | WS_CLIPSIBLINGS` window over the WebView2 HWND.
//! Other targets: `create` returns `Error::Unsupported`.

use serde::{Deserialize, Serialize};

#[cfg(windows)]
mod win;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub fn is_empty(self) -> bool {
        self.w <= 0 || self.h <= 0
    }
}

/// Skip `MoveWindow` when the rect did not change. A redundant MoveWindow on a
/// GStreamer-subclassed HWND deadlocks the UI thread.
pub fn should_move(last: Rect, next: Rect) -> bool {
    !next.is_empty() && last != next
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Unsupported,
    Failed(&'static str),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Unsupported => write!(f, "video pane unsupported on this OS"),
            Error::Failed(s) => write!(f, "{s}"),
        }
    }
}

impl std::error::Error for Error {}

pub struct Pane {
    #[cfg(windows)]
    inner: win::Inner,
    last: Rect,
}

unsafe impl Send for Pane {}
unsafe impl Sync for Pane {}

impl Pane {
    pub fn create(parent: usize) -> Result<Self, Error> {
        #[cfg(windows)]
        {
            let inner = win::Inner::create(parent)?;
            return Ok(Self {
                inner,
                last: Rect::default(),
            });
        }
        #[cfg(not(windows))]
        {
            let _ = parent;
            Err(Error::Unsupported)
        }
    }

    pub fn hwnd(&self) -> usize {
        #[cfg(windows)]
        {
            self.inner.hwnd()
        }
        #[cfg(not(windows))]
        {
            0
        }
    }

    pub fn set_rect(&mut self, rect: Rect) {
        if !should_move(self.last, rect) {
            return;
        }
        #[cfg(windows)]
        {
            self.inner.move_to(rect);
        }
        self.last = rect;
    }

    pub fn destroy(&mut self) {
        #[cfg(windows)]
        {
            self.inner.destroy();
        }
        self.last = Rect::default();
    }
}

impl Drop for Pane {
    fn drop(&mut self) {
        self.destroy();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_rect_is_not_moved() {
        let last = Rect {
            x: 0,
            y: 0,
            w: 100,
            h: 80,
        };
        assert!(!should_move(last, Rect { x: 0, y: 0, w: 0, h: 80 }));
        assert!(!should_move(last, Rect { x: 0, y: 0, w: 100, h: 0 }));
        assert!(!should_move(
            last,
            Rect {
                x: 0,
                y: 0,
                w: -1,
                h: 10
            }
        ));
    }

    #[test]
    fn unchanged_rect_is_noop() {
        let r = Rect {
            x: 10,
            y: 20,
            w: 640,
            h: 360,
        };
        assert!(!should_move(r, r));
        assert!(should_move(r, Rect { x: 11, y: 20, w: 640, h: 360 }));
    }

    #[cfg(not(windows))]
    #[test]
    fn non_windows_is_unsupported() {
        assert_eq!(Pane::create(0).err(), Some(Error::Unsupported));
    }

    #[cfg(windows)]
    #[test]
    fn zero_parent_fails() {
        assert!(Pane::create(0).is_err());
    }
}
