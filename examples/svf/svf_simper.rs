// implemented from https://cytomic.com/files/dsp/SvfLinearTrapOptimised2.pdf
// thanks, andy!

use std::f32::consts;

use std::simd::f32x4;


pub struct SVFSimper {
    pub a1: f32x4,
    pub a2: f32x4,
    pub a3: f32x4,

    pub ic1eq: f32x4,
    pub ic2eq: f32x4
}

impl SVFSimper {
    pub fn new(cutoff: f32, resonance: f32, sample_rate: f32) -> Self {
        let g = (consts::PI * (cutoff / sample_rate)).tan();
        let k = 2f32 - (1.9f32 * resonance.min(1f32).max(0f32));

        let a1 = 1.0 / (1.0 + (g * (g + k)));
        let a2 = g * a1;
        let a3 = g * a2;

        SVFSimper {
            a1: f32x4::splat(a1),
            a2: f32x4::splat(a2),
            a3: f32x4::splat(a3),

            ic1eq: f32x4::splat(0.0),
            ic2eq: f32x4::splat(0.0)
        }
    }

    pub fn set(&mut self, cutoff: f32, resonance: f32, sample_rate: f32) {
        let new = Self::new(cutoff, resonance, sample_rate);

        self.a1 = new.a1;
        self.a2 = new.a2;
        self.a3 = new.a3;
    }

    #[inline]
    pub fn process(&mut self, v0: f32x4) -> f32x4 {
        let v3 = v0 - self.ic2eq;
        let v1 = (self.a1 * self.ic1eq) + (self.a2 * v3);
        let v2 = self.ic2eq + (self.a2 * self.ic1eq) + (self.a3 * v3);

        self.ic1eq = (f32x4::splat(2.0) * v1) - self.ic1eq;
        self.ic2eq = (f32x4::splat(2.0) * v2) - self.ic2eq;

        v2
    }
}
