use std::f32::consts::PI;

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

    /// Evaluates a high-contrast, ultra-poppy cosine palette:
    /// c(t) = a + b * cos(2*pi * (c*t + d))
    /// Returns linear-ish floating point RGB in [0.0 .. 1.0] before final tone-mapping.
    #[inline]
    pub fn sample_float(&self, intensity: f32, phase: f32, core_glow: f32) -> [f32; 3] {
        let v = intensity.max(0.0);
        if v < 0.001 {
            return [0.0, 0.0, 0.0];
        }

        let t = phase.rem_euclid(1.0);
        let tau = 2.0 * PI;

        let (a, b, c, d) = match self {
            Palette::CosmicNebula => (
                [0.52, 0.28, 0.68],
                [0.48, 0.46, 0.35],
                [1.0, 1.0, 1.0],
                [0.00, 0.22, 0.58],
            ),
            Palette::Bioluminescence => (
                [0.10, 0.62, 0.68],
                [0.35, 0.38, 0.32],
                [1.0, 1.0, 0.8],
                [0.50, 0.15, 0.00],
            ),
            Palette::SolarFlare => (
                [0.65, 0.32, 0.12],
                [0.35, 0.45, 0.35],
                [0.8, 0.8, 0.6],
                [0.00, 0.18, 0.45],
            ),
            Palette::DmtHyperspace => (
                [0.55, 0.52, 0.58],
                [0.45, 0.48, 0.42],
                [1.0, 1.0, 1.0],
                [0.00, 0.33, 0.67],
            ),
            Palette::AuroraBorealis => (
                [0.22, 0.58, 0.52],
                [0.42, 0.42, 0.45],
                [1.0, 0.8, 1.0],
                [0.42, 0.08, 0.25],
            ),
            Palette::ZenLotus => (
                [0.62, 0.38, 0.55],
                [0.38, 0.36, 0.38],
                [0.8, 0.8, 0.8],
                [0.05, 0.25, 0.38],
            ),
            Palette::CyberpunkAcid => (
                [0.58, 0.42, 0.62],
                [0.42, 0.52, 0.38],
                [1.0, 1.0, 1.0],
                [0.85, 0.25, 0.50],
            ),
            Palette::ObsidianGold => (
                [0.58, 0.45, 0.20],
                [0.42, 0.38, 0.28],
                [0.6, 0.6, 0.5],
                [0.05, 0.12, 0.28],
            ),
        };

        let mut rgb = [0.0f32; 3];
        // Smooth S-curve brightness so darks are pitch black and ribbons pop with neon clarity
        let gain = v.powf(0.88);
        let white_hot = (core_glow * 0.65 + (v - 0.85).max(0.0) * 0.9).clamp(0.0, 1.0);

        for i in 0..3 {
            let base = (a[i] + b[i] * (tau * (c[i] * t + d[i])).cos()).clamp(0.0, 1.0);
            // Boost chroma contrast by curving base color
            let vivid = base * base * (3.0 - 2.0 * base);
            let lit = vivid * gain + white_hot * 0.85;
            // ACES-inspired filmic highlight roll-off to pure incandescent white
            rgb[i] = (lit * (2.51 * lit + 0.03)) / (lit * (2.43 * lit + 0.59) + 0.14);
            rgb[i] = rgb[i].clamp(0.0, 1.0);
        }

        rgb
    }

    #[inline]
    pub fn float_to_u8(rgb: [f32; 3]) -> [u8; 3] {
        [
            (rgb[0].clamp(0.0, 1.0) * 255.0 + 0.5) as u8,
            (rgb[1].clamp(0.0, 1.0) * 255.0 + 0.5) as u8,
            (rgb[2].clamp(0.0, 1.0) * 255.0 + 0.5) as u8,
        ]
    }
}
