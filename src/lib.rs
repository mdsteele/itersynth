//! An iterator-based sound synthesizer library.

#![warn(missing_docs)]

use std::f32::consts::PI;

/// One sample value from a waveform.
///
/// When a `Sample` is used as a [`Wave`](trait.Wave.html), it generates a
/// constant value forever.
pub type Sample = f32;

impl Wave for Sample {
    fn next(&mut self) -> Option<Sample> {
        Some(*self)
    }

    fn reset(&mut self) {}
}

/// A sequence of sample values, forming a waveform.
///
/// Normally the samples will represent sound amplitude values, but they can
/// also represent frequencies or other values, depending on context.
pub trait Wave: Clone + Send {
    /// Gets the next sample value, or returns `None` if the waveform has
    /// finished.
    fn next(&mut self) -> Option<Sample>;

    /// Resets the waveform back to the beginning.
    fn reset(&mut self);

    /// Returns a new waveform that repeats this one forever.
    fn looped(self) -> Looped<Self> {
        Looped { wave: self }
    }

    /// Sums two waveforms together; the new waveform ends only when both input
    /// waveforms have ended.
    fn add<W: Wave>(self, other: W) -> Sum<Self, W> {
        Sum {
            wave1: self,
            wave2: other,
        }
    }
}

/// A waveform consisting of some other waveform, repeated indefinitely.
#[derive(Clone)]
pub struct Looped<W> {
    wave: W,
}

impl<W: Wave> Wave for Looped<W> {
    fn next(&mut self) -> Option<Sample> {
        self.wave.next().or_else(|| {
            self.wave.reset();
            self.wave.next()
        })
    }

    fn reset(&mut self) {
        self.wave.reset();
    }
}

/// A variable-frequency sine wave, with an amplitude of 1.
#[derive(Clone)]
pub struct SineWave<F> {
    freq: F,
    phase: f32,
}

impl<F> SineWave<F> {
    /// Creates a sine wave whose frequency over time is controlled by the
    /// input waveform (which may be a constant).  The input frequency values
    /// are measured in cycles per sample; to convert from hertz (cycles per
    /// second), divide by the audio rate (samples per second) being used.
    pub fn new(freq: F) -> SineWave<F> {
        SineWave {
            freq: freq,
            phase: 0.0,
        }
    }
}

impl<F: Wave> Wave for SineWave<F> {
    fn next(&mut self) -> Option<Sample> {
        let freq = match self.freq.next() {
            Some(freq) => freq,
            None => return None,
        };
        let phase = self.phase;
        self.phase = (self.phase + freq) % 1.0;
        Some((2.0 * PI * phase).sin())
    }

    fn reset(&mut self) {
        self.freq.reset();
        self.phase = 0.0;
    }
}

/// A waveform consisting of the sum of two other waveforms.
#[derive(Clone)]
pub struct Sum<W, V> {
    wave1: W,
    wave2: V,
}

impl<W: Wave, V: Wave> Wave for Sum<W, V> {
    fn next(&mut self) -> Option<Sample> {
        match self.wave1.next() {
            Some(value1) => {
                Some(match self.wave2.next() {
                    Some(value2) => value1 + value2,
                    None => value1,
                })
            }
            None => self.wave2.next(),
        }
    }

    fn reset(&mut self) {
        self.wave1.reset();
        self.wave2.reset();
    }
}

#[cfg(test)]
mod tests {
    use std::f32::consts::SQRT_2;
    use super::*;

    macro_rules! assert_approx {
        ($left:expr, $right:expr) => ({
            match (&($left), &($right)) {
                (left_val, right_val) => {
                    if (*left_val - *right_val).abs() > 1e-6 {
                        panic!("assertion failed: `(left ~= right)` \
                                (left: `{:?}`, right: `{:?}`)",
                               left_val, right_val)
                    }
                }
            }
        })
    }

    #[test]
    fn sine_wave() {
        let mut wave = SineWave::new(0.125);
        assert_approx!(0.0, wave.next().unwrap());
        assert_approx!(0.5 * SQRT_2, wave.next().unwrap());
        assert_approx!(1.0, wave.next().unwrap());
        assert_approx!(0.5 * SQRT_2, wave.next().unwrap());
        assert_approx!(0.0, wave.next().unwrap());
        assert_approx!(-0.5 * SQRT_2, wave.next().unwrap());
        assert_approx!(-1.0, wave.next().unwrap());
        assert_approx!(-0.5 * SQRT_2, wave.next().unwrap());
        assert_approx!(0.0, wave.next().unwrap());
        wave.reset();
        assert_approx!(0.0, wave.next().unwrap());
        assert_approx!(0.5 * SQRT_2, wave.next().unwrap());
        assert_approx!(1.0, wave.next().unwrap());
    }

    #[test]
    fn wave_sum() {
        let mut wave = SineWave::new(0.25).add(1.5);
        assert_approx!(1.5, wave.next().unwrap());
        assert_approx!(2.5, wave.next().unwrap());
        assert_approx!(1.5, wave.next().unwrap());
        assert_approx!(0.5, wave.next().unwrap());
    }
}
