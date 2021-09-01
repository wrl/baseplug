use std::rc::Rc;

use crate::parameter::{
    dsp_val_to_unit_val, normal_to_unit_value, unit_val_to_dsp_val, unit_value_to_normal,
    Type, Unit,
};
use crate::{ParamInfo, PlugMsgHandles, UIToPlugMsg};

pub struct UIFloatParam<Model: 'static, SmoothModel: 'static> {
    dsp_value: f32,
    unit_value: f32,
    normalized: f32,

    param_info: &'static ParamInfo,

    plug_msg_handles: Rc<PlugMsgHandles<Model, SmoothModel>>,

    updated_by_host: bool,
}

impl<Model: 'static, SmoothModel: 'static> UIFloatParam<Model, SmoothModel> {
    pub fn new(
        dsp_value: f32,
        param_info: &'static ParamInfo,
        plug_msg_handles: Rc<PlugMsgHandles<Model, SmoothModel>>,
    ) -> Self {
        let unit_value = dsp_val_to_unit_val(param_info.unit, dsp_value);
        let normalized = unit_value_to_normal(&param_info.param_type, unit_value);

        Self {
            dsp_value,
            unit_value,
            normalized,
            param_info,
            plug_msg_handles,
            updated_by_host: true,
        }
    }

    pub fn set_from_normalized(&mut self, normalized: f32) {
        if self.normalized != normalized {
            // Make sure that `normalized` is withing range.
            self.normalized = normalized.clamp(0.0, 1.0);

            self.unit_value = normal_to_unit_value(&self.param_info.param_type, self.normalized);
            self.dsp_value = unit_val_to_dsp_val(self.param_info.unit, self.unit_value);

            self.send_to_host();
        }
    }

    pub fn set_from_unit_value(&mut self, unit_value: f32) {
        if self.unit_value != unit_value {
            // Make sure that `unit_value` is within range.
            self.unit_value = self.clamp_value(unit_value);

            self.normalized = unit_value_to_normal(&self.param_info.param_type, self.unit_value);
            self.dsp_value = unit_val_to_dsp_val(self.param_info.unit, self.unit_value);

            self.send_to_host();
        }
    }

    #[inline]
    fn send_to_host(&mut self) {
        self.plug_msg_handles.ui_host_cb
            .send_parameter_update(self.param_info.idx, self.normalized);
    
        if self.plug_msg_handles.notify_dsp {
            if let Err(_) = self.plug_msg_handles.push_msg(UIToPlugMsg::ParamChanged {
                param_idx: self.param_info.idx,
                normalized: self.normalized,
            }) {
                eprintln!("UI to Plug message buffer is full!");
            }
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
    pub fn updated_by_host(&self) -> bool {
        self.updated_by_host
    }

    #[inline]
    pub fn normal_to_unit_value(&self, normalized: f32) -> f32 {
        normal_to_unit_value(&self.param_type(), normalized)
    }

    #[inline]
    pub fn unit_value_to_normal(&self, unit_value: f32) -> f32 {
        unit_value_to_normal(&self.param_type(), unit_value)
    }

    #[inline]
    pub fn unit_value_to_dsp_value(&self, unit_value: f32) -> f32 {
        unit_val_to_dsp_val(self.param_info.unit, unit_value)
    }

    #[inline]
    pub fn dsp_val_to_unit_val(&self, dsp_value: f32) -> f32 {
        dsp_val_to_unit_val(self.param_info.unit, dsp_value)
    }

    /// Only to be used by `baseplug` itself.
    #[inline]
    pub fn _reset_update_flag(&mut self) {
        self.updated_by_host = false;
    }

    /// Only to be used by `baseplug` itself.
    #[inline]
    pub fn _set_from_host(&mut self, dsp_value: f32) {
        self.dsp_value = dsp_value;

        self.unit_value = dsp_val_to_unit_val(self.param_info.unit, dsp_value);
        self.normalized = unit_value_to_normal(&self.param_info.param_type, self.unit_value);

        self.updated_by_host = true;
    }
}

pub struct UIFloatValue<Model: 'static, SmoothModel: 'static> {
    val: f32,
    plug_msg_handles: Rc<PlugMsgHandles<Model, SmoothModel>>,
    cb: &'static fn(&mut SmoothModel, f32),
}

impl<Model: 'static, SmoothModel: 'static> UIFloatValue<Model, SmoothModel> {
    pub fn new(
        val: f32,
        plug_msg_handles: Rc<PlugMsgHandles<Model, SmoothModel>>,
        cb: &'static fn(&mut SmoothModel, f32),
    ) -> Self {
        Self {
            val,
            plug_msg_handles,
            cb,
        }
    }

    pub fn set(&mut self, val: f32) {
        self.val = val;

        if let Err(_) = self.plug_msg_handles.push_msg(UIToPlugMsg::ValueChanged {
            cb: self.cb,
            value: self.val
        }) {
            eprintln!("UI to Plug message buffer is full!");
        }
    }

    #[inline]
    pub fn get(&self) -> f32 {
        self.val
    }

    // Only to be used by baseplug itself.
    #[inline]
    pub fn _set_from_host(&mut self, val: f32) {
        self.val = val;
    }
}