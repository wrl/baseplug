use std::os::raw::c_void;

use vst::api::consts::*;
use vst::editor::Rect;
use vst::api::*;


use crate::*;
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

pub fn plugin_main<T: Plugin>(host_cb: HostCallbackProc, unique_id: &[u8; 4]) -> *mut AEffect {
    let mut flags =
        PluginFlags::CAN_REPLACING | PluginFlags::PROGRAM_CHUNKS;

    if WrappedPlugin::<T>::wants_midi_input() {
        flags |= PluginFlags::IS_SYNTH;
    }

    let unique_id =
          (unique_id[0] as u32) << 24
        | (unique_id[1] as u32) << 16
        | (unique_id[2] as u32) << 8
        | (unique_id[3] as u32);

    let adapter = Box::new(VST2Adapter::<T> {
        effect: AEffect {
            magic: VST_MAGIC,

            dispatcher: dispatch::<T>,
            setParameter: set_parameter::<T>,
            getParameter: get_parameter::<T>,

            _process: process_deprecated,

            numPrograms: 0,
            numParams: <T::Model as Model>::Smooth::PARAMS.len() as i32,
            numInputs: T::INPUT_CHANNELS as i32,
            numOutputs: T::OUTPUT_CHANNELS as i32,

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

            processReplacing: process_replacing::<T>,
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
        state: None
    });

    unsafe {
        &mut ((*Box::into_raw(adapter)).effect)
    }
}

#[macro_export]
macro_rules! vst2 {
    ($plugin:ty, $unique_id:expr) => {
        #[allow(non_snake_case)]
        #[no_mangle]
        pub extern "C" fn main(host_callback: ::vst::api::HostCallbackProc)
            -> *mut ::vst::api::AEffect
        {
            VSTPluginMain(host_callback)
        }

        #[allow(non_snake_case)]
        #[no_mangle]
        pub extern "C" fn VSTPluginMain(host_callback: ::vst::api::HostCallbackProc)
            -> *mut ::vst::api::AEffect
        {
            $crate::api::vst2::plugin_main::<$plugin>(
                host_callback, $unique_id)
        }
    }
}
