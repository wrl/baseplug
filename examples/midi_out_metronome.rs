#![allow(incomplete_features)]
#![feature(min_specialization)]

use serde::{Deserialize, Serialize};

use baseplug::{event::Data, Event, Plugin, ProcessContext};

baseplug::model! {
    #[derive(Debug, Serialize, Deserialize)]
    struct MidiOutMetronomeModel {
        #[model(min = 0.5, max = 2.0)]
        #[parameter(name = "len")]
        len: f32,
    }
}

impl Default for MidiOutMetronomeModel {
    fn default() -> Self {
        Self { len: 1.0 }
    }
}

struct MidiOutMetronome {
    note_on: bool,
    on_ct: u64,
    frame_ct: u64,
}

impl Plugin for MidiOutMetronome {
    const NAME: &'static str = "midi out metronome plug";
    const PRODUCT: &'static str = "midi out metronome plug";
    const VENDOR: &'static str = "spicy plugins & co";

    const INPUT_CHANNELS: usize = 2;
    const OUTPUT_CHANNELS: usize = 2;

    type Model = MidiOutMetronomeModel;

    fn new(_sample_rate: f32, _model: &Self::Model) -> Self {
        Self {
            note_on: false,
            on_ct: 0,
            frame_ct: 0,
        }
    }

    fn process<'proc>(&mut self, model: &MidiOutMetronomeModelProcess,
        ctx: &'proc mut ProcessContext<Self>)
    {
        let output = &mut ctx.outputs[0].buffers;
        let enqueue_midi = &mut ctx.enqueue_event;

        // get the current beat and tempo
        let curr_bpm = ctx.musical_time.bpm;
        let is_playing = ctx.musical_time.is_playing;

        for i in 0..ctx.nframes {
            // write silence
            output[0][i] = 0.0;
            output[1][i] = 0.0;

            // calc
            let beat_in_ms = 60_000.0 / curr_bpm;
            let beat_in_samples = beat_in_ms * ctx.sample_rate as f64 / 1000.0;
            let sixth_in_samples = (beat_in_samples / 4.0) * model.len[i] as f64;
            let beat_in_samples = beat_in_samples.round() as u64;
            let sixth_in_samples = sixth_in_samples.round() as u64;

            if is_playing && self.frame_ct % beat_in_samples == 0 {
                // send a note on (C2)
                let note_on = Event::<MidiOutMetronome> {
                    frame: i,
                    data: Data::Midi([144, 36, 120]),
                };

                enqueue_midi(note_on);
                self.note_on = true;
                self.on_ct = 0;
            }

            if is_playing && self.note_on && self.on_ct == sixth_in_samples {
                // send a note off (C2)
                let note_off = Event::<MidiOutMetronome> {
                    frame: i,
                    data: Data::Midi([128, 36, 0]),
                };

                enqueue_midi(note_off);
                self.note_on = false;
            }

            if is_playing {
                if self.note_on {
                    self.on_ct += 1;
                }
                self.frame_ct += 1;
            } else {
                self.frame_ct = 0;
            }
        }
    }
}

baseplug::vst2!(MidiOutMetronome, b"~MM~");
