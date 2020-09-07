#![allow(incomplete_features)]
#![feature(generic_associated_types)]

use serde::{Serialize, Deserialize};

use baseplug::{
    ProcessContext,
    Plugin,
};


baseplug::model! {
    #[derive(Debug, Serialize, Deserialize)]
    struct GainModel {
        #[model(min = -90.0, max = 3.0)]
        #[parameter(name = "gain", unit = "Decibels",
            gradient = "Power(0.15)")]
        gain: f32
    }
}

impl Default for GainModel {
    fn default() -> Self {
        Self {
            // "gain" is converted from dB to coefficient in the parameter handling code,
            // so in the model here it's a coeff.
            // -0dB == 1.0
            gain: 1.0
        }
    }
}

struct Gain;

impl Plugin for Gain {
    const NAME: &'static str = "basic gain plug";
    const PRODUCT: &'static str = "basic gain plug";
    const VENDOR: &'static str = "spicy plugins & co";

    const INPUT_CHANNELS: usize = 2;
    const OUTPUT_CHANNELS: usize = 2;

    type Model = GainModel;

    #[inline]
    fn new(_sample_rate: f32, _model: &GainModel) -> Self {
        Self
    }

    #[inline]
    fn process(&mut self, model: &GainModelProcess, ctx: &mut ProcessContext) {
        let input = &ctx.inputs[0].buffers;
        let output = &mut ctx.outputs[0].buffers;

        for i in 0..ctx.nframes {
            output[0][i] = input[0][i] * model.gain[i];
            output[1][i] = input[1][i] * model.gain[i];
        }
    }
}


baseplug::vst2!(Gain, b"tAnE");
fn make_cid () -> vst3_com::IID {
    vst3_com::IID {
        data: [0x44, 0xb0, 0xeb, 0xf7, 0x29, 0x55, 0x40, 0x6c, 0x8c, 0xcd, 0x69, 0xac, 0x57, 0xb1, 0x71, 0xb9]
    }
}

baseplug::vst3!{Gain, "gain.example.com/vst3", "gain.support@example.com", crate::make_cid()}

//$plugin:ident, $url:expr, $email:expr, $iid:expr