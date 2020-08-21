use std::fmt;
use std::io;

use crate::util::*;

#[derive(Debug)]
pub enum Gradient {
    Linear,
    Power(f32),
    Exponential
}

#[derive(Debug)]
pub enum Type {
    Numeric {
        min: f32,
        max: f32,

        gradient: Gradient
    },

    // eventually will have an Enum/Discrete type here
}

#[derive(Debug)]
pub enum Unit {
    Generic,
    Decibels
}

pub struct Format<Model> {
    pub display_cb: fn(&Param<Model>, &Model, &mut dyn io::Write) -> io::Result<()>,
    pub label: &'static str
}

pub struct Param<Model> {
    pub name: &'static str,
    pub short_name: Option<&'static str>,

    pub unit: Unit,

    pub param_type: Type,
    pub format: Format<Model>,

    pub set_cb: fn(&Param<Model>, &mut Model, f32),
    pub get_cb: fn(&Param<Model>, &Model) -> f32
}

impl<Model> Param<Model> {
    #[inline]
    pub fn set(&self, model: &mut Model, val: f32) {
        (self.set_cb)(self, model, val)
    }

    #[inline]
    pub fn get(&self, model: &Model) -> f32 {
        (self.get_cb)(self, model)
    }

    #[inline]
    pub fn get_name(&self) -> &'static str {
        self.short_name
            .unwrap_or_else(|| self.name)
    }

    #[inline]
    pub fn get_label(&self) -> &'static str {
        if let Unit::Decibels = self.unit {
            "dB"
        } else {
            self.format.label
        }
    }

    #[inline]
    pub fn get_display(&self, model: &Model, w: &mut dyn io::Write) -> io::Result<()> {
        (self.format.display_cb)(self, model, w)
    }
}

impl<Model> fmt::Debug for Param<Model> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Param")
            .field("name", &self.name)
            .field("short_name", &self.short_name)
            .field("unit", &self.unit)
            .field("param_type", &self.param_type)
            .finish()
    }
}

pub trait Translatable<T, Model> {
    fn xlate_in(param: &Param<Model>, normalised: f32) -> T;
    fn xlate_out(&self, param: &Param<Model>) -> f32;
}

impl<Model> Translatable<f32, Model> for f32 {
    fn xlate_in(param: &Param<Model>, normalised: f32) -> f32 {
        let (min, max, gradient) = match &param.param_type {
            Type::Numeric { min, max, gradient } => (min, max, gradient)
        };

        let normalised = normalised.min(1.0).max(0.0);

        let map = |x: f32| -> f32 {
            let range = max - min;
            let mapped = (x * range) + min;

            match param.unit {
                Unit::Decibels => db_to_coeff(mapped),
                _ => mapped
            }
        };

        match gradient {
            Gradient::Linear => map(normalised),

            Gradient::Power(exponent) =>
                map(normalised.powf(*exponent)),

            Gradient::Exponential => {
                if normalised == 0.0 {
                    return *min;
                }

                if normalised == 1.0 {
                    return *max;
                }

                let minl = min.log2();
                let range = max.log2() - minl;
                2.0f32.powf((normalised * range) + minl)
            }
        }
    }

    fn xlate_out(&self, param: &Param<Model>) -> f32 {
        let (min, max, gradient) = match &param.param_type {
            Type::Numeric { min, max, gradient } => (min, max, gradient)
        };

        if *self <= *min {
            return 0.0;
        }

        if *self >= *max {
            return 1.0;
        }

        let unmap = |x: f32| -> f32 {
            let range = max - min;

            let x = match param.unit {
                Unit::Decibels => coeff_to_db(x),
                _ => x
            };

            (x - min) / range
        };

        match gradient {
            Gradient::Linear => unmap(*self),

            Gradient::Power(exponent) =>
                unmap(*self).powf(1.0 / *exponent),

            Gradient::Exponential => {
                let minl = min.log2();
                let range = max.log2() - minl;
                (self.log2() - minl) / range
            }
        }
    }
}

pub trait TranslateFrom<F, T, Model>
    where T: Translatable<T, Model>
{
    fn xlate_from(self, param: &Param<Model>) -> T;
}

impl<T, Model> TranslateFrom<f32, T, Model> for f32
    where T: Translatable<T, Model>
{
    #[inline]
    fn xlate_from(self, param: &Param<Model>) -> T {
        T::xlate_in(param, self)
    }
}
