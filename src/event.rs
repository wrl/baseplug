use std::fmt;

use crate::{
    Plugin,
    Model,
    Param
};

pub enum Data<P: Plugin> {
    Midi([u8; 3]),

    Parameter {
        param: &'static Param<P, <P::Model as Model<P>>::Smooth, <P::Model as Model<P>>::UI>,
        val: f32,
        notify_ui: bool,
    }
}

pub struct Event<P: Plugin> {
    pub frame: usize,
    pub data: Data<P>
}

////
// debug impls
////

impl<P: Plugin> fmt::Debug for Data<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Data::Midi(m) =>
                f.debug_tuple("Data::Midi")
                    .field(&m)
                    .finish(),

            Data::Parameter { param, val, notify_ui } =>
                f.debug_struct("Data::Parameter")
                    .field("param", &param)
                    .field("value", &val)
                    .field("notify_ui", &notify_ui)
                    .finish()
        }
    }
}

impl<P: Plugin> fmt::Debug for Event<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Event")
            .field("frame", &self.frame)
            .field("data", &self.data)
            .finish()
    }
}
