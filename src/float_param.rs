use std::sync::Arc;

use crate::parameter::{
    dsp_val_to_unit_val, normal_to_unit_value, unit_val_to_dsp_val, unit_value_to_normal,
    ParamInfo, Type, Unit,
};
use crate::{
    AtomicFloat, Model, Parameters, Plugin, Smooth, SmoothStatus, SmoothOutput,
    UIHostCallback
};

pub struct SmoothFloatParam {
    smooth: Smooth<f32>,

    shared_dsp_value: Arc<AtomicFloat>,
    dsp_value: f32,

    param_info: &'static ParamInfo,
}

impl SmoothFloatParam {
    pub fn new(dsp_value: f32, param_info: &'static ParamInfo) -> Self {
        Self {
            smooth: Smooth::new(dsp_value),
            shared_dsp_value: Arc::new(AtomicFloat::new(dsp_value)),
            dsp_value,
            param_info,
        }
    }

    #[inline]
    pub fn reset(&mut self, dsp_value: f32) {
        self.shared_dsp_value.set(dsp_value);
        self.dsp_value = dsp_value;

        self.smooth.reset(dsp_value);
    }

    #[inline]
    pub fn set(&mut self, dsp_value: f32) {
        self.shared_dsp_value.set(dsp_value);
        self.dsp_value = dsp_value;

        self.smooth.set(dsp_value);
    }

    #[inline]
    pub fn dsp_value(&self) -> f32 {
        self.dsp_value
    }

    #[inline]
    pub fn output(&self) -> SmoothOutput<f32> {
        self.smooth.output()
    }

    #[inline]
    pub fn current_value(&self) -> SmoothOutput<f32> {
        self.smooth.current_value()
    }

    #[inline]
    pub fn update_status_with_epsilon(&mut self, epsilon: f32) -> SmoothStatus {
        self.smooth.update_status_with_epsilon(epsilon)
    }

    #[inline]
    pub fn process<P: Plugin>(&mut self, nframes: usize, plug: &mut P, poll_from_ui: bool) {
        if poll_from_ui {
            // Check for updated value from UI.
            let dsp_value = self.shared_dsp_value.get();
            if self.dsp_value != dsp_value {
                self.dsp_value = dsp_value;

                self.smooth.set(dsp_value);

                // Don't mind me, just sprinkling in some casual generic monstrosities.
                let param = <<P::Model as Model<P>>::Smooth as Parameters<
                    P,
                    <P::Model as Model<P>>::Smooth,
                >>::PARAMS[self.param_info.idx];
                if let Some(dsp_notify) = param.dsp_notify {
                    dsp_notify(plug);
                }
            }
        }

        self.smooth.process(nframes);
    }

    #[inline]
    pub fn is_active(&self) -> bool {
        self.smooth.is_active()
    }

    #[inline]
    pub fn set_speed_ms(&mut self, sample_rate: f32, ms: f32) {
        self.smooth.set_speed_ms(sample_rate, ms)
    }

    #[inline]
    pub fn update_status(&mut self) -> SmoothStatus {
        self.smooth.update_status()
    }

    #[inline]
    pub fn get_ui_param(&self, ui_host_callback: Arc<dyn UIHostCallback>) -> UIFloatParam {
        UIFloatParam::new(
            Arc::clone(&self.shared_dsp_value),
            self.param_info,
            ui_host_callback,
        )
    }
}

pub struct SmoothFloatEntry {
    smooth: Smooth<f32>,

    shared_dsp_value: Arc<AtomicFloat>,
    dsp_value: f32,
}

impl SmoothFloatEntry {
    pub fn new(dsp_value: f32) -> Self {
        Self {
            smooth: Smooth::new(dsp_value),
            shared_dsp_value: Arc::new(AtomicFloat::new(dsp_value)),
            dsp_value,
        }
    }

    #[inline]
    pub fn reset(&mut self, dsp_value: f32) {
        self.shared_dsp_value.set(dsp_value);
        self.dsp_value = dsp_value;

        self.smooth.reset(dsp_value);
    }

    #[inline]
    pub fn set(&mut self, dsp_value: f32) {
        self.shared_dsp_value.set(dsp_value);
        self.dsp_value = dsp_value;

        self.smooth.set(dsp_value);
    }

    #[inline]
    pub fn dsp_value(&self) -> f32 {
        self.dsp_value
    }

    #[inline]
    pub fn output(&self) -> SmoothOutput<f32> {
        self.smooth.output()
    }

    #[inline]
    pub fn current_value(&self) -> SmoothOutput<f32> {
        self.smooth.current_value()
    }

    #[inline]
    pub fn update_status_with_epsilon(&mut self, epsilon: f32) -> SmoothStatus {
        self.smooth.update_status_with_epsilon(epsilon)
    }

    #[inline]
    pub fn process(&mut self, nframes: usize) {
        // Check for updated value from UI.
        let dsp_value = self.shared_dsp_value.get();
        if self.dsp_value != dsp_value {
            self.dsp_value = dsp_value;

            self.smooth.set(dsp_value);
        }

        self.smooth.process(nframes);
    }

    #[inline]
    pub fn is_active(&self) -> bool {
        self.smooth.is_active()
    }

    #[inline]
    pub fn set_speed_ms(&mut self, sample_rate: f32, ms: f32) {
        self.smooth.set_speed_ms(sample_rate, ms)
    }

    #[inline]
    pub fn update_status(&mut self) -> SmoothStatus {
        self.smooth.update_status()
    }

    #[inline]
    pub fn get_ui_entry(&self) -> UIFloatEntry {
        UIFloatEntry::new(Arc::clone(&self.shared_dsp_value))
    }
}

pub struct UnsmoothedFloatParam {
    shared_dsp_value: Arc<AtomicFloat>,
    dsp_value: f32,

    param_info: &'static ParamInfo,
}

impl UnsmoothedFloatParam {
    pub fn new(dsp_value: f32, param_info: &'static ParamInfo) -> Self {
        Self {
            shared_dsp_value: Arc::new(AtomicFloat::new(dsp_value)),
            dsp_value,
            param_info,
        }
    }

    #[inline]
    pub fn reset(&mut self, dsp_value: f32) {
        self.shared_dsp_value.set(dsp_value);
        self.dsp_value = dsp_value;
    }

    #[inline]
    pub fn set(&mut self, dsp_value: f32) {
        self.shared_dsp_value.set(dsp_value);
        self.dsp_value = dsp_value;
    }

    #[inline]
    pub fn dsp_value(&self) -> f32 {
        self.dsp_value
    }

    #[inline]
    pub fn process<P: Plugin>(&mut self, _nframes: usize, plug: &mut P, poll_from_ui: bool) {
        if poll_from_ui {
            // Check for updated value from UI.
            let dsp_value = self.shared_dsp_value.get();
            if self.dsp_value != dsp_value {
                self.dsp_value = dsp_value;

                // Don't mind me, just sprinkling in some casual generic monstrosities.
                let param = <<P::Model as Model<P>>::Smooth as Parameters<
                    P,
                    <P::Model as Model<P>>::Smooth,
                >>::PARAMS[self.param_info.idx];
                if let Some(dsp_notify) = param.dsp_notify {
                    dsp_notify(plug);
                }
            }
        }
    }

    #[inline]
    pub fn get_ui_param(&self, ui_host_callback: Arc<dyn UIHostCallback>) -> UIFloatParam {
        UIFloatParam::new(
            Arc::clone(&self.shared_dsp_value),
            self.param_info,
            ui_host_callback,
        )
    }
}

pub struct UnsmoothedFloatEntry {
    shared_dsp_value: Arc<AtomicFloat>,
    dsp_value: f32,
}

impl UnsmoothedFloatEntry {
    pub fn new(dsp_value: f32) -> Self {
        Self {
            shared_dsp_value: Arc::new(AtomicFloat::new(dsp_value)),
            dsp_value,
        }
    }

    #[inline]
    pub fn reset(&mut self, dsp_value: f32) {
        self.shared_dsp_value.set(dsp_value);
        self.dsp_value = dsp_value;
    }

    #[inline]
    pub fn set(&mut self, dsp_value: f32) {
        self.shared_dsp_value.set(dsp_value);
        self.dsp_value = dsp_value;
    }

    #[inline]
    pub fn dsp_value(&self) -> f32 {
        self.dsp_value
    }

    #[inline]
    pub fn process(&mut self, _nframes: usize) {
        // Check for updated value from UI.
        let dsp_value = self.shared_dsp_value.get();
        if self.dsp_value != dsp_value {
            self.dsp_value = dsp_value;
        }
    }

    #[inline]
    pub fn get_ui_entry(&self) -> UIFloatEntry {
        UIFloatEntry::new(Arc::clone(&self.shared_dsp_value))
    }
}

pub struct UIFloatParam {
    shared_dsp_value: Arc<AtomicFloat>,

    ui_host_callback: Arc<dyn UIHostCallback>,

    dsp_value: f32,
    unit_value: f32,
    normalized: f32,

    param_info: &'static ParamInfo,

    did_change: bool,
}

impl UIFloatParam {
    fn new(
        shared_dsp_value: Arc<AtomicFloat>,
        param_info: &'static ParamInfo,
        ui_host_callback: Arc<dyn UIHostCallback>,
    ) -> Self {
        let dsp_value = shared_dsp_value.get();
        let unit_value = dsp_val_to_unit_val(param_info.unit, dsp_value);
        let normalized = unit_value_to_normal(&param_info.param_type, unit_value);

        Self {
            shared_dsp_value,
            ui_host_callback,
            dsp_value,
            unit_value,
            normalized,
            param_info,
            did_change: true,
        }
    }

    pub fn set_from_normalized(&mut self, normalized: f32) {
        if self.normalized != normalized {
            // Make sure that `normalized` is withing range.
            self.normalized = normalized.clamp(0.0, 1.0);

            self.unit_value = normal_to_unit_value(&self.param_info.param_type, self.normalized);
            self.dsp_value = unit_val_to_dsp_val(self.param_info.unit, self.unit_value);

            self.shared_dsp_value.set(self.dsp_value);
            self.did_change = true;

            self.ui_host_callback
                .send_parameter_update(self.param_info.idx, self.normalized);
        }
    }

    pub fn set_from_value(&mut self, unit_value: f32) {
        if self.unit_value != unit_value {
            // Make sure that `unit_value` is within range.
            self.unit_value = self.clamp_value(unit_value);

            self.normalized = unit_value_to_normal(&self.param_info.param_type, self.unit_value);
            self.dsp_value = unit_val_to_dsp_val(self.param_info.unit, self.unit_value);

            self.shared_dsp_value.set(self.dsp_value);
            self.did_change = true;

            self.ui_host_callback
                .send_parameter_update(self.param_info.idx, self.normalized);
        }
    }

    #[inline]
    pub fn clamp_value(&self, unit_value: f32) -> f32 {
        let (min, max) = match &self.param_info.param_type {
            Type::Numeric { min, max, .. } => (min, max),
        };
        unit_value.clamp(*min, *max)
    }

    #[inline]
    pub fn normalized(&self) -> f32 {
        self.normalized
    }

    #[inline]
    pub fn dsp_value(&self) -> f32 {
        self.dsp_value
    }

    #[inline]
    pub fn unit_value(&self) -> f32 {
        self.unit_value
    }

    #[inline]
    pub fn name(&self) -> &'static str {
        self.param_info.get_name()
    }

    #[inline]
    pub fn short_name(&self) -> Option<&'static str> {
        self.param_info.short_name
    }

    #[inline]
    pub fn long_name(&self) -> &'static str {
        self.param_info.name
    }

    #[inline]
    pub fn unit_label(&self) -> &'static str {
        self.param_info.label
    }

    #[inline]
    pub fn unit(&self) -> Unit {
        self.param_info.unit
    }

    #[inline]
    pub fn param_type(&self) -> &Type {
        &self.param_info.param_type
    }

    #[inline]
    pub fn min_max(&self) -> (f32, f32) {
        match &self.param_info.param_type {
            Type::Numeric { min, max, .. } => (*min, *max),
        }
    }

    #[inline]
    pub fn did_change(&self) -> bool {
        self.did_change
    }

    #[inline]
    pub fn normal_to_value(&self, normalized: f32) -> f32 {
        normal_to_unit_value(&self.param_type(), normalized)
    }

    #[inline]
    pub fn value_to_normal(&self, unit_value: f32) -> f32 {
        unit_value_to_normal(&self.param_type(), unit_value)
    }

    #[inline]
    pub fn _poll_update(&mut self) {
        let dsp_value = self.shared_dsp_value.get();
        if self.dsp_value != dsp_value {
            self.dsp_value = dsp_value;

            self.unit_value = dsp_val_to_unit_val(self.param_info.unit, dsp_value);
            self.normalized = unit_value_to_normal(&self.param_info.param_type, self.unit_value);
            self.did_change = true;
        } else {
            self.did_change = false;
        }
    }
}

pub struct UIFloatEntry {
    shared_dsp_value: Arc<AtomicFloat>,

    dsp_value: f32,

    did_change: bool,
}

impl UIFloatEntry {
    fn new(shared_dsp_value: Arc<AtomicFloat>) -> Self {
        let dsp_value = shared_dsp_value.get();

        Self {
            shared_dsp_value,
            dsp_value,
            did_change: true,
        }
    }

    pub fn set(&mut self, value: f32) {
        if self.dsp_value != value {
            self.shared_dsp_value.set(self.dsp_value);
            self.did_change = true;
        }
    }

    #[inline]
    pub fn get(&self) -> f32 {
        self.dsp_value
    }

    #[inline]
    pub fn did_change(&self) -> bool {
        self.did_change
    }

    #[inline]
    pub fn _poll_update(&mut self) {
        let dsp_value = self.shared_dsp_value.get();
        if self.dsp_value != dsp_value {
            self.dsp_value = dsp_value;

            self.did_change = true;
        } else {
            self.did_change = false;
        }
    }
}
