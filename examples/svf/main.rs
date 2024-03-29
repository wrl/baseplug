#![feature(portable_simd)]

use serde::{Serialize, Deserialize};
use std::simd::f32x4;

mod svf_simper;
use svf_simper::SVFSimper;

use baseplug::{
    Plugin,
    ProcessContext
};


baseplug::model! {
    #[derive(Debug, Serialize, Deserialize)]
    struct SVFModel {
        #[model(min = 10.0, max = 22000.0)]
        #[parameter(name = "cutoff", label = "hz", gradient = "Exponential")]
        cutoff: f32,

        #[model(min = 0.0, max = 1.0)]
        #[parameter(name = "resonance")]
        resonance: f32
    }
}

impl Default for SVFModel {
    fn default() -> Self {
        Self {
            cutoff: 10000.0,
            resonance: 0.6
        }
    }
}

struct SVFPlugin {
    lpf: SVFSimper
}

impl Plugin for SVFPlugin {
    const NAME: &'static str = "svf lowpass";
    const PRODUCT: &'static str = "svf lowpass";
    const VENDOR: &'static str = "spicy plugins & co";

    const INPUT_CHANNELS: usize = 2;
    const OUTPUT_CHANNELS: usize = 2;

    type Model = SVFModel;

    #[inline]
    fn new(sample_rate: f32, model: &SVFModel) -> Self {
        Self {
            lpf: SVFSimper::new(model.cutoff, model.resonance, sample_rate)
        }
    }

    #[inline]
    fn process(&mut self, model: &SVFModelProcess, ctx: &mut ProcessContext<Self>) {
        let input = &ctx.inputs[0].buffers;
        let output = &mut ctx.outputs[0].buffers;

        for i in 0..ctx.nframes {
            self.lpf.set(model.cutoff[i], model.resonance[i], ctx.sample_rate);

            let frame = f32x4::from_array(
                [input[0][i], input[1][i], 0.0, 0.0]);
            let lp = self.lpf.process(frame);

            let frame_out = lp.as_array();
            output[0][i] = frame_out[0];
            output[1][i] = frame_out[1];
        }
    }
}

baseplug::vst2!(SVFPlugin, b"sVf!");
