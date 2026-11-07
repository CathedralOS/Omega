//! One bounded, exact-source qualification through corpus and ordinary CLI owners.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::json;

use crate::{corpus, execution, producer, repo_root, samples};

#[derive(Default, serde::Serialize)]
struct Selection {
    checking: Vec<String>,
    runtime: Vec<String>,
    samples: Vec<String>,
}

fn exact_paths(text: &str, prefix: &str) -> Result<Vec<String>, String> {
    let mut paths = BTreeSet::new();
    for value in text.split(',') {
        let value = value.trim();
        if value.is_empty()
            || !value.starts_with(prefix)
            || value.contains('\\')
            || Path::new(value)
                .components()
                .any(|p| !matches!(p, Component::Normal(_)))
        {
            return Err(format!("expected an exact {prefix} path, got {value:?}"));
        }
        if !paths.insert(value.to_owned()) {
            return Err(format!("duplicate requested path: {value}"));
        }
    }
    Ok(paths.into_iter().collect())
}

fn corpus_paths(text: &str) -> Result<Vec<String>, String> {
    let paths = exact_paths(text, "")?;
    for path in &paths {
        let parts: Vec<_> = path.split('/').collect();
        if parts.len() != 3 || !matches!(parts[0], "pass" | "fail" | "run") {
            return Err(format!("expected tier/group/fixture, got {path}"));
        }
    }
    Ok(paths)
}

fn validate_roster(
    requested: &[String],
    actual: impl IntoIterator<Item = String>,
) -> Result<(), String> {
    let mut found = BTreeSet::new();
    for name in actual {
        if !found.insert(name.clone()) {
            return Err(format!("duplicate observed fixture: {name}"));
        }
    }
    let expected: BTreeSet<_> = requested.iter().cloned().collect();
    if expected != found {
        return Err(format!(
            "requested roster mismatch; missing {:?}, extra {:?}",
            expected.difference(&found).collect::<Vec<_>>(),
            found.difference(&expected).collect::<Vec<_>>()
        ));
    }
    Ok(())
}

fn check_selection(root: &Path, paths: &[String]) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    let mut matched = Vec::new();
    // The harness uses substring filters. Discover the same bounded layout
    // first, and refuse a requested name that would also select another fixture.
    for tier in ["pass", "fail", "run"] {
        let base = root.join("tests/omega").join(tier);
        for group in fs::read_dir(&base).map_err(|e| format!("read {}: {e}", base.display()))? {
            let group = group.map_err(|e| e.to_string())?;
            if !group.file_type().map_err(|e| e.to_string())?.is_dir() {
                continue;
            }
            for fixture in fs::read_dir(group.path()).map_err(|e| e.to_string())? {
                let fixture = fixture.map_err(|e| e.to_string())?;
                if !fixture.file_type().map_err(|e| e.to_string())?.is_dir() {
                    continue;
                }
                let name = format!(
                    "{tier}/{}/{}",
                    group.file_name().to_string_lossy(),
                    fixture.file_name().to_string_lossy()
                );
                if paths.iter().any(|p| name.contains(p)) {
                    matched.push(name);
                }
            }
        }
    }
    validate_roster(paths, matched)
}

fn expected_exit(directory: &Path, target: &str, interpreted: bool) -> Result<String, String> {
    let host = directory.join(format!("expected_native_exit.{target}.txt"));
    let portable = directory.join("expected_native_exit.txt");
    let interpreter = directory.join("expected_interpreter_exit.txt");
    let path = if interpreted && interpreter.is_file() {
        interpreter
    } else if !interpreted && host.is_file() {
        host
    } else {
        portable
    };
    let text = fs::read_to_string(&path).map_err(|e| {
        format!(
            "runtime selection needs an authored exit oracle {}: {e}",
            path.display()
        )
    })?;
    let value = text
        .trim()
        .parse::<i32>()
        .map_err(|e| format!("invalid exit oracle {}: {e}", path.display()))?;
    Ok(value.to_string())
}

fn validate_record(
    row: &corpus::CorpusRecord,
    root: &Path,
    target: &str,
    runtime: bool,
) -> Result<(), String> {
    let directory = root.join("tests/omega").join(&row.fixture);
    let mut facts = BTreeMap::new();
    for (key, value) in &row.facts {
        if facts.insert(key.as_str(), value.as_str()).is_some() {
            return Err(format!("{} duplicates fact {key}", row.fixture));
        }
    }
    if row.tier == "fail" {
        if row.status != "rejected"
            || row.expected_satisfied != Some(true)
            || row.diagnostics.is_empty()
            || facts.contains_key("exit")
            || facts.contains_key("interpreter-exit")
        {
            return Err(format!(
                "{} did not produce its intended rejection",
                row.fixture
            ));
        }
        return Ok(());
    }
    if !matches!(row.status.as_str(), "checked" | "built")
        || row.expected_satisfied == Some(false)
        || !row.diagnostics.is_empty()
    {
        return Err(format!("{} did not check successfully", row.fixture));
    }
    if runtime {
        let native = expected_exit(&directory, target, false)?;
        let interpreted = expected_exit(&directory, target, true)?;
        if row.status != "built"
            || facts.get("exit") != Some(&native.as_str())
            || facts.get("interpreter-exit") != Some(&interpreted.as_str())
            || facts.get("interpreter") != Some(&"agrees")
        {
            return Err(format!(
                "{} did not execute both engines at its authored exits",
                row.fixture
            ));
        }
        for (oracle, key) in [
            ("expected_stdout.txt", "stdout"),
            ("expected_stderr.txt", "stderr"),
        ] {
            if directory.join(oracle).is_file() && facts.get(key) != Some(&"match") {
                return Err(format!("{} did not satisfy {oracle}", row.fixture));
            }
        }
    }
    Ok(())
}

fn execute(
    root: &Path,
    directory: &Path,
    selection: &Selection,
    target: &str,
) -> Result<(), String> {
    let source = producer::source_identity(root)?;
    execution::write_json(&directory.join("source-before.json"), &source)?;
    if source["tracked_changes"] != "" {
        return Err("qualification requires a clean committed source tree".to_owned());
    }
    let archive = directory.join("source.tar.gz");
    let status = execution::run(
        Command::new("git")
            .args(["archive", "--format=tar.gz", "-o"])
            .arg(&archive)
            .arg("HEAD")
            .current_dir(root)
            .stdin(Stdio::null()),
        directory,
        "source-archive",
    )?;
    if !status.success() {
        return Err("could not retain exact source archive".to_owned());
    }
    execution::write_json(
        &directory.join("source-archive.json"),
        &json!({"source": source, "sha256": execution::digest(&archive)?}),
    )?;
    let prepared = producer::prepare(
        root,
        &directory.join("producer"),
        !selection.checking.is_empty() || !selection.runtime.is_empty(),
        !selection.samples.is_empty(),
    )?;
    let result = (|| {
        let mut failures = Vec::new();
        for (label, paths, runtime) in [
            ("checking", &selection.checking, false),
            ("both", &selection.runtime, true),
        ] {
            if paths.is_empty() {
                continue;
            }
            let runner = prepared
                .runner
                .as_deref()
                .ok_or("runner was not prepared")?;
            prepared.verify()?;
            let text = corpus::observe(
                root,
                runner,
                &paths.join(","),
                "",
                runtime.then_some(target),
                runtime,
                false,
                &directory.join(label),
            )?;
            let rows = corpus::parse_records(&text)?;
            validate_roster(paths, rows.iter().map(|row| row.fixture.clone()))?;
            execution::write_json(&directory.join(format!("{label}-rows.json")), &rows)?;
            for row in rows {
                if let Err(error) = validate_record(&row, root, target, runtime) {
                    failures.push(error);
                }
            }
        }
        let mut observations = Vec::new();
        for path in &selection.samples {
            let project = samples::project_at(&root.join(path), root)?;
            prepared.verify()?;
            let result = samples::verify_sample(
                prepared.cli.as_deref().ok_or("CLI was not prepared")?,
                &project,
                target,
                &directory.join("samples").join(path),
            );
            match result {
                Ok(state) if state.starts_with("pass (") || state == "rejected" => {
                    observations.push(json!({"project": path, "outcome": state}))
                }
                Ok(state) => {
                    failures.push(format!("{path} did not execute: {state}"));
                    observations.push(json!({"project": path, "error": state}));
                }
                Err(error) => {
                    failures.push(format!("{path}: {error}"));
                    observations.push(json!({"project": path, "error": error}));
                }
            }
        }
        execution::write_json(&directory.join("sample-rows.json"), &observations)?;
        if failures.is_empty() {
            Ok(())
        } else {
            Err(failures.join("\n"))
        }
    })();
    let verified = prepared.verify();
    let after = producer::source_identity(root)?;
    execution::write_json(&directory.join("source-after.json"), &after)?;
    verified?;
    if source != after {
        return Err(
            "source changed during qualification; before/after identities retained".to_owned(),
        );
    }
    result
}

pub fn run(args: &[String]) -> Result<(), u8> {
    let result = (|| {
        let root = repo_root();
        let mut selection = Selection::default();
        let mut directory = None;
        let mut target = producer::host_target().to_owned();
        let mut i = 0;
        while i < args.len() {
            if matches!(args[i].as_str(), "--help" | "-h") {
                println!(
                    "usage: mbx xtask qualify [--corpus tier/group/name,...] [--runtime tier/group/name,...] [--samples samples/path,...] --output <new-directory> [--target <host>]"
                );
                return Ok(());
            }
            let option = args[i].as_str();
            i += 1;
            let value = args
                .get(i)
                .ok_or_else(|| format!("{option} requires a value"))?;
            match option {
                "--corpus" => selection.checking = corpus_paths(value)?,
                "--runtime" => selection.runtime = corpus_paths(value)?,
                "--samples" => selection.samples = exact_paths(value, "samples/")?,
                "--target" => target = value.clone(),
                "--output" => directory = Some(PathBuf::from(value)),
                _ => return Err(format!("unknown qualification option: {option}")),
            }
            i += 1;
        }
        let count = selection.checking.len() + selection.runtime.len() + selection.samples.len();
        if count == 0 || count > 200 {
            return Err(
                "request 1 through 200 exact programs before preparing a producer".to_owned(),
            );
        }
        if target != producer::host_target() || target == "unsupported" {
            return Err(format!(
                "requested {target} does not match this hosted executor"
            ));
        }
        check_selection(&root, &selection.checking)?;
        check_selection(&root, &selection.runtime)?;
        for path in selection.checking.iter().chain(&selection.runtime) {
            let directory = root.join("tests/omega").join(path);
            if !directory.join("main.omg").is_file() {
                return Err(format!("missing source: {path}"));
            }
            if path.starts_with("fail/")
                && fs::read_to_string(directory.join("expected.txt"))
                    .map_or(true, |t| t.trim().is_empty())
            {
                return Err(format!(
                    "{path} needs nonempty intended-rejection fragments"
                ));
            }
        }
        for path in &selection.runtime {
            if path.starts_with("fail/") {
                return Err(format!(
                    "{path}: use --corpus for rejection checking; the runtime harness selects executable tiers"
                ));
            }
            let directory = root.join("tests/omega").join(path);
            expected_exit(&directory, &target, false)?;
            expected_exit(&directory, &target, true)?;
        }
        for path in &selection.samples {
            let project = samples::project_at(&root.join(path), &root)?;
            let entry = project
                .entry
                .as_deref()
                .ok_or_else(|| format!("{path}: missing entry"))?;
            if project.role == "intended-invalid" {
                if project.rejection.is_empty() && project.rejection_any.is_empty() {
                    return Err(format!("{path}: missing intended-rejection fragments"));
                }
            } else if !samples::bound_targets(&project.directory).contains(&target)
                || samples::parse_header_oracle(entry).0.is_none()
            {
                return Err(format!(
                    "{path}: requires a host ProgramEntry and authored Expected exit"
                ));
            }
        }
        let directory = directory.ok_or("--output is required")?;
        let directory = if directory.is_absolute() {
            directory
        } else {
            root.join(directory)
        };
        if directory.exists() {
            return Err(format!(
                "{} already exists; retain each execution separately",
                directory.display()
            ));
        }
        fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
        execution::write_json(
            &directory.join("request.json"),
            &json!({"target": target, "selection": selection}),
        )?;
        let outcome = execute(&root, &directory, &selection, &target);
        execution::write_json(
            &directory.join("summary.json"),
            &json!({"success": outcome.is_ok(), "error": outcome.as_ref().err(), "target": target, "requested_programs": count}),
        )?;
        outcome
    })();
    result.map_err(|error| {
        eprintln!("qualify: {error}");
        1u8
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refuses_missing_extra_and_duplicate_observations() {
        let request = vec!["pass/a/one".to_owned()];
        assert!(validate_roster(&request, request.clone()).is_ok());
        assert!(validate_roster(&request, Vec::new()).is_err());
        assert!(validate_roster(&request, vec!["pass/a/one_more".to_owned()]).is_err());
        assert!(validate_roster(&request, vec!["pass/a/one".to_owned(); 2]).is_err());
    }
    #[test]
    fn rejects_ambiguous_and_escaping_requests() {
        assert!(corpus_paths("pass/a/one").is_ok());
        assert!(corpus_paths("one").is_err());
        assert!(corpus_paths("pass/a/../one").is_err());
        assert!(exact_paths("samples/a,samples/a", "samples/").is_err());
        assert!(exact_paths("/samples/a", "samples/").is_err());
    }
    #[test]
    fn accepted_invalid_and_wrong_reason_never_count_as_success() {
        let mut row = corpus::parse_records("fail/a/b rejected 1ms expected\n\texact obligation\n")
            .unwrap()
            .remove(0);
        assert!(validate_record(&row, Path::new("unused"), "linux_x86_64", false).is_ok());
        row.expected_satisfied = Some(false);
        assert!(validate_record(&row, Path::new("unused"), "linux_x86_64", false).is_err());
        row.expected_satisfied = Some(true);
        row.status = "checked".to_owned();
        assert!(validate_record(&row, Path::new("unused"), "linux_x86_64", false).is_err());
    }
}
