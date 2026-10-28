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

/// 3D chaotic particle for the Aizawa / Lorenz strange attractor & starfield overlay
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
    pub prev_rgb_buf: Vec<[u8; 3]>,
    pub density_buf: Vec<f32>,
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
        let mut particles = Vec::with_capacity(420);
        for i in 0..420 {
            let angle = i as f32 * 0.17;
            let r = 0.15 + (i as f32 / 420.0) * 0.65;
            particles.push(AttractorParticle {
                x: r * angle.cos(),
                y: r * angle.sin(),
                z: 0.2 + 0.5 * (i as f32 * 0.05).sin(),
                hue_offset: (i as f32) / 420.0,
            });
        }

        Self {
            width,
            height,
            rgb_buf: vec![[4, 4, 9]; size],
            prev_rgb_buf: vec![[4, 4, 9]; size],
            density_buf: vec![0.0; size],
            particles,
            symmetry_folds: symmetry_folds.clamp(2, 24),
            zoom: 1.0,
            trail_persistence: 0.76,
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
        self.rgb_buf.resize(size, [4, 4, 9]);
        self.prev_rgb_buf.resize(size, [4, 4, 9]);
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

    /// Smoothly sample the 64-band FFT spectrum at normalized coordinate `u` in [0.0..1.0]
    #[inline]
    fn sample_spectrum(bands: &[f32; NUM_BANDS], u: f32) -> f32 {
        let pos = (u.clamp(0.0, 0.999) * (NUM_BANDS - 1) as f32).max(0.0);
        let idx = pos as usize;
        let frac = pos - idx as f32;
        let next = (idx + 1).min(NUM_BANDS - 1);
        bands[idx] * (1.0 - frac) + bands[next] * frac
    }

    /// Integrate 4th-order Runge-Kutta (RK4) for the 3D Aizawa Strange Attractor
    fn step_attractor(&mut self, dt: f32, audio: &AudioFrame) {
        // Audio-modulated Aizawa parameters
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

        let h = (dt * (0.65 + 1.15 * audio.rms_energy)).clamp(0.002, 0.025);

        for p in &mut self.particles {
            let (k1x, k1y, k1z) = deriv(p.x, p.y, p.z);
            let (k2x, k2y, k2z) = deriv(p.x + 0.5 * h * k1x, p.y + 0.5 * h * k1y, p.z + 0.5 * h * k1z);
            let (k3x, k3y, k3z) = deriv(p.x + 0.5 * h * k2x, p.y + 0.5 * h * k2y, p.z + 0.5 * h * k2z);
            let (k4x, k4y, k4z) = deriv(p.x + h * k3x, p.y + h * k3y, p.z + h * k3z);

            p.x += (h / 6.0) * (k1x + 2.0 * k2x + 2.0 * k3x + k4x);
            p.y += (h / 6.0) * (k1y + 2.0 * k2y + 2.0 * k3y + k4y);
            p.z += (h / 6.0) * (k1z + 2.0 * k2z + 2.0 * k3z + k4z);

            if !p.x.is_finite() || !p.y.is_finite() || !p.z.is_finite() || (p.x * p.x + p.y * p.y + p.z * p.z) > 12.0 {
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
        time: f32,
        dt: f32,
    ) {
        let w = self.width;
        let h = self.height;
        if w == 0 || h == 0 {
            return;
        }

        // Smoothly evolve Chladni quantum numbers (m, n) based on spectral centroid & bass
        let target_m = 2.0 + (audio.bass * 4.5).round();
        let target_n = 3.0 + (audio.spectral_centroid * 7.0).round();
        self.cymatic_m = self.cymatic_m * 0.94 + target_m * 0.06;
        self.cymatic_n = self.cymatic_n * 0.94 + target_n * 0.06;

        if mode == VisualMode::AizawaAttractor {
            self.step_attractor(dt, audio);
            for d in &mut self.density_buf {
                *d *= 0.86;
            }
            self.splat_attractor_particles(time, audio);
        }

        let aspect = w as f32 / h.max(1) as f32;
        let inv_zoom = 1.0 / (self.zoom * (1.0 + 0.16 * audio.sub_bass + 0.12 * audio.beat_impulse));
        let phase = audio.phase_integral;
        let folds = self.symmetry_folds;
        let half_sector = PI / (folds.max(2) as f32);

        for py in 0..h {
            let ny = (py as f32 / h as f32) * 2.0 - 1.0;
            for px in 0..w {
                let nx = ((px as f32 / w as f32) * 2.0 - 1.0) * aspect;
                let x = nx * inv_zoom;
                let y = ny * inv_zoom;

                let r = (x * x + y * y).sqrt();
                let raw_theta = y.atan2(x);

                // Apply slow counter-rotating twist before Dihedral folding
                let twisted_theta = raw_theta + 0.25 * phase * (1.0 - 0.35 * r).sin();
                let folded_theta = Self::fold_dihedral(twisted_theta, folds);
                let norm_angle = (folded_theta / half_sector).clamp(0.0, 1.0);

                let (intensity, color_phase, accent) = match mode {
                    VisualMode::SacredMandala => {
                        self.eval_sacred_mandala(r, folded_theta, norm_angle, phase, audio)
                    }
                    VisualMode::PoincareDisk => {
                        self.eval_poincare_disk(x, y, r, folded_theta, norm_angle, phase, audio)
                    }
                    VisualMode::ChladniCymatics => {
                        self.eval_chladni_cymatics(r, folded_theta, norm_angle, phase, audio)
                    }
                    VisualMode::QuantumVortex => {
                        self.eval_quantum_vortex(r, raw_theta, folded_theta, norm_angle, phase, audio)
                    }
                    VisualMode::AizawaAttractor => {
                        let d_val = self.density_buf[py * w + px];
                        self.eval_aizawa_field(r, folded_theta, norm_angle, d_val, phase, audio)
                    }
                    VisualMode::RadialCavaScope => {
                        self.eval_radial_cava_scope(r, raw_theta, folded_theta, norm_angle, phase, audio)
                    }
                };

                let new_rgb = palette.sample(intensity, color_phase, accent);
                let idx = py * w + px;
                let prev = self.prev_rgb_buf[idx];

                // Temporal persistence trail with instantaneous attack on bright impulses
                let persist = self.trail_persistence.clamp(0.0, 0.94);
                let r_out = (new_rgb[0] as f32).max(prev[0] as f32 * persist) as u8;
                let g_out = (new_rgb[1] as f32).max(prev[1] as f32 * persist) as u8;
                let b_out = (new_rgb[2] as f32).max(prev[2] as f32 * persist) as u8;

                let pixel = [r_out, g_out, b_out];
                self.rgb_buf[idx] = pixel;
                self.prev_rgb_buf[idx] = pixel;
            }
        }

        if self.bloom_enabled {
            self.apply_optical_bloom(audio.beat_impulse);
        }
    }

    /// Mode 1: Sacred Dihedral Mandala
    /// Combines lotus-petal harmonic interference, domain-warped concentric acoustic rings, and spectral rays.
    #[inline]
    fn eval_sacred_mandala(
        &self,
        r: f32,
        folded_theta: f32,
        norm_angle: f32,
        phase: f32,
        audio: &AudioFrame,
    ) -> (f32, f32, f32) {
        let spec_r = Self::sample_spectrum(&audio.bands, (r * 0.65).clamp(0.0, 1.0));
        let spec_ang = Self::sample_spectrum(&audio.bands, norm_angle);

        // Domain warping via acoustic waves
        let warp_r = r + 0.14 * spec_ang * (folded_theta * 6.0 - phase * 1.4).cos();
        let petal_curve = 0.35 + 0.42 * spec_r + 0.22 * (folded_theta * 4.0 + phase).cos();
        let d_petal = (warp_r - petal_curve).abs();
        let petal_glow = 0.038 / (d_petal + 0.032);

        // Concentric harmonic interference lattice
        let rings = (warp_r * (14.0 + 8.0 * audio.bass) - phase * 2.5 + spec_ang * 3.2).sin();
        let spokes = (folded_theta * (self.symmetry_folds as f32) + warp_r * 6.0).cos();
        let lattice = ((rings * spokes + 1.0) * 0.5).powf(2.4) * (0.45 + 0.95 * spec_r);

        // Core sacred geometry eye
        let core = (-r * r * (6.5 - 3.5 * audio.sub_bass)).exp() * (0.5 + 0.9 * audio.bass);

        let vignette = (-r * 0.42).exp();
        let intensity = ((petal_glow * 0.52 + lattice * 0.68 + core * 0.75) * vignette)
            + 0.18 * audio.beat_impulse * (-r * 1.2).exp();
        let color_phase = r * 0.38 - phase * 0.09 + norm_angle * 0.35 + 0.25 * spec_r;
        let accent = (spec_ang * 0.7 + audio.beat_impulse * 0.5).clamp(0.0, 1.0);

        (intensity, color_phase, accent)
    }

    /// Mode 2: Hyperbolic Poincare Disk & Complex Mobius Kaleidoscope
    /// Conformal transformation w = (z - a) / (1 - conj(a)*z) inside the hyperbolic unit disk.
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
        // Hyperbolic metric scale: ds = 2 |dz| / (1 - |z|^2)
        let r_disk = (r * 0.72).min(0.992);
        let hyp_dist = ((1.0 + r_disk) / (1.0 - r_disk + 0.008)).ln();

        // Complex Mobius translation parameter `a` orbiting with bass & phase
        let a_mag = (0.28 + 0.25 * audio.bass).min(0.68);
        let ax = a_mag * (phase * 0.7).cos();
        let ay = a_mag * (phase * 0.7).sin();

        // Numerator z - a, Denominator 1 - conj(a)*z
        let num_x = x * 0.65 - ax;
        let num_y = y * 0.65 - ay;
        let den_x = 1.0 - (ax * x * 0.65 + ay * y * 0.65);
        let den_y = -(ax * y * 0.65 - ay * x * 0.65);
        let den_norm2 = (den_x * den_x + den_y * den_y).max(0.001);

        let wx = (num_x * den_x + num_y * den_y) / den_norm2;
        let wy = (num_y * den_x - num_x * den_y) / den_norm2;
        let w_ang = Self::fold_dihedral(wy.atan2(wx), self.symmetry_folds);

        let band_u = (hyp_dist * 0.22 + norm_angle * 0.15).fract();
        let spec = Self::sample_spectrum(&audio.bands, band_u);

        // Hyperbolic geodesic arcs
        let geodesic_a = (hyp_dist * (3.5 + 2.0 * audio.low_mid) - phase * 1.8 + spec * 2.4).sin().abs();
        let geodesic_b = (w_ang * (self.symmetry_folds as f32 * 1.5) + hyp_dist * 2.2).cos().abs();

        let web_glow = 0.045 / (geodesic_a * geodesic_b + 0.038);
        let boundary_halo = (-(r - 1.28).powi(2) * 14.0).exp() * (0.6 + audio.treble);

        let intensity = (web_glow * (0.38 + 0.82 * spec) * (-r * 0.28).exp()) + boundary_halo;
        let color_phase = hyp_dist * 0.16 + folded_theta * 0.4 - phase * 0.08;
        let accent = (spec * 0.6 + audio.treble * 0.5).clamp(0.0, 1.0);

        (intensity, color_phase, accent)
    }

    /// Mode 3: Chladni Cymatic Resonance Plate
    /// Solves the 2D standing-wave Helmholtz nodal equation:
    /// U(x, y) = A * [cos(m*pi*x)*cos(n*pi*y) - cos(n*pi*x)*cos(m*pi*y)]
    #[inline]
    fn eval_chladni_cymatics(
        &self,
        r: f32,
        folded_theta: f32,
        norm_angle: f32,
        phase: f32,
        audio: &AudioFrame,
    ) -> (f32, f32, f32) {
        // Polar-folded symmetric coordinates mapped onto Chladni resonant membrane
        let sym_x = r * (folded_theta * 2.0 + phase * 0.15).cos();
        let sym_y = r * (folded_theta * 2.0 + phase * 0.15).sin();

        let m = self.cymatic_m;
        let n = self.cymatic_n;
        let spec = Self::sample_spectrum(&audio.bands, (r * 0.55 + norm_angle * 0.45).clamp(0.0, 1.0));

        let wave1 = (m * PI * sym_x).cos() * (n * PI * sym_y).cos();
        let wave2 = (n * PI * sym_x).cos() * (m * PI * sym_y).cos();
        let helmholtz = (wave1 - wave2 + 0.35 * spec * (r * 10.0 - phase * 2.0).sin()).abs();

        // Nodal lines glow intensely where standing wave displacement approaches zero
        let nodal_line = 0.032 / (helmholtz + 0.028);
        let antinode_shimmer = (1.0 - helmholtz.min(1.0)).powf(3.0) * spec * 1.1;

        let vignette = (-r * 0.36).exp();
        let intensity = (nodal_line * (0.45 + 0.75 * audio.rms_energy) + antinode_shimmer) * vignette;
        let color_phase = helmholtz * 0.45 + r * 0.3 - phase * 0.07;
        let accent = (1.0 / (helmholtz * 8.0 + 1.0) * audio.beat_impulse).clamp(0.0, 1.0);

        (intensity, color_phase, accent)
    }

    /// Mode 4: Quantum Relativistic Vortex & Spiral Interference Tunnel
    /// Logarithmic spiral wavepackets with frame-dragging relativistic lensing.
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
        let safe_r = r.max(0.04);
        let log_r = safe_r.ln();
        let spec = Self::sample_spectrum(&audio.bands, norm_angle);

        // Relativistic frame-dragging spiral arms
        let spiral_a = (log_r * (4.5 + 2.5 * audio.bass) + raw_theta * (self.symmetry_folds as f32 * 0.5) - phase * 2.8).sin();
        let spiral_b = (log_r * 6.0 - folded_theta * (self.symmetry_folds as f32) + phase * 1.9 + spec * 3.0).cos();

        let interference = (spiral_a * spiral_b + 1.0) * 0.5;
        let tunnel_depth = (1.0 / (safe_r + 0.35)).min(2.2);
        let event_horizon = (-((r - 0.28 - 0.12 * audio.sub_bass) * 6.5).powi(2)).exp();

        let intensity = (interference.powf(2.2) * (0.45 + 0.95 * spec) * tunnel_depth * 0.55
            + event_horizon * (0.7 + 0.8 * audio.bass))
            * (-r * 0.32).exp();
        let color_phase = log_r * 0.22 - phase * 0.12 + norm_angle * 0.5;
        let accent = (event_horizon * 0.8 + audio.beat_impulse * 0.4).clamp(0.0, 1.0);

        (intensity, color_phase, accent)
    }

    /// Mode 5: Aizawa Strange Attractor (RK4 3D Chaotic Flow folded into Dihedral Symmetry)
    fn splat_attractor_particles(&mut self, time: f32, audio: &AudioFrame) {
        let w = self.width as f32;
        let h = self.height as f32;
        let aspect = w / h.max(1.0);
        let rot = time * 0.45 + audio.phase_integral * 0.25;
        let (sin_r, cos_r) = rot.sin_cos();
        let folds = self.symmetry_folds.max(2);

        for (i, p) in self.particles.iter().enumerate() {
            // 3D rotation around Z and X axes
            let rx = p.x * cos_r - p.y * sin_r;
            let ry = p.x * sin_r + p.y * cos_r;
            let rz = p.z - 0.7;

            let scale = 1.25 / (2.4 - rz * 0.45).max(0.4);
            let proj_r = (rx * rx + ry * ry).sqrt() * scale * 0.52;
            let base_ang = ry.atan2(rx);
            let band_val = audio.bands[i % NUM_BANDS];

            // Replicate particle across all Dihedral symmetry sectors
            for k in 0..folds {
                let sector_ang = base_ang + (k as f32) * (2.0 * PI / folds as f32);
                let sx = (proj_r * sector_ang.cos() / aspect + 1.0) * 0.5 * w;
                let sy = (proj_r * sector_ang.sin() + 1.0) * 0.5 * h;

                let ix = sx as isize;
                let iy = sy as isize;
                if ix >= 1 && ix < (self.width as isize - 1) && iy >= 1 && iy < (self.height as isize - 1) {
                    let energy = 0.38 + 0.75 * band_val;
                    let center = iy as usize * self.width + ix as usize;
                    self.density_buf[center] = (self.density_buf[center] + energy).min(2.2);
                    // Soft 4-neighbor Gaussian splat
                    let neighbor_e = energy * 0.35;
                    self.density_buf[center - 1] = (self.density_buf[center - 1] + neighbor_e).min(2.0);
                    self.density_buf[center + 1] = (self.density_buf[center + 1] + neighbor_e).min(2.0);
                    self.density_buf[center - self.width] = (self.density_buf[center - self.width] + neighbor_e).min(2.0);
                    self.density_buf[center + self.width] = (self.density_buf[center + self.width] + neighbor_e).min(2.0);
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
        // Subtle background geometric harmonic web so the attractor floats in a sacred space
        let bg_web = ((r * 8.0 - phase).sin() * (folded_theta * self.symmetry_folds as f32).cos()).abs();
        let bg_glow = (0.018 / (bg_web + 0.06)) * (0.25 + 0.45 * spec) * (-r * 0.5).exp();

        let intensity = density * 0.85 + bg_glow;
        let color_phase = r * 0.35 + norm_angle * 0.4 - phase * 0.08 + density * 0.2;
        let accent = (density * 0.6 + audio.beat_impulse * 0.4).clamp(0.0, 1.0);
        (intensity, color_phase, accent)
    }

    /// Mode 6: Radial CAVA Mirror Scope
    /// Direct homage to classic CAVA bars & peak-gravity indicators, wrapped into a kaleidoscopic
    /// star-burst mandala with an inner Lissajous waveform core.
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
        // Quantize angle into discrete frequency bars within each mirror wedge
        let bars_per_wedge = 16.0;
        let bar_pos = norm_angle * bars_per_wedge;
        let bar_idx_norm = bar_pos.floor() / bars_per_wedge;
        let bar_edge = (bar_pos.fract() - 0.5).abs() * 2.0; // [0 at center of bar, 1 at gap]

        let band_amp = Self::sample_spectrum(&audio.bands, bar_idx_norm);
        let peak_amp = Self::sample_spectrum(&audio.peaks, bar_idx_norm);

        let inner_radius = 0.24 + 0.08 * audio.sub_bass;
        let bar_top = inner_radius + band_amp * 0.72;
        let peak_radius = inner_radius + peak_amp * 0.72;

        // Crisp bar body with rounded neon edges + mirror reflection inward
        let mut intensity = 0.0;
        if bar_edge < 0.78 {
            let edge_smooth = (1.0 - (bar_edge / 0.78).powi(2)).max(0.0);
            if r >= inner_radius && r <= bar_top {
                let height_grad = (r - inner_radius) / (bar_top - inner_radius + 0.001);
                intensity += (0.45 + 0.75 * height_grad) * edge_smooth;
            } else if r < inner_radius && r >= inner_radius * (1.0 - band_amp * 0.45) {
                // Inner CAVA reflection
                intensity += 0.35 * edge_smooth;
            }
            // Floating gravity peak indicator dot
            let d_peak = (r - peak_radius).abs();
            if d_peak < 0.024 {
                intensity += (1.0 - d_peak / 0.024) * 1.15 * edge_smooth;
            }
        }

        // Outer oscilloscope waveform ring
        let wave_idx = ((norm_angle * 255.0) as usize).min(255);
        let wave_r = 0.98 + audio.waveform[wave_idx] * 0.14;
        let d_wave = (r - wave_r).abs();
        let wave_glow = 0.016 / (d_wave + 0.018);

        intensity += wave_glow * 0.55 * (-r * 0.25).exp();
        let color_phase = bar_idx_norm * 0.75 + folded_theta * 0.2 - phase * 0.06;
        let accent = ((r - bar_top).abs() < 0.03) as i32 as f32 * 0.6 + audio.beat_impulse * 0.4;

        (intensity, color_phase, accent.clamp(0.0, 1.0))
    }

    /// Fast 5-point cross optical bloom pass for glowing neon highlights
    fn apply_optical_bloom(&mut self, beat_impulse: f32) {
        let w = self.width;
        let h = self.height;
        if w < 3 || h < 3 {
            return;
        }
        let boost = (0.16 + 0.12 * beat_impulse).clamp(0.1, 0.32);

        for y in 1..(h - 1) {
            let row = y * w;
            for x in 1..(w - 1) {
                let idx = row + x;
                let n = self.prev_rgb_buf[idx - w];
                let s = self.prev_rgb_buf[idx + w];
                let e = self.prev_rgb_buf[idx + 1];
                let w_px = self.prev_rgb_buf[idx - 1];

                for c in 0..3 {
                    let neighbor_avg = (n[c] as u16 + s[c] as u16 + e[c] as u16 + w_px[c] as u16) >> 2;
                    if neighbor_avg > 110 {
                        let glow = ((neighbor_avg - 110) as f32 * boost) as u16;
                        self.rgb_buf[idx][c] = (self.rgb_buf[idx][c] as u16 + glow).min(255) as u8;
                    }
                }
            }
        }
    }
}
