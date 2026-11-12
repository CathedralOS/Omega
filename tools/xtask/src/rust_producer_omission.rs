//! Developer-only audit of declared source dependencies and invocation surfaces.
//! This never builds or interprets a bootstrap stage. A complete declaration is
//! a premise: lexical inspection cannot discover an undeclared dependency.

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

fn finding(findings: &mut Vec<Value>, check: &str, member: &str, detail: String) {
    findings.push(json!({"check": check, "member": member, "detail": detail}));
}

fn regular_text(path: &Path) -> Result<String, String> {
    let metadata = fs::symlink_metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!("not a regular file: {}", path.display()));
    }
    fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

fn closure_header(line: &str) -> bool {
    line.split_once("SourceClosureV")
        .is_some_and(|(language, version)| {
            !language.is_empty()
                && language.bytes().all(|b| b.is_ascii_alphabetic())
                && !version.is_empty()
                && version.bytes().all(|b| b.is_ascii_digit())
        })
}

fn audit(manifests: &[PathBuf], steps: &[PathBuf], root: &Path) -> Result<Value, String> {
    let root = root
        .canonicalize()
        .map_err(|e| format!("declared root: {e}"))?;
    if !root.is_dir() {
        return Err("declared root is not a directory".into());
    }
    let mut findings = Vec::new();
    let mut member_count = 0;
    let mut step_count = 0;
    for manifest in manifests {
        let text = regular_text(manifest)?;
        if !text.is_ascii() {
            return Err(format!("manifest is not ASCII: {}", manifest.display()));
        }
        let closure = text
            .lines()
            .next()
            .is_some_and(|line| closure_header(line.trim()));
        let canonical = manifest.canonicalize().map_err(|e| e.to_string())?;
        let base = canonical.parent().ok_or("manifest has no parent")?;
        for (index, line) in text.lines().enumerate() {
            let line = line.trim();
            if (index == 0 && closure) || line.is_empty() || line.starts_with('#') {
                continue;
            }
            let spelling = if closure {
                let fields: Vec<_> = line.split(' ').collect();
                if fields.len() != 5 || fields[0] != "member" || fields.iter().any(|s| s.is_empty())
                {
                    return Err(format!(
                        "malformed member row at {}:{}",
                        manifest.display(),
                        index + 1
                    ));
                }
                fields[4]
            } else {
                line
            };
            member_count += 1;
            if spelling
                .bytes()
                .any(|b| !(33..=126).contains(&b) || b == b'\\' || b == b':')
                || spelling
                    .split('/')
                    .any(|part| part.is_empty() || matches!(part, "." | ".."))
            {
                finding(
                    &mut findings,
                    "canonical-spelling",
                    spelling,
                    format!("noncanonical member path: {spelling}"),
                );
                continue;
            }
            if !base.join(spelling).starts_with(&root) {
                finding(
                    &mut findings,
                    "checkout-derived-path",
                    spelling,
                    format!("member resolves outside declared root: {spelling}"),
                );
            }
            if spelling.split('/').any(|part| part == "omega-rust") {
                finding(
                    &mut findings,
                    "producer-artifact",
                    spelling,
                    format!("member is inside omega-rust/: {spelling}"),
                );
            }
        }
    }
    for path in steps {
        for (index, line) in regular_text(path)?.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            step_count += 1;
            // Quoted and escaped command names remain visible. A physical line
            // can be an incomplete shell fragment; preserve the old lexical
            // fallback rather than silently omitting such an invocation.
            let tokens = shlex::split(line)
                .unwrap_or_else(|| line.split_whitespace().map(str::to_owned).collect());
            for token in tokens {
                let label = format!("{}:{}", path.display(), index + 1);
                let name = token.rsplit('/').next().unwrap_or(&token);
                if matches!(
                    name,
                    "cargo" | "rustc" | "rustup" | "mbx" | "nextest" | "cargo-nextest"
                ) || token.contains("omega-rust")
                {
                    finding(
                        &mut findings,
                        "producer-build-step",
                        &label,
                        format!("step references Rust producer: {token}"),
                    );
                }
                if token.contains("CARGO_MANIFEST_DIR")
                    || token.contains("OUT_DIR")
                    || (token.contains("..") && token.contains("source/library"))
                {
                    finding(
                        &mut findings,
                        "checkout-derived-path",
                        &label,
                        format!("step derives a checkout path: {token}"),
                    );
                }
            }
        }
    }
    Ok(json!({
        "schema": "ReleaseRecordSubstrateV1",
        "subject": {"manifests": manifests, "steps": steps, "root": root},
        "member_count": member_count, "step_count": step_count,
        "checks": ["canonical-spelling", "producer-artifact", "producer-build-step", "checkout-derived-path"],
        "result": if findings.is_empty() { "omitted" } else { "present" },
        "findings": findings,
    }))
}

fn execute(args: &[String]) -> Result<bool, String> {
    let mut manifests = Vec::new();
    let mut steps = Vec::new();
    let mut root = crate::repo_root();
    let mut record_path = None;
    let mut expected = "omitted";
    let mut arguments = args.iter();
    while let Some(option) = arguments.next() {
        let value = arguments
            .next()
            .ok_or_else(|| format!("missing value for {option}"))?;
        match option.as_str() {
            "--manifest" => manifests.push(PathBuf::from(value)),
            "--steps" => steps.push(PathBuf::from(value)),
            "--root" => root = PathBuf::from(value),
            "--record" => record_path = Some(PathBuf::from(value)),
            "--require" if matches!(value.as_str(), "omitted" | "present") => expected = value,
            _ => return Err(format!("unknown option or value: {option} {value}")),
        }
    }
    if manifests.is_empty() && steps.is_empty() {
        return Err("nothing to audit: supply --manifest or --steps".into());
    }
    let record = audit(&manifests, &steps, &root)?;
    if let Some(path) = record_path {
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(
            path,
            serde_json::to_string_pretty(&record).map_err(|e| e.to_string())? + "\n",
        )
        .map_err(|e| e.to_string())?;
    }
    println!(
        "rust producer omission: {} ({} members, {} steps, {} findings)",
        record["result"].as_str().unwrap(),
        record["member_count"],
        record["step_count"],
        record["findings"].as_array().unwrap().len()
    );
    for entry in record["findings"].as_array().unwrap() {
        eprintln!(
            "  {}: {}",
            entry["check"].as_str().unwrap(),
            entry["detail"].as_str().unwrap()
        );
    }
    Ok(record["result"] == expected)
}

pub(crate) fn run(args: &[String]) -> Result<(), u8> {
    match execute(args) {
        Ok(true) => Ok(()),
        Ok(false) => Err(1),
        Err(message) => {
            eprintln!("rust producer omission: {message}");
            Err(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "omega-omission-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn write(&self, name: &str, text: &str) -> PathBuf {
            let path = self.0.join(name);
            fs::write(&path, text).unwrap();
            path
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn quoted_and_escaped_producers_cannot_hide_in_steps() {
        let fixture = Fixture::new();
        for source in [
            "'cargo' build",
            "ru\\stc input.rs",
            "/bin/mbx build",
            "cat omega-rust/input",
            "echo ${CARGO_MANIFEST_DIR}/source/library",
        ] {
            let steps = fixture.write("steps.sh", source);
            assert_eq!(
                audit(&[], &[steps], &fixture.0).unwrap()["result"],
                "present",
                "{source}"
            );
        }
        let steps = fixture.write("steps.sh", "# cargo build\npython3 source_closure.py\n");
        assert_eq!(
            audit(&[], &[steps], &fixture.0).unwrap()["result"],
            "omitted"
        );
    }

    #[test]
    fn manifest_path_and_shape_controls_preserve_the_declared_boundary() {
        let fixture = Fixture::new();
        for member in [
            "omega-rust/input",
            "../outside",
            "/absolute",
            "path//member",
            "path\\member",
            "C:/member",
        ] {
            let manifest = fixture.write("members", member);
            assert_eq!(
                audit(&[manifest], &[], &fixture.0).unwrap()["result"],
                "present",
                "{member}"
            );
        }
        let manifest = fixture.write("members", "source/library/declared.omg\n");
        assert_eq!(
            audit(&[manifest], &[], &fixture.0).unwrap()["result"],
            "omitted"
        );
        let malformed = fixture.write("members", "EpsilonSourceClosureV1\nmember too few\n");
        assert!(audit(&[malformed], &[], &fixture.0).is_err());
    }

    #[test]
    fn verdict_and_record_are_distinct_from_malformed_invocation() {
        let fixture = Fixture::new();
        let member = fixture.write("members", "omega-rust/input\n");
        let record = fixture.0.join("audit.json");
        let mut args = vec![
            "--root".into(),
            fixture.0.display().to_string(),
            "--manifest".into(),
            member.display().to_string(),
            "--record".into(),
            record.display().to_string(),
            "--require".into(),
            "present".into(),
        ];
        assert_eq!(execute(&args), Ok(true));
        let result: Value = serde_json::from_str(&fs::read_to_string(record).unwrap()).unwrap();
        assert_eq!(result["result"], "present");
        *args.last_mut().unwrap() = "omitted".into();
        assert_eq!(execute(&args), Ok(false));
        assert!(execute(&[]).is_err());
    }
}
