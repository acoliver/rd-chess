use crate::rules::pgn::{MAX_INPUT_BYTES, PgnError, PgnErrorKind, PgnGame, parse};
use std::{
    fs::{self, File, OpenOptions},
    io::{ErrorKind, Read, Write},
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

pub(super) fn directory(root: &Path) -> Result<PathBuf, String> {
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let path = root.join("exports");
    match fs::create_dir(&path) {
        Ok(()) => (),
        Err(error) if error.kind() == ErrorKind::AlreadyExists => (),
        Err(error) => return Err(error.to_string()),
    }
    // Drop folders and import files are external input. Do not follow symlinks.
    if !fs::symlink_metadata(&path)
        .map_err(|e| e.to_string())?
        .is_dir()
    {
        return Err("PGN directory must be a directory, not a symlink".into());
    }
    path.canonicalize().map_err(|e| e.to_string())
}

pub(super) fn list(root: &Path) -> Result<Vec<String>, String> {
    let mut names = Vec::new();
    for entry in fs::read_dir(directory(root)?).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_type().map_err(|e| e.to_string())?.is_file()
            && let Some(name) = entry.file_name().to_str()
            && valid_name(name)
        {
            names.push(name.to_owned());
        }
    }
    names.sort();
    Ok(names)
}

fn valid_name(name: &str) -> bool {
    let path = Path::new(name);
    matches!(path.components().next(), Some(Component::Normal(_)))
        && path.components().count() == 1
        && !name.contains(['/', '\\'])
        && !name.chars().any(char::is_control)
        && path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("pgn"))
}

pub(super) fn read(root: &Path, name: &str) -> Result<Vec<PgnGame>, String> {
    if !valid_name(name) {
        return Err("PGN file name must be a single .pgn file name".into());
    }
    let path = directory(root)?.join(name);
    if !fs::symlink_metadata(&path)
        .map_err(|e| e.to_string())?
        .is_file()
    {
        return Err("PGN import must be a regular file, not a symlink".into());
    }
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|e| e.to_string())?
        .take(u64::try_from(MAX_INPUT_BYTES).expect("PGN limit fits u64") + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(PgnError {
            game: 1,
            ply: 1,
            token: "<input>".into(),
            kind: PgnErrorKind::InputLimit,
        }
        .to_string());
    }
    let text = std::str::from_utf8(&bytes).map_err(|e| e.to_string())?;
    let games = parse(text).map_err(|e| e.to_string())?;
    if games.is_empty() {
        return Err("PGN file contains no games".into());
    }
    Ok(games)
}

pub(super) fn clock() -> Result<(String, String), String> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();
    timestamp(seconds)
}

fn timestamp(seconds: u64) -> Result<(String, String), String> {
    if seconds > 253_402_300_799 {
        return Err("system clock is outside PGN years 1970..9999".into());
    }
    // Gregorian civil date from Unix days, using 400-year eras (UTC).
    let days = seconds / 86_400 + 719_468;
    let era = days / 146_097;
    let doe = days % 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = year + u64::from(month <= 2);
    let hour = seconds % 86_400 / 3600;
    let minute = seconds % 3600 / 60;
    let second = seconds % 60;
    Ok((
        format!("{year:04}.{month:02}.{day:02}"),
        format!("{year:04}{month:02}{day:02}-{hour:02}{minute:02}{second:02}Z"),
    ))
}

pub(super) fn write(root: &Path, stamp: &str, text: &str) -> Result<PathBuf, String> {
    let directory = directory(root)?;
    for counter in 1..=u32::MAX {
        let path = directory.join(format!("game-{stamp}-{counter:04}.pgn"));
        let mut file = match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.to_string()),
        };
        file.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())?;
        return Ok(path);
    }
    Err("PGN export counter exhausted".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_dates_cover_epoch_leap_day_and_year_boundary() {
        for (seconds, date, stamp) in [
            (0, "1970.01.01", "19700101-000000Z"),
            (951_827_696, "2000.02.29", "20000229-123456Z"),
            (1_798_761_600, "2027.01.01", "20270101-000000Z"),
        ] {
            assert_eq!(timestamp(seconds).unwrap(), (date.into(), stamp.into()));
        }
        assert!(timestamp(u64::MAX).is_err());
    }

    #[test]
    fn identical_timestamps_use_exclusive_monotonic_counters() {
        let root = tempfile::tempdir().unwrap();
        let first = write(root.path(), "20261105-123456Z", "first").unwrap();
        let second = write(root.path(), "20261105-123456Z", "second").unwrap();
        assert!(first.ends_with("game-20261105-123456Z-0001.pgn"));
        assert!(second.ends_with("game-20261105-123456Z-0002.pgn"));
        assert_eq!(fs::read_to_string(first).unwrap(), "first");
        assert_eq!(fs::read_to_string(second).unwrap(), "second");
    }
}
