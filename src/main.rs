mod audio;
mod color;
mod config;
mod physics;
mod render;
mod term;

use crate::audio::AudioEngine;
use crate::color::Palette;
use crate::config::{CliArgs, FileConfig};
use crate::physics::{PhysicsField, VisualMode};
use crate::render::TerminalRenderer;
use crate::term::{poll_key, terminal_size, InputKey, RawTerminalGuard};

use std::io::{self, BufWriter};
use std::time::{Duration, Instant};

fn main() -> io::Result<()> {
    let args = CliArgs::parse();
    let file_cfg = FileConfig::load_default();

    let mode_idx = file_cfg.as_ref().and_then(|c| c.mode).unwrap_or(args.mode);
    let palette_str = file_cfg
        .as_ref()
        .and_then(|c| c.palette.clone())
        .unwrap_or_else(|| args.palette.clone());
    let folds = file_cfg.as_ref().and_then(|c| c.folds).unwrap_or(args.folds);
    let target_fps = file_cfg
        .as_ref()
        .and_then(|c| c.fps)
        .unwrap_or(args.fps)
        .clamp(15, 144);
    let mut sensitivity = file_cfg
        .as_ref()
        .and_then(|c| c.sensitivity)
        .unwrap_or(args.sensitivity)
        .clamp(0.1, 5.0);
    let custom_device = args
        .device
        .clone()
        .or_else(|| file_cfg.as_ref().and_then(|c| c.device.clone()));

    let mut mode = VisualMode::from_index(mode_idx);
    let mut palette = Palette::from_name(&palette_str);

    // Non-interactive preview mode for terminal verification / CI tests
    if let Some(frames) = args.preview_frames {
        let cols = 120u16;
        let rows = 42u16;
        let mut renderer = TerminalRenderer::new();
        renderer.show_hud = !args.no_hud;
        let (fw, fh) = renderer.field_dimensions(cols, rows);
        let mut field = PhysicsField::new(fw, fh, folds);
        let mut audio = AudioEngine::new(true, None, sensitivity);
        let mut stdout = BufWriter::with_capacity(262_144, io::stdout());

        for i in 0..frames {
            let t = 1.2 + i as f32 * (1.0 / 60.0);
            let a_frame = audio.update(t, 1.0 / 60.0, sensitivity);
            field.render(mode, palette, &a_frame, t, 1.0 / 60.0);
            if i + 1 == frames {
                if let Some(ref ppm_path) = args.dump_ppm {
                    let mut ppm = Vec::with_capacity(fw * fh * 3 + 64);
                    let header = format!("P6\n{} {}\n255\n", fw, fh);
                    ppm.extend_from_slice(header.as_bytes());
                    for px in &field.rgb_buf {
                        ppm.extend_from_slice(px);
                    }
                    std::fs::write(ppm_path, ppm)?;
                } else {
                    renderer.draw_frame(
                        &mut stdout,
                        cols,
                        rows,
                        &field,
                        &a_frame,
                        mode,
                        palette,
                        60.0,
                        sensitivity,
                    )?;
                    println!();
                }
            }
        }
        return Ok(());
    }

    let _guard = RawTerminalGuard::enter()?;
    let mut stdout = BufWriter::with_capacity(262_144, io::stdout());

    let (init_cols, init_rows) = terminal_size();
    let mut renderer = TerminalRenderer::new();
    renderer.show_hud = !args.no_hud;

    let (fw, fh) = renderer.field_dimensions(init_cols, init_rows);
    let mut field = PhysicsField::new(fw, fh, folds);
    if let Some(ref cfg) = file_cfg {
        if let Some(b) = cfg.bloom {
            field.bloom_enabled = b;
        }
        if let Some(tp) = cfg.trail_persistence {
            field.trail_persistence = tp.clamp(0.0, 0.94);
        }
    }

    let mut audio_engine = AudioEngine::new(args.synth, custom_device, sensitivity);

    let frame_duration = Duration::from_secs_f32(1.0 / target_fps as f32);
    let start_instant = Instant::now();
    let mut last_frame = Instant::now();
    let mut smoothed_fps = target_fps as f32;
    let mut paused = false;

    loop {
        let loop_start = Instant::now();
        let dt = loop_start
            .duration_since(last_frame)
            .as_secs_f32()
            .clamp(0.001, 0.1);
        last_frame = loop_start;
        smoothed_fps = smoothed_fps * 0.92 + (1.0 / dt) * 0.08;

        while let Some(key) = poll_key() {
            match key {
                InputKey::CtrlC | InputKey::Esc | InputKey::Char('q') | InputKey::Char('Q') => {
                    return Ok(());
                }
                InputKey::Char('1') => mode = VisualMode::SacredMandala,
                InputKey::Char('2') => mode = VisualMode::PoincareDisk,
                InputKey::Char('3') => mode = VisualMode::ChladniCymatics,
                InputKey::Char('4') => mode = VisualMode::QuantumVortex,
                InputKey::Char('5') => mode = VisualMode::AizawaAttractor,
                InputKey::Char('6') => mode = VisualMode::RadialCavaScope,
                InputKey::Char('m') | InputKey::Tab | InputKey::Right => mode = mode.next(),
                InputKey::Char('M') | InputKey::Left => mode = mode.prev(),
                InputKey::Char('c') | InputKey::Char('p') => palette = palette.next(),
                InputKey::Char('C') | InputKey::Char('P') => palette = palette.prev(),
                InputKey::Char(']') | InputKey::Char('}') => {
                    field.symmetry_folds = (field.symmetry_folds + 1).min(24);
                }
                InputKey::Char('[') | InputKey::Char('{') => {
                    field.symmetry_folds = field.symmetry_folds.saturating_sub(1).max(2);
                }
                InputKey::Char('+') | InputKey::Char('=') => {
                    sensitivity = (sensitivity + 0.1).min(5.0);
                }
                InputKey::Char('-') | InputKey::Char('_') => {
                    sensitivity = (sensitivity - 0.1).max(0.2);
                }
                InputKey::Up => {
                    field.zoom = (field.zoom * 1.08).min(3.5);
                }
                InputKey::Down => {
                    field.zoom = (field.zoom / 1.08).max(0.4);
                }
                InputKey::Char('r') | InputKey::Char('R') => {
                    renderer.style = renderer.style.next();
                }
                InputKey::Char('b') | InputKey::Char('B') => {
                    field.bloom_enabled = !field.bloom_enabled;
                }
                InputKey::Char('s') | InputKey::Char('S') => {
                    audio_engine.toggle_synth();
                }
                InputKey::Char('h') | InputKey::Char('H') => {
                    renderer.show_hud = !renderer.show_hud;
                }
                InputKey::Char('?') | InputKey::Char('/') => {
                    renderer.show_help = !renderer.show_help;
                }
                InputKey::Char(' ') => {
                    paused = !paused;
                }
                _ => {}
            }
        }

        let (cols, rows) = terminal_size();
        let (nfw, nfh) = renderer.field_dimensions(cols, rows);
        field.resize(nfw, nfh);

        if !paused {
            let elapsed = start_instant.elapsed().as_secs_f32();
            let audio_frame = audio_engine.update(elapsed, dt, sensitivity);
            field.render(mode, palette, &audio_frame, elapsed, dt);
            renderer.draw_frame(
                &mut stdout,
                cols,
                rows,
                &field,
                &audio_frame,
                mode,
                palette,
                smoothed_fps,
                sensitivity,
            )?;
        }

        let work_elapsed = loop_start.elapsed();
        if work_elapsed < frame_duration {
            std::thread::sleep(frame_duration - work_elapsed);
        }
    }
}
