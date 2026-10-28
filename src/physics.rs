use crate::audio::{AudioFrame, NUM_BANDS};
use crate::color::Palette;
use std::f32::consts::PI;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VisualMode {
    SacredMandala,
    PoincareDisk,
    ChladniCymatics,
    QuantumVortex,
    AizawaAttractor,
    RadialCavaScope,
}

impl VisualMode {
    pub const ALL: [VisualMode; 6] = [
        VisualMode::SacredMandala,
        VisualMode::PoincareDisk,
        VisualMode::ChladniCymatics,
        VisualMode::QuantumVortex,
        VisualMode::AizawaAttractor,
        VisualMode::RadialCavaScope,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            VisualMode::SacredMandala => "Sacred Dihedral Mandala",
            VisualMode::PoincareDisk => "Hyperbolic Poincare Disk",
            VisualMode::ChladniCymatics => "Chladni Cymatic Resonance",
            VisualMode::QuantumVortex => "Quantum Relativistic Vortex",
            VisualMode::AizawaAttractor => "Aizawa Strange Attractor",
            VisualMode::RadialCavaScope => "Radial CAVA Mirror Scope",
        }
    }

    pub fn from_index(idx: usize) -> Self {
        Self::ALL[idx.saturating_sub(1) % Self::ALL.len()]
    }

    pub fn next(&self) -> Self {
        let idx = Self::ALL.iter().position(|m| m == self).unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }

    pub fn prev(&self) -> Self {
        let idx = Self::ALL.iter().position(|m| m == self).unwrap_or(0);
        Self::ALL[(idx + Self::ALL.len() - 1) % Self::ALL.len()]
    }
}

/// 3D chaotic particle for the Aizawa strange attractor
#[derive(Clone, Copy)]
struct AttractorParticle {
    x: f32,
    y: f32,
    z: f32,
    hue_offset: f32,
}

pub struct PhysicsField {
    pub width: usize,
    pub height: usize,
    pub rgb_buf: Vec<[u8; 3]>,
    hdr_buf: Vec<[f32; 3]>,
    bloom_scratch: Vec<[f32; 3]>,
    density_buf: Vec<f32>,
    particles: Vec<AttractorParticle>,
    pub symmetry_folds: usize,
    pub zoom: f32,
    pub trail_persistence: f32,
    pub bloom_enabled: bool,
    pub cymatic_m: f32,
    pub cymatic_n: f32,
}

impl PhysicsField {
    pub fn new(width: usize, height: usize, symmetry_folds: usize) -> Self {
        let size = width * height;
        let mut particles = Vec::with_capacity(480);
        for i in 0..480 {
            let angle = i as f32 * 0.17;
            let r = 0.15 + (i as f32 / 480.0) * 0.65;
            particles.push(AttractorParticle {
                x: r * angle.cos(),
                y: r * angle.sin(),
                z: 0.2 + 0.5 * (i as f32 * 0.05).sin(),
                hue_offset: (i as f32) / 480.0,
            });
        }

        Self {
            width,
            height,
            rgb_buf: vec![[0, 0, 0]; size],
            hdr_buf: vec![[0.0, 0.0, 0.0]; size],
            bloom_scratch: vec![[0.0, 0.0, 0.0]; size],
            density_buf: vec![0.0; size],
            particles,
            symmetry_folds: symmetry_folds.clamp(2, 24),
            zoom: 1.0,
            trail_persistence: 0.72,
            bloom_enabled: true,
            cymatic_m: 3.0,
            cymatic_n: 5.0,
        }
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        if self.width == width && self.height == height {
            return;
        }
        self.width = width;
        self.height = height;
        let size = width * height;
        self.rgb_buf.resize(size, [0, 0, 0]);
        self.hdr_buf.resize(size, [0.0, 0.0, 0.0]);
        self.bloom_scratch.resize(size, [0.0, 0.0, 0.0]);
        self.density_buf.resize(size, 0.0);
    }

    /// Dihedral Symmetry Group D_N folding operator:
    /// Maps any angle `theta` into the canonical mirror wedge `[0, PI / N]`.
    #[inline]
    fn fold_dihedral(theta: f32, folds: usize) -> f32 {
        let n = folds.max(2) as f32;
        let sector = 2.0 * PI / n;
        let half_sector = PI / n;
        let local = theta.rem_euclid(sector);
        if local > half_sector {
            sector - local
        } else {
            local
        }
    }

    /// Smoothly sample the 64-band FFT spectrum using Catmull-Rom / Cosine interpolation
    /// so curves never look stepped or blocky.
    #[inline]
    fn sample_spectrum(bands: &[f32; NUM_BANDS], u: f32) -> f32 {
        let pos = (u.clamp(0.0, 0.999) * (NUM_BANDS - 1) as f32).max(0.0);
        let idx = pos as usize;
        let frac = pos - idx as f32;
        // Smooth cosine interpolation instead of linear
        let smooth_t = (1.0 - (frac * PI).cos()) * 0.5;
        let next = (idx + 1).min(NUM_BANDS - 1);
        bands[idx] * (1.0 - smooth_t) + bands[next] * smooth_t
    }

    #[inline]
    fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
        let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }

    /// Integrate 4th-order Runge-Kutta (RK4) for the 3D Aizawa Strange Attractor
    fn step_attractor(&mut self, dt: f32, audio: &AudioFrame) {
        if audio.rms_energy < 0.002 {
            return;
        }
        let a = 0.95 + 0.15 * audio.bass;
        let b = 0.70 + 0.12 * audio.sub_bass;
        let c = 0.60 + 0.10 * audio.low_mid;
        let d = 3.50 + 0.80 * audio.high_mid;
        let e = 0.25 + 0.15 * audio.treble;
        let f = 0.10 + 0.08 * audio.beat_impulse;

        let deriv = |x: f32, y: f32, z: f32| -> (f32, f32, f32) {
            let dx = (z - b) * x - d * y;
            let dy = d * x + (z - b) * y;
            let r2 = x * x + y * y;
            let dz = c + a * z - (z * z * z) / 3.0 - r2 * (1.0 + e * z) + f * z * x * x * x;
            (dx, dy, dz)
        };

        let h = (dt * (0.35 + 1.45 * audio.rms_energy)).clamp(0.001, 0.025);

        for p in &mut self.particles {
            let (k1x, k1y, k1z) = deriv(p.x, p.y, p.z);
            let (k2x, k2y, k2z) =
                deriv(p.x + 0.5 * h * k1x, p.y + 0.5 * h * k1y, p.z + 0.5 * h * k1z);
            let (k3x, k3y, k3z) =
                deriv(p.x + 0.5 * h * k2x, p.y + 0.5 * h * k2y, p.z + 0.5 * h * k2z);
            let (k4x, k4y, k4z) = deriv(p.x + h * k3x, p.y + h * k3y, p.z + h * k3z);

            p.x += (h / 6.0) * (k1x + 2.0 * k2x + 2.0 * k3x + k4x);
            p.y += (h / 6.0) * (k1y + 2.0 * k2y + 2.0 * k3y + k4y);
            p.z += (h / 6.0) * (k1z + 2.0 * k2z + 2.0 * k3z + k4z);

            if !p.x.is_finite()
                || !p.y.is_finite()
                || !p.z.is_finite()
                || (p.x * p.x + p.y * p.y + p.z * p.z) > 12.0
            {
                p.x = 0.1 * (p.hue_offset * 25.0).cos();
                p.y = 0.1 * (p.hue_offset * 25.0).sin();
                p.z = 0.2;
            }
        }
    }

    pub fn render(
        &mut self,
        mode: VisualMode,
        palette: Palette,
        audio: &AudioFrame,
        _time: f32,
        dt: f32,
    ) {
        let w = self.width;
        let h = self.height;
        if w == 0 || h == 0 {
            return;
        }

        // Strict audio gate: when no sound is playing, smoothly decay the canvas to pure black silence
        let energy_gate = Self::smoothstep(0.002, 0.045, audio.rms_energy);
        if energy_gate < 0.001 {
            let decay = (-dt * 12.0).exp();
            for i in 0..(w * h) {
                self.hdr_buf[i][0] *= decay;
                self.hdr_buf[i][1] *= decay;
                self.hdr_buf[i][2] *= decay;
                self.rgb_buf[i] = Palette::float_to_u8(self.hdr_buf[i]);
            }
            return;
        }

        if audio.rms_energy > 0.01 {
            let target_m = 2.0 + (audio.bass * 4.5).round();
            let target_n = 3.0 + (audio.spectral_centroid * 6.0).round();
            self.cymatic_m = self.cymatic_m * 0.95 + target_m * 0.05;
            self.cymatic_n = self.cymatic_n * 0.95 + target_n * 0.05;
        }

        if mode == VisualMode::AizawaAttractor {
            self.step_attractor(dt, audio);
            for d in &mut self.density_buf {
                *d *= 0.84;
            }
            self.splat_attractor_particles(audio);
        }

        // Terminal character cells are ~2x taller than wide, and half-block splits each cell vertically by 2,
        // meaning each subpixel is roughly square! Use exact aspect ratio:
        let aspect = w as f32 / h.max(1) as f32;
        let inv_zoom =
            1.0 / (self.zoom * (0.92 + 0.22 * audio.sub_bass + 0.14 * audio.beat_impulse));
        let phase = audio.phase_integral;
        let folds = self.symmetry_folds;
        let half_sector = PI / (folds.max(2) as f32);

        // 2x2 Rotated Grid Super-Sampling Anti-Aliasing (RGSSAA) subpixel offsets
        // Eliminates jagged terminal staircases and produces silky-smooth curves!
        const SUB_OFFSETS: [(f32, f32); 2] = [(-0.22, -0.22), (0.22, 0.22)];

        let persist = (self.trail_persistence * (0.65 + 0.25 * energy_gate)).clamp(0.0, 0.88);

        for py in 0..h {
            for px in 0..w {
                let idx = py * w + px;
                let d_val = self.density_buf[idx];

                let mut acc_rgb = [0.0f32; 3];
                for &(ox, oy) in &SUB_OFFSETS {
                    let ny = ((py as f32 + 0.5 + oy) / h as f32) * 2.0 - 1.0;
                    let nx = (((px as f32 + 0.5 + ox) / w as f32) * 2.0 - 1.0) * aspect;
                    let x = nx * inv_zoom;
                    let y = ny * inv_zoom;

                    let r = (x * x + y * y).sqrt();
                    let raw_theta = y.atan2(x);

                    let twisted_theta = raw_theta + 0.22 * phase * (1.0 - 0.35 * r).sin();
                    let folded_theta = Self::fold_dihedral(twisted_theta, folds);
                    // Apply C^1 smooth cosine boundary easing at 0 and PI/N so mirror seams never have sharp corners!
                    let raw_norm = (folded_theta / half_sector).clamp(0.0, 1.0);
                    let norm_angle = (1.0 - (raw_norm * PI).cos()) * 0.5;
                    let smooth_theta = norm_angle * half_sector;

                    let (intensity, color_phase, core_glow) = match mode {
                        VisualMode::SacredMandala => {
                            self.eval_sacred_mandala(r, smooth_theta, norm_angle, phase, audio)
                        }
                        VisualMode::PoincareDisk => {
                            self.eval_poincare_disk(x, y, r, smooth_theta, norm_angle, phase, audio)
                        }
                        VisualMode::ChladniCymatics => {
                            self.eval_chladni_cymatics(r, smooth_theta, norm_angle, phase, audio)
                        }
                        VisualMode::QuantumVortex => self.eval_quantum_vortex(
                            r,
                            raw_theta,
                            smooth_theta,
                            norm_angle,
                            phase,
                            audio,
                        ),
                        VisualMode::AizawaAttractor => {
                            self.eval_aizawa_field(r, smooth_theta, norm_angle, d_val, phase, audio)
                        }
                        VisualMode::RadialCavaScope => self.eval_radial_cava_scope(
                            r,
                            raw_theta,
                            smooth_theta,
                            norm_angle,
                            phase,
                            audio,
                        ),
                    };

                    let sample = palette.sample_float(
                        intensity * energy_gate,
                        color_phase,
                        core_glow * energy_gate,
                    );
                    acc_rgb[0] += sample[0];
                    acc_rgb[1] += sample[1];
                    acc_rgb[2] += sample[2];
                }

                let inv_samples = 1.0 / SUB_OFFSETS.len() as f32;
                let cur_r = acc_rgb[0] * inv_samples;
                let cur_g = acc_rgb[1] * inv_samples;
                let cur_b = acc_rgb[2] * inv_samples;

                // Smooth exponential temporal blending in floating-point HDR space
                let prev = self.hdr_buf[idx];
                self.hdr_buf[idx] = [
                    cur_r.max(prev[0] * persist),
                    cur_g.max(prev[1] * persist),
                    cur_b.max(prev[2] * persist),
                ];
            }
        }

        if self.bloom_enabled {
            self.apply_gaussian_bloom_and_tonemap(audio.beat_impulse);
        } else {
            for i in 0..(w * h) {
                self.rgb_buf[i] = Palette::float_to_u8(self.hdr_buf[i]);
            }
        }
    }

    /// Mode 1: Sacred Dihedral Mandala
    /// Volumetric, pillowy, anti-aliased lotus petals & concentric harmonic waves.
    #[inline]
    fn eval_sacred_mandala(
        &self,
        r: f32,
        folded_theta: f32,
        norm_angle: f32,
        phase: f32,
        audio: &AudioFrame,
    ) -> (f32, f32, f32) {
        let spec_ang = Self::sample_spectrum(&audio.bands, norm_angle * 0.65);
        let spec_rad = Self::sample_spectrum(&audio.bands, (r * 0.55).clamp(0.0, 1.0));

        // Smooth harmonic lotus curves (using smooth cosine envelopes instead of spiky high-frequency bins)
        let r1 = (0.20 + 0.38 * audio.bass)
            + (0.14 + 0.18 * audio.low_mid) * (folded_theta * 2.0 + phase * 0.7).cos()
            + 0.08 * spec_ang;
        let r2 = (0.42 + 0.35 * audio.low_mid)
            + (0.16 + 0.16 * audio.high_mid) * (folded_theta * 3.0 - phase * 0.5).cos()
            + 0.08 * spec_rad;
        let r3 = (0.64 + 0.30 * audio.high_mid)
            + (0.12 + 0.15 * audio.treble) * (folded_theta * 4.0 + phase * 0.9).sin();

        let d1 = (r - r1).abs();
        let d2 = (r - r2).abs();
        let d3 = (r - r3).abs();

        // Wide, silky Gaussian cushions so adjacent terminal cells blend with smooth gradients
        let w1 = 0.075 + 0.035 * audio.bass;
        let w2 = 0.085 + 0.035 * audio.low_mid;
        let w3 = 0.095 + 0.035 * audio.high_mid;

        let rib1 = (-d1 * d1 / (w1 * w1)).exp() * (0.75 + 0.45 * audio.bass);
        let rib2 = (-d2 * d2 / (w2 * w2)).exp() * (0.68 + 0.42 * spec_ang);
        let rib3 = (-d3 * d3 / (w3 * w3)).exp() * (0.58 + 0.38 * spec_rad);

        // Soft glowing core nebula & radial silk rays
        let spoke_wave = (folded_theta * 2.0).cos() * 0.5 + 0.5;
        let core_nebula = (-r * r / (0.18 + 0.16 * audio.sub_bass)).exp()
            * (0.55 + 0.45 * spoke_wave)
            * (0.6 + 0.6 * audio.bass);

        let vignette = Self::smoothstep(1.18, 0.32, r);
        let intensity = (rib1 + rib2 + rib3 + core_nebula) * vignette;
        let core_glow = ((-d1 * d1 / 0.0035).exp() * 0.65
            + (-d2 * d2 / 0.0040).exp() * 0.50
            + (-r * r / 0.04).exp() * 0.55 * audio.beat_impulse)
            * vignette;
        let color_phase = r * 0.48 + norm_angle * 0.32 - phase * 0.09 + 0.15 * spec_ang;

        (intensity, color_phase, core_glow)
    }

    /// Mode 2: Hyperbolic Poincare Disk & Complex Mobius Kaleidoscope
    #[inline]
    fn eval_poincare_disk(
        &self,
        x: f32,
        y: f32,
        r: f32,
        folded_theta: f32,
        norm_angle: f32,
        phase: f32,
        audio: &AudioFrame,
    ) -> (f32, f32, f32) {
        if r > 1.15 {
            return (0.0, 0.0, 0.0);
        }
        let r_disk = (r * 0.85).min(0.985);
        let hyp_dist = ((1.0 + r_disk) / (1.0 - r_disk + 0.015)).ln();

        let a_mag = (0.20 + 0.24 * audio.bass).min(0.58);
        let ax = a_mag * (phase * 0.55).cos();
        let ay = a_mag * (phase * 0.55).sin();

        let num_x = x * 0.78 - ax;
        let num_y = y * 0.78 - ay;
        let den_x = 1.0 - (ax * x * 0.78 + ay * y * 0.78);
        let den_y = -(ax * y * 0.78 - ay * x * 0.78);
        let den_norm2 = (den_x * den_x + den_y * den_y).max(0.01);

        let wx = (num_x * den_x + num_y * den_y) / den_norm2;
        let wy = (num_y * den_x - num_x * den_y) / den_norm2;
        let w_ang = Self::fold_dihedral(wy.atan2(wx), self.symmetry_folds);

        let band_u = (hyp_dist * 0.16 + norm_angle * 0.25).fract();
        let spec = Self::sample_spectrum(&audio.bands, band_u);

        let geo_wave = (hyp_dist * (2.2 + 1.1 * audio.low_mid) - phase * 1.3 + spec * 1.2).sin()
            * (w_ang * (self.symmetry_folds as f32) + hyp_dist * 1.2).cos();
        let d_geo = geo_wave.abs();

        // Wide, smooth hyperbolic silk cushions
        let arc_glow = (-d_geo * d_geo / 0.085).exp() * (0.55 + 0.65 * spec);
        let ring_d = (r - (0.88 + 0.06 * audio.sub_bass)).abs();
        let boundary_ring = (-ring_d * ring_d / 0.0085).exp() * (0.65 + 0.55 * audio.bass);

        let vignette = Self::smoothstep(1.12, 0.55, r);
        let intensity = arc_glow * vignette + boundary_ring;
        let core_glow = (-d_geo * d_geo / 0.014).exp() * spec * 0.65 + boundary_ring * 0.55;
        let color_phase = hyp_dist * 0.16 + folded_theta * 0.35 - phase * 0.07;

        (intensity, color_phase, core_glow)
    }

    /// Mode 3: Chladni Cymatic Resonance Plate
    #[inline]
    fn eval_chladni_cymatics(
        &self,
        r: f32,
        folded_theta: f32,
        norm_angle: f32,
        phase: f32,
        audio: &AudioFrame,
    ) -> (f32, f32, f32) {
        let sym_x = r * (folded_theta * 2.0 + phase * 0.12).cos();
        let sym_y = r * (folded_theta * 2.0 + phase * 0.12).sin();

        let m = self.cymatic_m;
        let n = self.cymatic_n;
        let spec = Self::sample_spectrum(&audio.bands, (r * 0.5 + norm_angle * 0.5).clamp(0.0, 1.0));

        let wave1 = (m * PI * sym_x * 0.78).cos() * (n * PI * sym_y * 0.78).cos();
        let wave2 = (n * PI * sym_x * 0.78).cos() * (m * PI * sym_y * 0.78).cos();
        let helmholtz = (wave1 - wave2 + 0.22 * spec * (r * 5.0 - phase * 1.4).sin()).abs();

        let line_width = 0.14 + 0.06 * audio.rms_energy;
        let nodal_ribbon =
            (-helmholtz * helmholtz / (line_width * line_width)).exp() * (0.68 + 0.58 * spec);

        let vignette = Self::smoothstep(1.15, 0.30, r);
        let intensity = nodal_ribbon * vignette;
        let core_glow =
            (-helmholtz * helmholtz / 0.008).exp() * (0.55 + 0.45 * audio.beat_impulse) * vignette;
        let color_phase = r * 0.38 + norm_angle * 0.35 - phase * 0.06 + helmholtz * 0.22;

        (intensity, color_phase, core_glow)
    }

    /// Mode 4: Quantum Relativistic Vortex
    #[inline]
    fn eval_quantum_vortex(
        &self,
        r: f32,
        raw_theta: f32,
        folded_theta: f32,
        norm_angle: f32,
        phase: f32,
        audio: &AudioFrame,
    ) -> (f32, f32, f32) {
        let safe_r = r.max(0.06);
        let log_r = safe_r.ln();
        let spec = Self::sample_spectrum(&audio.bands, norm_angle * 0.75);

        let spiral_phase = log_r * (2.8 + 1.4 * audio.bass)
            + folded_theta * (self.symmetry_folds as f32)
            - phase * 2.0
            + spec * 1.1;
        let d_spiral = spiral_phase.sin().abs();

        let counter_phase = log_r * 3.6 - raw_theta * 2.0 + phase * 1.2;
        let shimmer = counter_phase.cos() * 0.5 + 0.5;

        let ribbon =
            (-d_spiral * d_spiral / 0.11).exp() * (0.55 + 0.72 * spec) * (0.55 + 0.45 * shimmer);
        let horizon_d = (r - (0.26 + 0.12 * audio.sub_bass)).abs();
        let horizon = (-horizon_d * horizon_d / 0.0075).exp() * (0.75 + 0.55 * audio.bass);

        let vignette = Self::smoothstep(1.18, 0.28, r) * Self::smoothstep(0.05, 0.18, r);
        let intensity = ribbon * vignette + horizon;
        let core_glow = (-d_spiral * d_spiral / 0.016).exp() * spec * 0.65 + horizon * 0.65;
        let color_phase = log_r * 0.26 - phase * 0.10 + norm_angle * 0.42;

        (intensity, color_phase, core_glow)
    }

    /// Mode 5: Aizawa Strange Attractor (RK4 3D Chaotic Flow folded into Dihedral Symmetry)
    fn splat_attractor_particles(&mut self, audio: &AudioFrame) {
        let w = self.width as f32;
        let h = self.height as f32;
        let aspect = w / h.max(1.0);
        let rot = audio.phase_integral * 0.35;
        let (sin_r, cos_r) = rot.sin_cos();
        let folds = self.symmetry_folds.max(2);

        for (i, p) in self.particles.iter().enumerate() {
            let rx = p.x * cos_r - p.y * sin_r;
            let ry = p.x * sin_r + p.y * cos_r;
            let rz = p.z - 0.7;

            let scale = 1.25 / (2.4 - rz * 0.45).max(0.4);
            let proj_r = (rx * rx + ry * ry).sqrt() * scale * 0.52;
            let base_ang = ry.atan2(rx);
            let band_val = audio.bands[i % NUM_BANDS];

            for k in 0..folds {
                let sector_ang = base_ang + (k as f32) * (2.0 * PI / folds as f32);
                let sx = (proj_r * sector_ang.cos() / aspect + 1.0) * 0.5 * w;
                let sy = (proj_r * sector_ang.sin() + 1.0) * 0.5 * h;

                let ix = sx as isize;
                let iy = sy as isize;
                if ix >= 2
                    && ix < (self.width as isize - 2)
                    && iy >= 2
                    && iy < (self.height as isize - 2)
                {
                    let energy = 0.28 + 0.65 * band_val;
                    // Smooth 3x3 Gaussian splat so particles look like glowing silk orbs, not boxy pixels
                    for dy in -1isize..=1 {
                        for dx in -1isize..=1 {
                            let weight = match (dx.abs(), dy.abs()) {
                                (0, 0) => 1.0,
                                (1, 0) | (0, 1) => 0.55,
                                _ => 0.28,
                            };
                            let idx = (iy + dy) as usize * self.width + (ix + dx) as usize;
                            self.density_buf[idx] =
                                (self.density_buf[idx] + energy * weight).min(2.2);
                        }
                    }
                }
            }
        }
    }

    #[inline]
    fn eval_aizawa_field(
        &self,
        r: f32,
        folded_theta: f32,
        norm_angle: f32,
        density: f32,
        phase: f32,
        audio: &AudioFrame,
    ) -> (f32, f32, f32) {
        let spec = Self::sample_spectrum(&audio.bands, norm_angle);
        let ring_r = 0.35 + 0.35 * spec + 0.1 * (folded_theta * 4.0).cos();
        let d_ring = (r - ring_r).abs();
        let guide_ribbon = (-d_ring * d_ring / 0.0025).exp() * 0.45 * Self::smoothstep(1.2, 0.2, r);

        let intensity = density * 0.9 + guide_ribbon;
        let core_glow = (density - 0.8).max(0.0) * 0.7 + (-d_ring * d_ring / 0.0004).exp() * 0.3;
        let color_phase = r * 0.35 + norm_angle * 0.4 - phase * 0.08 + density * 0.18;
        (intensity, color_phase, core_glow)
    }

    /// Mode 6: Radial CAVA Mirror Scope
    /// Smooth continuous spline-interpolated CAVA spectrum mandala with glowing peak envelope
    /// and anti-aliased inner/outer harmonic rings.
    #[inline]
    fn eval_radial_cava_scope(
        &self,
        r: f32,
        _raw_theta: f32,
        folded_theta: f32,
        norm_angle: f32,
        phase: f32,
        audio: &AudioFrame,
    ) -> (f32, f32, f32) {
        let band_amp = Self::sample_spectrum(&audio.bands, norm_angle * 0.65);
        let peak_amp = Self::sample_spectrum(&audio.peaks, norm_angle * 0.65);

        let inner_radius = 0.22 + 0.08 * audio.sub_bass;
        let outer_contour = inner_radius + band_amp * 0.58;
        let peak_contour = inner_radius + peak_amp * 0.64;
        let inner_mirror = (inner_radius - band_amp * 0.14).max(0.06);

        let rib_wave = (folded_theta * (self.symmetry_folds as f32 * 2.0)).cos() * 0.25 + 0.75;

        let mut intensity = 0.0;
        let mut core_glow = 0.0;

        // Smoothly anti-aliased spectrum body with wide feathering
        let edge_in = Self::smoothstep(inner_mirror - 0.05, inner_mirror + 0.05, r);
        let edge_out = Self::smoothstep(outer_contour + 0.07, outer_contour - 0.04, r);
        let height_t =
            ((r - inner_radius) / (outer_contour - inner_radius + 0.01)).clamp(0.0, 1.0);
        intensity += edge_in * edge_out * (0.42 + 0.65 * height_t) * rib_wave;

        // Soft glowing neon contour along the top of the CAVA spectrum
        let d_top = (r - outer_contour).abs();
        let top_laser = (-d_top * d_top / 0.0085).exp() * (0.65 + 0.55 * band_amp);
        intensity += top_laser;
        core_glow += (-d_top * d_top / 0.0035).exp() * 0.65;

        // Floating peak-gravity silk ring
        let d_peak = (r - peak_contour).abs();
        let peak_laser = (-d_peak * d_peak / 0.0065).exp() * (0.45 + 0.45 * peak_amp);
        intensity += peak_laser;
        core_glow += (-d_peak * d_peak / 0.0025).exp() * 0.45;

        let vignette = Self::smoothstep(1.15, 0.45, r);
        let color_phase = r * 0.52 + norm_angle * 0.35 - phase * 0.07;
        (
            intensity * vignette,
            color_phase,
            (core_glow * vignette).clamp(0.0, 1.0),
        )
    }

    /// True 2-Pass Separable 5-Tap Gaussian Optical Bloom in floating-point HDR space
    fn apply_gaussian_bloom_and_tonemap(&mut self, beat_impulse: f32) {
        let w = self.width;
        let h = self.height;
        if w < 5 || h < 5 {
            for i in 0..(w * h) {
                self.rgb_buf[i] = Palette::float_to_u8(self.hdr_buf[i]);
            }
            return;
        }

        const WEIGHTS: [f32; 5] = [0.06136, 0.24477, 0.38774, 0.24477, 0.06136];
        let bloom_strength = (0.32 + 0.18 * beat_impulse).clamp(0.2, 0.55);

        // Pass 1: Horizontal Gaussian blur of bright highlights into `bloom_scratch`
        for y in 0..h {
            let row = y * w;
            for x in 0..w {
                let mut sum = [0.0f32; 3];
                for (k, &wt) in WEIGHTS.iter().enumerate() {
                    let sx = (x as isize + k as isize - 2).clamp(0, w as isize - 1) as usize;
                    let px = self.hdr_buf[row + sx];
                    // Extract bright neon highlights
                    let luma = px[0] * 0.299 + px[1] * 0.587 + px[2] * 0.114;
                    let highlight = (luma - 0.22).max(0.0);
                    sum[0] += px[0] * highlight * wt;
                    sum[1] += px[1] * highlight * wt;
                    sum[2] += px[2] * highlight * wt;
                }
                self.bloom_scratch[row + x] = sum;
            }
        }

        // Pass 2: Vertical Gaussian blur + additive HDR composite into `rgb_buf`
        for y in 0..h {
            for x in 0..w {
                let mut bloom = [0.0f32; 3];
                for (k, &wt) in WEIGHTS.iter().enumerate() {
                    let sy = (y as isize + k as isize - 2).clamp(0, h as isize - 1) as usize;
                    let s = self.bloom_scratch[sy * w + x];
                    bloom[0] += s[0] * wt;
                    bloom[1] += s[1] * wt;
                    bloom[2] += s[2] * wt;
                }
                let idx = y * w + x;
                let base = self.hdr_buf[idx];
                let final_rgb = [
                    (base[0] + bloom[0] * bloom_strength).clamp(0.0, 1.0),
                    (base[1] + bloom[1] * bloom_strength).clamp(0.0, 1.0),
                    (base[2] + bloom[2] * bloom_strength).clamp(0.0, 1.0),
                ];
                self.rgb_buf[idx] = Palette::float_to_u8(final_rgb);
            }
        }
    }
}
