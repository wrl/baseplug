use std::ffi::CStr;
use std::os::raw::c_void;
use std::ptr;
use std::{io, os::raw::c_char};
use std::{mem, slice};

pub use vst2_sys;
use vst2_sys::*;

use crate::wrapper::*;
use crate::*;

mod ui;
use ui::*;

mod abi;
pub use abi::plugin_main;

const MAX_PARAM_STR_LEN: usize = 32;
const MAX_EFFECT_NAME_LEN: usize = 32;
const MAX_VENDOR_STR_LEN: usize = 64;
const MAX_PRODUCT_STR_LEN: usize = 64;

const TRANSPORT_PLAYING: i32 = 2;

// output events buffer size
const OUTPUT_BUFFER_SIZE: usize = 256;

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
fn param_for_vst2_id<P, M>(id: i32) -> Option<&'static Param<P, M::Smooth>>
    where
        P: Plugin,
        M: Model<P>,
{
    M::Smooth::PARAMS.get(id as usize).copied()
}

macro_rules! param_for_idx {
    ($id:ident) => {
        match param_for_vst2_id::<P, P::Model>($id) {
            Some(p) => p,
            None => return 0,
        }
    }
}

// represents an output buffer to send events to host
#[repr(C)]
pub struct OutgoingEvents {
    num_events: i32,
    _reserved: isize,
    event_ptrs: [*mut MidiEvent; OUTPUT_BUFFER_SIZE],
    events: [MidiEvent; OUTPUT_BUFFER_SIZE],
}

impl OutgoingEvents {
    pub fn new() -> Self {
        // create placeholders, ownership stays here
        let blnk_evts = [vst2_sys::MidiEvent {
            event_type: MIDI_TYPE,
            byte_size: std::mem::size_of::<MidiEvent>() as i32,
            delta_frames: 0,
            flags: 0,
            ..unsafe { std::mem::zeroed() }
        }; OUTPUT_BUFFER_SIZE];

        // init ptrs to null
        let evts_ptrs: [*mut MidiEvent; OUTPUT_BUFFER_SIZE] = [ptr::null_mut(); OUTPUT_BUFFER_SIZE];

        OutgoingEvents {
            num_events: 0,
            _reserved: 0,
            events: blnk_evts,
            event_ptrs: evts_ptrs,
        }
    }
}

struct VST2Adapter<P: Plugin> {
    effect: AEffect,
    host_cb: HostCallbackProc,
    wrapped: WrappedPlugin<P>,

    editor_rect: Rect,

    // when the VST2 host asks us for the chunk/data/state, the lifetime for that data extends
    // until the *next* time that the host asks us for state. this means we have to just hold this
    // around in memory indefinitely.
    //
    // allow(dead_code) here because we don't read from it after assignment, we only hold onto it
    // here so that the host has access to it. compiler warns about "never read" without the allow.
    #[allow(dead_code)]
    state: Option<Vec<u8>>,

    // output events buffer
    output_events_buffer: OutgoingEvents,
}

impl<P: Plugin> VST2Adapter<P> {
    #[inline]
    fn dispatch(&mut self, opcode: i32, index: i32, value: isize, ptr: *mut c_void, opt: f32) -> isize {
        match opcode {
            ////
            // lifecycle
            ////
            effect_opcodes::CLOSE => {
                unsafe {
                    drop(Box::from_raw(self))
                };
            },

            effect_opcodes::SET_SAMPLE_RATE => self.wrapped.set_sample_rate(opt),

            effect_opcodes::MAINS_CHANGED => {
                if value == 1 {
                    self.wrapped.reset();
                }
            },

            ////
            // parameters
            ////
            effect_opcodes::GET_PARAM_NAME => {
                let param = param_for_idx!(index);
                cstrcpy(ptr, param.get_name(), MAX_PARAM_STR_LEN);
                return 0;
            },

            effect_opcodes::GET_PARAM_LABEL => {
                let param = param_for_idx!(index);
                cstrcpy(ptr, param.get_label(), MAX_PARAM_STR_LEN);
                return 0;
            },

            effect_opcodes::GET_PARAM_DISPLAY => {
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

            effect_opcodes::CAN_BE_AUTOMATED => return 1,

            ////
            // plugin metadata
            ////
            effect_opcodes::GET_EFFECT_NAME => {
                cstrcpy(ptr, P::NAME, MAX_EFFECT_NAME_LEN);
                return 1;
            },

            effect_opcodes::GET_PRODUCT_STRING => {
                cstrcpy(ptr, P::PRODUCT, MAX_PRODUCT_STR_LEN);
                return 1;
            },

            effect_opcodes::GET_VENDOR_STRING => {
                cstrcpy(ptr, P::VENDOR, MAX_VENDOR_STR_LEN);
                return 1;
            },

            ////
            // events
            ////
            effect_opcodes::PROCESS_EVENTS => unsafe {
                let vst_events = &*(ptr as *const Events);
                let ev_slice = slice::from_raw_parts(
                    vst_events.events.as_ptr() as *const *const MidiEvent,
                    vst_events.num_events as usize
                );

                for ev in ev_slice {
                    if (**ev).event_type == MIDI_TYPE {
                        let ev = *ev as *const MidiEvent;
                        self.wrapped.midi_input(
                            (*ev).delta_frames as usize,
                            [(*ev).midi_data[0], (*ev).midi_data[1], (*ev).midi_data[2]]
                        );
                    }
                }

                return 0;
            },

            ////
            // state
            ////
            effect_opcodes::GET_CHUNK => {
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

            effect_opcodes::SET_CHUNK => {
                let state = unsafe {
                    slice::from_raw_parts(ptr as *mut u8, value as usize)
                };

                self.wrapped.deserialise(state);
                return 0;
            },

            ////
            // editor
            ////
            effect_opcodes::EDIT_GET_RECT => {
                let ptr = ptr as *mut *mut c_void;

                let (width, height) = match self.ui_get_rect() {
                    Some((w, h)) => (w, h),
                    None => unsafe {
                        *ptr = ptr::null_mut();
                        return 0;
                    }
                };

                self.editor_rect = Rect {
                    top: 0,
                    left: 0,
                    bottom: height,
                    right: width,
                };

                unsafe {
                    // we never read from editor_rect, just set it.
                    *ptr = (&self.editor_rect as *const _) as *mut c_void;
                    return 1;
                }
            },

            effect_opcodes::EDIT_OPEN => {
                return match self.ui_open(ptr) {
                    Ok(_) => 1,
                    Err(_) => 0,
                };
            },

            effect_opcodes::EDIT_IDLE => {},

            effect_opcodes::EDIT_CLOSE => {
                self.ui_close();
            },

            effect_opcodes::CAN_DO => {
                // get the property
                let can_do = String::from_utf8_lossy(unsafe {
                    CStr::from_ptr(ptr as *mut c_char).to_bytes()
                })
                .into_owned();

                let can_do = match can_do.as_str() {
                    "sendVstEvents" => 1,
                    "sendVstMidiEvent" => 1,
                    "receiveVstTimeInfo" => 1,
                    _otherwise => 0,
                };

                return can_do;
            },

            ////
            // ~who knows~
            ////

            o => {
                eprintln!("unhandled opcode {:?}", o);
            },
        }

        0
    }

    #[inline]
    fn get_parameter(&self, index: i32) -> f32 {
        let param = match param_for_vst2_id::<P, P::Model>(index) {
            Some(p) => p,
            None => return 0.0
        };

        self.wrapped.get_parameter(param)
    }

    #[inline]
    fn set_parameter(&mut self, index: i32, val: f32) {
        let param = match param_for_vst2_id::<P, P::Model>(index) {
            Some(p) => p,
            None => return
        };

        self.wrapped.set_parameter(param, val);
    }

    fn get_musical_time(&mut self) -> MusicalTime {
        let mut mtime = MusicalTime {
            bpm: 0.0,
            beat: 0.0,
            is_playing: false
        };

        let time_info = {
            let flags = time_info_flags::TEMPO_VALID | time_info_flags::PPQ_POS_VALID;

            let vti = (self.host_cb)(&mut self.effect,
                host_opcodes::GET_TIME, 0,
                flags as isize,
                ptr::null_mut(), 0.0);

            match vti {
                0 => return mtime,
                ptr => unsafe { &*(ptr as *const TimeInfo) }
            }
        };

        if (time_info.flags | time_info_flags::TEMPO_VALID) != 0 {
            mtime.bpm = time_info.tempo;
        }

        if (time_info.flags | time_info_flags::PPQ_POS_VALID) != 0 {
            mtime.beat = time_info.ppq_pos;
        }

        if (time_info.flags | TRANSPORT_PLAYING) != 0 {
            mtime.is_playing = true;
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

        // write output_events in the buffer
        self.send_output_events();

        // clear
        self.wrapped.output_events.clear();
    }

    #[inline]
    fn send_output_events(&mut self) {
        self.output_events_buffer.num_events = 0;

        // write into output buffer
        for (bevt, ev) in self
            .wrapped
            .output_events
            .iter()
            .zip(self.output_events_buffer.events.iter_mut())
        {
            match bevt.data {
                event::Data::Midi(midi_data) => {
                    let midi_event: MidiEvent = MidiEvent {
                        event_type: MIDI_TYPE,
                        byte_size: mem::size_of::<MidiEvent>() as i32,
                        delta_frames: bevt.frame as i32,
                        flags: 1,
                        note_length: 0,
                        note_offset: 0,
                        midi_data: [midi_data[0], midi_data[1], midi_data[2], 0],
                        detune: 0,
                        note_off_velocity: 0,
                        reserved_1: 0,
                        reserved_2: 0,
                    };
                    *ev = midi_event;

                    self.output_events_buffer.num_events += 1;
                }

                _ => {}
            }
        }

        if self.output_events_buffer.num_events > 0 {
            // update pointers
            for (evt, evt_ptr) in self
                .output_events_buffer
                .events
                .iter_mut()
                .zip(self.output_events_buffer.event_ptrs.iter_mut())
            {
                *evt_ptr = evt as *mut MidiEvent;
            }

            // send to host
            (self.host_cb)(&mut self.effect as *mut AEffect,
                host_opcodes::PROCESS_EVENTS,
                0, 0, &self.output_events_buffer as *const _ as *mut _, 0.0);
        }
    }
}
