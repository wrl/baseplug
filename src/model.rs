use crate::*;
use crate::wrapper::UIHostCallback;
use std::sync::Arc;

pub trait Model<P: Plugin>: Sized + Default + 'static {
    type Smooth:
        SmoothModel<P, Self>
        + Parameters<P, Self::Smooth>;
    
    type UI: UIModel;
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

    fn process(&'_ mut self, nframes: usize, plug: &mut P, poll_from_ui: bool) -> Self::Process<'_>;

    fn as_ui_model(&self, ui_host_callback: Arc<dyn UIHostCallback>) -> T::UI;
}

pub trait UIModel: Sized + 'static {
    fn update(&mut self);
}