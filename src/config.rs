use std::env;
use std::fs;
use std::path::PathBuf;
use std::process;

#[derive(Debug, Clone)]
pub struct CliArgs {
    pub mode: usize,
    pub palette: String,
    pub folds: usize,
    pub fps: u32,
    pub sensitivity: f32,
    pub synth: bool,
    pub device: Option<String>,
    pub no_hud: bool,
    pub preview_frames: Option<u32>,
    pub dump_ppm: Option<String>,
}

impl Default for CliArgs {
    fn default() -> Self {
        Self {
            mode: 1,
            palette: String::from("cosmic"),
            folds: 8,
            fps: 60,
            sensitivity: 1.0,
            synth: false,
            device: None,
            no_hud: false,
            preview_frames: None,
            dump_ppm: None,
        }
    }
}

impl CliArgs {
    pub fn parse() -> Self {
        let mut cfg = Self::default();
        let args: Vec<String> = env::args().skip(1).collect();
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "-h" | "--help" => {
                    Self::print_help();
                    process::exit(0);
                }
                "-V" | "--version" => {
                    println!("kava 0.2.0");
                    process::exit(0);
                }
                "-m" | "--mode" => {
                    i += 1;
                    if let Some(v) = args.get(i).and_then(|s| s.parse().ok()) {
                        cfg.mode = v;
                    }
                }
                "-p" | "--palette" => {
                    i += 1;
                    if let Some(v) = args.get(i) {
                        cfg.palette = v.clone();
                    }
                }
                "-f" | "--folds" => {
                    i += 1;
                    if let Some(v) = args.get(i).and_then(|s| s.parse().ok()) {
                        cfg.folds = v;
                    }
                }
                "--fps" => {
                    i += 1;
                    if let Some(v) = args.get(i).and_then(|s| s.parse().ok()) {
                        cfg.fps = v;
                    }
                }
                "--sensitivity" => {
                    i += 1;
                    if let Some(v) = args.get(i).and_then(|s| s.parse().ok()) {
                        cfg.sensitivity = v;
                    }
                }
                "-s" | "--synth" => {
                    cfg.synth = true;
                }
                "-d" | "--device" => {
                    i += 1;
                    if let Some(v) = args.get(i) {
                        cfg.device = Some(v.clone());
                    }
                }
                "--no-hud" => {
                    cfg.no_hud = true;
                }
                "--preview-frames" => {
                    i += 1;
                    if let Some(v) = args.get(i).and_then(|s| s.parse().ok()) {
                        cfg.preview_frames = Some(v);
                    }
                }
                "--dump-ppm" => {
                    i += 1;
                    if let Some(v) = args.get(i) {
                        cfg.dump_ppm = Some(v.clone());
                    }
                }
                _ => {}
            }
            i += 1;
        }
        cfg
    }

    pub fn print_help() {
        println!(
            "❂ KAVA 0.2.0 — Physics-accurate terminal audio-reactive kaleidoscope & sacred geometry visualizer\n\n\
             USAGE:\n\
             \x20   kava [OPTIONS]\n\n\
             OPTIONS:\n\
             \x20   -m, --mode <1..6>          Initial visual physics engine [default: 1]\n\
             \x20                              (1=Mandala, 2=Poincare, 3=Chladni, 4=Vortex, 5=Attractor, 6=CavaScope)\n\
             \x20   -p, --palette <NAME>       Oklab color palette [default: cosmic]\n\
             \x20                              (cosmic, bio, solar, dmt, aurora, zen, cyber, gold)\n\
             \x20   -f, --folds <2..24>        Dihedral kaleidoscope mirror folds [default: 8]\n\
             \x20       --fps <15..144>        Target frames per second [default: 60]\n\
             \x20       --sensitivity <FLOAT>  Audio reactivity gain multiplier [default: 1.0]\n\
             \x20   -s, --synth                Start in autonomous Cosmic Harmonic Synthesizer mode\n\
             \x20   -d, --device <NAME>        Custom PulseAudio/PipeWire or ALSA source name\n\
             \x20       --no-hud               Hide bottom HUD status bar on startup\n\
             \x20       --preview-frames <N>   Render N frames and print a snapshot to stdout\n\
             \x20   -h, --help                 Print help information\n\
             \x20   -V, --version              Print version"
        );
    }
}

#[derive(Debug, Clone, Default)]
pub struct FileConfig {
    pub mode: Option<usize>,
    pub palette: Option<String>,
    pub folds: Option<usize>,
    pub fps: Option<u32>,
    pub sensitivity: Option<f32>,
    pub bloom: Option<bool>,
    pub trail_persistence: Option<f32>,
    pub device: Option<String>,
}

impl FileConfig {
    pub fn load_default() -> Option<Self> {
        let home = env::var("HOME").ok()?;
        let path = PathBuf::from(home).join(".config/kava/config.toml");
        let text = fs::read_to_string(path).ok()?;
        let mut cfg = Self::default();

        for line in text.lines() {
            let trimmed = line.split('#').next().unwrap_or("").trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Some((k, v)) = trimmed.split_once('=') {
                let key = k.trim();
                let val = v.trim().trim_matches('"').trim_matches('\'');
                match key {
                    "mode" => cfg.mode = val.parse().ok(),
                    "palette" => cfg.palette = Some(val.to_string()),
                    "folds" => cfg.folds = val.parse().ok(),
                    "fps" => cfg.fps = val.parse().ok(),
                    "sensitivity" => cfg.sensitivity = val.parse().ok(),
                    "bloom" => cfg.bloom = val.parse().ok(),
                    "trail_persistence" => cfg.trail_persistence = val.parse().ok(),
                    "device" => cfg.device = Some(val.to_string()),
                    _ => {}
                }
            }
        }
        Some(cfg)
    }
}
