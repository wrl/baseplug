use crate::{
    Model,
    SmoothModel,

    Plugin,
    MidiReceiver,
    Param,

    AudioBus,
    AudioBusMut,
    ProcessContext,
    MusicalTime,

    Event,
    event
};

pub(crate) struct WrappedPlugin<P: Plugin> {
    pub(crate) plug: P,

    // even though it is *strongly forbidden* to allocate in the RT audio thread, many plugin APIs
    // have no facilities for host-side allocation of event buffers which live through the
    // subsequent `process()` call.
    //
    // the best we can do is pre-allocate a reasonably large buffer and hope we never have to
    // enlarge it.
    //
    // see below in WrappedPlugin::new() for the capacity.
    //
    // XXX: there are *potential* threading issues with this. it would be completely possible for
    // an enqueue_event() call to come *during* a process() call, and we need to be able to handle
    // that in the future. we may need to use a different data structure here.
    events: Vec<Event<P>>,
    pub(crate) output_events: Vec<Event<P>>,

    pub(crate) smoothed_model: <P::Model as Model<P>>::Smooth,
    sample_rate: f32
}

impl<P: Plugin> WrappedPlugin<P> {
    #[inline]
    pub(crate) fn new() -> Self {
        Self {
            plug: P::new(48000.0, &P::Model::default()),
            events: Vec::with_capacity(512),
            output_events: Vec::with_capacity(256),
            smoothed_model:
                <P::Model as Model<P>>::Smooth::from_model(P::Model::default()),
            sample_rate: 0.0
        }
    }

    ////
    // lifecycle
    ////

    #[inline]
    pub(crate) fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.smoothed_model.set_sample_rate(sample_rate);

        self.reset();
    }

    #[inline]
    pub(crate) fn reset(&mut self) {
        let model = self.smoothed_model.as_model();
        self.plug = P::new(self.sample_rate, &model);
        self.smoothed_model.reset(&model);
    }

    ////
    // parameters
    ////

    #[inline]
    pub(crate) fn get_parameter(&self, param: &Param<P, <P::Model as Model<P>>::Smooth>) -> f32 {
        param.get(&self.smoothed_model)
    }

    #[inline]
    pub(crate) fn set_parameter(&mut self, param: &'static Param<P, <P::Model as Model<P>>::Smooth>, val: f32) {
        if param.dsp_notify.is_some() {
            self.enqueue_event(Event {
                frame: 0,
                data: event::Data::Parameter {
                    param,
                    val
                }
            });
        } else {
            param.set(&mut self.smoothed_model, val);
        }
    }

    fn set_parameter_from_event(&mut self, param: &Param<P, <P::Model as Model<P>>::Smooth>, val: f32) {
        param.set(&mut self.smoothed_model, val);

        if let Some(dsp_notify) = param.dsp_notify {
            dsp_notify(&mut self.plug);
        }
    }

    ////
    // state
    ////

    pub(crate) fn serialise(&self) -> Option<Vec<u8>>
    {
        let ser = self.smoothed_model.as_model();

        serde_json::to_string(&ser)
            .map(|s| s.into_bytes())
            .ok()
    }

    pub(crate) fn deserialise<'de>(&mut self, data: &'de [u8]) {
        let m: P::Model = match serde_json::from_slice(data) {
            Ok(m) => m,
            Err(_) => return
        };

        self.smoothed_model.set(&m);
    }

    ////
    // events
    ////

    fn enqueue_event_in(ev: Event<P>, buffer: &mut Vec<Event<P>>) {
        let latest_frame = match buffer.last() {
            Some(ev) => ev.frame,
            None => 0
        };

        if latest_frame <= ev.frame {
            buffer.push(ev);
            return;
        }

        let idx = buffer.iter()
            .position(|e| e.frame > ev.frame)
            .unwrap();

        buffer.insert(idx, ev);
    }

    pub(crate) fn enqueue_event(&mut self, ev: Event<P>) {
        Self::enqueue_event_in(ev, &mut self.events);
    }

    ////
    // process
    ////

    #[inline]
    fn dispatch_event(&mut self, ev_idx: usize) {
        let ev = &self.events[ev_idx];

        use event::Data;

        match ev.data {
            Data::Midi(m) => self.dispatch_midi_event(m),
            Data::Parameter { param, val } => {
                self.set_parameter_from_event(param, val);
            }
        }
    }

    #[inline]
    pub(crate) fn process(&mut self, musical_time: MusicalTime,
        input: [&[f32]; 2], mut output: [&mut [f32]; 2],
        mut nframes: usize)
    {
        let mut start = 0;
        let mut ev_idx = 0;

        while nframes > 0 {
            let mut block_frames = nframes;

            while ev_idx < self.events.len() && start == self.events[ev_idx].frame {
                self.dispatch_event(ev_idx);
                ev_idx += 1;
            }

            if ev_idx < self.events.len() {
                block_frames = block_frames.min(self.events[ev_idx].frame - start);
            }

            block_frames = block_frames.min(crate::MAX_BLOCKSIZE);
            let end = start + block_frames;

            let in_bus = AudioBus {
                connected_channels: 2,
                buffers: &[
                    &input[0][start..end],
                    &input[1][start..end]
                ]
            };

            let out_bus = AudioBusMut {
                connected_channels: 2,
                buffers: {
                    let split = output.split_at_mut(1);

                    // "cannot borrow output as mutable more than once"
                    // fuck you borrowck
                    &mut [
                        &mut split.0[0][start..end],
                        &mut split.1[0][start..end]
                    ]
                }
            };

            let output_events = &mut self.output_events;

            let mut context = ProcessContext {
                nframes: block_frames,
                sample_rate: self.sample_rate,

                inputs: &[in_bus],
                outputs: &mut [out_bus],

                enqueue_event: &mut |ev| {
                    Self::enqueue_event_in(ev, output_events);
                },

                // FIXME: should we advance the musical time when we do block subdivisions?
                //        we have all of the data necessary to do so.
                musical_time: musical_time.clone()
            };

            let proc_model = self.smoothed_model.process(block_frames);
            self.plug.process(&proc_model, &mut context);

            nframes -= block_frames;
            start += block_frames;
        }

        self.events.clear();
    }
}

pub(crate) trait WrappedPluginMidiInput {
    fn wants_midi_input() -> bool;

    fn midi_input(&mut self, frame: usize, data: [u8; 3]);
    fn dispatch_midi_event(&mut self, data: [u8; 3]);
}

impl<T: Plugin> WrappedPluginMidiInput for WrappedPlugin<T> {
    default fn wants_midi_input() -> bool {
        false
    }

    default fn midi_input(&mut self, _frame: usize, _data: [u8; 3]) {
        return
    }

    default fn dispatch_midi_event(&mut self, _data: [u8; 3]) {
        return
    }
}

impl<T: MidiReceiver> WrappedPluginMidiInput for WrappedPlugin<T> {
    fn wants_midi_input() -> bool {
        true
    }

    fn midi_input(&mut self, frame: usize, data: [u8; 3]) {
        self.enqueue_event(Event {
            frame,
            data: event::Data::Midi(data)
        })
    }

    fn dispatch_midi_event(&mut self, data: [u8; 3]) {
        let model = self.smoothed_model.current_value();
        self.plug.midi_input(&model, data)
    }
}
