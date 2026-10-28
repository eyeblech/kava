use std::f32::consts::PI;
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

pub const FFT_SIZE: usize = 2048;
pub const NUM_BANDS: usize = 64;

#[derive(Clone, Copy, Debug)]
struct Complex {
    re: f32,
    im: f32,
}

impl Complex {
    #[inline]
    const fn new(re: f32, im: f32) -> Self {
        Self { re, im }
    }

    #[inline]
    fn norm(self) -> f32 {
        (self.re * self.re + self.im * self.im).sqrt()
    }

    #[inline]
    fn mul(self, rhs: Self) -> Self {
        Self {
            re: self.re * rhs.re - self.im * rhs.im,
            im: self.re * rhs.im + self.im * rhs.re,
        }
    }
}

/// In-place Radix-2 Cooley-Tukey Fast Fourier Transform (N = 2048)
fn fft_in_place(buf: &mut [Complex]) {
    let n = buf.len();
    let mut j = 0usize;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            buf.swap(i, j);
        }
    }

    let mut len = 2;
    while len <= n {
        let half = len >> 1;
        let angle = -2.0 * PI / (len as f32);
        let wlen = Complex::new(angle.cos(), angle.sin());
        let mut i = 0;
        while i < n {
            let mut w = Complex::new(1.0, 0.0);
            for k in 0..half {
                let u = buf[i + k];
                let v = buf[i + k + half].mul(w);
                buf[i + k] = Complex::new(u.re + v.re, u.im + v.im);
                buf[i + k + half] = Complex::new(u.re - v.re, u.im - v.im);
                w = w.mul(wlen);
            }
            i += len;
        }
        len <<= 1;
    }
}

/// Extracted real-time acoustic features smoothed via spring-damper & Monstercat gravity physics.
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct AudioFrame {
    pub bands: [f32; NUM_BANDS],
    pub peaks: [f32; NUM_BANDS],
    pub waveform: [f32; 256],
    pub sub_bass: f32,
    pub bass: f32,
    pub low_mid: f32,
    pub high_mid: f32,
    pub treble: f32,
    pub rms_energy: f32,
    pub spectral_centroid: f32,
    pub spectral_flux: f32,
    pub beat_impulse: f32,
    pub phase_integral: f32,
    pub source_name: String,
    pub active_signal: bool,
}

impl Default for AudioFrame {
    fn default() -> Self {
        Self {
            bands: [0.0; NUM_BANDS],
            peaks: [0.0; NUM_BANDS],
            waveform: [0.0; 256],
            sub_bass: 0.0,
            bass: 0.0,
            low_mid: 0.0,
            high_mid: 0.0,
            treble: 0.0,
            rms_energy: 0.0,
            spectral_centroid: 0.35,
            spectral_flux: 0.0,
            beat_impulse: 0.0,
            phase_integral: 0.0,
            source_name: String::from("Initializing"),
            active_signal: false,
        }
    }
}

struct RingBuffer {
    samples: Vec<f32>,
    write_pos: usize,
    source_name: String,
    samples_received: u64,
}

impl RingBuffer {
    fn new(size: usize) -> Self {
        Self {
            samples: vec![0.0; size],
            write_pos: 0,
            source_name: String::from("Harmonic Synth"),
            samples_received: 0,
        }
    }

    fn push_slice(&mut self, data: &[f32]) {
        let len = self.samples.len();
        for &s in data {
            self.samples[self.write_pos] = s;
            self.write_pos = (self.write_pos + 1) % len;
        }
        self.samples_received = self.samples_received.wrapping_add(data.len() as u64);
    }

    fn read_latest(&self, out: &mut [f32]) {
        let len = self.samples.len();
        let n = out.len();
        let start = (self.write_pos + len - n) % len;
        for (i, item) in out.iter_mut().enumerate() {
            *item = self.samples[(start + i) % len];
        }
    }
}

pub struct AudioEngine {
    ring: Arc<Mutex<RingBuffer>>,
    capture_child: Option<Child>,
    window: Vec<f32>,
    fft_buf: Vec<Complex>,
    raw_samples: Vec<f32>,
    prev_magnitudes: Vec<f32>,
    band_velocities: [f32; NUM_BANDS],
    prev_bands: [f32; NUM_BANDS],
    peaks: [f32; NUM_BANDS],
    flux_history: [f32; 43],
    flux_idx: usize,
    beat_impulse: f32,
    phase_integral: f32,
    auto_gain: f32,
    synth_mode: bool,
    sample_rate: f32,
    last_sample_count: u64,
    silent_frames: u32,
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        if let Some(mut child) = self.capture_child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl AudioEngine {
    pub fn new(force_synth: bool, custom_device: Option<String>, sensitivity: f32) -> Self {
        let ring = Arc::new(Mutex::new(RingBuffer::new(FFT_SIZE * 4)));
        let mut capture_child = None;
        let sample_rate = 48000.0;

        if !force_synth {
            capture_child = Self::try_spawn_pulse_monitor(&ring, custom_device.as_deref())
                .or_else(|| Self::try_spawn_pw_record(&ring))
                .or_else(|| Self::try_spawn_arecord(&ring, custom_device.as_deref()))
                .or_else(|| Self::try_spawn_macos_ffmpeg(&ring))
                .or_else(|| Self::try_spawn_sox(&ring));
        } else if let Ok(mut rb) = ring.lock() {
            rb.source_name = String::from("Cosmic Harmonic Synth");
        }

        // Hann window for smooth spectral leakage suppression
        let window: Vec<f32> = (0..FFT_SIZE)
            .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f32 / (FFT_SIZE as f32 - 1.0)).cos()))
            .collect();

        Self {
            ring,
            capture_child,
            window,
            fft_buf: vec![Complex::new(0.0, 0.0); FFT_SIZE],
            raw_samples: vec![0.0; FFT_SIZE],
            prev_magnitudes: vec![0.0; FFT_SIZE / 2],
            band_velocities: [0.0; NUM_BANDS],
            prev_bands: [0.0; NUM_BANDS],
            peaks: [0.0; NUM_BANDS],
            flux_history: [0.0; 43],
            flux_idx: 0,
            beat_impulse: 0.0,
            phase_integral: 0.0,
            auto_gain: 2.5 * sensitivity.max(0.1),
            synth_mode: force_synth,
            sample_rate,
            last_sample_count: 0,
            silent_frames: 0,
        }
    }

    fn spawn_float32_reader(mut child: Child, ring: &Arc<Mutex<RingBuffer>>, label: String) -> Option<Child> {
        let mut stdout = child.stdout.take()?;
        let ring_clone = Arc::clone(ring);
        if let Ok(mut rb) = ring.lock() {
            rb.source_name = label;
        }

        thread::spawn(move || {
            let mut byte_buf = [0u8; 2048];
            let mut float_buf = Vec::with_capacity(512);
            while let Ok(n) = stdout.read(&mut byte_buf) {
                if n == 0 {
                    break;
                }
                float_buf.clear();
                for chunk in byte_buf[..n].chunks_exact(4) {
                    let val = f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                    if val.is_finite() {
                        float_buf.push(val);
                    }
                }
                if !float_buf.is_empty() {
                    if let Ok(mut rb) = ring_clone.lock() {
                        rb.push_slice(&float_buf);
                    }
                }
            }
        });
        Some(child)
    }

    /// Spawns a lightweight PulseAudio/PipeWire monitor reader (`parec`) capturing desktop audio output.
    fn try_spawn_pulse_monitor(
        ring: &Arc<Mutex<RingBuffer>>,
        custom_device: Option<&str>,
    ) -> Option<Child> {
        let monitor_source = if let Some(dev) = custom_device {
            dev.to_string()
        } else {
            let out = Command::new("pactl")
                .arg("get-default-sink")
                .output()
                .ok()?;
            if !out.status.success() {
                return None;
            }
            let sink = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if sink.is_empty() {
                return None;
            }
            format!("{}.monitor", sink)
        };

        let child = Command::new("parec")
            .args([
                "--format=float32le",
                "--rate=48000",
                "--channels=1",
                "--latency-msec=12",
                "-d",
                &monitor_source,
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;

        let label = if monitor_source.len() > 26 {
            "PipeWire/Pulse Monitor".to_string()
        } else {
            monitor_source
        };
        Self::spawn_float32_reader(child, ring, label)
    }

    fn try_spawn_pw_record(ring: &Arc<Mutex<RingBuffer>>) -> Option<Child> {
        let child = Command::new("pw-cat")
            .args([
                "--record",
                "--format=f32",
                "--rate=48000",
                "--channels=1",
                "-",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        Self::spawn_float32_reader(child, ring, "PipeWire Audio".to_string())
    }

    fn try_spawn_arecord(
        ring: &Arc<Mutex<RingBuffer>>,
        custom_device: Option<&str>,
    ) -> Option<Child> {
        let dev = custom_device.unwrap_or("default");
        let child = Command::new("arecord")
            .args([
                "-D",
                dev,
                "-f",
                "FLOAT_LE",
                "-r",
                "48000",
                "-c",
                "1",
                "-t",
                "raw",
                "-q",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        Self::spawn_float32_reader(child, ring, format!("ALSA ({})", dev))
    }

    fn try_spawn_macos_ffmpeg(ring: &Arc<Mutex<RingBuffer>>) -> Option<Child> {
        #[cfg(target_os = "macos")]
        {
            let child = Command::new("ffmpeg")
                .args([
                    "-f",
                    "avfoundation",
                    "-i",
                    ":0",
                    "-f",
                    "f32le",
                    "-ac",
                    "1",
                    "-ar",
                    "48000",
                    "-loglevel",
                    "quiet",
                    "pipe:1",
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .ok()?;
            return Self::spawn_float32_reader(child, ring, "CoreAudio (AVFoundation)".to_string());
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = ring;
            None
        }
    }

    fn try_spawn_sox(ring: &Arc<Mutex<RingBuffer>>) -> Option<Child> {
        let child = Command::new("rec")
            .args([
                "-q",
                "-t",
                "f32",
                "-r",
                "48000",
                "-c",
                "1",
                "-",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        Self::spawn_float32_reader(child, ring, "SoX Audio".to_string())
    }

    pub fn toggle_synth(&mut self) {
        self.synth_mode = !self.synth_mode;
    }

    /// Generates rich, evolving polyrhythmic cosmic synth audio when no external music is playing
    /// or when Synthetic Demo Mode is toggled on.
    fn fill_harmonic_synth(&mut self, time: f32) {
        let sr = self.sample_rate;
        let beat_freq = 124.0 / 60.0;
        let beat_phase = (time * beat_freq).fract();
        let kick_env = (-beat_phase * 7.5).exp();
        let kick_pitch = 48.0 + 95.0 * (-beat_phase * 28.0).exp();

        let chord_idx = ((time * 0.25) as usize) % 4;
        let root_freqs = [55.0, 43.65, 65.41, 49.0];
        let root = root_freqs[chord_idx];

        for i in 0..FFT_SIZE {
            let t = time + (i as f32 - FFT_SIZE as f32) / sr;
            let k_sample = (2.0 * PI * kick_pitch * t).sin() * kick_env * 0.65;

            let bass_mod = (time * beat_freq * 2.0 * PI).sin().abs();
            let bass = ((2.0 * PI * root * t).sin() * 0.45
                + (2.0 * PI * root * 2.0 * t).sin() * 0.25 * bass_mod)
                * 0.55;

            let fm_index = 1.8 + 1.2 * (time * 0.7).sin();
            let modulator = (2.0 * PI * root * 6.0 * t).sin() * fm_index;
            let mid_pad = (2.0 * PI * root * 4.0 * t + modulator).sin() * 0.28;

            let arp_step = ((time * beat_freq * 4.0) as usize) % 6;
            let arp_mult = [8.0, 9.0, 10.0, 12.0, 15.0, 16.0][arp_step];
            let arp_phase = (time * beat_freq * 4.0).fract();
            let arp_env = (-arp_phase * 5.0).exp();
            let treble_arp = (2.0 * PI * root * arp_mult * t).sin() * arp_env * 0.22;

            self.raw_samples[i] = (k_sample + bass + mid_pad + treble_arp) * 0.75;
        }
    }

    pub fn update(&mut self, time: f32, dt: f32, sensitivity: f32) -> AudioFrame {
        let mut source_name;
        let mut current_count = 0;

        if let Ok(rb) = self.ring.lock() {
            rb.read_latest(&mut self.raw_samples);
            source_name = rb.source_name.clone();
            current_count = rb.samples_received;
        } else {
            source_name = String::from("Audio Stream");
        }

        let mut raw_rms = 0.0f32;
        for &s in &self.raw_samples {
            raw_rms += s * s;
        }
        raw_rms = (raw_rms / FFT_SIZE as f32).sqrt();

        let stream_advancing = current_count != self.last_sample_count;
        self.last_sample_count = current_count;

        let active_signal = !self.synth_mode && stream_advancing && raw_rms > 0.00035;
        if active_signal {
            self.silent_frames = 0;
        } else {
            self.silent_frames = self.silent_frames.saturating_add(1);
        }

        // Strict CAVA behavior: ONLY synthesize audio if user explicitly pressed [S] or passed --synth.
        // When no sound is playing, zero out the FFT input so the visualizer rests in calm silence!
        if self.synth_mode {
            self.fill_harmonic_synth(time);
            source_name = String::from("Cosmic Synth [S]");
            raw_rms = 0.28;
        } else if !active_signal {
            for s in &mut self.raw_samples {
                *s = 0.0;
            }
            raw_rms = 0.0;
        }

        for i in 0..FFT_SIZE {
            self.fft_buf[i] = Complex::new(self.raw_samples[i] * self.window[i], 0.0);
        }
        fft_in_place(&mut self.fft_buf);

        let half = FFT_SIZE / 2;
        let mut magnitudes = vec![0.0f32; half];
        let mut flux = 0.0f32;
        let mut weighted_freq_sum = 0.0f32;
        let mut mag_sum = 0.0f32;

        let norm = 2.0 / (FFT_SIZE as f32).sqrt();
        for i in 1..half {
            let freq = i as f32 * self.sample_rate / FFT_SIZE as f32;
            let tilt = (freq / 120.0).clamp(0.6, 4.5).powf(0.32);
            let mag = self.fft_buf[i].norm() * norm * tilt;
            magnitudes[i] = mag;

            let diff = (mag - self.prev_magnitudes[i]).max(0.0);
            flux += diff;
            self.prev_magnitudes[i] = mag;

            weighted_freq_sum += (i as f32 / half as f32) * mag;
            mag_sum += mag;
        }

        let f_min = 24.0f32;
        let f_max = 18000.0f32;
        let ratio = (f_max / f_min).powf(1.0 / NUM_BANDS as f32);

        let mut target_bands = [0.0f32; NUM_BANDS];
        for b in 0..NUM_BANDS {
            let f_low = f_min * ratio.powi(b as i32);
            let f_high = f_min * ratio.powi(b as i32 + 1);
            let bin_low = ((f_low * FFT_SIZE as f32 / self.sample_rate).floor() as usize).clamp(1, half - 2);
            let bin_high = ((f_high * FFT_SIZE as f32 / self.sample_rate).ceil() as usize).clamp(bin_low + 1, half - 1);

            let mut peak_mag = 0.0f32;
            let mut sum_mag = 0.0f32;
            for &m in &magnitudes[bin_low..=bin_high] {
                peak_mag = peak_mag.max(m);
                sum_mag += m;
            }
            let avg_mag = sum_mag / (bin_high - bin_low + 1) as f32;
            target_bands[b] = (peak_mag * 0.65 + avg_mag * 0.35) * self.auto_gain * sensitivity;
        }

        // MonsterCat + Wide Gaussian spatial smoothing across neighboring bands
        // Ensures the spectrum envelope is a silky, continuous curve without spiky single-bin teeth
        let mut smoothed_targets = [0.0f32; NUM_BANDS];
        for b in 0..NUM_BANDS {
            let mut weighted_sum = 0.0f32;
            let mut weight_total = 0.0f32;
            for d in -5isize..=5 {
                let nb = (b as isize + d).clamp(0, NUM_BANDS as isize - 1) as usize;
                let w = (-((d * d) as f32) / 6.5).exp();
                weighted_sum += target_bands[nb] * w;
                weight_total += w;
            }
            let avg = weighted_sum / weight_total;
            smoothed_targets[b] = (1.0 + avg * 2.1).ln().clamp(0.0, 1.35);
        }

        let frame_peak = smoothed_targets.iter().copied().fold(0.0f32, f32::max);
        if frame_peak > 0.02 {
            let desired_gain = (0.95 / frame_peak.max(0.15)).clamp(0.4, 8.0);
            self.auto_gain = self.auto_gain * 0.96 + desired_gain * 0.04;
        }

        // Second-order damped harmonic oscillator (Spring-Damper physics) per frequency band
        let stiffness = 240.0;
        let damping = 19.5;
        let clamped_dt = dt.clamp(0.004, 0.05);

        let mut out_bands = [0.0f32; NUM_BANDS];
        for b in 0..NUM_BANDS {
            let pos = self.prev_bands[b];
            let vel = self.band_velocities[b];
            let target = smoothed_targets[b];

            let k = if target > pos { stiffness * 1.45 } else { stiffness * 0.72 };
            let accel = k * (target - pos) - damping * vel;
            let new_vel = vel + accel * clamped_dt;
            let new_pos = (pos + new_vel * clamped_dt).clamp(0.0, 1.35);

            self.band_velocities[b] = new_vel;
            self.prev_bands[b] = new_pos;
            out_bands[b] = new_pos;

            if new_pos >= self.peaks[b] {
                self.peaks[b] = new_pos;
            } else {
                self.peaks[b] = (self.peaks[b] - 0.42 * clamped_dt).max(new_pos);
            }
        }

        let avg_slice = |slice: &[f32]| slice.iter().sum::<f32>() / slice.len().max(1) as f32;
        let sub_bass = avg_slice(&out_bands[0..6]);
        let bass = avg_slice(&out_bands[4..14]);
        let low_mid = avg_slice(&out_bands[14..30]);
        let high_mid = avg_slice(&out_bands[30..48]);
        let treble = avg_slice(&out_bands[48..NUM_BANDS]);
        let rms_energy = (sub_bass * 0.30 + bass * 0.30 + low_mid * 0.22 + high_mid * 0.12 + treble * 0.06)
            .clamp(0.0, 1.3);

        let norm_flux = (flux * 0.08 * sensitivity).clamp(0.0, 2.5);
        self.flux_history[self.flux_idx] = norm_flux;
        self.flux_idx = (self.flux_idx + 1) % self.flux_history.len();
        let avg_flux = self.flux_history.iter().sum::<f32>() / self.flux_history.len() as f32;

        if norm_flux > avg_flux * 1.38 + 0.04 && self.beat_impulse < 0.55 {
            self.beat_impulse = (norm_flux - avg_flux).clamp(0.55, 1.25);
        } else {
            self.beat_impulse *= (-6.5 * clamped_dt).exp();
        }

        let spectral_centroid = if mag_sum > 0.0001 {
            (weighted_freq_sum / mag_sum * 2.8).clamp(0.05, 1.0)
        } else {
            0.35
        };

        // Integrate continuous phase velocity ONLY when sound energy is present (rests peacefully on silence)
        self.phase_integral += clamped_dt * (2.4 * rms_energy + 1.15 * self.beat_impulse);

        let mut waveform = [0.0f32; 256];
        let wave_gain = if raw_rms > 0.00035 {
            (1.5 / raw_rms.max(0.08)).clamp(0.8, 6.0)
        } else {
            0.0
        };
        for (i, w) in waveform.iter_mut().enumerate() {
            *w = (self.raw_samples[i * 4] * wave_gain).clamp(-1.0, 1.0);
        }

        let active_audio = self.synth_mode || active_signal || rms_energy > 0.008;

        AudioFrame {
            bands: out_bands,
            peaks: self.peaks,
            waveform,
            sub_bass,
            bass,
            low_mid,
            high_mid,
            treble,
            rms_energy,
            spectral_centroid,
            spectral_flux: norm_flux.clamp(0.0, 1.0),
            beat_impulse: self.beat_impulse,
            phase_integral: self.phase_integral,
            source_name,
            active_signal: active_audio,
        }
    }
}
