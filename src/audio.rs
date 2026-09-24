//! A tiny SDL_mixer work-alike on top of the SDL2 audio device: 8 sound channels plus one music
//! stream, 22050 Hz / S16 / stereo, volumes in 0..=128 mixed with SDL_MixAudioFormat's formula.

use sdl2::AudioSubsystem;
use sdl2::audio::{AudioCVT, AudioCallback, AudioDevice, AudioFormat, AudioSpecDesired};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex};

pub const FREQ: i32 = 22050;
pub const CHANNELS: u8 = 2;
const MIX_CHANNELS: usize = 8;
pub const MAX_VOLUME: i32 = 128;

/// A loaded sound (Mix_Chunk): interleaved S16 stereo at FREQ.
pub struct Chunk {
    data: Vec<i16>,
    volume: AtomicI32,
}

impl Chunk {
    /// Sound.set_volume(v): Mix_VolumeChunk(chunk, int(v * 128))
    pub fn set_volume(&self, v: f64) {
        self.volume.store(((v * MAX_VOLUME as f64) as i32).clamp(0, MAX_VOLUME), Ordering::Relaxed);
    }
}

struct Music {
    data: Arc<Vec<i16>>,
    pos: usize,
}

struct MixState {
    channels: [Option<(Arc<Chunk>, usize)>; MIX_CHANNELS],
    music: Option<Music>,
    music_volume: i32,
}

struct Callback {
    state: Arc<Mutex<MixState>>,
}

#[inline]
fn mix_into(dst: &mut [i16], src: &[i16], volume: i32) {
    if volume == 0 {
        return;
    }
    for (d, s) in dst.iter_mut().zip(src.iter()) {
        let s1 = (*s as i32 * volume) / MAX_VOLUME;
        *d = (s1 + *d as i32).clamp(-32768, 32767) as i16;
    }
}

impl AudioCallback for Callback {
    type Channel = i16;
    fn callback(&mut self, out: &mut [i16]) {
        out.fill(0);
        let Ok(mut st) = self.state.lock() else { return };
        let mv = st.music_volume;
        if let Some(m) = st.music.as_mut() {
            // looping music
            let mut done = 0;
            while done < out.len() && !m.data.is_empty() {
                let n = (out.len() - done).min(m.data.len() - m.pos);
                mix_into(&mut out[done..done + n], &m.data[m.pos..m.pos + n], mv);
                done += n;
                m.pos += n;
                if m.pos >= m.data.len() {
                    m.pos = 0;
                }
            }
        }
        for ch in st.channels.iter_mut() {
            let mut finished = false;
            if let Some((chunk, pos)) = ch.as_mut() {
                let n = out.len().min(chunk.data.len() - *pos);
                let vol = chunk.volume.load(Ordering::Relaxed);
                mix_into(&mut out[..n], &chunk.data[*pos..*pos + n], vol);
                *pos += n;
                finished = *pos >= chunk.data.len();
            }
            if finished {
                *ch = None;
            }
        }
    }
}

pub struct Mixer {
    _device: AudioDevice<Callback>,
    state: Arc<Mutex<MixState>>,
}

fn convert(bytes: Vec<u8>, channels: u8, rate: i32) -> Option<Vec<i16>> {
    let out = if channels == CHANNELS && rate == FREQ {
        bytes
    } else {
        let cvt = AudioCVT::new(AudioFormat::S16LSB, channels, rate, AudioFormat::S16LSB, CHANNELS, FREQ).ok()?;
        cvt.convert(bytes)
    };
    Some(out.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]])).collect())
}

fn decode_ogg(data: &[u8]) -> Option<(Vec<u8>, u8, i32)> {
    let mut r = lewton::inside_ogg::OggStreamReader::new(std::io::Cursor::new(data)).ok()?;
    let ch = r.ident_hdr.audio_channels;
    let rate = r.ident_hdr.audio_sample_rate as i32;
    let mut bytes = Vec::new();
    while let Ok(Some(pkt)) = r.read_dec_packet_itl() {
        for s in pkt {
            bytes.extend_from_slice(&s.to_le_bytes());
        }
    }
    Some((bytes, ch, rate))
}

impl Mixer {
    pub fn open(audio: &AudioSubsystem) -> Option<Mixer> {
        let state = Arc::new(Mutex::new(MixState { channels: Default::default(), music: None, music_volume: MAX_VOLUME }));
        let desired = AudioSpecDesired { freq: Some(FREQ), channels: Some(CHANNELS), samples: Some(512) };
        let st = state.clone();
        let device = audio.open_playback(None, &desired, |_spec| Callback { state: st }).ok()?;
        device.resume();
        Some(Mixer { _device: device, state })
    }

    /// pygame.mixer.Sound(buffer=bytes) in the mixer's own format.
    pub fn chunk_from_samples(&self, data: Vec<i16>) -> Arc<Chunk> {
        Arc::new(Chunk { data, volume: AtomicI32::new(MAX_VOLUME) })
    }

    /// pygame.mixer.Sound(path) for an .ogg file.
    pub fn load_chunk(&self, data: &[u8]) -> Option<Arc<Chunk>> {
        let (bytes, ch, rate) = decode_ogg(data)?;
        let data = convert(bytes, ch, rate)?;
        Some(self.chunk_from_samples(data))
    }

    /// Sound.play(): first free channel, nothing if all 8 are busy.
    pub fn play(&self, chunk: &Arc<Chunk>) {
        if let Ok(mut st) = self.state.lock() {
            if let Some(slot) = st.channels.iter_mut().find(|c| c.is_none()) {
                *slot = Some((chunk.clone(), 0));
            }
        }
    }

    /// pygame.mixer.music.load(path) + play(-1)
    pub fn play_music_loop(&self, data: &[u8]) -> bool {
        let Some((bytes, ch, rate)) = decode_ogg(data) else { return false };
        let Some(data) = convert(bytes, ch, rate) else { return false };
        if let Ok(mut st) = self.state.lock() {
            st.music = Some(Music { data: Arc::new(data), pos: 0 });
            return true;
        }
        false
    }

    /// pygame.mixer.music.set_volume(v)
    pub fn set_music_volume(&self, v: f64) {
        if let Ok(mut st) = self.state.lock() {
            st.music_volume = ((v * MAX_VOLUME as f64) as i32).clamp(0, MAX_VOLUME);
        }
    }
}

/// make_tone_bytes: a simple beep / sweep as interleaved S16 samples.
pub fn make_tone(freq: f64, duration: f64, volume: f64, freq_end: Option<f64>) -> Vec<i16> {
    let sr = FREQ as f64;
    let n = 1.max((sr * duration) as i64);
    let amp = (32767.0 * volume) as i64;
    let fade = 1.max((sr * 0.015) as i64);
    let mut buf = Vec::with_capacity(n as usize * 2);
    for i in 0..n {
        let t = i as f64 / sr;
        let f = match freq_end {
            None => freq,
            Some(fe) => freq + (fe - freq) * (i as f64 / (n - 1).max(1) as f64),
        };
        let mut env = 1.0;
        if i < fade {
            env = i as f64 / fade as f64;
        } else if i > n - fade {
            env = (n - i) as f64 / fade as f64;
        }
        let sample = (amp as f64 * env * (2.0 * std::f64::consts::PI * f * t).sin()) as i16;
        for _ in 0..CHANNELS {
            buf.push(sample);
        }
    }
    buf
}
