use crate::{
    Model,
    SmoothModel,

    Plugin,
    Param,
    parameter::Translatable,
    AudioBus,
    AudioBusMut,
    ProcessContext,
    MusicalTime,

    Event,
    event
};

pub struct WrappedPlugin<T: Plugin> {
    plug: T,

    // even though it is *strongly forbidden* to allocate in the RT audio thread, many plugin APIs
    // have no facilities for host-side allocation of event buffers which live through the
    // subsequent `process()` call.
    //
    // the best we can do is pre-allocate a reasonably large buffer and hope we never have to
    // enlarge it.
    //
    // see below in WrappedPlugin::new() for the capacity.
    events: Vec<Event<T>>,

    pub smoothed_model: <T::Model as Model>::Smooth,
    sample_rate: f32
}

impl<T: Plugin> WrappedPlugin<T> {
    #[inline]
    pub fn new() -> Self {
        Self {
            plug: T::new(48000.0, &T::Model::default()),
            events: Vec::with_capacity(512),
            smoothed_model:
                <T::Model as Model>::Smooth::from_model(T::Model::default()),
            sample_rate: 0.0
        }
    }

    ////
    // lifecycle
    ////

    #[inline]
    pub fn set_sample_rate(&mut self, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.smoothed_model.set_sample_rate(sample_rate);

        self.reset();
    }

    #[inline]
    pub fn reset(&mut self) {
        let model = self.smoothed_model.as_model();
        self.plug = T::new(self.sample_rate, &model);
        self.smoothed_model.reset(&model);
    }

    ////
    // parameters
    ////

    #[inline]
    pub fn set_parameter(&mut self, param: &Param<<T::Model as Model>::Smooth>, val: f32) {
        param.set(&mut self.smoothed_model, val)
    }

    #[inline]
    pub fn get_parameter(&self, param: &Param<<T::Model as Model>::Smooth>) -> f32 {
        param.get(&self.smoothed_model)
    }

    ////
    // state
    ////

    pub fn serialise(&self) -> Option<Vec<u8>>
    {
        let ser = self.smoothed_model.as_model();

        serde_json::to_string(&ser)
            .map(|s| s.into_bytes())
            .ok()
    }

    pub fn deserialise<'de>(&mut self, data: &'de [u8]) {
        let m: T::Model = match serde_json::from_slice(data) {
            Ok(m) => m,
            Err(_) => return
        };

        self.smoothed_model.set(&m);
    }

    ////
    // events
    ////

    pub fn enqueue_event(&mut self, ev: Event<T>) {
        let latest_frame = match self.events.last() {
            Some(ev) => ev.frame,
            None => 0
        };

        if latest_frame <= ev.frame {
            self.events.push(ev);
            return;
        }

        let idx = self.events.iter()
            .position(|e| e.frame > ev.frame)
            .unwrap();

        self.events.insert(idx, ev);
    }

    ////
    // process
    ////

    #[inline]
    fn dispatch_event(&mut self, ev_idx: usize) {
        let ev = &self.events[ev_idx];

        use event::Data;

        match ev.data {
            Data::Midi(m) => self.plug.midi_input(m),
            Data::Parameter { param, val } => {
                self.set_parameter(param, val);
            }
        }
    }

    #[inline]
    pub fn normalize(&self, param: &Param<<T::Model as Model>::Smooth>, val: f32) -> f32 {
        val.xlate_out(param)
    }

    #[inline]
    pub fn denormalize(&self, param: &Param<<T::Model as Model>::Smooth>, val: f32) -> f32 {
        f32::xlate_in(param, val)
    }

    #[inline]
    pub fn process(&mut self, musical_time: MusicalTime,
        input: [&[f32]; 2], mut output: [&mut [f32]; 2],
        mut nframes: usize)
    {
        let mut start = 0;
        let mut ev_idx = 0;

        while nframes > 0 {
            let mut block_frames = nframes;

            if ev_idx < self.events.len() {
                while start == self.events[ev_idx].frame {
                    self.dispatch_event(ev_idx);
                    ev_idx += 1;
                }

                if ev_idx < self.events.len() {
                    block_frames = block_frames.min(
                        self.events[ev_idx].frame - start);
                }
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

            let mut context = ProcessContext {
                nframes: block_frames,
                sample_rate: self.sample_rate,

                inputs: &[in_bus],
                outputs: &mut [out_bus],

                // FIXME: should we advance the musical time when we do block subdivisions?
                //        we have all of the data necessary to do so.
                musical_time: musical_time.clone()
            };

            let proc_model = self.smoothed_model.process(block_frames);
            self.plug.process(&proc_model, &mut context);

            nframes -= block_frames;
            start += block_frames;
        }
    }

    pub fn midi_input(&mut self, frame: usize, data: [u8; 3]) {
        if !T::MIDI_INPUT {
            return
        }

        self.enqueue_event(Event {
            frame,
            data: event::Data::Midi(data)
        })
    }
}
