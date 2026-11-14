//! Frame one Gamma source and run it through a selected evaluator.
//!
//! The request is a little-endian u32 source length, the source bytes, then
//! the optional sealed input. A timeout or a source longer than 2^32-1 bytes
//! exits 3. A nonzero evaluator status with empty stdout leaves the output
//! path untouched; any produced stdout is published with an fsync and rename.

use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub(crate) fn run(args: &[String]) -> Result<(), u8> {
    let invocation = match parse_args(args) {
        Ok(invocation) => invocation,
        Err(message) => {
            eprintln!("{message}");
            return Err(2);
        }
    };
    match invoke(&invocation) {
        Ok(0) => Ok(()),
        Ok(status) => Err(status),
        Err(message) => {
            eprintln!("gamma invoke: {message}");
            Err(1)
        }
    }
}

struct Invocation {
    evaluator: PathBuf,
    source: PathBuf,
    input: Option<PathBuf>,
    output: PathBuf,
    timeout: Duration,
}

fn parse_args(args: &[String]) -> Result<Invocation, String> {
    let mut evaluator = None;
    let mut source = None;
    let mut input = None;
    let mut output = None;
    let mut timeout = Duration::from_secs(120);
    let mut index = 0;
    while index < args.len() {
        let flag = &args[index];
        let value = args
            .get(index + 1)
            .ok_or_else(|| format!("missing value for {flag}"))?;
        match flag.as_str() {
            "--evaluator" => evaluator = Some(PathBuf::from(value)),
            "--source" => source = Some(PathBuf::from(value)),
            "--input" => input = Some(PathBuf::from(value)),
            "--output" => output = Some(PathBuf::from(value)),
            "--timeout" => {
                let seconds: f64 = value
                    .parse()
                    .map_err(|_| format!("timeout {value:?} is not a number"))?;
                if !seconds.is_finite() || seconds < 0.0 {
                    return Err(format!("timeout {value:?} is not a positive duration"));
                }
                timeout = Duration::try_from_secs_f64(seconds)
                    .map_err(|_| format!("timeout {value:?} is not a positive duration"))?;
            }
            _ => return Err(usage()),
        }
        index += 2;
    }
    Ok(Invocation {
        evaluator: evaluator.ok_or_else(usage)?,
        source: source.ok_or_else(usage)?,
        input,
        output: output.ok_or_else(usage)?,
        timeout,
    })
}

fn usage() -> String {
    "usage: mbx xtask gamma-invoke --evaluator PATH --source PATH --output PATH [--input PATH] [--timeout SECONDS]".to_owned()
}

fn invoke(invocation: &Invocation) -> Result<u8, String> {
    let source = fs::read(&invocation.source).map_err(|error| error.to_string())?;
    let sealed = invocation
        .input
        .as_ref()
        .map(fs::read)
        .transpose()
        .map_err(|error| error.to_string())?
        .unwrap_or_default();
    let request = match frame_request(&source, &sealed) {
        Ok(request) => request,
        Err(status) => return Ok(status),
    };
    let (output, status) = match run_evaluator(&invocation.evaluator, &request, invocation.timeout)
    {
        Ok(completed) => completed,
        Err(EvaluatorStop::TimedOut) => return Ok(3),
        Err(EvaluatorStop::Failed(message)) => return Err(message),
    };
    let code = exit_code(status);
    if code != 0 && output.is_empty() {
        return Ok(code);
    }
    publish(&invocation.output, &output)?;
    Ok(code)
}

enum EvaluatorStop {
    TimedOut,
    Failed(String),
}

fn frame_request(source: &[u8], sealed_input: &[u8]) -> Result<Vec<u8>, u8> {
    frame_request_len(source.len(), source, sealed_input)
}

fn frame_request_len(source_len: usize, source: &[u8], sealed_input: &[u8]) -> Result<Vec<u8>, u8> {
    let Ok(len) = u32::try_from(source_len) else {
        return Err(3);
    };
    if source.len() != source_len {
        return Err(3);
    }
    let mut request = Vec::with_capacity(4 + source.len() + sealed_input.len());
    request.extend_from_slice(&len.to_le_bytes());
    request.extend_from_slice(source);
    request.extend_from_slice(sealed_input);
    Ok(request)
}

fn run_evaluator(
    evaluator: &Path,
    request: &[u8],
    timeout: Duration,
) -> Result<(Vec<u8>, ExitStatus), EvaluatorStop> {
    let mut command = Command::new(evaluator);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command
        .spawn()
        .map_err(|error| EvaluatorStop::Failed(error.to_string()))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| EvaluatorStop::Failed("evaluator stdin was not piped".into()))?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| EvaluatorStop::Failed("evaluator stdout was not piped".into()))?;
    let request = request.to_vec();
    let writer = thread::spawn(move || {
        let _ = stdin.write_all(&request);
        let _ = stdin.flush();
    });
    let reader = thread::spawn(move || {
        let mut output = Vec::new();
        let _ = stdout.read_to_end(&mut output);
        output
    });
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| EvaluatorStop::Failed(error.to_string()))?
        {
            break status;
        }
        if started.elapsed() >= timeout {
            terminate(&mut child);
            let _ = reader.join();
            let _ = writer.join();
            return Err(EvaluatorStop::TimedOut);
        }
        thread::sleep(Duration::from_millis(10));
    };
    let output = reader.join().unwrap_or_default();
    let _ = writer.join();
    Ok((output, status))
}

fn terminate(child: &mut Child) {
    #[cfg(unix)]
    {
        if let Ok(pid) = i32::try_from(child.id()) {
            // The child is a process-group leader (`process_group(0)`). A
            // negative pid asks the kernel to signal that whole group, which
            // is what stops a shell evaluator and the command it spawned.
            unsafe { libc::kill(-pid, libc::SIGKILL) };
        }
    }
    #[cfg(not(unix))]
    {
        let _ = child.kill();
    }
    let _ = child.wait();
}

fn exit_code(status: ExitStatus) -> u8 {
    if let Some(code) = status.code() {
        return code as u8;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return (-signal) as u8;
        }
    }
    1
}

fn publish(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let parent = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let mut attempts = 0u32;
    let temporary = loop {
        attempts += 1;
        if attempts > 100 {
            return Err("could not create a temporary evaluator output".into());
        }
        let candidate = parent.join(format!(".invoke.{}.{attempts}", std::process::id()));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(mut file) => {
                file.write_all(bytes).map_err(|error| error.to_string())?;
                file.flush().map_err(|error| error.to_string())?;
                file.sync_all().map_err(|error| error.to_string())?;
                break candidate;
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.to_string()),
        }
    };
    if let Err(error) = fs::rename(&temporary, path) {
        let _ = fs::remove_file(&temporary);
        return Err(error.to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_prefixes_the_source_length() {
        let source = b"abc";
        let sealed = b"XYZ";
        let request = frame_request(source, sealed).unwrap();
        assert_eq!(&request[..4], &3u32.to_le_bytes());
        assert_eq!(&request[4..], b"abcXYZ");
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn source_longer_than_u32_returns_status_3() {
        assert_eq!(frame_request_len(u32::MAX as usize + 1, b"", b""), Err(3));
    }

    /// The evaluator in these tests is a `/bin/sh` script, so they and the
    /// fixture that writes it build only where a shell script is executable.
    #[cfg(unix)]
    mod evaluator_process {
        use super::super::*;
        use std::os::unix::fs::PermissionsExt;
        use std::sync::atomic::{AtomicUsize, Ordering};

        static NEXT: AtomicUsize = AtomicUsize::new(0);

        struct Fixture(PathBuf);

        impl Fixture {
            fn new() -> Self {
                let path = std::env::temp_dir().join(format!(
                    "omega-invoke-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));
                fs::create_dir_all(&path).unwrap();
                Self(path)
            }

            fn script(&self, body: &str) -> PathBuf {
                let path = self.0.join("evaluator.sh");
                fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
                let mut permissions = fs::metadata(&path).unwrap().permissions();
                permissions.set_mode(0o755);
                fs::set_permissions(&path, permissions).unwrap();
                path
            }
        }

        impl Drop for Fixture {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }

        fn prepare(fixture: &Fixture, evaluator: PathBuf, timeout: Duration) -> Invocation {
            let source = fixture.0.join("source.gamma");
            fs::write(&source, b"(def main () Int 1)\n").unwrap();
            Invocation {
                evaluator,
                source,
                input: None,
                output: fixture.0.join("out"),
                timeout,
            }
        }

        fn invocation_source_len() -> [u8; 4] {
            u32::try_from(b"(def main () Int 1)\n".len())
                .unwrap()
                .to_le_bytes()
        }

        #[test]
        fn success_publishes_stdout_and_empty_failure_does_not() {
            let fixture = Fixture::new();
            let evaluator = fixture.script("cat");
            let invocation = prepare(&fixture, evaluator, Duration::from_secs(5));
            assert_eq!(invoke(&invocation).unwrap(), 0);
            let published = fs::read(&invocation.output).unwrap();
            assert_eq!(&published[..4], invocation_source_len().as_slice());
            assert!(published.ends_with(b"(def main () Int 1)\n"));

            let fixture = Fixture::new();
            let evaluator = fixture.script("exit 7");
            let invocation = prepare(&fixture, evaluator, Duration::from_secs(5));
            fs::write(&invocation.output, b"keep").unwrap();
            assert_eq!(invoke(&invocation).unwrap(), 7);
            assert_eq!(fs::read(&invocation.output).unwrap(), b"keep");

            let fixture = Fixture::new();
            let evaluator = fixture.script("printf hi; exit 2");
            let invocation = prepare(&fixture, evaluator, Duration::from_secs(5));
            assert_eq!(invoke(&invocation).unwrap(), 2);
            assert_eq!(fs::read(&invocation.output).unwrap(), b"hi");
        }

        #[test]
        fn timeout_exits_3_and_leaves_the_output_in_place() {
            let fixture = Fixture::new();
            let evaluator = fixture.script("sleep 30");
            let invocation = prepare(&fixture, evaluator, Duration::from_millis(200));
            fs::write(&invocation.output, b"keep").unwrap();
            assert_eq!(invoke(&invocation).unwrap(), 3);
            assert_eq!(fs::read(&invocation.output).unwrap(), b"keep");
        }
    }
}
