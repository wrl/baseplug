use std::os::raw::c_void;

use vst::api::consts::*;
use vst::editor::Rect;
use vst::api::*;


use super::*;


macro_rules! adapter_from_effect {
    ($ptr:ident) => (
        &mut *container_of!($ptr, VST2Adapter<T>, effect)
    )
}

macro_rules! forward_to_adapter {
    ($method:ident, ($($arg:ident: $ty:ty),+), $ret:ty) => {
        fn $method<T: Plugin>(effect: *mut AEffect, $($arg: $ty,)+) -> $ret {
            let adapter = unsafe { adapter_from_effect!(effect) };
            adapter.$method($($arg,)+)
        }
    }
}

forward_to_adapter!(
    dispatch,
    (opcode: i32, index: i32, value: isize, ptr: *mut c_void, opt: f32),
    isize);

forward_to_adapter!(
    get_parameter,
    (index: i32),
    f32);

forward_to_adapter!(
    set_parameter,
    (index: i32, val: f32),
    ());

forward_to_adapter!(
    process_replacing,
    (in_buffers: *const *const f32, out_buffers: *mut *mut f32, nframes: i32),
    ());

fn process_deprecated(_effect: *mut AEffect, _in: *const *const f32,
    _out: *mut *mut f32, _nframes: i32)
{
}

fn process_replacing_f64(_effect: *mut AEffect, _in: *const *const f64,
    _out: *mut *mut f64, _nframes: i32)
{
}

pub fn plugin_main<P: Plugin>(host_cb: HostCallbackProc, unique_id: &[u8; 4]) -> *mut AEffect {
    let mut flags =
        PluginFlags::CAN_REPLACING | PluginFlags::PROGRAM_CHUNKS;

    if WrappedPlugin::<P>::wants_midi_input() {
        flags |= PluginFlags::IS_SYNTH;
    }

    if VST2Adapter::<P>::has_ui() {
        flags |= PluginFlags::HAS_EDITOR;
    }

    let unique_id =
          (unique_id[0] as u32) << 24
        | (unique_id[1] as u32) << 16
        | (unique_id[2] as u32) << 8
        | (unique_id[3] as u32);

    let adapter = Box::new(VST2Adapter::<P> {
        effect: AEffect {
            magic: VST_MAGIC,

            dispatcher: dispatch::<P>,
            setParameter: set_parameter::<P>,
            getParameter: get_parameter::<P>,

            _process: process_deprecated,

            numPrograms: 0,
            numParams: <P::Model as Model<P>>::Smooth::PARAMS.len() as i32,
            numInputs: P::INPUT_CHANNELS as i32,
            numOutputs: P::OUTPUT_CHANNELS as i32,

            flags: flags.bits(),

            reserved1: 0,
            reserved2: 0,

            initialDelay: 0,

            _realQualities: 0,
            _offQualities: 0,
            _ioRatio: 0.0,

            object: ptr::null_mut(),
            user: ptr::null_mut(),

            uniqueId: unique_id as i32,
            version: 0,

            processReplacing: process_replacing::<P>,
            processReplacingF64: process_replacing_f64,

            future: [0u8; 56]
        },
        
        host_cb,

        editor_rect: Rect {
            top: 0,
            bottom: 0,
            left: 0,
            right: 0,
        },

        wrapped: WrappedPlugin::new(),
        state: None,

        output_events_buffer: OutgoingEvents::new()
    });

    unsafe {
        &mut ((*Box::into_raw(adapter)).effect)
    }
}

#[macro_export]
macro_rules! vst2 {
    ($plugin:ty, $unique_id:expr) => {
        use std::os::raw::c_void;
        use std::mem::transmute;

        #[cfg(crate_type="bin")]
        std::compile_error!("vst2 requires an exported main() symbol, this will conflict for example with `cargo test` and non dynamic library crates.");

        #[cfg(test)]
        std::compile_error!("vst2 requires an exported main() symbol, this will conflict for example with `cargo test` and non dynamic library crates.");

        #[allow(non_snake_case)]
        #[no_mangle]
        pub unsafe extern "C" fn main(host_callback: fn()) -> *mut c_void {
            VSTPluginMain(host_callback)
        }

        #[allow(non_snake_case)]
        #[no_mangle]
        pub unsafe extern "C" fn VSTPluginMain(host_callback: fn()) -> *mut c_void {
            $crate::api::vst2::plugin_main::<$plugin>(
                transmute(host_callback), $unique_id) as *mut _
        }
    }
}
