use std::fmt;

use crate::{
    Smooth,
    SmoothStatus
};

const DECLICK_SETTLE: f32 = 0.001;

pub struct DeclickOutput<'a, T> {
    pub from: &'a T,
    pub to: &'a T,

    pub fade: &'a [f32],
    pub status: SmoothStatus
}

pub struct Declick<T: Sized + Clone> {
    current: T,
    next: Option<T>,
    staged: Option<T>,

    fade: Smooth<f32>
}

impl<T> Declick<T>
    where T: Sized + Clone + Eq
{
    pub fn new(initial: T) -> Self {
        Self {
            current: initial,
            next: None,
            staged: None,

            fade: Smooth::new(0.0)
        }
    }

    pub fn reset(&mut self, to: T) {
        self.current = to;
        self.next = None;
        self.staged = None;

        self.fade.reset(0.0);
    }

    pub fn set(&mut self, to: T) {
        if self.dest() == &to {
            return
        }

        if self.next.is_none() {
            self.next = Some(to);

            self.fade.reset(0.0);
            self.fade.set(1.0);
        } else {
            self.staged = Some(to);
        }
    }

    pub fn set_speed_ms(&mut self, sample_rate: f32, ms: f32) {
        self.fade.set_speed_ms(sample_rate, ms);
    }

    #[inline]
    pub fn output(&self) -> DeclickOutput<T> {
        let fade = self.fade.output();

        DeclickOutput {
            from: &self.current,
            to: self.next.as_ref().unwrap_or(&self.current),

            fade: fade.values,
            status: fade.status
        }
    }

    #[inline]
    pub fn current_value(&self) -> DeclickOutput<T> {
        let fade = self.fade.current_value();

        DeclickOutput {
            from: &self.current,
            to: self.next.as_ref().unwrap_or(&self.current),

            fade: fade.values,
            status: fade.status
        }
    }

    #[inline]
    pub fn dest(&self) -> &T {
        self.staged.as_ref()
            .or_else(|| self.next.as_ref())
            .unwrap_or(&self.current)
    }

    #[inline]
    pub fn is_active(&self) -> bool {
        self.next.is_some()
    }

    #[inline]
    pub fn process(&mut self, nframes: usize) {
        self.update_status();
        self.fade.process(nframes);
    }

    pub fn update_status(&mut self) {
        if !self.is_active() {
            return;
        }

        self.fade.update_status_with_epsilon(DECLICK_SETTLE);

        if self.fade.is_active() {
            return;
        }

        self.current = self.next.take().unwrap();
        self.next = self.staged.take();
    }
}

impl<T> fmt::Debug for Declick<T>
    where T: fmt::Debug + Sized + Clone
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct(concat!("Declick<", stringify!(T), ">"))
            .field("current", &self.current)
            .field("next", &self.next)
            .field("staged", &self.staged)
            .field("fade", &self.fade)
            .finish()
    }
}

#[cfg(test)]
mod test{
    use super::*;

    #[test]
    fn reset_test() {
        let mut declick = Declick::new(0 as isize);
        let declick_expected = Declick::new(1 as isize);
        declick.reset(1);
        assert!(cmp_output(&declick.output(), &declick_expected.output()));
    }

    #[test]
    fn set_same_test() {
        let mut declick = Declick::new(0 as isize);
        let declick_expected = Declick::new(0 as isize);
        declick.set(0);
        assert!(cmp_output(&declick.output(), &declick_expected.output()));
    }

    #[test]
    fn set_new_value_test() {
        let mut declick = Declick::new(0 as isize);
        let mut declick_expected = Declick::new(0 as isize);
        declick_expected.next = Some(1);
        let mut declick_expected_output = declick_expected.output();
        declick_expected_output.status = SmoothStatus::Active;
        declick.set(1);
        assert!(cmp_output(&declick.output(), &declick_expected_output));
    }

    #[test]
    fn set_new_value_next_exists_test() {
        let mut declick = Declick::new(0 as isize);
        let mut declick_expected = Declick::new(0 as isize);
        declick_expected.next = Some(1);
        let mut declick_expected_output = declick_expected.output();
        declick_expected_output.status = SmoothStatus::Active;
        declick.set(1);
        declick.set(2);
        assert!(cmp_output(&declick.output(), &declick_expected_output));
    }

    #[test]
    fn process_no_next_test() {
        let mut declick = Declick::new(0 as isize);
        let declick_expected = Declick::new(0 as isize);
        declick.process(64);
        assert!(cmp_output(&declick.output(), &declick_expected.output()));
    }

    #[test]
    fn process_next_fading_active_test() {
        let mut declick = Declick::new(0 as isize);
        let mut declick_expected = Declick::new(0 as isize);
        declick_expected.next = Some(1);
        let mut declick_expected_output = declick_expected.output();
        declick_expected_output.status = SmoothStatus::Active;
        let mut fade = [0.0; 128];
        fade[0] = 1.0;
        declick_expected_output.fade = &fade;
        declick.set(1);
        declick.process(1);

        assert!(cmp_output(&declick.output(), &declick_expected_output));
    }

    #[test]
    fn process_next_fading_finished_test() {
        let mut declick = Declick::new(0 as isize);
        let mut declick_expected = Declick::new(0 as isize);
        declick_expected.current = 1;
        declick_expected.next = Some(1);
        let mut declick_expected_output = declick_expected.output();
        declick_expected_output.status = SmoothStatus::Inactive;
        let mut fade = [1.0; 128];
        fade[0] = 1.0;
        declick_expected_output.fade = &fade;
        declick.set(1);
        // We must call process 3 times before current, next and staged are updated.
        declick.process(1);
        declick.process(1);
        declick.process(1);

        assert!(cmp_output(&declick.output(), &declick_expected_output));
    }

    #[test]
    fn process_next_staged_fading_finished_test() {
        let mut declick = Declick::new(0 as isize);
        let mut declick_expected = Declick::new(0 as isize);
        declick_expected.current = 1;
        declick_expected.next = Some(2);
        let mut declick_expected_output = declick_expected.output();
        declick_expected_output.status = SmoothStatus::Inactive;
        let mut fade = [1.0; 128];
        fade[0] = 1.0;
        declick_expected_output.fade = &fade;
        declick.set(1);
        declick.set(2);
        // We must call process 3 times before current, next and staged are updated.
        declick.process(1);
        declick.process(1);
        declick.process(1);

        assert!(cmp_output(&declick.output(), &declick_expected_output));
    }

    fn cmp_output(output: &DeclickOutput<isize>, output_expected: &DeclickOutput<isize>) -> bool {
        output.from == output_expected.from &&
        output.to == output_expected.to &&
        output.status == output_expected.status &&
        output.fade == output_expected.fade
    }
}