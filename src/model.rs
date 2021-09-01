use crate::*;

pub trait Model<P: Plugin>: Sized + Default + 'static {
    type Smooth:
        SmoothModel<P, Self>
        + Parameters<P, Self::Smooth, Self::UI>;
    
    type UI:
        UIModel<P, Self>;
}

pub trait SmoothModel<P: Plugin, T: Model<P>>: Sized + 'static {
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

pub trait UIModel<P: Plugin, T: Model<P>>: Sized + 'static {
    /// Poll for updates from the host. This should be called periodically, typically
    /// at the top of each render frame.
    fn poll_updates(&mut self);

    /// Returns true if the host loaded a new program (preset).
    fn new_program_loaded(&self) -> bool;

    /// Returns true if the host has requested that the UI should close.
    fn close_requested(&self) -> bool;

    fn from_model(
        model: T,
        plug_msg_handles: PlugMsgHandles<T, T::Smooth>,
    ) -> Self;
}