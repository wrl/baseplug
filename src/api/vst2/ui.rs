use std::os::raw::c_void;

use raw_window_handle::RawWindowHandle;


use crate::*;
use super::*;


struct VST2WindowHandle(*mut c_void);

impl From<VST2WindowHandle> for RawWindowHandle {
    #[cfg(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    fn from(handle: VST2WindowHandle) -> RawWindowHandle {
        use raw_window_handle::unix::*;

        RawWindowHandle::Xcb(XcbHandle {
            window: handle.0 as u32,
            ..XcbHandle::empty()
        })
    }

    #[cfg(target_os = "windows")]
    fn from(handle: VST2WindowHandle) -> RawWindowHandle {
        use raw_window_handle::windows::*;

        RawWindowHandle::Windows(WindowsHandle {
            hwnd: handle.0,
            ..WindowsHandle::empty()
        })
    }

    #[cfg(target_os = "macos")]
    fn from(handle: VST2WindowHandle) -> RawWindowHandle {
        use raw_window_handle::macos::*;

        RawWindowHandle::MacOS(MacOSHandle {
            ns_view: handle.0,
            ..MacOSHandle::empty()
        })
    }
}

pub(super) trait VST2UI {
    fn has_ui() -> bool;

    fn ui_get_rect(&self) -> Option<(i16, i16)>;
    fn ui_open(&mut self, parent: *mut c_void) -> WindowOpenResult<()>;
    fn ui_close(&mut self);
}

impl<P: Plugin> VST2UI for VST2Adapter<P> {
    default fn has_ui() -> bool {
        false
    }

    default fn ui_get_rect(&self) -> Option<(i16, i16)> {
        None
    }

    default fn ui_open(&mut self, _parent: *mut c_void) -> WindowOpenResult<()> {
        Err(())
    }

    default fn ui_close(&mut self) { }
}

impl<P: PluginUI> VST2UI for VST2Adapter<P> {
    fn has_ui() -> bool {
        true
    }

    fn ui_get_rect(&self) -> Option<(i16, i16)> {
        Some(P::ui_size())
    }

    fn ui_open(&mut self, parent: *mut c_void) -> WindowOpenResult<()> {
        let parent = VST2WindowHandle(parent);

        if self.wrapped.ui_handle.is_none() {
            P::ui_open(parent.into())
                .map(|handle| self.wrapped.ui_handle = Some(handle))
        } else {
            Ok(())
        }
    }

    fn ui_close(&mut self) {
        if let Some(handle) = self.wrapped.ui_handle.take() {
            P::ui_close(handle)
        }
    }
}
