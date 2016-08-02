//! An iterator-based sound synthesizer library.

#![warn(missing_docs)]

use std::f32::consts::PI;
use std::ops::{Add, Mul};

/// One sample value from a waveform.
///
/// When a `Sample` is used as a [`Wave`](trait.Wave.html), it generates a
/// constant value forever.
pub type Sample = f32;

/// A waveform generator.
pub trait WaveGen: Send {
    /// Gets the next sample value, or returns `None` if the waveform has
    /// finished.  The `sample_rate` gives the number of samples per second.
    fn next(&mut self, sample_rate: f32) -> Option<Sample>;

    /// Resets the waveform back to the beginning.
    fn reset(&mut self);
}

impl WaveGen for Sample {
    fn next(&mut self, _: f32) -> Option<Sample> {
        Some(*self)
    }

    fn reset(&mut self) {}
}

/// A sequence of sample values, forming a waveform.
///
/// Normally the samples will represent sound amplitude values, but they can
/// also represent frequencies or other values, depending on context.
pub struct Wave {
    generator: Box<WaveGen>,
}

impl Wave {
    /// Creates a sine wave, with an amplitude of 1, whose frequency over time
    /// is controlled by the input waveform (which may be a constant).  The
    /// input frequency values are measured in hertz (cycles per second).
    pub fn sine<F: Into<Wave>>(freq: F) -> Wave {
        Wave::new(Box::new(SineWave::new(freq.into())))
    }

    /// Creates a pulse wave whose frequency over time is controlled by the
    /// `freq` waveform, and whose duty cycle over time is controlled by the
    /// `duty` waveform (either or both of which may be constants).  The input
    /// frequency values are measured in hertz (cycles per second); the input
    /// duty values should be between 0 and 1 (with 0.5 being a square wave).
    pub fn pulse<F: Into<Wave>, D: Into<Wave>>(freq: F, duty: D) -> Wave {
        Wave::new(Box::new(PulseWave::new(freq.into(), duty.into())))
    }

    /// Creates a waveform using the given generator.
    pub fn new(generator: Box<WaveGen>) -> Wave {
        Wave { generator: generator }
    }

    /// Returns a new waveform that repeats this one forever.
    pub fn looped(self) -> Wave {
        Wave::new(Box::new(Looped { wave: self }))
    }
}

impl<W: Into<Wave>> Add<W> for Wave {
    type Output = Wave;

    fn add(self, rhs: W) -> Wave {
        Wave::new(Box::new(Sum {
            wave1: self,
            wave2: rhs.into(),
        }))
    }
}

impl From<Sample> for Wave {
    fn from(sample: Sample) -> Wave {
        Wave::new(Box::new(sample))
    }
}

impl<W: Into<Wave>> Mul<W> for Wave {
    type Output = Wave;

    fn mul(self, rhs: W) -> Wave {
        Wave::new(Box::new(Product {
            wave1: self,
            wave2: rhs.into(),
        }))
    }
}

impl WaveGen for Wave {
    fn next(&mut self, sample_rate: f32) -> Option<Sample> {
        self.generator.next(sample_rate)
    }

    fn reset(&mut self) {
        self.generator.reset();
    }
}

/// A waveform consisting of some other waveform, repeated indefinitely.
struct Looped {
    wave: Wave,
}

impl WaveGen for Looped {
    fn next(&mut self, sample_rate: f32) -> Option<Sample> {
        self.wave.next(sample_rate).or_else(|| {
            self.wave.reset();
            self.wave.next(sample_rate)
        })
    }

    fn reset(&mut self) {
        self.wave.reset();
    }
}

/// A waveform consisting of the product of two other waveforms.
struct Product {
    wave1: Wave,
    wave2: Wave,
}

impl WaveGen for Product {
    fn next(&mut self, sample_rate: f32) -> Option<Sample> {
        match self.wave1.next(sample_rate) {
            Some(value1) => {
                match self.wave2.next(sample_rate) {
                    Some(value2) => Some(value1 * value2),
                    None => None,
                }
            }
            None => None,
        }
    }

    fn reset(&mut self) {
        self.wave1.reset();
        self.wave2.reset();
    }
}

/// A variable-frequency, variable-duty pulse wave, with an amplitude of 1.
struct PulseWave {
    freq: Wave,
    duty: Wave,
    phase: f32,
}

impl PulseWave {
    fn new(freq: Wave, duty: Wave) -> PulseWave {
        PulseWave {
            freq: freq,
            duty: duty,
            phase: 0.0,
        }
    }
}

impl WaveGen for PulseWave {
    fn next(&mut self, audio_rate: f32) -> Option<Sample> {
        let freq = match self.freq.next(audio_rate) {
            Some(freq) => freq,
            None => return None,
        };
        let duty = match self.duty.next(audio_rate) {
            Some(duty) => duty,
            None => return None,
        };
        let phase = self.phase;
        self.phase = (self.phase + freq / audio_rate) % 1.0;
        Some(if phase < duty {
            1.0
        } else {
            -1.0
        })
    }

    fn reset(&mut self) {
        self.freq.reset();
        self.duty.reset();
        self.phase = 0.0;
    }
}

/// A variable-frequency sine wave, with an amplitude of 1.
struct SineWave {
    freq: Wave,
    phase: f32,
}

impl SineWave {
    fn new(freq: Wave) -> SineWave {
        SineWave {
            freq: freq,
            phase: 0.0,
        }
    }
}

impl WaveGen for SineWave {
    fn next(&mut self, sample_rate: f32) -> Option<Sample> {
        let freq = match self.freq.next(sample_rate) {
            Some(freq) => freq,
            None => return None,
        };
        let phase = self.phase;
        self.phase = (self.phase + freq / sample_rate) % 1.0;
        Some((2.0 * PI * phase).sin())
    }

    fn reset(&mut self) {
        self.freq.reset();
        self.phase = 0.0;
    }
}

/// A waveform consisting of the sum of two other waveforms.
struct Sum {
    wave1: Wave,
    wave2: Wave,
}

impl WaveGen for Sum {
    fn next(&mut self, sample_rate: f32) -> Option<Sample> {
        match self.wave1.next(sample_rate) {
            Some(value1) => {
                Some(match self.wave2.next(sample_rate) {
                    Some(value2) => value1 + value2,
                    None => value1,
                })
            }
            None => self.wave2.next(sample_rate),
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
        let sample_rate = 22050.0;
        let mut wave = Wave::sine(2756.25);
        assert_approx!(0.0, wave.next(sample_rate).unwrap());
        assert_approx!(0.5 * SQRT_2, wave.next(sample_rate).unwrap());
        assert_approx!(1.0, wave.next(sample_rate).unwrap());
        assert_approx!(0.5 * SQRT_2, wave.next(sample_rate).unwrap());
        assert_approx!(0.0, wave.next(sample_rate).unwrap());
        assert_approx!(-0.5 * SQRT_2, wave.next(sample_rate).unwrap());
        assert_approx!(-1.0, wave.next(sample_rate).unwrap());
        assert_approx!(-0.5 * SQRT_2, wave.next(sample_rate).unwrap());
        assert_approx!(0.0, wave.next(sample_rate).unwrap());
        wave.reset();
        assert_approx!(0.0, wave.next(sample_rate).unwrap());
        assert_approx!(0.5 * SQRT_2, wave.next(sample_rate).unwrap());
        assert_approx!(1.0, wave.next(sample_rate).unwrap());
    }

    #[test]
    fn wave_sum() {
        let mut wave = Wave::sine(0.25) + 1.5;
        assert_approx!(1.5, wave.next(1.0).unwrap());
        assert_approx!(2.5, wave.next(1.0).unwrap());
        assert_approx!(1.5, wave.next(1.0).unwrap());
        assert_approx!(0.5, wave.next(1.0).unwrap());
    }
}
