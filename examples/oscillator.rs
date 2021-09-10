#![allow(incomplete_features)]
#![feature(generic_associated_types)]

use serde::{Serialize, Deserialize};

use baseplug::{
    ProcessContext,
    Plugin,
};

use std::f32::consts::PI;

const TWO_PI: f32 = 2.0 * PI;

baseplug::model! {
    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Copy)]
    enum OscillatorMode {
        Sine,
        Saw,
        Square,
        Trangle,
    }

    #[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Copy)]
    enum Switch {
        Off,
        On,
    }

    #[derive(Debug, Serialize, Deserialize)]
    struct OscillatorModel {
        #[parameter(name = "mode")]
        mode: OscillatorMode,

        #[parameter(name = "switch")]
        switch: Switch,

        #[model(min = 220, max = 880.0)]
        #[parameter(name = "frequency", 
            gradient = "Linear")]
        frequency: f32,
    }
}

impl Default for OscillatorModel {
    fn default() -> Self {
        Self {
            mode: OscillatorMode::Sine,
            switch: Switch::Off,
            frequency: 440.0,
        }
    }
}

struct Oscillator {
    phase: f32,
    phase_increment: f32,
}

impl Oscillator {
    fn update_phase(&mut self) {
        self.phase += self.phase_increment;
        while self.phase >= TWO_PI {
            self.phase -= TWO_PI;
        }        
    }

    fn update_phase_increment(&mut self, frequency: f32) {
        self.phase_increment = frequency * TWO_PI / 44100.0;
    }
}

impl Plugin for Oscillator {
    const NAME: &'static str = "basic oscillator plug";
    const PRODUCT: &'static str = "basic oscillator plug";
    const VENDOR: &'static str = "spicy plugins & co";

    const INPUT_CHANNELS: usize = 0;
    const OUTPUT_CHANNELS: usize = 2;

    type Model = OscillatorModel;

    #[inline]
    fn new(_sample_rate: f32, model: &OscillatorModel) -> Self {
        Self {
            phase: 0.0,
            phase_increment: model.frequency * TWO_PI / 44100.0,
        }
    }

    #[inline]
    fn process(&mut self, model: &OscillatorModelProcess, ctx: &mut ProcessContext<Self>) {
        let output = &mut ctx.outputs[0].buffers;

        for i in 0..ctx.nframes {
            if model.frequency.is_smoothing() {
                self.update_phase_increment(model.frequency[i]);
            }

            match model.switch.to {
                Switch::On => {
                    let new_output = match model.mode.to {
                        OscillatorMode::Sine => {
                            let output = self.phase.sin();
                            self.update_phase();
                            output
                        },
                        OscillatorMode::Saw => {
                            let output = 1.0 - (2.0 * self.phase / TWO_PI);
                            self.update_phase();
                            output
                        },
                        OscillatorMode::Square => {
                            let mut output = -1.0;
                            if self.phase <= PI {
                                output = 1.0;
                            }
                            self.update_phase();
                            output
                        },
                        OscillatorMode::Trangle => {
                            let mut output = -1.0 + (2.0 * self.phase / TWO_PI);
                            output = 2.0 * (output.abs() - 0.5);
                            self.update_phase();
                            output
                        },
                    };
                    output[0][i] = new_output;
                    output[1][i] = new_output;
                },
                Switch::Off => {
                    output[0][i] = 0.0;
                    output[1][i] = 0.0;
                }
            }

        }
    }
}

baseplug::vst2!(Oscillator, b"SSST");
