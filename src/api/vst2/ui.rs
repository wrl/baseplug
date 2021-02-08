use std::os::raw::c_void;

use raw_window_handle::{RawWindowHandle, HasRawWindowHandle};


use super::*;

struct VST2WindowHandle(RawWindowHandle);

impl VST2WindowHandle {
    pub(crate) fn new(raw: *mut c_void) -> Self {
        let handle = {
            #[cfg(any(
                target_os = "linux",
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "netbsd",
                target_os = "openbsd"
            ))]
            {
                use raw_window_handle::unix::*;

                RawWindowHandle::Xcb(XcbHandle {
                    window: raw as u32,
                    ..XcbHandle::empty()
                })
            }

            #[cfg(target_os = "windows")]
            {
                use raw_window_handle::windows::*;

                RawWindowHandle::Windows(WindowsHandle {
                    hwnd: raw,
                    ..WindowsHandle::empty()
                })
            }

            #[cfg(target_os = "macos")]
            {
                use raw_window_handle::macos::*;

                RawWindowHandle::MacOS(MacOSHandle {
                    ns_view: raw,
                    ..MacOSHandle::empty()
                })
            }
        };

        Self(handle)
    }
}

unsafe impl HasRawWindowHandle for VST2WindowHandle {
    fn raw_window_handle(&self) -> RawWindowHandle {
        self.0
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
        let parent = VST2WindowHandle::new(parent);

        if self.wrapped.ui_handle.is_none() {
            P::ui_open(&parent)
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
