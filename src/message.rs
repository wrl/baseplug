use std::cell::UnsafeCell;
use ringbuf::{Consumer, Producer};

use crate::{Plugin, Model};

pub trait UIHostCallback: Send + Sync {
    fn send_parameter_update(&self, param_idx: usize, normalized: f32);

    // Called when the UI Model received the `ShouldClose` message.
    fn close_msg_received(&self);
}

pub enum PlugToUIMsg<Model: 'static> {
    ParamChanged {
        // Sending parameter by index because I cannot for the life of me figure out how to make
        // the rust compiler happy with all the generics.
        param_idx: usize,
        normalized: f32,
    },
    ProgramChanged(Box<Model>),
    ShouldClose,
}

pub enum UIToPlugMsg<SmoothModel: 'static> {
    ParamChanged {
        // Sending parameter by index because I cannot for the life of me figure out how to make
        // the rust compiler happy with all the generics.
        param_idx: usize,
        normalized: f32,
    },
    ValueChanged {
        // Holy crap this actually works.
        cb: &'static fn(&mut SmoothModel, f32),
        value: f32,
    },
    Closed
}

pub(crate) struct UIMsgHandles<P: Plugin> {
    pub plug_to_ui_tx: Producer<PlugToUIMsg<P::Model>>,
    pub ui_to_plug_rx: Consumer<UIToPlugMsg<<P::Model as Model<P>>::Smooth>>,
}

pub struct PlugMsgHandles<Model: 'static, SmoothModel: 'static> {
    pub ui_host_cb: Box<dyn UIHostCallback>,
    pub notify_dsp: bool,

    plug_to_ui_rx: UnsafeCell<Consumer<PlugToUIMsg<Model>>>,
    ui_to_plug_tx: UnsafeCell<Producer<UIToPlugMsg<SmoothModel>>>,
}

impl<Model: 'static, SmoothModel: 'static> PlugMsgHandles<Model, SmoothModel> {
    pub fn new(
        ui_host_cb: Box<dyn UIHostCallback>,
        plug_to_ui_rx: Consumer<PlugToUIMsg<Model>>,
        ui_to_plug_tx: Producer<UIToPlugMsg<SmoothModel>>,
        notify_dsp: bool,
    ) -> Self {
        Self {
            ui_host_cb,
            notify_dsp,
            plug_to_ui_rx: UnsafeCell::new(plug_to_ui_rx),
            ui_to_plug_tx: UnsafeCell::new(ui_to_plug_tx),
        }
    }

    pub fn pop_msg(&self) -> Option<PlugToUIMsg<Model>> {
        // Safe because this is only place this is borrowed, and this is just a message queue.
        unsafe { (&mut *self.plug_to_ui_rx.get()).pop() }
    }

    pub fn push_msg(&self, msg: UIToPlugMsg<SmoothModel>) -> Result<(), UIToPlugMsg<SmoothModel>> {
        // Safe because this is only place this is borrowed, and this is just a message queue.
        unsafe { (&mut *self.ui_to_plug_tx.get()).push(msg) }
    }
}

impl<Model: 'static, SmoothModel: 'static> Drop for PlugMsgHandles<Model, SmoothModel> {
    fn drop(&mut self) {
        if let Err(_) = self.push_msg(UIToPlugMsg::Closed) {
            eprintln!("UI to Plug message buffer is full!");
        }
    }
}