extern crate itersynth;
#[macro_use]
extern crate nom;
extern crate sdl2;

use itersynth::{Wave, WaveGen};
use std::str::{self, FromStr};
use std::time::Duration;

// ========================================================================= //

named!(any_wave<Wave>,
       alt!(const_wave | noise_wave | product_wave | pulse_wave | sine_wave |
            sum_wave | triangle_wave));

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

named!(float_literal<f32>,
       map_res!(map_res!(recognize!(pair!(nom::digit,
                                          opt!(pair!(char!('.'),
                                                     nom::digit)))),
                         str::from_utf8),
                FromStr::from_str));

// ========================================================================= //

struct WaveCallback {
    wave: itersynth::Wave,
    audio_rate: i32,
}

impl WaveCallback {
    fn new(wave: itersynth::Wave, audio_rate: i32) -> WaveCallback {
        WaveCallback {
            wave: wave,
            audio_rate: audio_rate,
        }
    }
}

impl sdl2::audio::AudioCallback for WaveCallback {
    type Channel = itersynth::Sample;

    fn callback(&mut self, out: &mut [itersynth::Sample]) {
        let audio_rate = self.audio_rate as f32;
        for sample in out.iter_mut() {
            *sample = self.wave.next(audio_rate).unwrap_or(0.0);
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
        nom::IResult::Done(_, wave) => wave,
        _ => {
            println!("Failed to parse spec.");
            return;
        }
    };

    let sdl_context = sdl2::init().unwrap();
    let audio_subsystem = sdl_context.audio().unwrap();
    let desired_spec = sdl2::audio::AudioSpecDesired {
        freq: Some(44100),
        channels: Some(1), // mono
        samples: None, // default sample size
    };
    let device = audio_subsystem.open_playback(None, &desired_spec, |spec| {
                                    WaveCallback::new(wave, spec.freq)
                                })
                                .unwrap();
    device.resume();
    std::thread::sleep(Duration::from_millis(2000));
}

// ========================================================================= //
