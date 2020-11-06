#![allow(incomplete_features)]
#![feature(generic_associated_types)]
#![feature(min_specialization)]

use serde::{Deserialize, Serialize};

use baseplug::{Plugin, ProcessContext, Event, event::Data};

baseplug::model! {
    #[derive(Debug, Serialize, Deserialize)]
    struct MidiSenderModel {
        #[model(min = 1.0, max = 8.0)]
        #[parameter(name = "speed")]
        speed: f32,
    }
}

impl Default for MidiSenderModel {
    fn default() -> Self {
        Self { speed: 1.0 }
    }
}

struct MidiSender {
    sample_rate: f32,
    frame_count: u64,
}

impl Plugin for MidiSender {
    const NAME: &'static str = "midi sender plug";

    const PRODUCT: &'static str = "midi sender plug";

    const VENDOR: &'static str = "spicy plugins & co";

    const INPUT_CHANNELS: usize = 0;

    const OUTPUT_CHANNELS: usize = 2;

    type Model = MidiSenderModel;

    fn new(sample_rate: f32, model: &Self::Model) -> Self {
        Self {
            sample_rate: sample_rate,
            frame_count: 0,
        }
    }

    fn process<'proc>(
        &mut self,
        model: &MidiSenderModelProcess,
        ctx: &'proc mut ProcessContext<Self>,
    ) {
        let output = &mut ctx.outputs[0].buffers;

        for i in 0..ctx.nframes {
            // write silence
            output[0][i] = 0.0;
            output[1][i] = 0.0;

            // get the current beat and tempo
            // let curr_beat = ctx.musical_time.beat;
            let curr_bpm = ctx.musical_time.bpm;
            let beat_in_ms = 60_000.0 / curr_bpm;
            let beat_in_samples = beat_in_ms * self.sample_rate as f64 / 1000.0;
            let beat_in_samples = beat_in_samples.round() as u64;

            let enqueue_midi = &mut ctx.enqueue_event;

            if self.frame_count % beat_in_samples == 0 {
                // send a note on (C2)
                let note_on = Event::<MidiSender> {
                    frame: i,
                    data: Data::Midi([144, 36, 120])
                };

                enqueue_midi(note_on);
            }

            if self.frame_count % (beat_in_samples + beat_in_samples / 16) == 0 {
                // send a note off (C2)
                let note_off = Event::<MidiSender> {
                    frame: i,
                    data: Data::Midi([128, 36, 0])
                };

                enqueue_midi(note_off);
            }

            // inc absolute frame count
            self.frame_count += 1;
        }
    }
}

baseplug::vst2!(MidiSender, b"~Ms~");
