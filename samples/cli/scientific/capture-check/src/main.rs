//! Compare saved Terminal captures with the expectations shipped by each sample.
use std::{env, fs, path::Path, process::ExitCode};

struct Case {
    name: String,
    exit: i32,
    stdout: Option<Vec<u8>>,
}

fn checked_name(name: &str) -> Result<&str, String> {
    if name.is_empty()
        || name.len() > 64
        || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err(format!("invalid case name {name:?}"));
    }
    Ok(name)
}

fn read(path: &Path) -> Result<Vec<u8>, String> {
    fs::read(path).map_err(|error| format!("{}: {error}", path.display()))
}

fn cases(sample: &Path) -> Result<Vec<Case>, String> {
    let data = sample.join("data");
    let mut cases = Vec::new();
    match sample.file_name().and_then(|s| s.to_str()) {
        Some("pareto_experiment_selection") => {
            for entry in fs::read_dir(&data).map_err(|e| format!("{}: {e}", data.display()))? {
                let path = entry.map_err(|e| e.to_string())?.path();
                if path.extension().is_none_or(|ext| ext != "stdin") {
                    continue;
                }
                let name = checked_name(path.file_stem().and_then(|s| s.to_str()).unwrap_or(""))?;
                let stdout = read(&sample.join("expected").join(format!("{name}.stdout")))?;
                cases.push(Case {
                    name: name.to_owned(),
                    exit: if name.starts_with("invalid_") { 2 } else { 0 },
                    stdout: Some(stdout),
                });
            }
        }
        Some("spatial_diffusion") => {
            let file = data.join("oracle.json");
            let oracle: serde_json::Value = serde_json::from_slice(&read(&file)?)
                .map_err(|e| format!("{}: {e}", file.display()))?;
            for row in oracle["cases"].as_array().ok_or("missing frozen cases")? {
                let name = checked_name(row["name"].as_str().ok_or("non-string case name")?)?;
                let exit = i32::try_from(row["exit"].as_i64().ok_or("non-integer case exit")?)
                    .map_err(|_| format!("{name}: case exit outside i32"))?;
                cases.push(Case {
                    name: name.to_owned(),
                    exit,
                    stdout: None,
                });
            }
        }
        _ => {
            return Err(format!(
                "no frozen capture contract for {}",
                sample.display()
            ));
        }
    }
    if cases.is_empty() {
        return Err(format!("no frozen cases in {}", sample.display()));
    }
    cases.sort_by(|a, b| a.name.cmp(&b.name));
    if let Some(pair) = cases.windows(2).find(|pair| pair[0].name == pair[1].name) {
        return Err(format!("duplicate frozen case {}", pair[0].name));
    }
    Ok(cases)
}

impl Case {
    fn compare(&self, exit: i32, stdout: &[u8], stderr: &[u8]) -> Result<(), String> {
        let stdout_matches = self
            .stdout
            .as_deref()
            .is_none_or(|expected| expected == stdout);
        if exit == self.exit && stdout_matches && stderr.is_empty() {
            return Ok(());
        }
        Err(format!(
            "{}: exit={exit} expected={}, stdout_match={stdout_matches}, stderr_empty={}",
            self.name,
            self.exit,
            stderr.is_empty()
        ))
    }

    fn check(&self, captures: &Path) -> Result<(), String> {
        let name = &self.name;
        let status = captures.join(format!("{name}.exit"));
        let status_bytes = read(&status)?;
        let exit = std::str::from_utf8(&status_bytes)
            .ok()
            .and_then(|s| s.trim().parse().ok())
            .ok_or_else(|| format!("{}: expected an integer exit code", status.display()))?;
        self.compare(
            exit,
            &read(&captures.join(format!("{name}.stdout")))?,
            &read(&captures.join(format!("{name}.stderr")))?,
        )
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args_os().skip(1);
    let (Some(sample), Some(captures), None) = (args.next(), args.next(), args.next()) else {
        return Err("usage: scientific-capture-check SAMPLE_DIRECTORY CAPTURE_DIRECTORY".into());
    };
    let cases = cases(Path::new(&sample))?;
    for case in &cases {
        case.check(Path::new(&captures))?;
    }
    println!(
        "{} recorded cases matched the sample's frozen expectations",
        cases.len()
    );
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_shipped_cases_enforce_their_own_frozen_fields() {
        let samples = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        for (sample, count, checks_stdout) in [
            ("spatial_diffusion", 20, false),
            ("pareto_experiment_selection", 28, true),
        ] {
            let roster = cases(&samples.join(sample)).unwrap();
            assert_eq!(roster.len(), count);
            for case in roster {
                assert_eq!(case.stdout.is_some(), checks_stdout);
                let stdout = case.stdout.as_deref().unwrap_or(b"diffusion field output");
                assert!(
                    case.compare(case.exit, stdout, b"").is_ok(),
                    "{}",
                    case.name
                );
                assert!(
                    case.compare(case.exit.wrapping_add(1), stdout, b"")
                        .is_err()
                );
                assert!(case.compare(case.exit, stdout, b"diagnostic").is_err());
                let mut wrong = stdout.to_vec();
                wrong.push(b'!');
                assert_eq!(case.compare(case.exit, &wrong, b"").is_err(), checks_stdout);
            }
        }
    }
}
