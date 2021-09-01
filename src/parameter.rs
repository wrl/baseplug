use std::fmt;
use std::io;

use crate::*;
use crate::util::coeff_to_db;
use crate::util::db_to_coeff;

#[derive(Debug, Copy, Clone)]
pub enum Gradient {
    Linear,
    Power(f32),
    Exponential
}

#[derive(Debug, Copy, Clone)]
pub enum Type {
    Numeric {
        min: f32,
        max: f32,

        gradient: Gradient
    },

    // eventually will have an Enum/Discrete type here
}

#[derive(Debug, Clone, Copy)]
pub enum Unit {
    Generic,
    Decibels
}

pub struct Format<P: Plugin, SmoothModel, UIModel> {
    pub display_cb: fn(&Param<P, SmoothModel, UIModel>, &SmoothModel, &mut dyn io::Write) -> io::Result<()>,
    pub label: &'static str
}

pub struct ParamInfo {
    pub name: &'static str,
    pub short_name: Option<&'static str>,
    pub label: &'static str,

    pub unit: Unit,
    pub param_type: Type,

    pub idx: usize,
}

impl ParamInfo {
    #[inline]
    pub fn get_name(&self) -> &'static str {
        self.short_name
            .unwrap_or_else(|| self.name)
    }
}

pub struct Param<P: Plugin, SmoothModel, UIModel> {
    pub info: ParamInfo,

    pub format: Format<P, SmoothModel, UIModel>,

    pub dsp_notify: Option<fn(&mut P)>,

    pub set_cb: fn(&Param<P, SmoothModel, UIModel>, &mut SmoothModel, f32),
    pub get_cb: fn(&Param<P, SmoothModel, UIModel>, &SmoothModel) -> f32,

    pub set_ui_cb: fn(&mut UIModel, f32),
}

impl<P: Plugin, SmoothModel, UIModel> Param<P, SmoothModel, UIModel> {
    #[inline]
    pub fn set(&self, model: &mut SmoothModel, val: f32) {
        (self.set_cb)(self, model, val)
    }

    #[inline]
    pub fn get(&self, model: &SmoothModel) -> f32 {
        (self.get_cb)(self, model)
    }

    #[inline]
    pub fn get_name(&self) -> &'static str {
        self.info.get_name()
    }

    #[inline]
    pub fn get_label(&self) -> &'static str {
        self.info.label
    }

    #[inline]
    pub fn get_display(&self, model: &SmoothModel, w: &mut dyn io::Write) -> io::Result<()> {
        (self.format.display_cb)(self, model, w)
    }

    #[inline]
    pub fn set_ui(&self, model: &mut UIModel, val: f32) {
        (self.set_ui_cb)(model, val)
    }
}

impl<P: Plugin, SmoothModel, UIModel> fmt::Debug for Param<P, SmoothModel, UIModel> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Param")
            .field("name", &self.info.name)
            .field("short_name", &self.info.short_name)
            .field("unit", &self.info.unit)
            .field("param_type", &self.info.param_type)
            .finish()
    }
}

pub trait Translatable<T, P: Plugin, SmoothModel, UIModel> {
    fn xlate_in(param: &Param<P, SmoothModel, UIModel>, normalized: f32) -> T;
    fn xlate_out(&self, param: &Param<P, SmoothModel, UIModel>) -> f32;
}

impl<P: Plugin, SmoothModel, UIModel> Translatable<f32, P, SmoothModel, UIModel> for f32 {
    #[inline]
    fn xlate_in(param: &Param<P, SmoothModel, UIModel>, normalized: f32) -> f32 {
        normal_to_dsp_val(param.info.unit, &param.info.param_type, normalized)
    }

    #[inline]
    fn xlate_out(&self, param: &Param<P, SmoothModel, UIModel>) -> f32 {
        dsp_val_to_normal(param.info.unit, &param.info.param_type, *self)
    }
}

pub trait TranslateFrom<F, T, P: Plugin, SmoothModel, UIModel>
    where T: Translatable<T, P, SmoothModel, UIModel>
{
    fn xlate_from(self, param: &Param<P, SmoothModel, UIModel>) -> T;
}

impl<T, P: Plugin, SmoothModel, UIModel> TranslateFrom<f32, T, P, SmoothModel, UIModel> for f32
    where T: Translatable<T, P, SmoothModel, UIModel>
{
    #[inline]
    fn xlate_from(self, param: &Param<P, SmoothModel, UIModel>) -> T {
        T::xlate_in(param, self)
    }
}

pub fn normal_to_unit_value(param_type: &Type, normalized: f32) -> f32 {
    let (min, max, gradient) = match param_type {
        Type::Numeric { min, max, gradient } => (min, max, gradient)
    };

    let normalized = normalized.min(1.0).max(0.0);

    let map = |x: f32| -> f32 {
        let range = max - min;
        (x * range) + min
    };

    match gradient {
        Gradient::Linear => map(normalized),

        Gradient::Power(exponent) =>
            map(normalized.powf(*exponent)),

        Gradient::Exponential => {
            if normalized == 0.0 {
                *min
            } else if normalized == 1.0 {
                *max
            } else {
                let minl = min.log2();
                let range = max.log2() - minl;
                2.0f32.powf((normalized * range) + minl)
            }
        }
    }
}

pub fn unit_value_to_normal(param_type: &Type, unit_value: f32) -> f32 {
    let (min, max, gradient) = match param_type {
        Type::Numeric { min, max, gradient } => (min, max, gradient)
    };

    if unit_value <= *min {
        return 0.0;
    }
    if unit_value >= *max {
        return 1.0;
    }

    let unmap = |x: f32| -> f32 {
        let range = max - min;
        (x - min) / range
    };

    match gradient {
        Gradient::Linear => unmap(unit_value),

        Gradient::Power(exponent) =>
            unmap(unit_value).powf(1.0 / *exponent),

        Gradient::Exponential => {
            let minl = min.log2();
            let range = max.log2() - minl;

            (unit_value.log2() - minl) / range
        }
    }
}

#[inline]
pub fn unit_val_to_dsp_val(unit: Unit, unit_value: f32) -> f32 {
    match unit {
        Unit::Decibels => db_to_coeff(unit_value),
        _ => unit_value
    }
}

#[inline]
pub fn dsp_val_to_unit_val(unit: Unit, dsp_value: f32) -> f32 {
    match unit {
        Unit::Decibels => coeff_to_db(dsp_value),
        _ => dsp_value
    }
}

#[inline]
pub fn normal_to_dsp_val(unit: Unit, param_type: &Type, normalized: f32) -> f32 {
    let unit_val = normal_to_unit_value(param_type, normalized);
    unit_val_to_dsp_val(unit, unit_val)
}

#[inline]
pub fn dsp_val_to_normal(unit: Unit, param_type: &Type, dsp_value: f32) -> f32 {
    let unit_val = dsp_val_to_unit_val(unit, dsp_value);
    unit_value_to_normal(param_type, unit_val)
}