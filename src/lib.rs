#![allow(incomplete_features)]
#![feature(generic_associated_types)]
#![feature(min_specialization)]
#![feature(rustc_attrs)]

use serde::{
    Serialize,
    de::DeserializeOwned
};

use raw_window_handle::RawWindowHandle;


#[macro_use]
pub mod util;

#[macro_use]
pub mod api;

mod smooth;
pub use smooth::{
    Smooth,
    SmoothOutput,
    SmoothStatus
};

mod declick;
pub use declick::{
    Declick,
    DeclickOutput
};

pub mod event;
pub use event::Event;

mod model;
pub use model::*;

pub mod parameter;
pub use parameter::Param;

mod wrapper;

pub use baseplug_derive::model;


const MAX_BLOCKSIZE: usize = 128;

#[derive(Clone)]
pub struct MusicalTime {
    pub bpm: f64,
    pub beat: f64
}

pub struct AudioBus<'a> {
    pub connected_channels: isize,
    pub buffers: &'a[&'a [f32]]
}

pub struct AudioBusMut<'a, 'b> {
    pub connected_channels: isize,
    pub buffers: &'a mut [&'b mut [f32]]
}

pub struct ProcessContext<'a, 'b> {
    pub nframes: usize,
    pub sample_rate: f32,

    pub inputs: &'a [AudioBus<'a>],
    pub outputs: &'a mut [AudioBusMut<'a, 'b>],

    pub musical_time: MusicalTime
}

pub trait Parameters<T: 'static> {
    const PARAMS: &'static [&'static Param<T>];
}

pub trait Plugin: Send + Sync {
    const NAME: &'static str;
    const PRODUCT: &'static str;
    const VENDOR: &'static str;

    const INPUT_CHANNELS: usize;
    const OUTPUT_CHANNELS: usize;

    type Model: Model + Serialize + DeserializeOwned;

    fn new(sample_rate: f32, model: &Self::Model) -> Self;

    fn process<'proc>(&mut self,
        model: &<<Self::Model as Model>::Smooth
                    as SmoothModel<Self::Model>>::Process<'proc>,
        ctx: &'proc mut ProcessContext);
}

#[rustc_specialization_trait]
pub trait MidiReceiver: Plugin {
    fn midi_input<'proc>(&mut self, model: &<<Self::Model as Model>::Smooth as SmoothModel<Self::Model>>::Process<'proc>,
        data: [u8; 3]);
}

pub type WindowOpenResult = Result<(), ()>;

#[rustc_specialization_trait]
pub trait PluginUI: Plugin {
    fn ui_size(&self) -> (i16, i16);

    fn ui_open(&mut self, parent: RawWindowHandle) -> WindowOpenResult;
    fn ui_close(&mut self);
}
