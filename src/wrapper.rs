use ringbuf::RingBuffer;

use crate::{
    AudioBus, AudioBusMut, Event, MidiReceiver, Model, MusicalTime, Param, Parameters,
    Plugin, PluginUI, PlugToUIMsg, PlugMsgHandles, ProcessContext, SmoothModel,
    UIMsgHandles, UIToPlugMsg, UIHostCallback, event
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
    sample_rate: f32,

    pub(crate) ui_handle: Option<<Self as WrappedPluginUI<P>>::UIHandle>,
    pub(crate) ui_msg_handles: Option<UIMsgHandles<P>>,
}

impl<P: Plugin> WrappedPlugin<P> {
    #[inline]
    pub(crate) fn new() -> Self {
        Self {
            plug: P::new(48000.0, &P::Model::default()),
            events: Vec::with_capacity(512),
            output_events: Vec::with_capacity(256),
            smoothed_model: <P::Model as Model<P>>::Smooth::from_model(P::Model::default()),
            sample_rate: 0.0,

            ui_handle: None,
            ui_msg_handles: None,
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

        if let Some(ui_msg_handles) = &mut self.ui_msg_handles {
            if let Err(_) = ui_msg_handles.plug_to_ui_tx.push(PlugToUIMsg::ProgramChanged(Box::new(model))) {
                eprintln!("Plug to UI message buffer is full!");
            }
        }
    }

    ////
    // parameters
    ////

    #[inline]
    pub(crate) fn get_parameter(&self, param: &Param<P, <P::Model as Model<P>>::Smooth, <P::Model as Model<P>>::UI>) -> f32 {
        param.get(&self.smoothed_model)
    }

    #[inline]
    pub(crate) fn set_parameter(&mut self, param: &'static Param<P, <P::Model as Model<P>>::Smooth, <P::Model as Model<P>>::UI>, val: f32) {
        if param.dsp_notify.is_some() {
            self.enqueue_event(Event {
                frame: 0,
                data: event::Data::Parameter {
                    param,
                    val,
                    notify_ui: true,  // Notify the UI that the host changed this value.
                }
            });
        } else {
            param.set(&mut self.smoothed_model, val);

            self.notify_ui_of_param_change(param, val);
        }
    }

    fn set_parameter_from_event(&mut self, param: &'static Param<P, <P::Model as Model<P>>::Smooth, <P::Model as Model<P>>::UI>, val: f32, notify_ui: bool) {
        param.set(&mut self.smoothed_model, val);

        if let Some(dsp_notify) = param.dsp_notify {
            dsp_notify(&mut self.plug);
        }
        
        // Do not notify UI if this parameter change originated from the UI itself.
        if notify_ui {
            self.notify_ui_of_param_change(param, val);
        }
    }

    fn notify_ui_of_param_change(&mut self, param: &'static Param<P, <P::Model as Model<P>>::Smooth, <P::Model as Model<P>>::UI>, val: f32) {
        if let Some(ui_msg_handles) = &mut self.ui_msg_handles {
            if let Err(_) = ui_msg_handles.plug_to_ui_tx.push(PlugToUIMsg::ParamChanged {
                param_idx: param.info.idx,
                normalized: val,
            }) {
                eprintln!("Plug to UI message buffer is full!");
            }
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

    pub(crate) fn as_ui_model(&mut self, ui_host_callback: Box<dyn UIHostCallback>, notify_dsp: bool) -> <P::Model as Model<P>>::UI {
        use crate::UIModel;

        // TODO: Set capacity based on number of parameters.
        let (plug_to_ui_tx, plug_to_ui_rx) = RingBuffer::<PlugToUIMsg<P::Model>>::new(512).split();
        let (ui_to_plug_tx, ui_to_plug_rx) = RingBuffer::<UIToPlugMsg<<P::Model as Model<P>>::Smooth>>::new(512).split();

        self.ui_msg_handles = Some(UIMsgHandles {
            plug_to_ui_tx,
            ui_to_plug_rx,
        });

        let model = self.smoothed_model.as_model();

        let plug_msg_handles = PlugMsgHandles::new(
            ui_host_callback,
            plug_to_ui_rx,
            ui_to_plug_tx,
            notify_dsp,
        );

        <<P::Model as Model<P>>::UI as UIModel<P, P::Model>>::from_model(
            model,
            plug_msg_handles,
        )
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

    #[inline]
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
            Data::Parameter { param, val, notify_ui } => {
                self.set_parameter_from_event(param, val, notify_ui);
            }
        }
    }

    #[inline]
    pub(crate) fn process(&mut self, mut musical_time: MusicalTime,
        input: [&[f32]; 2], mut output: [&mut [f32]; 2],
        mut nframes: usize)
    {
        let mut start = 0;
        let mut ev_idx = 0;

        let mut ui_closed = false;
        if let Some(mut ui_msg_handles) = self.ui_msg_handles.take() {
            while let Some(msg) = ui_msg_handles.ui_to_plug_rx.pop() {
                match msg {
                    // The UI Model will only send parameter update messages if the plugin API
                    // requested it.
                    UIToPlugMsg::ParamChanged { param_idx, normalized } => {
                        // What a monstrosity this is.
                        let param = &<<P::Model as Model<P>>::Smooth as Parameters<P, <P::Model as Model<P>>::Smooth, <P::Model as Model<P>>::UI>>::PARAMS[param_idx];

                        self.enqueue_event(Event { frame: 0, data: event::Data::Parameter {
                            param,
                            val: normalized,
                            notify_ui: false, // Don't notify the UI since it was the one that changed it.
                        } });
                    }
                    // We still need to update all non-parameter values from the UI.
                    UIToPlugMsg::ValueChanged { cb, value } => {
                        // This actually works!
                        (cb)(&mut self.smoothed_model, value);
                    }
                    // Sent when the UI Model is dropped due to the user manually closing theplugin window.
                    UIToPlugMsg::Closed => {
                        ui_closed = true;
                    }
                }
            }
            // Get around borrow checker.
            self.ui_msg_handles = Some(ui_msg_handles);
        }
        if ui_closed {
            self.ui_msg_handles = None;
            self.ui_handle = None;
        }

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

            // this scope is here so that we drop ProcessContext right after we're done with it.
            // since `enqueue_event()` holds a reference to `start`, we need to have that reference
            // released when we update `start` at the bottom of the loop iteration.
            {
                let output_events = &mut self.output_events;

                let mut context = ProcessContext {
                    nframes: block_frames,
                    sample_rate: self.sample_rate,

                    inputs: &[in_bus],
                    outputs: &mut [out_bus],

                    enqueue_event: &mut |mut ev| {
                        ev.frame += start;
                        Self::enqueue_event_in(ev, output_events);
                    },

                    musical_time: &musical_time
                };

                let proc_model = self.smoothed_model.process(block_frames);
                self.plug.process(&proc_model, &mut context);
            }

            nframes -= block_frames;
            start += block_frames;

            musical_time.step_by_samples(self.sample_rate.into(), block_frames);
        }

        self.events.clear();
    }
}

/////
// midi input
/////

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

/////
// UI
/////

pub(crate) trait WrappedPluginUI<P: Plugin> {
    type UIHandle;
}

impl<P: Plugin> WrappedPluginUI<P> for WrappedPlugin<P> {
    default type UIHandle = ();
}

impl<P: PluginUI> WrappedPluginUI<P> for WrappedPlugin<P> {
    type UIHandle = P::Handle;
}
