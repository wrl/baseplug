#![allow(incomplete_features)]
#![feature(min_specialization)]

use std::f32::consts::PI;

use serde::{Serialize, Deserialize};

use baseplug::{
    ProcessContext,
    Plugin,
    MidiReceiver,
    util::db_to_coeff
};


baseplug::model! {
    #[derive(Debug, Serialize, Deserialize)]
    struct MidiSineModel {
        #[model(min = -90.0, max = 3.0)]
        #[parameter(name = "gain", unit = "Decibels",
            gradient = "Power(0.15)")]
        gain: f32,

        #[model(min = 0.05, max = 0.95)]
        #[parameter(name = "phase distortion")]
        pd: f32,

        #[model(min = 220.0, max = 880.0)]
        #[parameter(name = "a4 tuning", gradient = "Exponential")]
        a4: f32
    }
}

impl Default for MidiSineModel {
    fn default() -> Self {
        Self {
            gain: db_to_coeff(-3.0),
            pd: 0.5,
            a4: 440.0
        }
    }
}

struct Oscillator {
    phase: f64,
    step: f64
}

impl Oscillator {
    #[inline]
    fn new() -> Self {
        Self {
            // cheeky little hack to keep cosine output from jumping to +1.0 when adding the plugin
            // to the host ;>
            phase: 0.25,
            step: 0.0
        }
    }

    #[inline]
    fn set_frequency(&mut self, frequency: f64, sample_rate: f64) {
        self.step = frequency / sample_rate;
    }

    #[inline]
    fn tick(&mut self) {
        self.phase += self.step;

        // usually cheaper than modulo
        while self.phase > 1.0 {
            self.phase -= 1.0;
        }
    }

    #[inline]
    fn pd_phase(&self, d: f32) -> f32 {
        let mut phase = self.phase as f32;

        if phase < d {
            phase /= d;
        } else {
            phase = 1.0 + ((phase - d) / (1.0 - d));
        }

        phase * 0.5
    }
}

struct MidiSine {
    osc: Oscillator,
    sample_rate: f32,

    freq_ratio: f32,
}

impl Plugin for MidiSine {
    const NAME: &'static str = "midi sine plug";
    const PRODUCT: &'static str = "midi sine plug";
    const VENDOR: &'static str = "spicy plugins & co";

    const INPUT_CHANNELS: usize = 0;
    const OUTPUT_CHANNELS: usize = 2;

    type Model = MidiSineModel;

    #[inline]
    fn new(sample_rate: f32, _model: &MidiSineModel) -> Self {
        Self {
            osc: Oscillator::new(),
            sample_rate,

            freq_ratio: 0.0
        }
    }

    #[inline]
    fn process(&mut self, model: &MidiSineModelProcess, ctx: &mut ProcessContext<Self>) {
        let output = &mut ctx.outputs[0].buffers;

        for i in 0..ctx.nframes {
            if model.a4.is_smoothing() {
                self.osc.set_frequency((self.freq_ratio * model.a4[i]) as f64, self.sample_rate as f64);
            }

            let wave = {
                let phase = self.osc.pd_phase(model.pd[i]);
                (phase * 2.0 * PI).cos()
            };
            self.osc.tick();

            output[0][i] = wave * model.gain[i];
            output[1][i] = wave * model.gain[i];
        }
    }
}

impl MidiReceiver for MidiSine {
    fn midi_input(&mut self, model: &MidiSineModelProcess, data: [u8; 3]) {
        match data[0] {
            // note on
            0x90 => {
                let ratio = ((data[1] as f32 - 69.0) / 12.0).exp2();
                self.freq_ratio = ratio;

                let freq = ratio * model.a4[0];
                self.osc.set_frequency(freq as f64, self.sample_rate as f64);
            },

            _ => ()
        }
    }
}

baseplug::vst2!(MidiSine, b"~Ss~");
