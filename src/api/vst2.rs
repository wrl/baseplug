use std::slice;
use std::ptr;
use std::io;
use std::os::raw::c_void;

use vst::api::*;
use vst::host;
use vst::api::consts::*;
use vst::plugin::OpCode;

use crate::{
    Model,
    Plugin,
    Parameters,
    Param,
    MusicalTime,
};

use crate::wrapper::*;

// vst-rs doesn't have this for some reason
const MAX_EFFECT_NAME_LEN: usize = 32;

#[inline]
fn cstr_as_slice<'a>(ptr: *mut c_void, len: usize) -> &'a mut [u8] {
    unsafe {
        slice::from_raw_parts_mut(ptr as *mut u8, len)
    }
}

fn cstrcpy(ptr: *mut c_void, src: &str, max_len: usize) {
    let dest = cstr_as_slice(ptr, max_len);
    let src_bytes = src.as_bytes();
    let len = src_bytes.len().min(max_len - 1);

    dest[..len].copy_from_slice(&src_bytes[..len]);
    dest[len] = 0;
}

#[inline]
fn param_for_vst2_id<T>(id: i32) -> Option<&'static Param<T::Smooth>>
    where T: Model
{
    T::Smooth::PARAMS.get(id as usize).copied()
}

macro_rules! param_for_idx {
    ($id:ident) => {
        match param_for_vst2_id::<T::Model>($id) {
            Some(p) => p,
            None => return 0
        }
    }
}

struct VST2Adapter<T: Plugin> {
    effect: AEffect,
    host_cb: HostCallbackProc,
    wrapped: WrappedPlugin<T>,

    // when the VST2 host asks us for the chunk/data/state, the lifetime for that data extends
    // until the *next* time that the host asks us for state. this means we have to just hold this
    // around in memory indefinitely.
    state: Option<Vec<u8>>
}

impl<T: Plugin> VST2Adapter<T> {
    #[inline]
    fn dispatch(&mut self, opcode: i32, index: i32, value: isize, ptr: *mut c_void, opt: f32) -> isize {
        match OpCode::from(opcode) {
            ////
            // lifecycle
            ////

            OpCode::GetApiVersion => return 2400,
            OpCode::Shutdown => {
                unsafe {
                    drop(Box::from_raw(self))
                };
            },

            OpCode::SetSampleRate => self.wrapped.set_sample_rate(opt),

            OpCode::StateChanged => {
                if value == 1 {
                    self.wrapped.reset();
                }
            },

            ////
            // parameters
            ////

            OpCode::GetParameterName => {
                let param = param_for_idx!(index);
                cstrcpy(ptr, param.get_name(), MAX_PARAM_STR_LEN);
                return 0;
            },

            OpCode::GetParameterLabel => {
                let param = param_for_idx!(index);
                cstrcpy(ptr, param.get_label(), MAX_PARAM_STR_LEN);
                return 0;
            },

            OpCode::GetParameterDisplay => {
                let param = param_for_idx!(index);
                let dest = cstr_as_slice(ptr, MAX_PARAM_STR_LEN);
                let mut cursor = io::Cursor::new(
                    &mut dest[..MAX_PARAM_STR_LEN - 1]);

                match param.get_display(&self.wrapped.smoothed_model, &mut cursor) {
                    Ok(_) => {
                        let len = cursor.position();
                        dest[len as usize] = 0;
                        return len as isize;
                    },

                    Err(_) => {
                        dest[0] = 0;
                        return 0;
                    }
                }
            },

            OpCode::CanBeAutomated => return 1,

            ////
            // plugin metadata
            ////

            OpCode::GetEffectName => {
                cstrcpy(ptr, T::NAME, MAX_EFFECT_NAME_LEN);
                return 1;
            },

            OpCode::GetProductName => {
                cstrcpy(ptr, T::PRODUCT, MAX_PRODUCT_STR_LEN);
                return 1;
            },

            OpCode::GetVendorName => {
                cstrcpy(ptr, T::VENDOR, MAX_VENDOR_STR_LEN);
                return 1;
            },

            ////
            // events
            ////

            OpCode::ProcessEvents => unsafe {
                let vst_events = &*(ptr as *const Events);
                let ev_slice = slice::from_raw_parts(
                    &vst_events.events[0],
                    vst_events.num_events as usize
                );

                for ev in ev_slice {
                    if let EventType::Midi = (**ev).event_type {
                        let ev = *ev as *const vst::api::MidiEvent;
                        self.wrapped.midi_input(
                            (*ev).delta_frames as usize,
                            (*ev).midi_data
                        );
                    }
                }

                return 0;
            }

            ////
            // state
            ////

            OpCode::GetData => {
                let new_state = match self.wrapped.serialise() {
                    None => return 0,
                    Some(s) => s
                };

                unsafe {
                    *(ptr as *mut *const c_void) =
                        new_state.as_ptr() as *const c_void;
                }

                let len = new_state.len() as isize;
                self.state = Some(new_state);
                return len;
            },

            OpCode::SetData => {
                let state = unsafe {
                    slice::from_raw_parts(ptr as *mut u8, value as usize)
                };

                self.wrapped.deserialise(state);
                return 0;
            },

            ////
            // ~who knows~
            ////

            o => {
                eprintln!("unhandled opcode {:?}", o);
            }
        }

        0
    }

    #[inline]
    fn get_parameter(&self, index: i32) -> f32 {
        let param = match param_for_vst2_id::<T::Model>(index) {
            Some(p) => p,
            None => return 0.0
        };

        self.wrapped.get_parameter(param)
    }

    #[inline]
    fn set_parameter(&mut self, index: i32, val: f32) {
        let param = match param_for_vst2_id::<T::Model>(index) {
            Some(p) => p,
            None => return
        };

        self.wrapped.set_parameter(param, val);
    }

    fn get_musical_time(&mut self) -> MusicalTime {
        let mut mtime = MusicalTime {
            bpm: 0.0,
            beat: 0.0
        };

        let time_info = {
            let flags = TimeInfoFlags::TEMPO_VALID | TimeInfoFlags::PPQ_POS_VALID;

            let vti = (self.host_cb)(&mut self.effect,
                host::OpCode::GetTime as i32, 0,
                flags.bits() as isize,
                ptr::null_mut(), 0.0);

            match vti {
                0 => return mtime,
                ptr => unsafe { *(ptr as *const TimeInfo) }
            }
        };

        let flags = TimeInfoFlags::from_bits_truncate(time_info.flags);

        if flags.contains(TimeInfoFlags::TEMPO_VALID) {
            mtime.bpm = time_info.tempo;
        }

        if flags.contains(TimeInfoFlags::PPQ_POS_VALID) {
            mtime.beat = time_info.ppq_pos;
        }

        mtime
    }

    #[inline]
    fn process_replacing(&mut self,
        in_buffers: *const *const f32,
        out_buffers: *mut *mut f32,
        nframes: i32)
    {
        let input = unsafe {
            let b = slice::from_raw_parts(in_buffers, 2);

            [slice::from_raw_parts(b[0], nframes as usize),
             slice::from_raw_parts(b[1], nframes as usize)]
        };

        let output = unsafe {
            let b = slice::from_raw_parts(out_buffers, 2);

            [slice::from_raw_parts_mut(b[0], nframes as usize),
             slice::from_raw_parts_mut(b[1], nframes as usize)]
        };

        let musical_time = self.get_musical_time();
        self.wrapped.process(musical_time, input, output, nframes as usize);
    }
}

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

pub fn vst_plugin_main<T: Plugin>(host_cb: HostCallbackProc,
        unique_id: &[u8; 4]) -> *mut AEffect {
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
            $crate::api::vst2::vst_plugin_main::<$plugin>(
                host_callback, $unique_id)
        }
    }
}
