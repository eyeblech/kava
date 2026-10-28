use std::f32::consts::PI;

/// Oklab perceptual color space representation.
/// Produces mathematically uniform, vivid, glowing gradients without muddy gray dead-zones.
#[derive(Clone, Copy, Debug)]
pub struct Oklab {
    pub l: f32,
    pub a: f32,
    pub b: f32,
}

#[allow(dead_code)]
impl Oklab {
    pub const fn new(l: f32, a: f32, b: f32) -> Self {
        Self { l, a, b }
    }

    /// Construct from Polar Oklch (Lightness, Chroma, Hue in radians)
    pub fn from_lch(l: f32, c: f32, h: f32) -> Self {
        Self {
            l,
            a: c * h.cos(),
            b: c * h.sin(),
        }
    }

    pub fn lerp(self, other: Self, t: f32) -> Self {
        Self {
            l: self.l + (other.l - self.l) * t,
            a: self.a + (other.a - self.a) * t,
            b: self.b + (other.b - self.b) * t,
        }
    }

    /// Convert Oklab to standard sRGB [0..255] with soft highlight roll-off
    #[inline]
    pub fn to_rgb(self) -> [u8; 3] {
        let l_ = self.l + 0.3963377774 * self.a + 0.2158037573 * self.b;
        let m_ = self.l - 0.1055613458 * self.a - 0.0638541728 * self.b;
        let s_ = self.l - 0.0894841775 * self.a - 1.2914855480 * self.b;

        let l = l_ * l_ * l_;
        let m = m_ * m_ * m_;
        let s = s_ * s_ * s_;

        let r_lin = 4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s;
        let g_lin = -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s;
        let b_lin = -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s;

        [
            linear_to_srgb(r_lin),
            linear_to_srgb(g_lin),
            linear_to_srgb(b_lin),
        ]
    }
}

#[inline]
fn linear_to_srgb(x: f32) -> u8 {
    let c = x.clamp(0.0, 1.0);
    let s = if c <= 0.0031308 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0 + 0.5) as u8
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Palette {
    CosmicNebula,
    Bioluminescence,
    SolarFlare,
    DmtHyperspace,
    AuroraBorealis,
    ZenLotus,
    CyberpunkAcid,
    ObsidianGold,
}

impl Palette {
    pub const ALL: [Palette; 8] = [
        Palette::CosmicNebula,
        Palette::Bioluminescence,
        Palette::SolarFlare,
        Palette::DmtHyperspace,
        Palette::AuroraBorealis,
        Palette::ZenLotus,
        Palette::CyberpunkAcid,
        Palette::ObsidianGold,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            Palette::CosmicNebula => "Cosmic Nebula",
            Palette::Bioluminescence => "Bioluminescence",
            Palette::SolarFlare => "Solar Flare",
            Palette::DmtHyperspace => "DMT Hyperspace",
            Palette::AuroraBorealis => "Aurora Borealis",
            Palette::ZenLotus => "Zen Lotus",
            Palette::CyberpunkAcid => "Cyberpunk Acid",
            Palette::ObsidianGold => "Obsidian Gold",
        }
    }

    pub fn from_name(s: &str) -> Self {
        let lower = s.to_lowercase();
        if lower.contains("bio") {
            Palette::Bioluminescence
        } else if lower.contains("solar") || lower.contains("fire") {
            Palette::SolarFlare
        } else if lower.contains("dmt") || lower.contains("hyper") || lower.contains("trip") {
            Palette::DmtHyperspace
        } else if lower.contains("aurora") {
            Palette::AuroraBorealis
        } else if lower.contains("zen") || lower.contains("lotus") {
            Palette::ZenLotus
        } else if lower.contains("cyber") || lower.contains("acid") {
            Palette::CyberpunkAcid
        } else if lower.contains("gold") || lower.contains("obsidian") {
            Palette::ObsidianGold
        } else {
            Palette::CosmicNebula
        }
    }

    pub fn next(&self) -> Self {
        let idx = Self::ALL.iter().position(|p| p == self).unwrap_or(0);
        Self::ALL[(idx + 1) % Self::ALL.len()]
    }

    pub fn prev(&self) -> Self {
        let idx = Self::ALL.iter().position(|p| p == self).unwrap_or(0);
        Self::ALL[(idx + Self::ALL.len() - 1) % Self::ALL.len()]
    }

    /// Sample color using cosine-spectral Oklch synthesis with intensity, phase, and spectral flux.
    /// `intensity` in [0.0..1.5], `phase` in [0.0..1.0], `accent` in [0.0..1.0]
    #[inline]
    pub fn sample(&self, intensity: f32, phase: f32, accent: f32) -> [u8; 3] {
        let v = intensity.clamp(0.0, 1.35);
        if v < 0.008 {
            return [4, 4, 9];
        }

        let p = phase.rem_euclid(1.0);
        let tau = 2.0 * PI;

        let lab = match self {
            Palette::CosmicNebula => {
                // Deep indigo -> electric violet -> magenta -> cyan-gold core
                let l = (0.08 + 0.84 * v.powf(0.85) + 0.10 * accent).clamp(0.05, 0.98);
                let c = (0.24 * (v * (1.15 - 0.45 * v)).max(0.0) + 0.05 * accent).clamp(0.0, 0.30);
                let h = 4.6 + 1.9 * (p * tau).sin() + 0.9 * v;
                Oklab::from_lch(l, c, h)
            }
            Palette::Bioluminescence => {
                // Abyssal navy -> glowing teal -> seafoam emerald -> electric cyan
                let l = (0.07 + 0.85 * v.powf(0.82) + 0.08 * accent).clamp(0.04, 0.98);
                let c = (0.22 * (v * (1.2 - 0.5 * v)).max(0.0) + 0.04 * accent).clamp(0.0, 0.28);
                let h = 3.3 + 0.85 * (p * tau).cos() - 0.55 * v;
                Oklab::from_lch(l, c, h)
            }
            Palette::SolarFlare => {
                // Crimson shadow -> molten amber -> solar gold -> incandescent white
                let l = (0.06 + 0.88 * v.powf(0.80) + 0.08 * accent).clamp(0.04, 0.99);
                let c = (0.25 * (v * (1.25 - 0.65 * v)).max(0.0)).clamp(0.0, 0.28);
                let h = 0.45 + 0.85 * v + 0.35 * (p * tau).sin();
                Oklab::from_lch(l, c, h)
            }
            Palette::DmtHyperspace => {
                // Full iridescent psychedelic spectral wheel with high chroma & harmonic ripples
                let l = (0.09 + 0.82 * v.powf(0.80) + 0.08 * accent).clamp(0.05, 0.97);
                let c = (0.28 * (v * (1.15 - 0.35 * v)).max(0.0) + 0.05).clamp(0.02, 0.32);
                let h = p * tau + 2.2 * v + 0.8 * accent;
                Oklab::from_lch(l, c, h)
            }
            Palette::AuroraBorealis => {
                // Boreal night -> emerald green -> borealis violet -> polar rose
                let l = (0.07 + 0.84 * v.powf(0.84) + 0.08 * accent).clamp(0.04, 0.97);
                let c = (0.23 * (v * (1.18 - 0.45 * v)).max(0.0)).clamp(0.0, 0.27);
                let h = 2.6 + 1.65 * (p * tau).sin() - 0.7 * v;
                Oklab::from_lch(l, c, h)
            }
            Palette::ZenLotus => {
                // Warm twilight plum -> sakura rose -> lotus peach -> soft pearl cream
                let l = (0.09 + 0.83 * v.powf(0.82) + 0.06 * accent).clamp(0.06, 0.97);
                let c = (0.18 * (v * (1.15 - 0.55 * v)).max(0.0)).clamp(0.0, 0.21);
                let h = 5.8 + 0.95 * (p * tau).sin() + 0.75 * v;
                Oklab::from_lch(l, c, h)
            }
            Palette::CyberpunkAcid => {
                // Neon laser magenta -> electric cyan -> hyper lime
                let l = (0.07 + 0.86 * v.powf(0.82) + 0.08 * accent).clamp(0.04, 0.98);
                let c = (0.29 * (v * (1.2 - 0.4 * v)).max(0.0)).clamp(0.0, 0.32);
                let h = 5.4 + 2.4 * (p * tau).cos() + 1.1 * v;
                Oklab::from_lch(l, c, h)
            }
            Palette::ObsidianGold => {
                // Velvet obsidian -> antique bronze -> royal liquid gold -> diamond glint
                let l = (0.05 + 0.88 * v.powf(0.84) + 0.09 * accent).clamp(0.03, 0.99);
                let c = (0.19 * (v * (1.2 - 0.6 * v)).max(0.0)).clamp(0.0, 0.22);
                let h = 1.28 + 0.28 * (p * tau).sin() + 0.2 * v;
                Oklab::from_lch(l, c, h)
            }
        };

        lab.to_rgb()
    }
}
