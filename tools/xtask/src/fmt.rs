//! Format every workspace target without exceeding Windows command-line limits.
//!
//! `cargo fmt --all` batches all workspace targets into a single `rustfmt`
//! invocation (>33,800 characters), exceeding Windows `CreateProcess`'s 32,767
//! character limit (os error 206). This command enumerates workspace targets
//! through `cargo metadata --no-deps`, groups them by edition, and invokes
//! `rustfmt` in chunks bounded below 24,000 characters.

use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

const ARGV_BUDGET: usize = 24_000;
const TARGET_KINDS: &[&str] = &[
    "lib",
    "bin",
    "test",
    "bench",
    "example",
    "custom-build",
    "proc-macro",
];

#[derive(Deserialize)]
struct Metadata {
    workspace_members: Vec<String>,
    packages: Vec<Package>,
}

#[derive(Deserialize)]
struct Package {
    id: String,
    targets: Vec<Target>,
}

#[derive(Deserialize)]
struct Target {
    kind: Vec<String>,
    edition: String,
    src_path: String,
}

fn workspace_targets(repository: &Path) -> Result<BTreeMap<String, Vec<String>>, String> {
    let output = Command::new("cargo")
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .current_dir(repository)
        .output()
        .map_err(|error| format!("cargo metadata failed: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let metadata: Metadata = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("cargo metadata JSON invalid: {error}"))?;
    let members: BTreeSet<&str> = metadata
        .workspace_members
        .iter()
        .map(String::as_str)
        .collect();
    let mut groups: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for package in &metadata.packages {
        if !members.contains(package.id.as_str()) {
            continue;
        }
        for target in &package.targets {
            if target
                .kind
                .iter()
                .any(|kind| TARGET_KINDS.contains(&kind.as_str()))
            {
                groups
                    .entry(target.edition.clone())
                    .or_default()
                    .insert(target.src_path.clone());
            }
        }
    }
    Ok(groups
        .into_iter()
        .map(|(edition, paths)| (edition, paths.into_iter().collect()))
        .collect())
}

pub(crate) fn chunked_invocations(prefix: &[String], files: &[String]) -> Vec<Vec<String>> {
    let prefix_size: usize = prefix.iter().map(|arg| arg.len() + 1).sum();
    let mut invocations = Vec::new();
    let mut current = prefix.to_vec();
    let mut size = prefix_size;
    for path in files {
        let cost = path.len() + 1;
        if size + cost > ARGV_BUDGET && current.len() > prefix.len() {
            invocations.push(current);
            current = prefix.to_vec();
            size = prefix_size;
        }
        current.push(path.clone());
        size += cost;
    }
    if current.len() > prefix.len() {
        invocations.push(current);
    }
    invocations
}

pub fn run(args: &[String]) -> Result<(), u8> {
    let mut check = false;
    let mut verbose = false;
    let mut repository = PathBuf::from(".");
    let mut passthrough = Vec::new();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--check" => check = true,
            "--verbose" | "-v" => verbose = true,
            "--repository" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    eprintln!("fmt: --repository requires a path");
                    return Err(2);
                };
                repository = PathBuf::from(value);
            }
            "--" => {
                passthrough.extend_from_slice(&args[index + 1..]);
                break;
            }
            other => passthrough.push(other.to_owned()),
        }
        index += 1;
    }

    let groups = match workspace_targets(&repository) {
        Ok(groups) => groups,
        Err(error) => {
            eprintln!("fmt: {error}");
            return Err(1);
        }
    };

    let mut failures = 0usize;
    for (edition, files) in groups {
        let mut prefix = vec!["rustfmt".to_owned(), "--edition".to_owned(), edition];
        if check {
            prefix.push("--check".to_owned());
        }
        prefix.extend(passthrough.iter().cloned());
        for invocation in chunked_invocations(&prefix, &files) {
            if verbose {
                println!("{}", invocation.join(" "));
            }
            let status = Command::new(&invocation[0])
                .args(&invocation[1..])
                .current_dir(&repository)
                .status();
            match status {
                Ok(status) if status.success() => {}
                _ => failures += 1,
            }
        }
    }
    if failures > 0 {
        eprintln!("fmt: {failures} rustfmt invocation(s) failed");
        return Err(1);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_stay_within_windows_command_line_budget() {
        let prefix = vec![
            "rustfmt".to_owned(),
            "--edition".to_owned(),
            "2024".to_owned(),
        ];
        let files: Vec<String> = (0..1000)
            .map(|index| format!("omega-rust/psi/stage_{index:04}/src/lib.rs"))
            .collect();
        let chunks = chunked_invocations(&prefix, &files);
        assert!(chunks.len() > 1);
        for chunk in &chunks {
            let total: usize = chunk.iter().map(|arg| arg.len() + 1).sum();
            assert!(total <= ARGV_BUDGET);
        }
    }
}
