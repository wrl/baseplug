#![allow(incomplete_features)]
#![feature(generic_associated_types)]
#![feature(specialization)]

#[macro_use]
pub mod util;

#[macro_use]
pub mod api;

mod atomic_float;
pub use atomic_float::AtomicFloat;

mod smooth;
pub use smooth::{
    Smooth,
    SmoothOutput,
    SmoothStatus,
};

mod declick;
pub use declick::{
    DeclickParam,
    DeclickOutput
};

pub mod event;
pub use event::Event;

mod message;
pub use message::*;

mod model;
pub use model::*;

pub mod parameter;
pub use parameter::{Param, ParamInfo};

mod plugin;
pub use plugin::*;

mod time;
pub use time::*;

mod ui_param;
pub use ui_param::{UIFloatParam, UIFloatValue};

mod wrapper;

pub use baseplug_derive::model;


const MAX_BLOCKSIZE: usize = 128;