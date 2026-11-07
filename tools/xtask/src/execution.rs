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
    fs::create_dir_all(directory).map_err(|e| format!("create {}: {e}", directory.display()))?;
    let stdout = directory.join(format!("{name}.stdout"));
    let stderr = directory.join(format!("{name}.stderr"));
    let mut receipt = serde_json::json!({
        "program": command.get_program().to_string_lossy(),
        "arguments": command.get_args().map(|a| a.to_string_lossy()).collect::<Vec<_>>(),
        "directory": command.get_current_dir(),
        "environment_overrides": command.get_envs().map(|(key, value)| (key.to_string_lossy().into_owned(), value.map(|v| v.to_string_lossy().into_owned()))).collect::<std::collections::BTreeMap<_, _>>(),
        "stdout": stdout, "stderr": stderr,
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
        loop {
            if let Some(status) = child.try_wait().map_err(|e| format!("wait {name}: {e}"))? {
                return Ok(status);
            }
            // The workflow also bounds the whole job. Kill a stuck individual
            // command so its original files can still be uploaded on failure.
            if started.elapsed() > Duration::from_secs(1800) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("{name} exceeded 1800 seconds"));
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
