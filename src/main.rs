use chess3d::{
    CameraPreset, Piece, PieceKind, Position, Side, Square, piece_lineup_scene, render_scene,
    rules::ChessState,
};
use std::error::Error;
use std::io::{BufWriter, Write};
use std::path::PathBuf;
#[cfg(not(feature = "viewer"))]
use worldrunner::scene::gpu;
mod cli;
mod review_cli;

fn main() -> Result<(), Box<dyn Error>> {
    let Some(options) = cli::Options::parse()? else {
        return Ok(());
    };
    if let Some(moves) = &options.review_moves {
        return review_cli::run(moves, options.review_fen.as_deref());
    }
    let position = position(&options)?;
    if let Some(path) = &options.save {
        position.save_atomic(path)?;
    }
    if options.capture.is_some() || options.window {
        present(&options, &position)?;
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

fn position(options: &cli::Options) -> Result<Position, Box<dyn Error>> {
    if let Some(path) = &options.load {
        return Ok(Position::load(path)?);
    }
    let seed = options.seed.unwrap_or(342);
    let Some(moves) = &options.moves else {
        return Ok(Position::initial(seed));
    };
    let mut game = ChessState::initial();
    for uci in moves.split_whitespace() {
        let chess_move = game
            .parse_uci(uci)
            .map_err(|error| format!("invalid UCI move '{uci}': {error}"))?;
        game.play(chess_move)?;
    }
    Ok(game.to_position(seed))
}

fn prepare_view(
    position: &Position,
    view: &str,
) -> Result<(worldrunner::scene::plan::RenderScene, CameraPreset), chess3d::SceneBuildError> {
    let scene = if view == "lineup" {
        piece_lineup_scene(&lineup_position(position))?
    } else {
        render_scene(position)?
    };
    let preset = match view {
        "hero" => CameraPreset::Hero,
        "overhead" => CameraPreset::Overhead,
        _ => CameraPreset::Lineup,
    };
    Ok((scene, preset))
}

#[cfg(feature = "viewer")]
fn present(options: &cli::Options, position: &Position) -> Result<(), Box<dyn Error>> {
    use chess3d::retained::{BoardDriver, RetainedBoard, capture_frame, viewer_options};
    use worldrunner::scene::viewer::RetainedHost;
    let mut driver = BoardDriver::default();
    let (scene, preset) = if options.load.is_none() && options.view != "lineup" {
        driver = BoardDriver::with_board(RetainedBoard::new(ChessState::initial(), position.seed)?);
        (
            driver
                .board()
                .expect("rules driver owns the game")
                .scene()
                .clone(),
            if options.view == "overhead" {
                CameraPreset::Overhead
            } else {
                CameraPreset::Hero
            },
        )
    } else {
        prepare_view(position, &options.view)?
    };
    configure_game(&mut driver, options, position.seed)?;
    let preset = if options.window && !options.view_explicit && driver.board().is_some() {
        if driver.settings().overhead {
            CameraPreset::Overhead
        } else {
            CameraPreset::Hero
        }
    } else {
        preset
    };
    let host = RetainedHost::new(
        scene.clone(),
        viewer_options(&scene, preset, [options.width, options.height]),
    )?;
    if let Some(path) = &options.capture {
        let queue = driver.queue();
        let inbox = driver.captures();
        let mut run = host.windowless(&mut driver)?;
        let image = capture_frame(&mut run, &queue, &inbox)?;
        write_png_atomic(
            path,
            image.viewport.width,
            image.viewport.height,
            &image.pixels,
        )?;
        run.shutdown()?;
    } else {
        host.run(&mut driver)?;
    }
    Ok(())
}

#[cfg(feature = "viewer")]
fn configure_game(
    driver: &mut chess3d::retained::BoardDriver,
    options: &cli::Options,
    seed: u64,
) -> Result<(), Box<dyn Error>> {
    if options.window && driver.board().is_some() {
        let root = options.data_dir.clone().map_or_else(
            || worldrunner::scene::viewer::application_root("org.acoliver.chess3d"),
            Ok,
        )?;
        driver.open_store(root);
        if options.play.is_some() || options.moves.is_some() {
            driver.restart()?;
        }
    }
    if let Some(human) = options.play {
        driver.set_computer(chess3d::retained::ComputerGame::new(
            human,
            options.level.expect("validated level"),
            seed,
        )?)?;
    }
    for uci in options.moves.as_deref().unwrap_or("").split_whitespace() {
        driver.play_uci(uci)?;
    }
    if options.window
        && driver.board().is_some()
        && options.play.is_none()
        && options.moves.is_none()
    {
        driver.show_title()?;
    }
    Ok(())
}

#[cfg(not(feature = "viewer"))]
fn present(options: &cli::Options, position: &Position) -> Result<(), Box<dyn Error>> {
    let (scene, preset) = prepare_view(position, &options.view)?;
    let viewport = gpu::Viewport {
        width: options.width,
        height: options.height,
    };
    let camera_view = gpu::SceneView::new(
        preset.pose(),
        *scene
            .effects()
            .expect("chess supplies studio lighting")
            .camera(),
        viewport,
    )?;
    let context = pollster::block_on(gpu::device::GpuContext::request_with_features(
        gpu::wgpu::Features::empty(),
    ))?;
    let image = gpu::shadows::capture_scene_shadowed(
        &context,
        &scene,
        &camera_view,
        chess3d::shadow_settings(),
    )?;
    write_png_atomic(
        options.capture.as_ref().expect("capture without viewer"),
        options.width,
        options.height,
        &image.pixels,
    )?;
    Ok(())
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

#[cfg(all(test, feature = "viewer"))]
mod game_start_tests {
    use super::*;
    #[test]
    fn direct_window_moves_autosave_but_title_start_preserves_current() {
        let root = tempfile::tempdir().unwrap();
        let mut options = cli::Options {
            load: None,
            data_dir: Some(root.path().to_path_buf()),
            save: None,
            capture: None,
            window: true,
            view: "hero".into(),
            view_explicit: false,
            width: 800,
            height: 600,
            seed: Some(42),
            moves: Some("e2e4".into()),
            review_moves: None,
            review_fen: None,
            play: None,
            level: None,
        };
        let make_driver = || {
            chess3d::retained::BoardDriver::with_board(
                chess3d::retained::RetainedBoard::new(ChessState::initial(), 42).unwrap(),
            )
        };
        let mut driver = make_driver();
        configure_game(&mut driver, &options, 42).unwrap();
        assert_eq!(driver.board().unwrap().game().side_to_move(), Side::Onyx);
        let current = root.path().join("slot-current.rds");
        let saved = std::fs::read(&current).unwrap();
        options.moves = None;
        let mut title = make_driver();
        configure_game(&mut title, &options, 42).unwrap();
        assert!(title.can_continue());
        assert_eq!(title.screen(), chess3d::retained::Screen::Title);
        assert_eq!(std::fs::read(current).unwrap(), saved);
    }
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
