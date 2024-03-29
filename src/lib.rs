#![allow(incomplete_features)]
#![feature(specialization)]

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

mod plugin;
pub use plugin::*;

mod time;
pub use time::*;

mod wrapper;

pub use baseplug_derive::model;


pub const MAX_BLOCKSIZE: usize = 128;
