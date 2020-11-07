#![allow(incomplete_features)]
#![feature(generic_associated_types)]
#![feature(min_specialization)]

use serde::{Deserialize, Serialize};

use baseplug::{event::Data, Event, Plugin, ProcessContext};

baseplug::model! {
    #[derive(Debug, Serialize, Deserialize)]
    struct MidiSenderModel {
        #[model(min = 0.5, max = 2.0)]
        #[parameter(name = "len")]
        len: f32,
    }
}

impl Default for MidiSenderModel {
    fn default() -> Self {
        Self { len: 1.0 }
    }
}

struct MidiSender {
    sample_rate: f32,
    note_on: bool,
    on_ct: u64,
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
            note_on: false,
            on_ct: 0,
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
            let curr_beat = ctx.musical_time.beat;
            let curr_bpm = ctx.musical_time.bpm;
            let is_playing = ctx.musical_time.is_playing;

            // calc
            let beat_in_ms = 60_000.0 / curr_bpm;
            let beat_in_samples = beat_in_ms * self.sample_rate as f64 / 1000.0;
            let sixth_in_samples = (beat_in_samples / 4.0) * model.len[i] as f64;
            let curr_beat_in_samples = beat_in_samples * curr_beat;
            let beat_in_samples = beat_in_samples.round() as u64;
            let curr_beat_in_samples = curr_beat_in_samples.round() as u64;
            let sixth_in_samples = sixth_in_samples.round() as u64;

            let enqueue_midi = &mut ctx.enqueue_event;

            let on_beat = curr_beat_in_samples % beat_in_samples == 0;
            // let on_sixth = curr_beat_in_samples % sixth_in_samples == 0;

            if on_beat && is_playing && !self.note_on {
                // send a note on (C2)
                let note_on = Event::<MidiSender> {
                    frame: i,
                    data: Data::Midi([144, 36, 120]),
                };

                enqueue_midi(note_on);
                self.note_on = true;
                self.on_ct = 0;
            }

            if self.on_ct == sixth_in_samples {
                // send a note off (C2)
                let note_off = Event::<MidiSender> {
                    frame: i,
                    data: Data::Midi([128, 36, 0]),
                };

                enqueue_midi(note_off);
                self.note_on = false;
            }

            if self.note_on {
                self.on_ct += 1;
            }
        }
    }
}

baseplug::vst2!(MidiSender, b"~Ms~");
