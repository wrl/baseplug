use std::fmt;
use std::ops;
use std::slice;
use std::sync::Arc;

use num_traits::Float;

use crate::AtomicFloat;

const SETTLE: f32 = 0.00001f32;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum SmoothStatus {
    Inactive,
    Active,
    Deactivating
}

impl SmoothStatus {
    #[inline]
    fn is_active(&self) -> bool {
        self != &SmoothStatus::Inactive
    }
}

pub struct SmoothOutput<'a, T> {
    pub values: &'a [T],
    pub status: SmoothStatus
}

impl<'a, T> SmoothOutput<'a, T> {
    #[inline]
    pub fn is_smoothing(&self) -> bool {
        self.status.is_active()
    }
}

impl<'a, T, I> ops::Index<I> for SmoothOutput<'a, T>
    where I: slice::SliceIndex<[T]>
{
    type Output = I::Output;

    #[inline]
    fn index(&self, idx: I) -> &I::Output {
        &self.values[idx]
    }
}

pub struct UIShared {
    shared: Arc<AtomicFloat>,
}

impl UIShared {
    pub fn new(value: f32) -> Self {
        Self {
            shared: Arc::new(AtomicFloat::new(value))
        }
    }

    #[inline]
    pub fn get(&self) -> f32 {
        self.shared.get()
    }

    #[inline]
    pub fn set(&self, value: f32) {
        self.shared.set(value);
    }

    #[inline]
    pub fn clone(&self) -> UIShared {
        UIShared { shared: Arc::clone(&self.shared) }
    }
}

pub struct SmoothParam {
    ui_shared_value: UIShared,
    smooth: Smooth<f32>,
}

impl SmoothParam {
    pub fn new(input: f32) -> Self {
        Self {
            ui_shared_value: UIShared::new(input),
            smooth: Smooth::new(input),
        }
    }

    #[inline]
    pub fn reset(&mut self, val: f32) {
        self.ui_shared_value.set(val);
        self.reset(val);
    }

    #[inline]
    pub fn set(&mut self, val: f32) {
        self.ui_shared_value.set(val);
        self.set(val);
    }

    #[inline]
    pub fn dest(&self) -> f32 {
        self.smooth.dest()
    }

    #[inline]
    pub fn output(&self) -> SmoothOutput<f32> {
        self.smooth.output()
    }

    #[inline]
    pub fn current_value(&self) -> SmoothOutput<f32> {
        self.smooth.current_value()
    }

    #[inline]
    pub fn update_status_with_epsilon(&mut self, epsilon: f32) -> SmoothStatus {
        self.smooth.update_status_with_epsilon(epsilon)
    }

    pub fn process(&mut self, nframes: usize) {
        // Check for updated value from UI.
        let value = self.ui_shared_value.get();
        if self.smooth.dest() != value {
            self.smooth.set(value);
        }

        self.smooth.process(nframes);
    }

    #[inline]
    pub fn is_active(&self) -> bool {
        self.smooth.is_active()
    }

    #[inline]
    pub fn set_speed_ms(&mut self, sample_rate: f32, ms: f32) {
        self.smooth.set_speed_ms(sample_rate, ms)
    }

    #[inline]
    pub fn update_status(&mut self) -> SmoothStatus {
        self.smooth.update_status()
    }

    #[inline]
    pub fn ui_shared(&self) -> UIShared {
        self.ui_shared_value.clone()
    }
}

pub struct Smooth<T: Float> {
    output: [T; crate::MAX_BLOCKSIZE],
    input: T,

    status: SmoothStatus,

    a: T,
    b: T,
    last_output: T
}

impl<T> Smooth<T>
    where T: Float + fmt::Display
{
    pub fn new(input: T) -> Self {
        Self {
            status: SmoothStatus::Inactive,
            input,
            output: [input; crate::MAX_BLOCKSIZE],

            a: T::one(),
            b: T::zero(),
            last_output: input
        }
    }

    pub fn reset(&mut self, val: T)
    {
        *self = Self {
            a: self.a,
            b: self.b,

            ..Self::new(val)
        };
    }

    pub fn set(&mut self, val: T) {
        self.input = val;
        self.status = SmoothStatus::Active;
    }

    #[inline]
    pub fn dest(&self) -> T {
        self.input
    }

    #[inline]
    pub fn output(&self) -> SmoothOutput<T> {
        SmoothOutput {
            values: &self.output,
            status: self.status
        }
    }

    #[inline]
    pub fn current_value(&self) -> SmoothOutput<T> {
        SmoothOutput {
            values: slice::from_ref(&self.last_output),
            status: self.status
        }
    }

    pub fn update_status_with_epsilon(&mut self, epsilon: T) -> SmoothStatus {
        let status = self.status;

        match status {
            SmoothStatus::Active => {
                if (self.input - self.output[0]).abs() < epsilon {
                    self.reset(self.input);
                    self.status = SmoothStatus::Deactivating;
                }
            },

            SmoothStatus::Deactivating =>
                self.status = SmoothStatus::Inactive,

            _ => ()
        };

        self.status
    }

    pub fn process(&mut self, nframes: usize) {
        if self.status != SmoothStatus::Active {
            return
        }

        let nframes = nframes.min(crate::MAX_BLOCKSIZE);
        let input = self.input * self.a;

        self.output[0] = input + (self.last_output * self.b);

        for i in 1..nframes {
            self.output[i] = input + (self.output[i - 1] * self.b);
        }

        self.last_output = self.output[nframes - 1];
    }

    #[inline]
    pub fn is_active(&self) -> bool {
        self.status.is_active()
    }
}

impl Smooth<f32> {
    pub fn set_speed_ms(&mut self, sample_rate: f32, ms: f32) {
        self.b = (-1.0f32 / (ms * (sample_rate / 1000.0f32))).exp();
        self.a = 1.0f32 - self.b;
    }

    #[inline]
    pub fn update_status(&mut self) -> SmoothStatus {
        self.update_status_with_epsilon(SETTLE)
    }
}

impl<T> From<T> for Smooth<T>
    where T: Float + fmt::Display
{
    fn from(val: T) -> Self {
        Self::new(val)
    }
}

impl<T, I> ops::Index<I> for Smooth<T>
    where I: slice::SliceIndex<[T]>,
          T: Float
{
    type Output = I::Output;

    #[inline]
    fn index(&self, idx: I) -> &I::Output {
        &self.output[idx]
    }
}

impl<T> fmt::Debug for Smooth<T>
    where T: Float + fmt::Debug
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct(concat!("Smooth<", stringify!(T), ">"))
            .field("output[0]", &self.output[0])
            .field("input", &self.input)
            .field("status", &self.status)
            .field("last_output", &self.last_output)
            .finish()
    }
}
