//! Compare the published host executable with Psi interpretation of the same entry.

pub(super) mod process_exit;

use super::file_outputs;
use super::{SampleProject, host_target, optional_text, parse_header_oracle, with_source_inputs};
use crate::execution;
use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
};

#[derive(Clone, Copy)]
enum ExpectedTermination {
    Exit(i32),
    Trap,
}

pub(super) struct Oracle {
    termination: Option<ExpectedTermination>,
    stdout: Option<Vec<u8>>,
    stdout_contains: Option<String>,
    stderr: Option<Vec<u8>>,
    files: file_outputs::ObservedFiles,
}

impl Oracle {
    pub(super) fn load(
        project: &SampleProject,
        entry: &Path,
        target: &str,
        evidence: &Path,
    ) -> Result<Self, String> {
        let directory = &project.directory;
        let (exit, stdout_contains) = parse_header_oracle(entry)?;
        let mut termination = exit.map(ExpectedTermination::Exit);
        for path in [
            directory.join(format!("expected_native_exit.{target}.txt")),
            directory.join("expected_native_exit.txt"),
        ] {
            if let Some(text) = optional_text(&path)? {
                termination = Some(match text.trim() {
                    "trap" => ExpectedTermination::Trap,
                    code => ExpectedTermination::Exit(code.parse().map_err(|_| {
                        format!("{} requires an i32 exit status or `trap`", path.display())
                    })?),
                });
                break;
            }
        }
        Ok(Self {
            termination,
            stdout: optional_bytes(&directory.join("expected_stdout.txt"))?,
            stdout_contains,
            stderr: optional_bytes(&directory.join("expected_stderr.txt"))?,
            files: file_outputs::expected(
                directory,
                &project.output_files,
                &evidence.join("expected-files"),
            )?,
        })
    }

    pub(super) fn is_empty(&self) -> bool {
        self.termination.is_none()
            && self.stdout.is_none()
            && self.stdout_contains.is_none()
            && self.stderr.is_none()
            && self.files.is_empty()
    }

    fn check(&self, observation: &Observation) -> Result<(), String> {
        match self.termination {
            Some(ExpectedTermination::Exit(code)) if observation.exit != Some(code) => {
                return Err(format!(
                    "expected exit {code}, got {}",
                    observation.termination
                ));
            }
            Some(ExpectedTermination::Trap)
                if observation.trap != Some(true) || observation.explicit_native_exit =>
            {
                return Err(format!("expected Trap, got {}", observation.termination));
            }
            _ => {}
        }
        if let Some(expected) = &self.stdout {
            equal_bytes("expected stdout", expected, &observation.stdout)?;
        } else if let Some(text) = &self.stdout_contains
            && !text.is_empty()
            && !observation
                .stdout
                .windows(text.len())
                .any(|bytes| bytes == text.as_bytes())
        {
            return Err(format!("expected stdout containing {text:?}"));
        }
        if let Some(expected) = &self.stderr {
            equal_bytes("expected stderr", expected, &observation.stderr)?;
        }
        file_outputs::compare(&self.files, &observation.files)
    }
}

fn optional_bytes(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match path
        .try_exists()
        .map_err(|e| format!("{}: {e}", path.display()))?
    {
        true => execution::read_bounded(path).map(Some),
        false => Ok(None),
    }
}

fn equal_bytes(channel: &str, left: &[u8], right: &[u8]) -> Result<(), String> {
    if left == right {
        return Ok(());
    }
    let offset = left
        .iter()
        .zip(right)
        .position(|(a, b)| a != b)
        .unwrap_or(left.len().min(right.len()));
    let byte = |value: Option<&u8>| value.map_or_else(|| "EOF".into(), |n| format!("0x{n:02x}"));
    Err(format!(
        "{channel} differs at byte {offset} ({} vs {})",
        byte(left.get(offset)),
        byte(right.get(offset))
    ))
}

struct Observation {
    termination: String,
    returned_unit: bool,
    explicit_native_exit: bool,
    signed_native_zero: bool,
    exit: Option<i32>,
    trap: Option<bool>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    files: file_outputs::ObservedFiles,
}

impl Observation {
    fn read(directory: &Path, source: &str) -> Result<Self, String> {
        let path = directory.join("termination.tsv");
        let record = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let (record_source, termination) = record
            .trim_end()
            .split_once('\t')
            .ok_or("missing source in termination record")?;
        if record_source != source {
            return Err(format!(
                "expected {source} observation, found {record_source}"
            ));
        }
        let mut fields: Vec<_> = termination.split('\t').collect();
        let process_exit_excluded = if source == "native" {
            match fields.pop() {
                Some("process-exit-excluded") => true,
                Some("process-exit-possible" | "process-exit-unknown") => false,
                _ => return Err("missing native process-exit admission".into()),
            }
        } else {
            false
        };
        let number = |text: &str| {
            text.parse::<i32>()
                .map_err(|_| format!("invalid process status {text:?}"))
        };
        let (exit, trap) = match (source, fields.as_slice()) {
            ("native", ["explicit-exit", code, _]) if !process_exit_excluded => {
                (Some(number(code)?), native_trap(&fields)?)
            }
            ("native", ["signed-zero", _]) if !process_exit_excluded => {
                (Some(0), native_trap(&fields)?)
            }
            ("native", ["exit", code, _]) => (Some(number(code)?), native_trap(&fields)?),
            ("native", ["signal", signal, _]) => {
                number(signal)?;
                (None, native_trap(&fields)?)
            }
            ("native", ["terminated", _]) => (None, native_trap(&fields)?),
            ("psi", ["exit", code]) => (Some(number(code)?), None),
            ("psi", ["return"]) => (Some(0), None),
            ("psi", ["crash", "Trap"]) => (None, Some(true)),
            ("psi", ["crash", "Abort"]) => (None, None),
            ("psi", ["failed" | "unavailable" | "incomplete", ..]) => {
                return Err(termination.replace('\t', " "));
            }
            _ => return Err(format!("invalid {source} termination {termination:?}")),
        };
        Ok(Self {
            termination: termination.replace('\t', " "),
            returned_unit: fields.first() == Some(&"return")
                || (exit == Some(0) && process_exit_excluded),
            explicit_native_exit: fields.first() == Some(&"explicit-exit"),
            signed_native_zero: fields.first() == Some(&"signed-zero"),
            exit,
            trap,
            stdout: execution::read_bounded(&directory.join("stdout"))?,
            stderr: execution::read_bounded(&directory.join("stderr"))?,
            files: Default::default(),
        })
    }
}

fn native_trap(fields: &[&str]) -> Result<Option<bool>, String> {
    match fields.last() {
        Some(&"trap") => Ok(Some(true)),
        Some(&"other") => Ok(Some(false)),
        Some(&"unknown") => Ok(None),
        _ => Err("invalid native trap presentation".into()),
    }
}

fn agreement(native: &Observation, psi: &Observation) -> Result<(), String> {
    equal_bytes("stdout", &native.stdout, &psi.stdout)?;
    equal_bytes("stderr", &native.stderr, &psi.stderr)?;
    file_outputs::compare(&native.files, &psi.files)?;
    if let Some(code) = psi.exit {
        if native.signed_native_zero && code != 0 {
            return Err(format!(
                "native signed exit status 0 vs Psi {}",
                psi.termination
            ));
        }
        if native.explicit_native_exit {
            if !psi.returned_unit && native.exit == Some(code) {
                return Ok(());
            }
            return Err(format!(
                "native {} vs Psi {}",
                native.termination, psi.termination
            ));
        }
        if psi.returned_unit && !native.returned_unit && native.exit == Some(0) {
            return Err("unavailable: the native program can exit the process; wait status 0 cannot establish a unit return".into());
        }
        if !psi.returned_unit && native.returned_unit {
            return Err(format!("native returned unit vs explicit Psi exit {code}"));
        }
        if cfg!(unix)
            && !native.signed_native_zero
            && !psi.returned_unit
            && native.exit == Some(code & 255)
        {
            return Err(format!(
                "unavailable: native wait status {} exposes only the low byte of the process exit; Psi explicitly exited {code}",
                code & 255
            ));
        }
        if !psi.returned_unit && code == 0 && native.exit == Some(0) {
            return Err("unavailable: native wait status 0 cannot distinguish explicit exit 0 from a unit return".into());
        }
        if native.exit != Some(code) {
            return Err(format!(
                "native {} vs Psi {}",
                native.termination, psi.termination
            ));
        }
    } else {
        match (psi.trap, native.trap) {
            (Some(true), Some(true)) => {}
            (Some(true), Some(false)) => {
                return Err(format!(
                    "native {} vs Psi {}",
                    native.termination, psi.termination
                ));
            }
            _ => {
                return Err(format!(
                    "unavailable: no host presentation for Psi {}",
                    psi.termination
                ));
            }
        }
    }
    Ok(())
}

/// Read only the requested build report, never a path printed by the guest.
fn report_value<'a>(report: &'a str, prefix: &str) -> Result<&'a str, String> {
    let mut lines = report.lines().filter_map(|line| line.strip_prefix(prefix));
    let value = lines
        .next()
        .ok_or_else(|| format!("missing {prefix} in build report"))?;
    if lines.next().is_some() {
        return Err(format!("ambiguous {prefix} in build report"));
    }
    Ok(value)
}

fn run_artifact(
    cli: &Path,
    project: &SampleProject,
    entry: &Path,
    scratch: &Path,
    directory: &Path,
    evidence: &Path,
    stdin: Stdio,
) -> Result<(), String> {
    let build = evidence.join("native-build");
    let report = evidence.join("native-build.txt");
    let mut compiler = Command::new(cli);
    compiler
        .arg("--build-dir")
        .arg(&build)
        .arg("--report-file")
        .arg(&report);
    with_source_inputs(&mut compiler, &project.source_inputs);
    compiler
        .arg(entry)
        .current_dir(scratch)
        .stdin(Stdio::null());
    let output = execution::output(&mut compiler, evidence, "compile-native")?;
    if !output.status.success() {
        return Err(format!(
            "build {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let report = fs::read_to_string(&report).map_err(|e| format!("{}: {e}", report.display()))?;
    let artifact = scratch
        .join(report_value(&report, "published native output to ")?)
        .canonicalize()
        .map_err(|e| format!("published executable: {e}"))?;
    if !artifact.is_file()
        || !artifact.starts_with(build.canonicalize().map_err(|e| e.to_string())?)
    {
        return Err("reported executable is outside this sample's native build directory".into());
    }
    let admission = report_value(&report, "process-exit-")?;
    if !matches!(admission, "excluded" | "possible" | "unknown") {
        return Err(format!("unknown process-exit admission {admission}"));
    }
    let expected_trap = report_value(&report, "trap-presentation: ")?;
    let (output, explicit_exit) = process_exit::run(&artifact, scratch, stdin, evidence, &report)?;
    let status = bounded_process::BoundedProcessExitStatus::from(output.status);
    let termination = match (status.code(), status.unix_signal()) {
        (Some(code), _) => format!("exit {code}"),
        (_, Some(signal)) => format!("signal {signal}"),
        _ => "terminated".into(),
    };
    let trap = if expected_trap == "unknown" {
        "unknown"
    } else if termination == expected_trap {
        "trap"
    } else {
        "other"
    };
    let termination = match explicit_exit {
        Some(process_exit::ExitEvidence::Explicit(code)) => format!("explicit-exit\t{code}"),
        #[cfg(target_os = "macos")]
        Some(process_exit::ExitEvidence::SignedZero) => String::from("signed-zero"),
        None => termination.replace(' ', "\t"),
    };
    fs::create_dir(directory)
        .and_then(|_| fs::write(directory.join("stdout"), output.stdout))
        .and_then(|_| fs::write(directory.join("stderr"), output.stderr))
        .and_then(|_| {
            fs::write(
                directory.join("termination.tsv"),
                format!("native\t{termination}\t{trap}\tprocess-exit-{admission}\n"),
            )
        })
        .map_err(|e| format!("{}: {e}", directory.display()))
}

pub(super) fn compare(
    cli: &Path,
    project: &SampleProject,
    entry: &Path,
    target: &str,
    evidence: &Path,
    input: Option<&Path>,
    oracle: &Oracle,
) -> Result<String, String> {
    if target != host_target() || target == "unsupported" {
        return Err(format!(
            "native/Psi execution requires host target {}, requested {target}",
            host_target()
        ));
    }
    fs::create_dir_all(evidence).map_err(|e| format!("{}: {e}", evidence.display()))?;
    let snapshot = evidence.join("stdin");
    if let Some(path) = input {
        fs::copy(path, &snapshot).map_err(|e| format!("capture {}: {e}", path.display()))?;
    }
    let run = |source: &str| -> Result<Observation, String> {
        let scratch = evidence.join(format!("{source}-cwd"));
        fs::create_dir(&scratch).map_err(|e| format!("{}: {e}", scratch.display()))?;
        let directory = evidence.join(format!("{source}-outputs"));
        let stdin = match input {
            Some(_) => fs::File::open(&snapshot)
                .map(Stdio::from)
                .map_err(|e| format!("{}: {e}", snapshot.display()))?,
            None => Stdio::null(),
        };
        if source == "native" {
            run_artifact(cli, project, entry, &scratch, &directory, evidence, stdin)?;
        } else {
            let mut cmd = Command::new(cli);
            cmd.args(["run", "--outputs"]).arg(&directory);
            if let Some(budget) = project.psi_work_budget {
                cmd.arg("--psi-work-budget").arg(budget.to_string());
            }
            with_source_inputs(&mut cmd, &project.source_inputs);
            cmd.arg(entry).current_dir(&scratch).stdin(stdin);
            let output = execution::output(&mut cmd, evidence, "run-psi")?;
            if !output.status.success() {
                return Err(format!(
                    "command {}: {} {}",
                    output.status,
                    Observation::read(&directory, source)
                        .err()
                        .unwrap_or_default(),
                    String::from_utf8_lossy(&output.stderr),
                ));
            }
        }
        let mut observation = Observation::read(&directory, source)?;
        observation.files =
            file_outputs::collect(&scratch, &oracle.files, &directory.join("files"))?;
        Ok(observation)
    };
    let native = run("native");
    let psi = run("psi");
    let mut failures = Vec::new();
    for (name, observation) in [("native", &native), ("Psi", &psi)] {
        match observation {
            Ok(observation) => {
                if let Err(reason) = oracle.check(observation) {
                    failures.push(format!("{name} expectation: {reason}"));
                }
            }
            Err(reason) => failures.push(format!("{name} execution: {reason}")),
        }
    }
    let comparison = match (&native, &psi) {
        (Ok(native), Ok(psi)) => agreement(native, psi),
        _ => Err("unavailable: both observations are required".into()),
    };
    if let Err(reason) = &comparison {
        failures.push(format!("comparison: {reason}"));
    } else if !failures.is_empty() {
        failures.push("comparison: native and Psi agree".into());
    }
    if !failures.is_empty() {
        return Err(failures.join("; "));
    }
    let psi = psi?;
    Ok(match psi.exit {
        Some(0) if psi.returned_unit => "pass (native wait status 0, Psi returned unit)".into(),
        Some(code) => format!("pass (native exit {code}, Psi exit {code})"),
        None => format!("pass (native and Psi {})", psi.termination),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compare_recorded_exits_crashes_and_incomplete_runs() {
        let dir = execution::reserve_directory(&std::env::temp_dir()).unwrap();
        fs::write(dir.join("stdout"), []).unwrap();
        fs::write(dir.join("stderr"), []).unwrap();
        let read = |source: &str, record: &str| {
            fs::write(dir.join("termination.tsv"), record).unwrap();
            Observation::read(&dir, source)
        };
        let native_zero =
            read("native", "native\texit\t0\tother\tprocess-exit-possible\n").unwrap();
        let psi_return = read("psi", "psi\treturn\n").unwrap();
        for code in [0, 326] {
            let native = read(
                "native",
                &format!("native\texplicit-exit\t{code}\tother\tprocess-exit-possible\n"),
            )
            .unwrap();
            let explicit = read("psi", &format!("psi\texit\t{code}\n")).unwrap();
            agreement(&native, &explicit).unwrap();
            assert!(agreement(&native, &psi_return).is_err());
            let other = read("psi", &format!("psi\texit\t{}\n", code ^ 256)).unwrap();
            assert!(agreement(&native, &other).is_err());
        }
        assert!(
            agreement(&native_zero, &psi_return)
                .unwrap_err()
                .contains("cannot establish")
        );
        let known_return =
            read("native", "native\texit\t0\tother\tprocess-exit-excluded\n").unwrap();
        agreement(&known_return, &psi_return).unwrap();
        for code in [0, 23, 279, -1] {
            let host = if cfg!(unix) { code & 255 } else { code };
            let native = read(
                "native",
                &format!("native\texit\t{host}\tother\tprocess-exit-possible\n"),
            )
            .unwrap();
            let explicit = read("psi", &format!("psi\texit\t{code}\n")).unwrap();
            let comparison = agreement(&native, &explicit);
            if cfg!(unix) {
                assert!(comparison.unwrap_err().contains("only the low byte"));
            } else if code == 0 {
                assert!(comparison.unwrap_err().contains("distinguish"));
            } else {
                comparison.unwrap();
            }
        }
        let signed_zero = read(
            "native",
            "native\tsigned-zero\tother\tprocess-exit-possible\n",
        )
        .unwrap();
        let psi_256 = read("psi", "psi\texit\t256\n").unwrap();
        assert!(
            agreement(&signed_zero, &psi_256)
                .unwrap_err()
                .contains("signed exit status 0")
        );
        let psi_0 = read("psi", "psi\texit\t0\n").unwrap();
        assert!(
            agreement(&signed_zero, &psi_0)
                .unwrap_err()
                .contains("cannot distinguish")
        );
        let native_trap =
            read("native", "native\tsignal\t4\ttrap\tprocess-exit-excluded\n").unwrap();
        let psi_trap = read("psi", "psi\tcrash\tTrap\n").unwrap();
        assert!(agreement(&native_trap, &psi_trap).is_ok());
        assert!(agreement(&native_zero, &psi_trap).is_err());
        let psi_abort = read("psi", "psi\tcrash\tAbort\n").unwrap();
        assert!(
            agreement(&native_trap, &psi_abort)
                .unwrap_err()
                .contains("unavailable")
        );
        let entry = dir.join("main.omg");
        fs::write(&entry, "// Expected exit: 0.\n").unwrap();
        fs::write(dir.join("expected_native_exit.txt"), "trap\n").unwrap();
        let project = super::super::project_at(&dir, &dir).unwrap();
        let oracle = Oracle::load(&project, &entry, "linux_x86_64", &dir).unwrap();
        oracle.check(&native_trap).unwrap();
        oracle.check(&psi_trap).unwrap();
        for wrong in [&known_return, &psi_return, &psi_abort] {
            assert!(oracle.check(wrong).unwrap_err().contains("expected Trap"));
        }
        for kind in ["incomplete", "unavailable", "failed"] {
            let report = format!("psi\t{kind}\treason\n");
            let result = read("psi", &report);
            assert_eq!(result.err().unwrap(), format!("{kind} reason"));
        }
        fs::remove_dir_all(dir).unwrap();
    }
}
