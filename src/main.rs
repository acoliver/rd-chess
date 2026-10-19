use chess3d::{Piece, PieceKind, Position, Side, Square, piece_lineup_scene, render_scene};
use glam::Vec3;
use std::error::Error;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use worldrunner::scene::gpu;

#[expect(
    clippy::too_many_lines,
    reason = "CLI argument validation and capture dispatch form one compact application boundary"
)]
fn main() -> Result<(), Box<dyn Error>> {
    let mut load = None;
    let mut save = None;
    let mut capture = None;
    let mut window = false;
    let mut view = String::from("hero");
    let mut width = 1280u32;
    let mut height = 960u32;
    let mut seed = None;
    let mut args = std::env::args_os().skip(1);
    while let Some(arg) = args.next() {
        let flag = arg.to_string_lossy().into_owned();
        let value =
            |args: &mut std::iter::Skip<std::env::ArgsOs>| -> Result<PathBuf, Box<dyn Error>> {
                args.next()
                    .map(PathBuf::from)
                    .ok_or_else(|| format!("{flag} requires a value").into())
            };
        match flag.as_str() {
            "--load" => load = Some(value(&mut args)?),
            "--save" => save = Some(value(&mut args)?),
            "--capture" => capture = Some(value(&mut args)?),
            "--window" => window = true,
            "--view" => view = value(&mut args)?.to_string_lossy().into_owned(),
            "--width" => width = value(&mut args)?.to_string_lossy().parse()?,
            "--height" => height = value(&mut args)?.to_string_lossy().parse()?,
            "--seed" => seed = Some(value(&mut args)?.to_string_lossy().parse::<u64>()?),
            "--help" => {
                println!(
                    "chess3d [--load position.json] [--save position.json] [--capture image.png | --window] [--view hero|overhead|lineup] [--width px --height px] [--seed n]
\
                     Window mode requires: cargo run -p chess3d --features viewer -- --window
\
                     Controls: left-drag orbit, wheel zoom, R/Home reset, Escape/close exit.
\
                     --save writes the current position before capture/window; camera orbit is not saved."
                );
                return Ok(());
            }
            other => return Err(format!("unknown argument {other}; use --help").into()),
        }
    }
    if window && capture.is_some() {
        return Err("--capture and --window cannot be combined; run them separately".into());
    }
    if window && !cfg!(feature = "viewer") {
        return Err("--window requires the optional viewer feature; run: cargo run -p chess3d --features viewer -- --window".into());
    }
    if !(64..=4096).contains(&width)
        || !(64..=4096).contains(&height)
        || u64::from(width) * u64::from(height) > 16_777_216
    {
        return Err(
            "dimensions must be 64..=4096 pixels and at most 16,777,216 pixels total".into(),
        );
    }
    if !["hero", "overhead", "lineup"].contains(&view.as_str()) {
        return Err("--view must be hero, overhead, or lineup".into());
    }
    if load.is_some() && seed.is_some() {
        return Err(
            "--seed cannot be combined with --load; loaded seed/style are authoritative".into(),
        );
    }
    if let Some(capture_path) = &capture {
        let capture_path = resolved_path(capture_path)?;
        for path in load.iter().chain(save.iter()) {
            if resolved_path(path)? == capture_path {
                return Err("capture path must differ from load/save position paths".into());
            }
        }
    }
    let position = if let Some(path) = load {
        Position::load(&path)?
    } else {
        Position::initial(seed.unwrap_or(342))
    };
    if let Some(path) = save {
        position.save_atomic(&path)?;
    }
    if let Some(path) = capture {
        let presentation = prepare_view(&position, &view)?;
        let viewport = gpu::Viewport { width, height };
        let camera = *presentation.scene.effects().camera();
        let camera_view = gpu::SceneView::new(presentation.pose, camera, viewport)?;
        let context = pollster::block_on(gpu::device::GpuContext::request_with_features(
            gpu::wgpu::Features::empty(),
        ))?;
        let image = gpu::shadows::capture_scene_shadowed(
            &context,
            &presentation.scene,
            &camera_view,
            presentation.shadows,
        )?;
        write_png_atomic(&path, width, height, &image.pixels)?;
        eprintln!(
            "captured {}x{} {} view to {}",
            width,
            height,
            view,
            path.display()
        );
    } else if window {
        #[cfg(feature = "viewer")]
        open_window(prepare_view(&position, &view)?, width, height)?;
    } else {
        println!(
            "Chess3D position ready: {} pieces, seed {}, style {}",
            position.pieces.len(),
            position.seed,
            position.style
        );
    }
    Ok(())
}

struct Presentation {
    scene: worldrunner::scene::plan::RenderScene,
    pose: gpu::CameraPose,
    shadows: gpu::shadows::DirectionalShadowSettings,
}

fn prepare_view(position: &Position, view: &str) -> Result<Presentation, chess3d::SceneBuildError> {
    let scene = if view == "lineup" {
        piece_lineup_scene(&lineup_position(position))?
    } else {
        render_scene(position)?
    };
    let pose = match view {
        "hero" => chess3d::CameraPreset::Hero,
        "overhead" => chess3d::CameraPreset::Overhead,
        _ => chess3d::CameraPreset::Lineup,
    }
    .pose();
    Ok(Presentation {
        scene,
        pose,
        shadows: gpu::shadows::DirectionalShadowSettings {
            focus_meters: Vec3::ZERO,
            half_extent_meters: 8.0,
            depth_half_extent_meters: 16.0,
            resolution: 2048,
            depth_bias: 0.0008,
            normal_bias_meters: 0.012,
        },
    })
}

#[cfg(feature = "viewer")]
fn open_window(
    presentation: Presentation,
    width: u32,
    height: u32,
) -> Result<(), worldrunner::scene::viewer::ViewerError> {
    use worldrunner::scene::viewer::{OrbitLimits, ViewerOptions, run};
    run(
        presentation.scene,
        ViewerOptions {
            title: "Chess3D — Nocturne / Ivory".into(),
            size: [width, height],
            pose: presentation.pose,
            orbit: OrbitLimits {
                min_distance_millimeters: 2_000.0,
                max_distance_millimeters: 40_000.0,
                min_elevation_radians: 0.1,
                max_elevation_radians: 1.47,
            },
            shadows: Some(presentation.shadows),
        },
    )
}

fn lineup_position(source: &Position) -> Position {
    let mut lineup = source.clone();
    let kinds = [
        PieceKind::Pawn,
        PieceKind::Rook,
        PieceKind::Knight,
        PieceKind::Bishop,
        PieceKind::Queen,
        PieceKind::King,
    ];
    lineup.pieces = kinds
        .into_iter()
        .enumerate()
        .map(|(file, kind)| Piece {
            id: format!("lineup-{kind:?}"),
            side: if file % 2 == 0 {
                Side::Ivory
            } else {
                Side::Onyx
            },
            kind,
            square: Square {
                file: u8::try_from(file).expect("six lineup files fit on the board") + 1,
                rank: 3,
            },
        })
        .collect();
    lineup
}

fn resolved_path(path: &std::path::Path) -> std::io::Result<PathBuf> {
    let absolute = std::path::absolute(path)?;
    let mut resolved = PathBuf::new();
    for component in absolute.components() {
        if component == std::path::Component::ParentDir {
            resolved.pop();
        } else {
            resolved.push(component.as_os_str());
            match resolved.canonicalize() {
                Ok(canonical) => resolved = canonical,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                Err(error) => return Err(error),
            }
        }
    }
    Ok(resolved)
}

fn write_png_atomic(
    path: &std::path::Path,
    width: u32,
    height: u32,
    pixels: &[u8],
) -> Result<(), Box<dyn Error>> {
    let mut file = atomic_write_file::AtomicWriteFile::open(path)?;
    {
        let mut buffer = BufWriter::new(&mut file);
        {
            let mut encoder = png::Encoder::new(&mut buffer, width, height);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header()?;
            writer.write_image_data(pixels)?;
            writer.finish()?;
        }
        buffer.flush()?;
    }
    file.sync_all()?;
    file.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_png_encoding_preserves_existing_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("capture.png");
        std::fs::write(&path, b"original capture").unwrap();
        assert!(write_png_atomic(&path, 64, 64, &[0; 4]).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"original capture");
    }

    #[test]
    fn png_encoding_round_trips_rgba_pixels() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("capture.png");
        write_png_atomic(&path, 2, 1, &[1, 2, 3, 255, 4, 5, 6, 255]).unwrap();
        let decoder =
            png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).unwrap()));
        let mut reader = decoder.read_info().unwrap();
        let mut buffer = vec![0; reader.output_buffer_size().unwrap()];
        let info = reader.next_frame(&mut buffer).unwrap();
        assert_eq!((info.width, info.height), (2, 1));
        assert_eq!(&buffer[..info.buffer_size()], &[1, 2, 3, 255, 4, 5, 6, 255]);
    }
}
