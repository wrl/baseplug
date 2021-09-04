use std::os::raw::c_void;


use super::*;


macro_rules! adapter_from_effect {
    ($ptr:ident) => (
        &mut *container_of!($ptr, VST2Adapter<T>, effect)
    )
}

macro_rules! forward_to_adapter {
    ($method:ident, ($($arg:ident: $ty:ty),+), $ret:ty) => {
        extern "C" fn $method<T: Plugin>(effect: *mut AEffect, $($arg: $ty,)+) -> $ret {
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

extern "C" fn process_deprecated(_effect: *mut AEffect, _in: *const *const f32,
    _out: *mut *mut f32, _nframes: i32)
{
}

extern "C" fn process_replacing_f64(_effect: *mut AEffect, _in: *const *const f64,
    _out: *mut *mut f64, _nframes: i32)
{
}

pub fn plugin_main<P: Plugin>(host_cb: HostCallbackProc, unique_id: &[u8; 4]) -> *mut AEffect {
    let mut flags = effect_flags::CAN_REPLACING | effect_flags::PROGRAM_CHUNKS;

    if WrappedPlugin::<P>::wants_midi_input() {
        flags |= effect_flags::IS_SYNTH;
    }

    if VST2Adapter::<P>::has_ui() {
        flags |= effect_flags::HAS_EDITOR;
    }

    let unique_id =
          (unique_id[0] as u32) << 24
        | (unique_id[1] as u32) << 16
        | (unique_id[2] as u32) << 8
        | (unique_id[3] as u32);

    let adapter = Box::new(VST2Adapter::<P> {
        effect: AEffect {
            magic: MAGIC,

            dispatcher: dispatch::<P>,
            process: process_deprecated,
            set_parameter: set_parameter::<P>,
            get_parameter: get_parameter::<P>,

            num_programs: 0,
            num_params: <P::Model as Model<P>>::Smooth::PARAMS.len() as i32,
            num_inputs: P::INPUT_CHANNELS as i32,
            num_outputs: P::OUTPUT_CHANNELS as i32,

            flags: flags,

            ptr_1: ptr::null_mut(),
            ptr_2: ptr::null_mut(),

            initial_delay: 0,

            empty_2: [0; 8],
            unknown_float: 0.0,

            object: ptr::null_mut(),
            user: ptr::null_mut(),

            unique_id: unique_id as i32,
            version: 0,

            process_replacing: process_replacing::<P>,
            process_double_replacing: process_replacing_f64,
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
        #[cfg(crate_type="bin")]
        std::compile_error!("vst2 requires an exported main() symbol, this will conflict for example with `cargo test` and non dynamic library crates.");

        #[cfg(test)]
        std::compile_error!("vst2 requires an exported main() symbol, this will conflict for example with `cargo test` and non dynamic library crates.");

        #[allow(non_snake_case)]
        #[no_mangle]
        pub extern "C" fn main(host_callback: $crate::api::vst2::vst2_sys::HostCallbackProc) -> *mut $crate::api::vst2::vst2_sys::AEffect {
            VSTPluginMain(host_callback)
        }

        #[allow(non_snake_case)]
        #[no_mangle]
        pub extern "C" fn VSTPluginMain(host_callback: $crate::api::vst2::vst2_sys::HostCallbackProc) -> *mut $crate::api::vst2::vst2_sys::AEffect {
            $crate::api::vst2::plugin_main::<$plugin>(host_callback, $unique_id) as *mut $crate::api::vst2::vst2_sys::AEffect
        }
    }
}
