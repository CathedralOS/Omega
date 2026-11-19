//! One bounded, exact-source qualification of sample programs through the CLI.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::json;

use crate::{execution, producer, samples};

/// The most sample programs one request may name.
const SELECTION_BUDGET: usize = 200;

fn exact_paths(text: &str, prefix: &str) -> Result<Vec<String>, String> {
    let mut paths = BTreeSet::new();
    for value in text.split(',') {
        let value = value.trim();
        if value.is_empty()
            || !value.starts_with(prefix)
            || value.contains('\\')
            || value.split('/').any(|part| matches!(part, "" | "." | ".."))
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

fn execute(
    root: &Path,
    directory: &Path,
    selection: &[String],
    target: &str,
) -> Result<(), String> {
    let source = producer::source_identity(root)?;
    execution::write_json(&directory.join("source-before.json"), &source)?;
    if source["tracked_changes"] != "" {
        return Err("qualification requires a clean committed source tree".to_owned());
    }
    let archive = directory.join("source.zip");
    let status = execution::run(
        Command::new("git")
            .args(["archive", "--format=zip", "-o"])
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
        &directory.join("source-archive-metadata.json"),
        &json!({"source": source, "path": archive, "sha256": execution::digest(&archive)?}),
    )?;
    let prepared = producer::prepare(root, &directory.join("producer"))?;
    let result = (|| {
        let mut failures = Vec::new();
        let mut observations = Vec::new();
        for path in selection {
            let project = samples::project_at(&root.join(path), root)?;
            prepared.verify()?;
            let result = samples::verify_sample(
                &prepared.cli,
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
    let mut errors = Vec::new();
    if let Err(error) = result {
        errors.push(error);
    }
    if let Err(error) = verified {
        errors.push(error);
    }
    if source != after {
        errors.push(
            "source changed during qualification; before/after identities retained".to_owned(),
        );
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

pub fn run(args: &[String]) -> Result<(), u8> {
    let result = (|| {
        let root = crate::repository::current()?;
        let mut selection = Vec::new();
        let mut directory = None;
        let mut target = producer::host_target().to_owned();
        let mut i = 0;
        let mut options = BTreeSet::new();
        while i < args.len() {
            if matches!(args[i].as_str(), "--help" | "-h") {
                println!(
                    "usage: mbx xtask qualify --samples samples/path,... --output <new-directory> [--target <host>]"
                );
                return Ok(());
            }
            let option = args[i].as_str();
            if !options.insert(option) {
                return Err(format!("duplicate qualification option: {option}"));
            }
            i += 1;
            let value = args
                .get(i)
                .ok_or_else(|| format!("{option} requires a value"))?;
            match option {
                "--samples" => selection = exact_paths(value, "samples/")?,
                "--target" => target = value.clone(),
                "--output" => directory = Some(PathBuf::from(value)),
                _ => return Err(format!("unknown qualification option: {option}")),
            }
            i += 1;
        }
        let count = selection.len();
        if count == 0 || count > SELECTION_BUDGET {
            return Err(
                "request 1 through 200 exact programs before preparing a producer".to_owned(),
            );
        }
        if target != producer::host_target() || target == "unsupported" {
            return Err(format!(
                "requested {target} does not match this hosted executor"
            ));
        }
        for path in &selection {
            let project = samples::project_at(&root.join(path), &root)?;
            let entry = project
                .entry
                .as_deref()
                .ok_or_else(|| format!("{path}: missing entry"))?;
            if project.role == "intended-invalid" {
                if project.rejection.is_empty() && project.rejection_any.is_empty() {
                    return Err(format!("{path}: missing intended-rejection fragments"));
                }
            } else if !matches!(project.role.as_str(), "program" | "example")
                || !samples::bound_targets(&project.directory).contains(&target)
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
        fs::create_dir_all(directory.parent().ok_or("output has no parent")?)
            .map_err(|e| e.to_string())?;
        fs::create_dir(&directory)
            .map_err(|e| format!("reserve new output {}: {e}", directory.display()))?;
        execution::write_json(
            &directory.join("request.json"),
            &json!({"target": target, "selection": {"samples": selection}}),
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
    fn rejects_ambiguous_and_escaping_requests() {
        assert!(exact_paths("samples/a", "samples/").is_ok());
        assert!(exact_paths("samples/a/../b", "samples/").is_err());
        assert!(exact_paths("samples/a,samples/a", "samples/").is_err());
        assert!(exact_paths("/samples/a", "samples/").is_err());
    }
}
