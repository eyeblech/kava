<div align="center">

# ❂ KAVA

### **K**aleidoscopic **A**udio **V**isualizer & **A**ttractor

*Like [CAVA](https://github.com/karlstav/cava), reimagined as a physics-accurate, hypnotic sacred-geometry kaleidoscope for your terminal.*

[![Rust](https://img.shields.io/badge/built_with-Rust_1.96+-dca282.svg?style=for-the-badge&logo=rust)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-a371f7.svg?style=for-the-badge)](./LICENSE)
[![TrueColor](https://img.shields.io/badge/Terminal-24--bit_Oklab_TrueColor-00f5d4.svg?style=for-the-badge)](#perceptual-oklab-color-palettes)

</div>

---

**KAVA** is an ultra-fast, zero-flicker terminal audio visualizer written in pure Rust. While classic **CAVA** renders 1D vertical frequency bars, **KAVA** projects real-time acoustic spectra through **Dihedral Symmetry Groups ($\mathcal{D}_N$)**, **Hyperbolic Poincaré Conformal Disks**, **2D Helmholtz Chladni Cymatics**, and **4th-Order Runge-Kutta Strange Attractors**—all rendered at 60–144 FPS using subpixel Unicode half-blocks (`▀`), 2×4 Braille vector grids (`⣿`), and **Oklab perceptual color science**.

---

## ✨ Highlights & Mathematical Foundations

### 1. Real-Time Acoustic DSP & CAVA Physics
* **Desktop Audio Loopback Out-of-the-Box**: Automatically hooks into your default **PipeWire / PulseAudio** desktop sink `.monitor` stream (or falls back to **CPAL** ALSA/CoreAudio/WASAPI input devices)—no virtual cable or manual routing required.
* **Autonomous Cosmic Harmonic Synth**: If no external music is playing (or when toggled with `[S]`), KAVA seamlessly crossfades into an internal polyrhythmic FM/sub-bass synthesizer so the visuals breathe and groove continuously.
* **2048-Point Hann-Windowed FFT**: Maps frequencies logarithmically across 64 perceptual bands ($24\text{ Hz} \to 18\text{ kHz}$) with equal-loudness pink-noise tilt compensation.
* **Damped Harmonic Oscillator & MonsterCat Filter**: Every frequency band obeys a 2nd-order differential spring-damper equation ($\ddot{x} = k(T - x) - c\dot{x}$) paired with CAVA's spatial neighbor falloff and parabolic peak gravity.

### 2. Six Physics & Sacred Geometry Engines (`[1]`–`[6]`)
| Key | Mode Name | Mathematical Formulation |
| :-: | :--- | :--- |
| **`1`** | **Sacred Dihedral Mandala** | Polar domain-warped harmonic interference folded across the dihedral group $\mathcal{D}_N$ ($2 \le N \le 24$ mirror axes). |
| **`2`** | **Hyperbolic Poincaré Disk** | Conformal Möbius transformations $w = e^{i\phi}\frac{z - a}{1 - \bar{a}z}$ inside the hyperbolic metric $ds = \frac{2\|dz\|}{1 - \|z\|^2}$. |
| **`3`** | **Chladni Cymatic Resonance** | Real-time 2D Helmholtz standing wave nodal plate: $\cos(m\pi x)\cos(n\pi y) - \cos(n\pi x)\cos(m\pi y) = 0$. |
| **`4`** | **Quantum Relativistic Vortex** | Logarithmic spiral wavepackets with relativistic frame-dragging and bass-driven event horizon lensing. |
| **`5`** | **Aizawa Strange Attractor** | 420 chaotic 3D particles integrated every frame via **4th-order Runge-Kutta (RK4)** and replicated across $\mathcal{D}_N$ sectors. |
| **`6`** | **Radial CAVA Mirror Scope** | Direct homage to classic CAVA: radial frequency bars, floating gravity peaks, inner mirror reflections, and an outer Lissajous waveform ring. |

### 3. Perceptual Oklab Color Palettes (`[C]` / `[P]`)
Instead of naïve linear RGB interpolation (which creates muddy gray dead-zones), KAVA synthesizes colors in **Oklch / Oklab perceptual color space** (`L`, `C`, `h`) before applying sRGB gamma transfer and a **5-point optical neon bloom pass**:
* 🌌 **Cosmic Nebula** — Deep indigo, electric violet, magenta, solar gold core
* 🪼 **Bioluminescence** — Abyssal navy, glowing teal, seafoam emerald, electric cyan
* ☀️ **Solar Flare** — Crimson shadow, molten amber, incandescent white-gold
* 🍄 **DMT Hyperspace** — Full-spectrum iridescent psychedelic harmonics
* ❄️ **Aurora Borealis** — Boreal night, emerald green, polar rose
* 🌸 **Zen Lotus** — Warm twilight plum, sakura rose, lotus peach, soft pearl
* ⚡ **Cyberpunk Acid** — Laser magenta, electric cyan, hyper lime
* 👑 **Obsidian Gold** — Velvet obsidian, antique bronze, royal liquid gold

---

## 🚀 Installation

### From Source (Cargo)
Ensure you have the Rust toolchain (`cargo`) and standard audio headers (`alsa-lib` / `libpulse` on Linux) installed:

```bash
git clone https://github.com/eyeblech/kava.git
cd kava
cargo install --path .
```

Or build and run directly in release mode:

```bash
cargo run --release
```

---

## 🎮 Interactive Controls

While **KAVA** is running in your terminal, you can sculpt the physics and aesthetics in real time:

| Keybinding | Action |
| :--- | :--- |
| **`1` – `6`** or **`M` / `Tab`** | Switch between the 6 Physics & Geometry Engines |
| **`C`** or **`P`** | Cycle through the 8 Oklab Perceptual Color Palettes |
| **`[`** / **`]`** | Decrease / Increase Dihedral Mirror Symmetry Folds ($\mathcal{D}_2 \dots \mathcal{D}_{24}$) |
| **`+`** / **`-`** | Increase / Decrease Audio Sensitivity & Gain Boost |
| **`Up`** / **`Down`** | Smoothly Zoom In / Out of the Kaleidoscope Core |
| **`R`** | Cycle Render Style: **TrueColor Subpixel (`▀`)** $\to$ **2×4 Braille (`⣿`)** $\to$ **Sacred Glyphs (`✦`)** |
| **`B`** | Toggle 5-Point Optical Neon Bloom Pass |
| **`S`** | Toggle Autonomous Cosmic Harmonic Synthesizer Mode |
| **`H`** | Toggle Bottom CAVA Sparkline & Telemetry HUD |
| **`Space`** | Freeze / Unfreeze Simulation Frame |
| **`?`** | Toggle On-Screen Interactive Help Overlay |
| **`Q`** / **`Esc`** | Exit cleanly |

---

## ⚙️ Command-Line Options & Configuration

```text
Usage: kava [OPTIONS]

Options:
  -m, --mode <MODE>              Initial visual physics mode (1..6) [default: 1]
  -p, --palette <PALETTE>        Color palette (cosmic, bio, solar, dmt, aurora, zen, cyber, gold) [default: cosmic]
  -f, --folds <FOLDS>            Dihedral kaleidoscope mirror folds (2 to 24) [default: 8]
      --fps <FPS>                Target frames per second [default: 60]
      --sensitivity < SENS >     Audio sensitivity multiplier [default: 1.0]
  -s, --synth                    Start in autonomous Cosmic Harmonic Synthesizer mode
  -d, --device <DEVICE>          Custom PulseAudio/PipeWire source or CPAL input device name
      --no-hud                   Hide bottom HUD status bar on startup
  -h, --help                     Print help
  -V, --version                  Print version
```

### Persistent Config File
KAVA automatically loads `~/.config/kava/config.toml` if present. Copy the included [`config.example.toml`](./config.example.toml) to get started:

```bash
mkdir -p ~/.config/kava
cp config.example.toml ~/.config/kava/config.toml
```

---

## License

Licensed under the [MIT License](./LICENSE).
