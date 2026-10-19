use std::env;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("xtask failed: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("xtask has no workspace parent")?;
    env::set_current_dir(root)?;
    let mut args = env::args().skip(1);
    let task = args.next().unwrap_or_else(|| "help".into());
    let rest: Vec<_> = args.collect();
    match task.as_str() {
        "help" | "--help" | "-h" => {
            println!(
                "cargo xtask <check|build|test|python-tests|acceptance|window|ci|bootstrap>\n\
                 check: format and strict Clippy, default + viewer\n\
                 build: all workspace targets with viewer\n\
                 test: all targets with viewer; explicit VK_DRIVER_FILES required\n\
                 acceptance: real offscreen harness (pass --output DIR)\n\
                 window: original Xvfb window harness (pass --output DIR)\n\
                 ci: all gates; pass --output NEW_DIR\n\
                 bootstrap: optional engine verification (--revision REF); initial fetch uses Python directly"
            );
            Ok(())
        }
        "acceptance" => python("tools/verify_chess3d.py", &rest),
        "window" => window(&rest),
        "bootstrap" => python("tools/bootstrap_engine.py", &rest),
        "ci" => ci(&rest),
        _ if !rest.is_empty() => Err(format!("unexpected arguments for {task}: {rest:?}").into()),
        "python-tests" => python_tests(),
        "check" => check(),
        "build" => cargo(&["build", "--workspace", "--all-targets", "--all-features"]),
        "test" => tests(),
        _ => Err(format!("unknown task: {task}").into()),
    }
}

fn execute(mut command: Command) -> Result<()> {
    command.env("CARGO_BUILD_JOBS", "2");
    command.env("CARGO_INCREMENTAL", "0");
    command.env("LP_NUM_THREADS", "2");
    eprintln!("+ {command:?}");
    let status = command.status()?;
    if !status.success() {
        return Err(format!("command exited with {status}: {command:?}").into());
    }
    Ok(())
}

fn cargo(args: &[&str]) -> Result<()> {
    let mut command = Command::new("cargo");
    command.args(args).args(["--locked", "-j2"]);
    execute(command)
}

fn python(script: &str, args: &[String]) -> Result<()> {
    let mut command = Command::new("python3");
    command.arg(script).args(args);
    execute(command)
}

fn python_tests() -> Result<()> {
    let mut command = Command::new("python3");
    command.args([
        "-m",
        "unittest",
        "discover",
        "-s",
        "tools",
        "-p",
        "test_*.py",
        "-v",
    ]);
    execute(command)
}

fn check() -> Result<()> {
    let mut format = Command::new("cargo");
    format.args(["fmt", "-p", "chess3d", "-p", "xtask", "--", "--check"]);
    execute(format)?;
    for features in [false, true] {
        let mut clippy = Command::new("cargo");
        clippy.args(["clippy", "--locked", "-j2", "--workspace", "--all-targets"]);
        if features {
            clippy.arg("--all-features");
        }
        clippy.args(["--", "-D", "warnings"]);
        execute(clippy)?;
    }
    Ok(())
}

fn vulkan_icd() -> Result<PathBuf> {
    let path = PathBuf::from(env::var_os("VK_DRIVER_FILES").ok_or(
        "set VK_DRIVER_FILES to one explicit Vulkan ICD JSON; GPU tests must not be skipped",
    )?);
    if !path.is_file() {
        return Err(format!("Vulkan ICD is not a regular file: {}", path.display()).into());
    }
    Ok(path.canonicalize()?)
}

fn tests() -> Result<()> {
    let mut command = Command::new("cargo");
    command.args([
        "test",
        "--locked",
        "-j2",
        "--workspace",
        "--all-targets",
        "--all-features",
        "--",
        "--test-threads=2",
        "--nocapture",
    ]);
    command.env("VK_DRIVER_FILES", vulkan_icd()?);
    command.env("CHESS3D_REQUIRE_GPU", "1");
    execute(command)
}

fn window(args: &[String]) -> Result<()> {
    vulkan_icd()?;
    cargo(&[
        "build",
        "-p",
        "chess3d",
        "--features",
        "viewer",
        "--bin",
        "chess3d",
    ])?;
    python("tools/verify_window.py", args)
}

fn ci(args: &[String]) -> Result<()> {
    if args.len() != 2 || args[0] != "--output" {
        return Err("ci requires --output NEW_DIR".into());
    }
    vulkan_icd()?;
    let output = PathBuf::from(&args[1]);
    create_output(&output)?;
    python_tests()?;
    check()?;
    tests()?;
    let offscreen = output.join("offscreen").to_string_lossy().into_owned();
    python("tools/verify_chess3d.py", &["--output".into(), offscreen])?;
    let windows = output.join("window").to_string_lossy().into_owned();
    window(&["--output".into(), windows])
}

fn create_output(output: &Path) -> Result<()> {
    if let Some(parent) = output.parent().filter(|path| !path.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::create_dir(output)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_output_creates_missing_parents() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("artifacts/ci");
        create_output(&output).unwrap();
        assert!(output.is_dir());
        assert_eq!(std::fs::read_dir(output).unwrap().count(), 0);
    }

    #[test]
    fn existing_output_is_not_reused_or_changed() {
        let directory = tempfile::tempdir().unwrap();
        let output = directory.path().join("ci");
        std::fs::create_dir(&output).unwrap();
        let sentinel = output.join("report.json");
        std::fs::write(&sentinel, "previous evidence").unwrap();
        assert!(create_output(&output).is_err());
        assert_eq!(
            std::fs::read_to_string(sentinel).unwrap(),
            "previous evidence"
        );
    }

    #[test]
    fn ci_requires_an_explicit_output_before_starting_any_gate() {
        for args in [
            vec![],
            vec!["--output".into()],
            vec!["other".into(), "path".into()],
        ] {
            assert!(
                ci(&args)
                    .unwrap_err()
                    .to_string()
                    .contains("--output NEW_DIR")
            );
        }
    }
}
