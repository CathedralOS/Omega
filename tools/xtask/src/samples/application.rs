//! Compare the two engines on the sample's actual application entry.

use super::{SampleProject, host_target, optional_text, parse_header_oracle, with_source_inputs};
use crate::execution;
use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
};

pub(super) struct Oracle {
    exit: Option<i32>,
    stdout: Option<Vec<u8>>,
    stdout_contains: Option<String>,
    stderr: Option<Vec<u8>>,
}

impl Oracle {
    pub(super) fn load(
        directory: &Path,
        entry: &Path,
        target: &str,
        has_input: bool,
    ) -> Result<Self, String> {
        let (exit, stdout_contains) = parse_header_oracle(entry)?;
        let mut oracle = Self {
            exit,
            stdout: None,
            stdout_contains,
            stderr: None,
        };
        if has_input {
            let target_exit = directory.join(format!("expected_native_exit.{target}.txt"));
            let generic_exit = directory.join("expected_native_exit.txt");
            for path in [target_exit, generic_exit] {
                if let Some(text) = optional_text(&path)? {
                    oracle.exit =
                        Some(text.trim().parse::<i32>().map_err(|_| {
                            format!("{} requires an i32 exit status", path.display())
                        })?);
                    break;
                }
            }
            oracle.stdout = optional_bytes(&directory.join("expected_stdout.txt"))?;
            oracle.stderr = optional_bytes(&directory.join("expected_stderr.txt"))?;
        }
        Ok(oracle)
    }

    pub(super) fn is_empty(&self) -> bool {
        self.exit.is_none()
            && self.stdout.is_none()
            && self.stdout_contains.is_none()
            && self.stderr.is_none()
    }

    fn check(&self, observation: &Observation) -> Result<(), String> {
        if let Some(expected) = self.exit
            && observation.exit != Some(expected)
        {
            return Err(format!(
                "expected exit {expected}, got {}",
                observation.termination
            ));
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
        Ok(())
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
    exit: Option<i32>,
    trap: Option<bool>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl Observation {
    fn read(directory: &Path, engine: &str) -> Result<Self, String> {
        let path = directory.join("termination.tsv");
        let record = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let (source, termination) = record
            .trim_end()
            .split_once('\t')
            .ok_or("missing engine in termination record")?;
        if source != engine {
            return Err(format!("expected {engine} observation, found {source}"));
        }
        let fields: Vec<_> = termination.split('\t').collect();
        let number = |text: &str| {
            text.parse::<i32>()
                .map_err(|_| format!("invalid process status {text:?}"))
        };
        let (exit, trap) = match (engine, fields.as_slice()) {
            ("native", ["return", "0", _]) => (Some(0), native_trap(&fields)?),
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
            _ => return Err(format!("invalid {engine} termination {termination:?}")),
        };
        Ok(Self {
            termination: termination.replace('\t', " "),
            returned_unit: fields.first() == Some(&"return"),
            exit,
            trap,
            stdout: execution::read_bounded(&directory.join("stdout"))?,
            stderr: execution::read_bounded(&directory.join("stderr"))?,
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
    if let Some(code) = psi.exit {
        if psi.returned_unit && !native.returned_unit && native.exit == Some(0) {
            return Err("unavailable: the native program can exit the process; wait status 0 cannot establish a unit return".into());
        }
        if !psi.returned_unit && native.returned_unit {
            return Err(format!("native returned unit vs explicit Psi exit {code}"));
        }
        if cfg!(unix) && !psi.returned_unit && native.exit == Some(code & 255) {
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

pub(super) fn compare(
    cli: &Path,
    project: &SampleProject,
    entry: &Path,
    target: &str,
    evidence: &Path,
    input: Option<&Path>,
    oracle: &Oracle,
) -> Result<String, String> {
    if target != host_target() {
        return Err(format!(
            "native/Psi execution requires host target {}, requested {target}",
            host_target()
        ));
    }
    let snapshot = evidence.join("stdin");
    if let Some(path) = input {
        fs::copy(path, &snapshot).map_err(|e| format!("capture {}: {e}", path.display()))?;
    }
    let run = |engine: &str| -> Result<Observation, String> {
        let scratch = evidence.join(format!("{engine}-cwd"));
        fs::create_dir(&scratch).map_err(|e| format!("{}: {e}", scratch.display()))?;
        let directory = evidence.join(format!("{engine}-outputs"));
        let mut cmd = Command::new(cli);
        cmd.args(["run", "--engine", engine, "--outputs"])
            .arg(&directory);
        with_source_inputs(&mut cmd, &project.source_inputs);
        cmd.arg(entry).current_dir(&scratch);
        cmd.stdin(match input {
            Some(_) => fs::File::open(&snapshot)
                .map(Stdio::from)
                .map_err(|e| format!("{}: {e}", snapshot.display()))?,
            None => Stdio::null(),
        });
        let name = format!("run-{engine}");
        let output = execution::output(&mut cmd, evidence, &name)?;
        let observation = Observation::read(&directory, engine);
        if !output.status.success() {
            return Err(format!(
                "command {}: {} {}",
                output.status,
                observation.err().unwrap_or_default(),
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        observation
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
        let read = |engine: &str, record: &str| {
            fs::write(dir.join("termination.tsv"), record).unwrap();
            Observation::read(&dir, engine)
        };
        let native_zero = read("native", "native\texit\t0\tother\n").unwrap();
        let psi_return = read("psi", "psi\treturn\n").unwrap();
        assert!(
            agreement(&native_zero, &psi_return)
                .unwrap_err()
                .contains("cannot establish")
        );
        let known_return = read("native", "native\treturn\t0\tother\n").unwrap();
        agreement(&known_return, &psi_return).unwrap();
        for code in [0, 23, 279, -1] {
            let host = if cfg!(unix) { code & 255 } else { code };
            let native = read("native", &format!("native\texit\t{host}\tother\n")).unwrap();
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
        let native_trap = read("native", "native\tsignal\t4\ttrap\n").unwrap();
        let psi_trap = read("psi", "psi\tcrash\tTrap\n").unwrap();
        assert!(agreement(&native_trap, &psi_trap).is_ok());
        assert!(agreement(&native_zero, &psi_trap).is_err());
        let psi_abort = read("psi", "psi\tcrash\tAbort\n").unwrap();
        assert!(
            agreement(&native_trap, &psi_abort)
                .unwrap_err()
                .contains("unavailable")
        );
        for kind in ["incomplete", "unavailable", "failed"] {
            let report = format!("psi\t{kind}\treason\n");
            let result = read("psi", &report);
            assert_eq!(result.err().unwrap(), format!("{kind} reason"));
        }
        fs::remove_dir_all(dir).unwrap();
    }
}
