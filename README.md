# kava

**kava** is a terminal audio visualizer written in Rust. If you like [cava](https://github.com/karlstav/cava) for its speed and simplicity, `kava` takes the same desktop audio loopback workflow and maps the frequency spectrum into radial kaleidoscopes, standing-wave cymatics, and chaotic attractors.

It has zero external Rust crate dependencies, compiles in about ten seconds, and stays completely dark and idle until sound is actually playing on your system.

---

## How it works

1. **Audio capture**: On Linux, `kava` queries your default PipeWire or PulseAudio sink (`pactl`) and attaches to its `.monitor` stream via `parec` (falling back to `pw-cat` or `arecord`). On macOS, it reads via `ffmpeg` (`avfoundation`) or `sox` (`rec`).
2. **Signal processing**: Audio samples pass through a 2048-point Hann-windowed Cooley-Tukey FFT and are grouped into 64 logarithmic frequency bands from 24 Hz to 18 kHz. Each band is smoothed with a second-order spring-damper solver and a spatial neighbor falloff filter (the same MonsterCat smoothing approach used in CAVA) plus spectral-flux onset detection for beat impulses.
3. **Silence gate**: When no audio is playing, the simulation decays smoothly to a blank canvas and waits. If you want to preview the visuals without playing music, press `s` (or pass `--synth`) to turn on the built-in harmonic test signal.
4. **Rendering**: Visuals are evaluated in floating-point HDR space with rotated-grid supersampling (RGSSAA), a two-pass separable 5-tap Gaussian bloom filter, and cosine color palettes tone-mapped to 24-bit TrueColor. Frames are flushed using DEC Mode 2026 synchronized terminal output (`\x1b[?2026h`) to prevent tearing.

---

## Visual modes

Press `1` through `6` (or `m` / `Tab`) at runtime to switch engines:

* **1 — Sacred Dihedral Mandala**: Concentric signed-distance-field lotus ribbons and radial interference filaments folded across the dihedral symmetry group $D_N$ ($2 \le N \le 24$ mirror axes).
* **2 — Hyperbolic Poincare Disk**: Conformal Mobius transformations $w = \frac{z - a}{1 - \bar{a}z}$ inside the hyperbolic Poincare disk metric.
* **3 — Chladni Cymatic Resonance**: 2D Helmholtz standing-wave nodal plate ($\cos(m\pi x)\cos(n\pi y) - \cos(n\pi x)\cos(m\pi y) = 0$) where mode numbers $(m, n)$ shift with bass and spectral centroid.
* **4 — Quantum Relativistic Vortex**: Logarithmic spiral wavepackets with frame-dragging twist and a bass-driven inner event horizon ring.
* **5 — Aizawa Strange Attractor**: 480 particles integrated in 3D via a 4th-order Runge-Kutta (RK4) solver and projected across all $D_N$ symmetry sectors.
* **6 — Radial CAVA Mirror Scope**: Continuous spline-interpolated radial frequency spectrum with a floating peak-gravity contour and an inner mirror reflection.

---

## Installation

### Quick install script (Linux & macOS)

Downloads a prebuilt binary from GitHub Releases if available, or builds from source via `cargo`:

```bash
curl -fsSL https://raw.githubusercontent.com/eyeblech/kava/main/install.sh | bash
```

### Cargo install

```bash
cargo install --git https://github.com/eyeblech/kava.git
```

### Build from source

```bash
git clone https://github.com/eyeblech/kava.git
cd kava
cargo build --release
./target/release/kava
```

---

## Keybindings

| Key | Action |
| :--- | :--- |
| `1` – `6`, `m`, `Tab` | Switch visual mode |
| `c`, `p` | Cycle color palette (`cosmic`, `bio`, `solar`, `dmt`, `aurora`, `zen`, `cyber`, `gold`) |
| `[` / `]` | Decrease / increase kaleidoscope mirror folds ($D_2 \dots D_{24}$) |
| `+` / `-` | Increase / decrease audio sensitivity |
| `Up` / `Down` | Zoom in / out |
| `r` | Cycle character renderer: Half-block (`▀`), Braille (`⣿`), or ASCII glyphs |
| `b` | Toggle Gaussian bloom pass |
| `s` | Toggle internal synth test signal |
| `h` | Toggle bottom status bar |
| `Space` | Pause / resume frame |
| `?` | Toggle help overlay |
| `q`, `Esc` | Quit |

---

## Command-line options

```text
Usage: kava [OPTIONS]

Options:
  -m, --mode <1..6>          Initial visual mode [default: 1]
  -p, --palette <NAME>       Color palette (cosmic, bio, solar, dmt, aurora, zen, cyber, gold) [default: cosmic]
  -f, --folds <2..24>        Kaleidoscope mirror folds [default: 8]
      --fps <15..144>        Target frame rate [default: 60]
      --sensitivity <FLOAT>  Audio gain multiplier [default: 1.0]
  -s, --synth                Start with the internal synth signal enabled
  -d, --device <NAME>        Custom PulseAudio/PipeWire or ALSA source name
      --no-hud               Hide the bottom status bar on startup
  -h, --help                 Print help
  -V, --version              Print version
```

### Configuration file

If `~/.config/kava/config.toml` exists, `kava` reads your default preferences from it on startup. See [`config.example.toml`](./config.example.toml) for all supported keys:

```bash
mkdir -p ~/.config/kava
cp config.example.toml ~/.config/kava/config.toml
```

---

## License

MIT — see [LICENSE](./LICENSE).
