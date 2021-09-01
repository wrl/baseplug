use std::os::raw::c_void;

use raw_window_handle::{RawWindowHandle, HasRawWindowHandle};


use super::*;

struct VST2WindowHandle(*mut c_void);

impl From<&VST2WindowHandle> for RawWindowHandle {
    #[cfg(any(
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd"
    ))]
    fn from(handle: &VST2WindowHandle) -> RawWindowHandle {
        use raw_window_handle::unix::*;

        RawWindowHandle::Xcb(XcbHandle {
            window: handle.0 as u32,
            ..XcbHandle::empty()
        })
    }

    #[cfg(target_os = "windows")]
    fn from(handle: &VST2WindowHandle) -> RawWindowHandle {
        use raw_window_handle::windows::*;

        RawWindowHandle::Windows(WindowsHandle {
            hwnd: handle.0,
            ..WindowsHandle::empty()
        })
    }

    #[cfg(target_os = "macos")]
    fn from(handle: &VST2WindowHandle) -> RawWindowHandle {
        use raw_window_handle::macos::*;

        RawWindowHandle::MacOS(MacOSHandle {
            ns_view: handle.0,
            ..MacOSHandle::empty()
        })
    }
}

unsafe impl HasRawWindowHandle for VST2WindowHandle {
    fn raw_window_handle(&self) -> RawWindowHandle {
        self.into()
    }
}

pub(super) trait VST2UI {
    type P: Plugin;

    fn has_ui() -> bool;

    fn ui_get_rect(&self) -> Option<(i16, i16)>;
    fn ui_open(&mut self, model: <<Self::P as Plugin>::Model as Model<Self::P>>::UI, parent: *mut c_void) -> WindowOpenResult<()>;
    fn ui_close(&mut self);
}

impl<P: Plugin> VST2UI for VST2Adapter<P> {
    type P = P;

    default fn has_ui() -> bool {
        false
    }

    default fn ui_get_rect(&self) -> Option<(i16, i16)> {
        None
    }

    default fn ui_open(&mut self, _model: <P::Model as Model<P>>::UI, _parent: *mut c_void) -> WindowOpenResult<()> {
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

    fn ui_open(&mut self, model: <P::Model as Model<P>>::UI, parent: *mut c_void) -> WindowOpenResult<()> {
        let parent = VST2WindowHandle(parent);

        if self.wrapped.ui_handle.is_none() {
            match P::ui_open(&parent, model) {
                WindowOpenResult::Ok(handle) => {
                    self.wrapped.ui_handle = Some(handle);
                    WindowOpenResult::Ok(())
                }
                WindowOpenResult::Err(_) => {
                    self.wrapped.ui_msg_handles = None;
                    WindowOpenResult::Err(())
                }
            }
        } else {
            Ok(())
        }
    }

    fn ui_close(&mut self) {
        if let Some(mut ui_msg_handles) = self.wrapped.ui_msg_handles.take() {
            // We can send a message directly to the UI Model if the user wants
            // to handle it that way. This should also take care of making sure the UI Model
            // stops using the host callback between the time it receives the close signal and
            // the UI actually closes.
            if let Err(_) = ui_msg_handles.plug_to_ui_tx.push(PlugToUIMsg::ShouldClose) {
                eprintln!("Plug to UI message buffer is full!");
            }
        }

        if let Some(handle) = self.wrapped.ui_handle.take() {
            // Tell the window handle to close. Ideally the window handle should automatically
            // alert the UI that it will close and close it gracefully, so as to avoid relying
            // on the user to remember.
            P::ui_close(handle);
        }
    }
}