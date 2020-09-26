use crate::*;


pub trait Model<P: Plugin>: Sized + Default {
    type Smooth:
        SmoothModel<P, Self>
        + Parameters<Self::Smooth>
        + 'static;
}

pub trait SmoothModel<P: Plugin, T: Model<P>>: Sized {
    type Process<'proc>;

    fn from_model(model: T) -> Self;
    fn as_model(&self) -> T;

    fn set_sample_rate(&mut self, sample_rate: f32);

    // set values from model with smoothing
    fn set(&mut self, from: &T);

    // set values from model without smoothing
    fn reset(&mut self, from: &T);

    fn current_value(&'_ mut self) -> Self::Process<'_>;
    fn process(&'_ mut self, nframes: usize) -> Self::Process<'_>;
}
