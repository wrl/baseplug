use std::ffi::CStr;
use std::os::raw::c_void;
use std::ptr;
use std::{io, os::raw::c_char};
use std::{mem, slice};

use vst::api::consts::*;
use vst::api::Event;
use vst::api::*;
use vst::editor::Rect;
use vst::host;
use vst::plugin::OpCode;

use crate::wrapper::*;
use crate::*;

mod ui;
use ui::*;

mod abi;
pub use abi::plugin_main;

// vst-rs doesn't have this for some reason
const MAX_EFFECT_NAME_LEN: usize = 32;

// output events buffer size
const OUTPUT_BUFFER_SIZE: usize = 256;

#[inline]
fn cstr_as_slice<'a>(ptr: *mut c_void, len: usize) -> &'a mut [u8] {
    unsafe { slice::from_raw_parts_mut(ptr as *mut u8, len) }
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
    };
}

// represents an output buffer to send events to host
#[repr(C)]
pub struct OutgoingEvents {
    num_events: i32,
    _reserved: isize,
    event_ptrs: [*mut Event; OUTPUT_BUFFER_SIZE],
    events: [Event; OUTPUT_BUFFER_SIZE],
}

impl OutgoingEvents {
    pub fn new() -> Self {
        // create placeholders, ownership stays here
        let blnk_evts = [Event {
            event_type: EventType::Midi,
            byte_size: std::mem::size_of::<MidiEvent>() as i32,
            delta_frames: 0,
            _flags: 0,
            _reserved: [0; 16],
        }; OUTPUT_BUFFER_SIZE];

        // init ptrs to null
        let evts_ptrs: [*mut Event; OUTPUT_BUFFER_SIZE] = [ptr::null_mut(); OUTPUT_BUFFER_SIZE];

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

    ui_handle: Option<<Self as VST2UI>::UIHandle>,

    // when the VST2 host asks us for the chunk/data/state, the lifetime for that data extends
    // until the *next* time that the host asks us for state. this means we have to just hold this
    // around in memory indefinitely.
    state: Option<Vec<u8>>,

    // output events buffer
    output_events_buffer: OutgoingEvents,
}

impl<P: Plugin> VST2Adapter<P> {
    #[inline]
    fn dispatch(
        &mut self,
        opcode: i32,
        index: i32,
        value: isize,
        ptr: *mut c_void,
        opt: f32,
    ) -> isize {
        match OpCode::from(opcode) {
            ////
            // lifecycle
            ////
            OpCode::GetApiVersion => return 2400,
            OpCode::Shutdown => {
                unsafe { drop(Box::from_raw(self)) };
            }

            OpCode::SetSampleRate => self.wrapped.set_sample_rate(opt),

            OpCode::StateChanged => {
                if value == 1 {
                    self.wrapped.reset();
                }
            }

            ////
            // parameters
            ////
            OpCode::GetParameterName => {
                let param = param_for_idx!(index);
                cstrcpy(ptr, param.get_name(), MAX_PARAM_STR_LEN);
                return 0;
            }

            OpCode::GetParameterLabel => {
                let param = param_for_idx!(index);
                cstrcpy(ptr, param.get_label(), MAX_PARAM_STR_LEN);
                return 0;
            }

            OpCode::GetParameterDisplay => {
                let param = param_for_idx!(index);
                let dest = cstr_as_slice(ptr, MAX_PARAM_STR_LEN);
                let mut cursor = io::Cursor::new(&mut dest[..MAX_PARAM_STR_LEN - 1]);

                match param.get_display(&self.wrapped.smoothed_model, &mut cursor) {
                    Ok(_) => {
                        let len = cursor.position();
                        dest[len as usize] = 0;
                        return len as isize;
                    }

                    Err(_) => {
                        dest[0] = 0;
                        return 0;
                    }
                }
            }

            OpCode::CanBeAutomated => return 1,

            ////
            // plugin metadata
            ////
            OpCode::GetEffectName => {
                cstrcpy(ptr, P::NAME, MAX_EFFECT_NAME_LEN);
                return 1;
            }

            OpCode::GetProductName => {
                cstrcpy(ptr, P::PRODUCT, MAX_PRODUCT_STR_LEN);
                return 1;
            }

            OpCode::GetVendorName => {
                cstrcpy(ptr, P::VENDOR, MAX_VENDOR_STR_LEN);
                return 1;
            }

            ////
            // events
            ////
            OpCode::GetCurrentPresetName => {
                return 0;
            }

            ////
            // events
            ////
            OpCode::ProcessEvents => unsafe {
                let vst_events = &*(ptr as *const Events);
                let ev_slice =
                    slice::from_raw_parts(&vst_events.events[0], vst_events.num_events as usize);

                for ev in ev_slice {
                    if let EventType::Midi = (**ev).event_type {
                        let ev = *ev as *const vst::api::MidiEvent;
                        self.wrapped
                            .midi_input((*ev).delta_frames as usize, (*ev).midi_data);
                    }
                }

                return 0;
            },

            ////
            // state
            ////
            OpCode::GetData => {
                let new_state = match self.wrapped.serialise() {
                    None => return 0,
                    Some(s) => s,
                };

                unsafe {
                    *(ptr as *mut *const c_void) = new_state.as_ptr() as *const c_void;
                }

                let len = new_state.len() as isize;
                self.state = Some(new_state);
                return len;
            }

            OpCode::SetData => {
                let state = unsafe { slice::from_raw_parts(ptr as *mut u8, value as usize) };

                self.wrapped.deserialise(state);
                return 0;
            }

            ////
            // editor
            ////
            OpCode::EditorGetRect => {
                let ptr = ptr as *mut *mut c_void;

                let (width, height) = match self.ui_get_rect() {
                    Some((w, h)) => (w, h),
                    None => unsafe {
                        *ptr = ptr::null_mut();
                        return 0;
                    },
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
            }

            OpCode::EditorOpen => {
                return match self.ui_open(ptr) {
                    Ok(_) => 1,
                    Err(_) => 0,
                };
            }

            OpCode::EditorClose => {
                self.ui_close();
            }

            OpCode::CanDo => {
                // get the property
                let can_do = String::from_utf8_lossy(unsafe {
                    CStr::from_ptr(ptr as *mut c_char).to_bytes()
                })
                .into_owned();

                let can_do = match can_do.as_str() {
                    "sendVstEvents" => Supported::Yes,
                    "sendVstMidiEvent" => Supported::Yes,
                    // "receiveVstEvents" => Supported::Maybe,
                    // "receiveVstMidiEvent" => Supported::Maybe,
                    "receiveVstTimeInfo" => Supported::Yes,
                    // "offline" => Offline,
                    // "midiProgramNames" => MidiProgramNames,
                    // "bypass" => Bypass,

                    // "receiveVstSysexEvent" => ReceiveSysExEvent,
                    // "midiSingleNoteTuningChange" => MidiSingleNoteTuningChange,
                    // "midiKeyBasedInstrumentControl" => MidiKeyBasedInstrumentControl,
                    otherwise => Supported::Maybe,
                };

                return can_do.into();
            }

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
        let param = match param_for_vst2_id::<P, P::Model>(index) {
            Some(p) => p,
            None => return 0.0,
        };

        self.wrapped.get_parameter(param)
    }

    #[inline]
    fn set_parameter(&mut self, index: i32, val: f32) {
        let param = match param_for_vst2_id::<P, P::Model>(index) {
            Some(p) => p,
            None => return,
        };

        self.wrapped.set_parameter(param, val);
    }

    fn get_musical_time(&mut self) -> MusicalTime {
        let mut mtime = MusicalTime {
            bpm: 0.0,
            beat: 0.0,
            is_playing: false,
        };

        let time_info = {
            let flags = TimeInfoFlags::TEMPO_VALID | TimeInfoFlags::PPQ_POS_VALID;

            let vti = (self.host_cb)(
                &mut self.effect,
                host::OpCode::GetTime as i32,
                0,
                flags.bits() as isize,
                ptr::null_mut(),
                0.0,
            );

            match vti {
                0 => return mtime,
                ptr => unsafe { *(ptr as *const TimeInfo) },
            }
        };

        let flags = TimeInfoFlags::from_bits_truncate(time_info.flags);

        if flags.contains(TimeInfoFlags::TEMPO_VALID) {
            mtime.bpm = time_info.tempo;
        }

        if flags.contains(TimeInfoFlags::PPQ_POS_VALID) {
            mtime.beat = time_info.ppq_pos;
        }

        if flags.contains(TimeInfoFlags::TRANSPORT_PLAYING) {
            mtime.is_playing = true;
        }

        mtime
    }

    #[inline]
    fn process_replacing(
        &mut self,
        in_buffers: *const *const f32,
        out_buffers: *mut *mut f32,
        nframes: i32,
    ) {
        let input = unsafe {
            let b = slice::from_raw_parts(in_buffers, 2);

            [
                slice::from_raw_parts(b[0], nframes as usize),
                slice::from_raw_parts(b[1], nframes as usize),
            ]
        };

        let output = unsafe {
            let b = slice::from_raw_parts(out_buffers, 2);

            [
                slice::from_raw_parts_mut(b[0], nframes as usize),
                slice::from_raw_parts_mut(b[1], nframes as usize),
            ]
        };

        let musical_time = self.get_musical_time();
        self.wrapped
            .process(musical_time, input, output, nframes as usize);

        // write output_events in the buffer
        self.send_output_events();

        // clear
        self.wrapped.output_events.clear();
    }

    #[inline]
    fn send_output_events(&mut self) {
        // init
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
                        event_type: EventType::Midi,
                        byte_size: mem::size_of::<MidiEvent>() as i32,
                        delta_frames: bevt.frame as i32,
                        flags: MidiEventFlags::REALTIME_EVENT.bits(),
                        note_length: 0,
                        note_offset: 0,
                        midi_data: [midi_data[0], midi_data[1], midi_data[2]],
                        _midi_reserved: 0,
                        detune: 0,
                        note_off_velocity: 0,
                        _reserved1: 0,
                        _reserved2: 0,
                    };
                    *ev = unsafe { std::mem::transmute(midi_event) };

                    self.output_events_buffer.num_events += 1;
                }
                event::Data::Parameter { param, val } => {
                    // not yet supported
                }
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
                *evt_ptr = evt as *mut Event;
            }

            // send to host
            let callback = self.host_cb;
            let res = callback(
                &mut self.effect as *mut AEffect,
                host::OpCode::ProcessEvents.into(),
                0,
                0,
                &self.output_events_buffer as *const _ as *mut _,
                0.0,
            );
        }
    }
}
