use std::{error::Error, path::PathBuf};

fn explanation_args(
    args: &mut impl Iterator<Item = std::ffi::OsString>,
) -> Result<(String, String)> {
    let fen = args.next().ok_or("--explain requires FEN and UCI")?;
    let uci = args.next().ok_or("--explain requires FEN and UCI")?;
    Ok((
        fen.into_string().map_err(|_| "FEN must be UTF-8")?,
        uci.into_string().map_err(|_| "UCI must be UTF-8")?,
    ))
}

const VALUE_FLAGS: &[&str] = &[
    "--load",
    "--data-dir",
    "--save",
    "--capture",
    "--view",
    "--width",
    "--height",
    "--seed",
    "--moves",
    "--play",
    "--level",
    "--lesson",
    "--puzzle",
];

type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub struct Options {
    pub load: Option<PathBuf>,
    pub data_dir: Option<PathBuf>,
    pub save: Option<PathBuf>,
    pub capture: Option<PathBuf>,
    pub window: bool,
    pub view: String,
    pub view_explicit: bool,
    pub width: u32,
    pub height: u32,
    pub seed: Option<u64>,
    pub moves: Option<String>,
    pub play: Option<chess3d::Side>,
    pub level: Option<u8>,
    pub explain: Option<(String, String)>,
    pub explain_verbose: bool,
    pub lesson: Option<String>,
    pub puzzle: Option<String>,
}

impl Options {
    fn initial() -> Self {
        Self {
            load: None,
            data_dir: None,
            save: None,
            capture: None,
            window: false,
            view: "hero".into(),
            view_explicit: false,
            width: 1280,
            height: 960,
            seed: None,
            moves: None,
            play: None,
            level: None,
            explain: None,
            explain_verbose: false,
            lesson: None,
            puzzle: None,
        }
    }

    pub fn parse() -> Result<Option<Self>> {
        let mut options = Self::initial();
        let mut args = std::env::args_os().skip(1);
        while let Some(arg) = args.next() {
            let flag = arg.to_string_lossy();
            if flag == "--help" {
                print_help();
                return Ok(None);
            }
            if flag == "--explain" {
                options.explain = Some(explanation_args(&mut args)?);
                continue;
            }
            if flag == "--explain-verbose" {
                if options.explain_verbose {
                    return Err("--explain-verbose may only be specified once".into());
                }
                options.explain_verbose = true;
                continue;
            }
            if flag == "--window" {
                options.window = true;
                continue;
            }
            if !VALUE_FLAGS.contains(&flag.as_ref()) {
                return Err(format!("unknown argument {flag}; use --help").into());
            }
            let value = args
                .next()
                .ok_or_else(|| format!("{flag} requires a value"))?;
            options.set_value(&flag, value)?;
        }
        options.validate()?;
        Ok(Some(options))
    }

    fn set_value(&mut self, flag: &str, value: std::ffi::OsString) -> Result<()> {
        let text = value.to_string_lossy().into_owned();
        match flag {
            "--load" => self.load = Some(PathBuf::from(value)),
            "--data-dir" => self.data_dir = Some(PathBuf::from(value)),
            "--save" => self.save = Some(PathBuf::from(value)),
            "--capture" => self.capture = Some(PathBuf::from(value)),
            "--view" => {
                self.view = text;
                self.view_explicit = true;
            }
            "--width" => self.width = text.parse()?,
            "--height" => self.height = text.parse()?,
            "--seed" => self.seed = Some(text.parse()?),
            "--moves" => self.moves = Some(text),
            "--lesson" => self.lesson = Some(text),
            "--puzzle" => self.puzzle = Some(text),
            "--play" => {
                self.play = Some(match text.as_str() {
                    "white" => chess3d::Side::Ivory,
                    "black" => chess3d::Side::Onyx,
                    _ => return Err("--play must be white or black".into()),
                });
            }
            "--level" => {
                let level = text.parse()?;
                if !(1..=8).contains(&level) {
                    return Err("--level must be 1..=8".into());
                }
                self.level = Some(level);
            }
            _ => unreachable!(),
        }
        Ok(())
    }

    fn validate_computer(&self) -> Result<()> {
        match (self.play, self.level) {
            (None, Some(_)) => return Err("--level requires --play".into()),
            (Some(_), None) => return Err("--play requires --level".into()),
            _ => (),
        }
        if self.play.is_some() {
            if !self.window {
                return Err("--play requires --window".into());
            }
            if self.load.is_some() || self.view == "lineup" || self.moves.is_some() {
                return Err(
                    "--play starts a new game; cannot use --load, --moves or lineup".into(),
                );
            }
        }
        Ok(())
    }

    fn validate(&self) -> Result<()> {
        if self.explain_verbose && self.explain.is_none() {
            return Err("--explain-verbose requires --explain".into());
        }
        if self.explain.is_some()
            && std::env::args_os().len() != 4 + usize::from(self.explain_verbose)
        {
            return Err(
                "--explain cannot be combined with other arguments except --explain-verbose".into(),
            );
        }
        self.validate_teaching()?;
        self.validate_computer()?;
        if self.window && self.capture.is_some() {
            return Err("--capture and --window cannot be combined; run them separately".into());
        }
        if self.window && !cfg!(feature = "viewer") {
            return Err("--window requires the optional viewer feature; run: cargo run -p chess3d --features viewer -- --window".into());
        }
        if !(64..=4096).contains(&self.width) || !(64..=4096).contains(&self.height) {
            return Err("dimensions must be 64..=4096 pixels".into());
        }
        if !["hero", "overhead", "lineup"].contains(&self.view.as_str()) {
            return Err("--view must be hero, overhead, or lineup".into());
        }
        if self.load.is_some() && self.seed.is_some() {
            return Err(
                "--seed cannot be combined with --load; loaded seed/style are authoritative".into(),
            );
        }
        if self.moves.is_some() && (self.load.is_some() || self.view == "lineup") {
            return Err(
                "--moves starts a legal initial game and cannot use --load or lineup".into(),
            );
        }
        if self.moves.is_some() && self.capture.is_some() && !cfg!(feature = "viewer") {
            return Err("--moves --capture requires --features viewer for retained capture".into());
        }
        if let Some(path) = &self.capture {
            let capture = super::resolved_path(path)?;
            for path in self.load.iter().chain(self.save.iter()) {
                if super::resolved_path(path)? == capture {
                    return Err("capture path must differ from load/save position paths".into());
                }
            }
        }
        Ok(())
    }
    fn validate_teaching(&self) -> Result<()> {
        if self.lesson.is_none() && self.puzzle.is_none() {
            return Ok(());
        }
        if self.lesson.is_some() && self.puzzle.is_some() {
            return Err("choose --lesson or --puzzle, not both".into());
        }
        if self.window
            || self.capture.is_some()
            || self.load.is_some()
            || self.save.is_some()
            || self.moves.is_some()
            || self.play.is_some()
            || self.level.is_some()
            || self.seed.is_some()
            || self.view != "hero"
        {
            return Err("teaching inspection cannot combine with game/editor options".into());
        }
        Ok(())
    }
}

fn print_help() {
    println!(
        "chess3d [--load position.json] [--save position.json] [--moves \"e2e4 e7e5\"]\n\
        [--capture image.png | --window] [--view hero|overhead|lineup]\n\
        [--width px --height px] [--seed n] [--play white|black --level 1..8]\n\
        [--lesson ID | --puzzle ID] (CPU-only teaching inspection)\n\
        Window and retained move capture require: cargo run -p chess3d --features viewer --\n\
        Play: left click select/move, U/Backspace undo, R restart, Escape cancel/menu (editor: exit).\n\
        View: left-drag orbit, wheel zoom, Home reset; editor R/Home reset.\n\
        --data-dir PATH overrides the application save/settings directory.\n\
        --play chooses the human side against a computer; --seed also seeds its replies.\n\
        --explain <fen> <uci> prints a CPU-only verified explanation.\n\
        --explain-verbose adds developer diagnostics; no other options may accompany --explain.\n\
        --moves applies legal UCI moves from the initial game; --load is editor-only.\n\
        --save writes the current position before capture/window; camera orbit is not saved."
    );
}
