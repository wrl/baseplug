#![allow(incomplete_features)]
#![feature(generic_associated_types)]

use serde::{Serialize, de::DeserializeOwned};


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

pub mod parameter;
pub use parameter::Param;

pub mod event;
pub use event::Event;

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

pub trait Model: Sized + Default {
    type Smooth:
        SmoothModel<Self>
        + Parameters<Self::Smooth>
        + 'static;
}

pub trait SmoothModel<T: Model + Default>: Sized {
    type Process<'proc>;

    fn from_model(model: T) -> Self;
    fn as_model(&self) -> T;

    fn set_sample_rate(&mut self, sample_rate: f32);

    // set values from model with smoothing
    fn set(&mut self, from: &T);

    // set values from model without smoothing
    fn reset(&mut self, from: &T);

    fn process(&'_ mut self, nframes: usize) -> Self::Process<'_>;
}

pub trait Plugin {
    const NAME: &'static str;
    const PRODUCT: &'static str;
    const VENDOR: &'static str;

    type Model:
        Model
        + Serialize + DeserializeOwned
        + Default
        + 'static;

    fn new(sample_rate: f32, model: &Self::Model) -> Self;

    fn process<'proc>(&mut self,
        model: &<<Self::Model as Model>::Smooth
                    as SmoothModel<Self::Model>>::Process<'proc>,
        ctx: &'proc mut ProcessContext);

    const MIDI_INPUT: bool = false;
    fn midi_input(&mut self, _data: [u8; 3]) {}
}
