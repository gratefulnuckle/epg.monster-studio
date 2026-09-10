// SPDX-License-Identifier: GPL-3.0-or-later

//! In-process libmpv, loaded with `libloading` at runtime. Never a link-time
//! import of `libmpv-2.dll` / `libmpv.so.2` / `libmpv.dylib`.

use std::ffi::CString;
use std::os::raw::{c_char, c_int};
use std::path::PathBuf;
use std::ptr;

use libloading::{Library, Symbol};

use crate::engine::{Engine, EngineId, PlayRequest, PlayState};
use crate::paths;

type MpvHandle = *mut std::ffi::c_void;

struct SendHandle(MpvHandle);

unsafe impl Send for SendHandle {}
type FnCreate = unsafe extern "C" fn() -> MpvHandle;
type FnSetOpt = unsafe extern "C" fn(MpvHandle, *const c_char, *const c_char) -> c_int;
type FnInit = unsafe extern "C" fn(MpvHandle) -> c_int;
type FnCommand = unsafe extern "C" fn(MpvHandle, *const *const c_char) -> c_int;
type FnSetProp = unsafe extern "C" fn(MpvHandle, *const c_char, *const c_char) -> c_int;
type FnDestroy = unsafe extern "C" fn(MpvHandle);

struct Api {
    create: FnCreate,
    set_option: FnSetOpt,
    initialize: FnInit,
    command: FnCommand,
    set_property: FnSetProp,
    terminate: FnDestroy,
    _lib: Library,
}

impl Api {
    fn load(path: &std::path::Path) -> Result<Self, String> {
        let lib = unsafe { Library::new(path) }.map_err(|_| "libmpv failed to start".to_string())?;
        unsafe {
            let create: Symbol<FnCreate> = lib
                .get(b"mpv_create\0")
                .map_err(|_| "libmpv failed to start".to_string())?;
            let set_option: Symbol<FnSetOpt> = lib
                .get(b"mpv_set_option_string\0")
                .map_err(|_| "libmpv failed to start".to_string())?;
            let initialize: Symbol<FnInit> = lib
                .get(b"mpv_initialize\0")
                .map_err(|_| "libmpv failed to start".to_string())?;
            let command: Symbol<FnCommand> = lib
                .get(b"mpv_command\0")
                .map_err(|_| "libmpv failed to start".to_string())?;
            let set_property: Symbol<FnSetProp> = lib
                .get(b"mpv_set_property_string\0")
                .map_err(|_| "libmpv failed to start".to_string())?;
            let terminate: Symbol<FnDestroy> = lib
                .get(b"mpv_terminate_destroy\0")
                .map_err(|_| "libmpv failed to start".to_string())?;
            Ok(Self {
                create: *create,
                set_option: *set_option,
                initialize: *initialize,
                command: *command,
                set_property: *set_property,
                terminate: *terminate,
                _lib: lib,
            })
        }
    }
}

pub struct Libmpv {
    app_root: PathBuf,
    api: Option<Api>,
    handle: SendHandle,
    state: PlayState,
}

impl Libmpv {
    pub fn new(app_root: PathBuf) -> Self {
        Self {
            app_root,
            api: None,
            handle: SendHandle(ptr::null_mut()),
            state: PlayState::Stopped,
        }
    }

    fn handle(&self) -> MpvHandle {
        self.handle.0
    }

    fn set_opt(&self, key: &str, val: &str) {
        let Some(api) = self.api.as_ref() else {
            return;
        };
        if self.handle().is_null() {
            return;
        }
        let Ok(k) = CString::new(key) else {
            return;
        };
        let Ok(v) = CString::new(val) else {
            return;
        };
        unsafe {
            (api.set_option)(self.handle(), k.as_ptr(), v.as_ptr());
        }
    }

    fn set_prop(&self, key: &str, val: &str) {
        let Some(api) = self.api.as_ref() else {
            return;
        };
        if self.handle().is_null() {
            return;
        }
        let Ok(k) = CString::new(key) else {
            return;
        };
        let Ok(v) = CString::new(val) else {
            return;
        };
        unsafe {
            (api.set_property)(self.handle(), k.as_ptr(), v.as_ptr());
        }
    }

    fn cmd(&self, args: &[&str]) {
        let Some(api) = self.api.as_ref() else {
            return;
        };
        if self.handle().is_null() {
            return;
        }
        let cstrs: Vec<CString> = args.iter().filter_map(|s| CString::new(*s).ok()).collect();
        if cstrs.len() != args.len() {
            return;
        }
        let mut ptrs: Vec<*const c_char> = cstrs.iter().map(|s| s.as_ptr()).collect();
        ptrs.push(ptr::null());
        unsafe {
            (api.command)(self.handle(), ptrs.as_ptr());
        }
    }
}

impl Drop for Libmpv {
    fn drop(&mut self) {
        self.stop();
    }
}

impl Engine for Libmpv {
    fn id(&self) -> EngineId {
        EngineId::Libmpv
    }

    fn start(&mut self, req: &PlayRequest) -> Result<(), String> {
        self.stop();
        let path = paths::libmpv_path(&self.app_root);
        if !path.is_file() {
            self.state = PlayState::Error("libmpv missing".into());
            return Err("libmpv missing".into());
        }
        let api = Api::load(&path)?;
        let handle = unsafe { (api.create)() };
        if handle.is_null() {
            return Err("libmpv failed to start".into());
        }
        self.api = Some(api);
        self.handle = SendHandle(handle);
        if req.pane != 0 {
            self.set_opt("wid", &req.pane.to_string());
        }
        self.set_opt("force-window", "yes");
        self.set_opt("keep-open", "yes");
        self.set_opt("ytdl", "no");
        self.set_opt("user-agent", &req.ua);
        let fields = req.http_header_fields();
        if !fields.is_empty() {
            self.set_opt("http-header-fields", &fields);
        }
        let rc = unsafe { (self.api.as_ref().unwrap().initialize)(self.handle()) };
        if rc < 0 {
            self.stop();
            return Err("libmpv failed to start".into());
        }
        self.cmd(&["loadfile", &req.url, "replace"]);
        self.state = PlayState::Playing;
        Ok(())
    }

    fn stop(&mut self) {
        if let Some(api) = self.api.take() {
            if !self.handle().is_null() {
                unsafe { (api.terminate)(self.handle()) };
            }
        }
        self.handle = SendHandle(ptr::null_mut());
        self.state = PlayState::Stopped;
    }

    fn pause(&mut self, paused: bool) {
        self.set_prop("pause", if paused { "yes" } else { "no" });
        if matches!(self.state, PlayState::Playing | PlayState::Paused) {
            self.state = if paused {
                PlayState::Paused
            } else {
                PlayState::Playing
            };
        }
    }

    fn mute(&mut self, muted: bool) {
        self.set_prop("mute", if muted { "yes" } else { "no" });
    }

    fn volume(&mut self, vol: f64) {
        let v = (vol.clamp(0.0, 1.0) * 100.0).round();
        self.set_prop("volume", &format!("{v}"));
    }

    fn status(&self) -> PlayState {
        self.state.clone()
    }
}
