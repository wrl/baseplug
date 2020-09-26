use std::fmt;

use crate::{
    Plugin,
    Model,
    Param
};

pub enum Data<T: Plugin> {
    Midi([u8; 3]),

    Parameter {
        param: &'static Param<<T::Model as Model<T>>::Smooth>,
        val: f32
    }
}

pub struct Event<T: Plugin> {
    pub frame: usize,
    pub data: Data<T>
}

////
// debug impls
////

impl<T: Plugin> fmt::Debug for Data<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Data::Midi(m) =>
                f.debug_tuple("Data::Midi")
                    .field(&m)
                    .finish(),

            Data::Parameter { param, val } =>
                f.debug_struct("Data::Parameter")
                    .field("param", &param)
                    .field("val", &val)
                    .finish()
        }
    }
}

impl<T: Plugin> fmt::Debug for Event<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Event")
            .field("frame", &self.frame)
            .field("data", &self.data)
            .finish()
    }
}
