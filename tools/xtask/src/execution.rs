//! File-backed command evidence shared by producer preparation and program gates.

use std::fs;
use std::io::Read;
use std::path::Path;
use std::process::{Command, ExitStatus, Output, Stdio};
use std::time::{Duration, Instant};

pub fn write_json(path: &Path, value: &impl serde::Serialize) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    fs::write(path, bytes).map_err(|e| format!("write {}: {e}", path.display()))
}

pub fn reserve_directory(parent: &Path) -> Result<std::path::PathBuf, String> {
    fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let path = parent.join(format!("{}-{nonce}", std::process::id()));
    fs::create_dir(&path).map_err(|e| format!("reserve fresh run {}: {e}", path.display()))?;
    Ok(path)
}

pub fn run(command: &mut Command, directory: &Path, name: &str) -> Result<ExitStatus, String> {
    run_with_timeout(command, directory, name, Duration::from_secs(1800))
}

/// Runs with no time limit, and copies the command's stderr to this process's
/// stderr while it runs, so a long command reports as it goes instead of
/// only in the file read after it ends.
pub fn run_echoing_stderr(
    command: &mut Command,
    directory: &Path,
    name: &str,
) -> Result<ExitStatus, String> {
    run_bounded(command, directory, name, Duration::MAX, true)
}

pub fn run_with_timeout(
    command: &mut Command,
    directory: &Path,
    name: &str,
    timeout: Duration,
) -> Result<ExitStatus, String> {
    run_bounded(command, directory, name, timeout, false)
}

/// Copies what `path` gained since `offset` to this process's stderr.
fn echo_growth(path: &Path, offset: &mut u64) {
    use std::io::{Seek, SeekFrom, Write};
    let Ok(mut file) = fs::File::open(path) else {
        return;
    };
    let mut bytes = Vec::new();
    if file.seek(SeekFrom::Start(*offset)).is_ok()
        && let Ok(read) = file.read_to_end(&mut bytes)
    {
        *offset += read as u64;
        let _ = std::io::stderr().write_all(&bytes);
    }
}

fn run_bounded(
    command: &mut Command,
    directory: &Path,
    name: &str,
    timeout: Duration,
    echo_stderr: bool,
) -> Result<ExitStatus, String> {
    if timeout.is_zero() {
        return Err(format!("{name} requires a positive command timeout"));
    }
    fs::create_dir_all(directory).map_err(|e| format!("create {}: {e}", directory.display()))?;
    let stdout = directory.join(format!("{name}.stdout"));
    let stderr = directory.join(format!("{name}.stderr"));
    let mut receipt = serde_json::json!({
        "program": command.get_program().to_string_lossy(),
        "arguments": command.get_args().map(|a| a.to_string_lossy()).collect::<Vec<_>>(),
        "directory": command.get_current_dir(),
        "environment_overrides": command.get_envs().map(|(key, value)| (key.to_string_lossy().into_owned(), value.map(|v| v.to_string_lossy().into_owned()))).collect::<std::collections::BTreeMap<_, _>>(),
        "stdout": stdout, "stderr": stderr,
        "timeout_ms": u64::try_from(timeout.as_millis()).ok(), "timed_out": false,
    });
    let receipt_path = directory.join(format!("{name}.json"));
    let receipt_file = fs::File::create_new(&receipt_path)
        .map_err(|e| format!("reserve command receipt {}: {e}", receipt_path.display()))?;
    serde_json::to_writer_pretty(receipt_file, &receipt).map_err(|e| e.to_string())?;
    command.stdout(Stdio::from(
        fs::File::create_new(&stdout).map_err(|e| e.to_string())?,
    ));
    command.stderr(Stdio::from(
        fs::File::create_new(&stderr).map_err(|e| e.to_string())?,
    ));
    let started = Instant::now();
    let result = (|| {
        let mut child = command.spawn().map_err(|e| format!("start {name}: {e}"))?;
        receipt["child_id"] = serde_json::json!(child.id());
        let mut echoed = 0;
        loop {
            let finished = child.try_wait().map_err(|e| format!("wait {name}: {e}"))?;
            if echo_stderr {
                echo_growth(&stderr, &mut echoed);
            }
            if let Some(status) = finished {
                return Ok(status);
            }
            // The workflow also bounds the whole job. Kill a stuck individual
            // command so its original files can still be uploaded on failure.
            if started.elapsed() > timeout {
                let killed = child.kill();
                let reaped = child.wait();
                receipt["timed_out"] = serde_json::json!(true);
                receipt["timeout_cleanup"] = serde_json::json!({
                    "kill_error": killed.as_ref().err().map(|error| error.to_string()),
                    "wait_error": reaped.as_ref().err().map(|error| error.to_string()),
                    "reaped": reaped.is_ok(),
                });
                if let Ok(status) = &reaped {
                    receipt["exit_code"] = serde_json::json!(status.code());
                }
                let mut reason = format!("{name} exceeded {} seconds", timeout.as_secs_f64());
                if let Err(error) = killed {
                    reason.push_str(&format!("; stopping child failed: {error}"));
                }
                if let Err(error) = reaped {
                    reason.push_str(&format!("; reaping child failed: {error}"));
                }
                return Err(reason);
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    })();
    receipt["elapsed_ms"] = serde_json::json!(started.elapsed().as_millis());
    match &result {
        Ok(status) => receipt["exit_code"] = serde_json::json!(status.code()),
        Err(error) => receipt["error"] = serde_json::json!(error),
    }
    write_json(&receipt_path, &receipt)?;
    result
}

pub fn read_bounded(path: &Path) -> Result<Vec<u8>, String> {
    const LIMIT: u64 = 16 * 1024 * 1024;
    let file = fs::File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    let mut bytes = Vec::new();
    file.take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > LIMIT {
        return Err(format!(
            "{} exceeds the parsing limit; complete output is retained",
            path.display()
        ));
    }
    Ok(bytes)
}

pub fn output(command: &mut Command, directory: &Path, name: &str) -> Result<Output, String> {
    let status = run(command, directory, name)?;
    Ok(Output {
        status,
        stdout: read_bounded(&directory.join(format!("{name}.stdout")))?,
        stderr: read_bounded(&directory.join(format!("{name}.stderr")))?,
    })
}

pub fn digest(path: &Path) -> Result<String, String> {
    let mut command = if cfg!(windows) {
        let mut command = Command::new("certutil");
        command.args(["-hashfile"]).arg(path).arg("SHA256");
        command
    } else if cfg!(target_os = "macos") {
        let mut command = Command::new("shasum");
        command.args(["-a", "256"]).arg(path);
        command
    } else {
        let mut command = Command::new("sha256sum");
        command.arg(path);
        command
    };
    let output = command
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("hash {}: {e}", path.display()))?;
    if !output.status.success() {
        return Err(format!("hash {} failed", path.display()));
    }
    String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .find(|word| word.len() == 64 && word.bytes().all(|c| c.is_ascii_hexdigit()))
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| "SHA256 tool returned no digest".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn test_directory(label: &str) -> std::path::PathBuf {
        let parent = std::env::temp_dir().join(format!(
            "omega-command-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&parent).unwrap();
        parent
    }

    fn fixture_command(mode: &str) -> Command {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "execution::tests::command_fixture",
                "--ignored",
                "--nocapture",
            ])
            .env("OMEGA_XTASK_EXECUTION_TEST_MODE", mode)
            .stdin(Stdio::null());
        command
    }

    fn receipt(directory: &Path, name: &str) -> serde_json::Value {
        serde_json::from_slice(&fs::read(directory.join(format!("{name}.json"))).unwrap()).unwrap()
    }

    #[test]
    #[ignore = "child process used by command budget tests"]
    fn command_fixture() {
        let Ok(mode) = std::env::var("OMEGA_XTASK_EXECUTION_TEST_MODE") else {
            return;
        };
        println!("original command stdout");
        eprintln!("original command stderr");
        std::io::stdout().flush().unwrap();
        std::io::stderr().flush().unwrap();
        match mode.as_str() {
            "delayed" => std::thread::sleep(Duration::from_millis(500)),
            "hung" => std::thread::sleep(Duration::from_secs(10)),
            "failure" => std::process::exit(23),
            _ => panic!("unknown command fixture mode"),
        }
    }

    #[test]
    fn a_longer_explicit_budget_allows_the_same_command_to_complete() {
        let directory = test_directory("explicit-budget");
        let mut command = fixture_command("delayed");
        let error = run_with_timeout(
            &mut command,
            &directory,
            "short",
            Duration::from_millis(150),
        )
        .unwrap_err();
        assert!(error.contains("exceeded"));
        assert_eq!(receipt(&directory, "short")["timed_out"], true);
        let status =
            run_with_timeout(&mut command, &directory, "prepared", Duration::from_secs(2)).unwrap();
        assert!(status.success());
        let recorded = receipt(&directory, "prepared");
        assert_eq!(recorded["timeout_ms"], 2000);
        assert_eq!(recorded["timed_out"], false);
        assert_eq!(recorded["exit_code"], 0);
        drop(command);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn a_hung_command_is_reaped_without_losing_its_original_output() {
        let directory = test_directory("hung-command");
        let mut command = fixture_command("hung");
        let started = Instant::now();
        let error = run_with_timeout(&mut command, &directory, "build", Duration::from_secs(2))
            .unwrap_err();
        assert!(started.elapsed() < Duration::from_secs(8));
        assert!(error.contains("exceeded 2 seconds"));
        let recorded = receipt(&directory, "build");
        assert_eq!(recorded["timed_out"], true);
        assert_eq!(recorded["timeout_cleanup"]["reaped"], true);
        assert!(recorded["timeout_cleanup"]["kill_error"].is_null());
        assert!(recorded["timeout_cleanup"]["wait_error"].is_null());
        assert!(
            fs::read_to_string(directory.join("build.stdout"))
                .unwrap()
                .contains("original command stdout")
        );
        assert!(
            fs::read_to_string(directory.join("build.stderr"))
                .unwrap()
                .contains("original command stderr")
        );
        drop(command);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn the_default_budget_preserves_failure_even_with_a_build_label() {
        let directory = test_directory("command-failure");
        let mut command = fixture_command("failure");
        let status = run(&mut command, &directory, "runner-build").unwrap();
        assert_eq!(status.code(), Some(23));
        let recorded = receipt(&directory, "runner-build");
        assert_eq!(recorded["timeout_ms"], 1_800_000);
        assert_eq!(recorded["exit_code"], 23);
        assert_eq!(recorded["timed_out"], false);
        assert!(recorded.get("error").is_none());
        drop(command);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn a_zero_budget_refuses_before_starting_a_command() {
        let directory = test_directory("zero-budget");
        let error = run_with_timeout(
            &mut fixture_command("failure"),
            &directory,
            "invalid-budget",
            Duration::ZERO,
        )
        .unwrap_err();
        assert!(error.contains("positive command timeout"));
        assert_eq!(fs::read_dir(&directory).unwrap().count(), 0);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn separate_runs_and_repeated_commands_cannot_overwrite_evidence() {
        let parent = std::env::temp_dir().join(format!(
            "omega-command-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let first = reserve_directory(&parent).unwrap();
        let second = reserve_directory(&parent).unwrap();
        assert_ne!(first, second);
        let mut command = Command::new(parent.join("nonexistent-executable"));
        assert!(run(&mut command, &first, "operation").is_err());
        let receipt = fs::read(first.join("operation.json")).unwrap();
        assert!(run(&mut command, &first, "operation").is_err());
        assert_eq!(fs::read(first.join("operation.json")).unwrap(), receipt);
        fs::remove_dir_all(parent).unwrap();
    }
}
