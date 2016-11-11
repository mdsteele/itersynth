extern crate itersynth;
#[macro_use]
extern crate nom;
extern crate sdl2;

use itersynth::{Wave, WaveGen};
use std::str::{self, FromStr};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};

// ========================================================================= //

pub enum WaveOp {
    Add(Wave),
    Adshr(f32, f32, f32, f32, f32),
    Delayed(f32),
    Looped,
    Mul(Wave),
}

impl WaveOp {
    fn apply(self, wave: Wave) -> Wave {
        match self {
            WaveOp::Add(other) => wave + other,
            WaveOp::Adshr(a, d, s, h, r) => wave.adshr(a, d, s, h, r),
            WaveOp::Delayed(time) => wave.delayed(time),
            WaveOp::Looped => wave.looped(),
            WaveOp::Mul(other) => wave * other,
        }
    }
}

// ========================================================================= //

named!(any_wave<Wave>,
       map!(pair!(base_wave, many0!(wave_suffix)),
            |(wave, ops): (Wave, Vec<WaveOp>)| {
                ops.into_iter().fold(wave, |wave, op| op.apply(wave))
            }));

named!(base_wave<Wave>,
       alt!(const_wave | noise_wave | product_wave | pulse_wave | sine_wave |
            slide_wave | sum_wave | triangle_wave));

named!(const_wave<Wave>, map!(float_literal, Into::into));

named!(noise_wave<Wave>,
       map!(preceded!(tag!("noise"),
                      delimited!(char!('('),
                                 any_wave,
                                 char!(')'))),
            Wave::noise));

named!(product_wave<Wave>,
       map!(preceded!(tag!("mul"),
                      delimited!(char!('('),
                                 separated_pair!(any_wave,
                                                 char!(','),
                                                 any_wave),
                                 char!(')'))),
            |(wave1, wave2)| wave1 * wave2));

named!(pulse_wave<Wave>,
       map!(preceded!(tag!("pulse"),
                      delimited!(char!('('),
                                 separated_pair!(any_wave,
                                                 char!(','),
                                                 any_wave),
                                 char!(')'))),
            |(freq, duty)| Wave::pulse(freq, duty)));

named!(sine_wave<Wave>,
       map!(preceded!(tag!("sine"),
                      delimited!(char!('('),
                                 any_wave,
                                 char!(')'))),
            Wave::sine));

named!(slide_wave<Wave>,
       map!(preceded!(tag!("slide"),
                      delimited!(char!('('),
                                 separated_pair!(
                                     separated_pair!(float_literal,
                                                     char!(','),
                                                     float_literal),
                                     char!(','),
                                     float_literal),
                                 char!(')'))),
            |((pos, vel), acc)| Wave::slide(pos, vel, acc)));

named!(sum_wave<Wave>,
       map!(preceded!(tag!("add"),
                      delimited!(char!('('),
                                 separated_pair!(any_wave,
                                                 char!(','),
                                                 any_wave),
                                 char!(')'))),
            |(wave1, wave2)| wave1 + wave2));

named!(triangle_wave<Wave>,
       map!(preceded!(tag!("triangle"),
                      delimited!(char!('('),
                                 separated_pair!(any_wave,
                                                 char!(','),
                                                 any_wave),
                                 char!(')'))),
            |(freq, duty)| Wave::triangle(freq, duty)));

// ========================================================================= //

named!(wave_suffix<WaveOp>,
       alt!(add_suffix | adshr_suffix | delayed_suffix | looped_suffix |
            mul_suffix));

named!(add_suffix<WaveOp>,
       map!(preceded!(tag!(".add"),
                      delimited!(char!('('),
                                 any_wave,
                                 char!(')'))),
            WaveOp::Add));

named!(adshr_suffix<WaveOp>,
       map!(preceded!(tag!(".adshr"),
                      delimited!(char!('('),
                                 separated_pair!(
                                     separated_pair!(float_literal,
                                                     char!(','),
                                                     float_literal),
                                     char!(','),
                                     separated_pair!(
                                         separated_pair!(float_literal,
                                                         char!(','),
                                                         float_literal),
                                         char!(','),
                                         float_literal)),
                                 char!(')'))),
            |((a, d), ((s, h), r))| WaveOp::Adshr(a, d, s, h, r)));

named!(delayed_suffix<WaveOp>,
       map!(preceded!(tag!(".delayed"),
                      delimited!(char!('('),
                                 float_literal,
                                 char!(')'))),
            WaveOp::Delayed));

named!(looped_suffix<WaveOp>,
       value!(WaveOp::Looped, tag!(".looped()")));

named!(mul_suffix<WaveOp>,
       map!(preceded!(tag!(".mul"),
                      delimited!(char!('('),
                                 any_wave,
                                 char!(')'))),
            WaveOp::Mul));

// ========================================================================= //

named!(float_literal<f32>,
       map_res!(map_res!(recognize!(pair!(pair!(opt!(char!('-')),
                                                nom::digit),
                                          opt!(pair!(char!('.'),
                                                     nom::digit)))),
                         str::from_utf8),
                FromStr::from_str));

// ========================================================================= //

struct WaveCallback {
    wave: itersynth::Wave,
    step: f32,
    notification: Arc<(Mutex<bool>, Condvar)>,
}

impl WaveCallback {
    fn new(wave: itersynth::Wave,
           audio_rate: i32,
           notification: Arc<(Mutex<bool>, Condvar)>)
           -> WaveCallback {
        WaveCallback {
            wave: wave,
            step: 1.0 / audio_rate as f32,
            notification: notification,
        }
    }
}

impl sdl2::audio::AudioCallback for WaveCallback {
    type Channel = itersynth::Sample;

    fn callback(&mut self, out: &mut [itersynth::Sample]) {
        let mut done = false;
        for sample in out.iter_mut() {
            *sample = match self.wave.next(self.step) {
                Some(value) => value,
                None => {
                    done = true;
                    0.0
                }
            };
        }
        if done {
            // Signal that the sound is complete.
            let &(ref lock, ref cvar) = &*self.notification;
            let mut done_guard: MutexGuard<bool> = lock.lock().unwrap();
            *done_guard = true;
            cvar.notify_all();
        }
    }
}

// ========================================================================= //

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let spec: &[u8] = if args.len() >= 2 {
        args[1].as_bytes()
    } else {
        b"sine(440)"
    };
    let wave = match any_wave(spec) {
        nom::IResult::Done(rest, ref wave) if rest.is_empty() => wave.clone(),
        _ => {
            println!("Failed to parse spec.");
            return;
        }
    };

    let notification = Arc::new((Mutex::new(false), Condvar::new()));

    let sdl_context = sdl2::init().unwrap();
    let audio_subsystem = sdl_context.audio().unwrap();
    let desired_spec = sdl2::audio::AudioSpecDesired {
        freq: Some(44100),
        channels: Some(1), // mono
        samples: None, // default sample size
    };
    let device = audio_subsystem.open_playback(None, &desired_spec, |spec| {
                                    WaveCallback::new(wave,
                                                      spec.freq,
                                                      notification.clone())
                                })
                                .unwrap();
    device.resume();

    // Wait for the sound to complete.
    let &(ref lock, ref cvar) = &*notification;
    let mut done_guard: MutexGuard<bool> = lock.lock().unwrap();
    while !*done_guard {
        done_guard = cvar.wait(done_guard).unwrap();
    }
}

// ========================================================================= //
