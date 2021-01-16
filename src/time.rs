#[derive(Clone)]
pub struct MusicalTime {
    pub bpm: f64,
    pub beat: f64,
    pub is_playing: bool
}

impl MusicalTime {
    pub(crate) fn step_by_samples(&mut self, sample_rate: f64, samples: usize) {
        let beats_per_second = self.bpm / 60f64;
        let seconds = (samples as f64) / (sample_rate as f64);

        self.beat += seconds * beats_per_second;
    }
}
