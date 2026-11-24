//! Check the lines introduced since a Git base, including local edits to those lines.
use super::{ARGV_BUDGET, Metadata};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .map_err(|error| format!("cannot run git: {error}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
    }
    String::from_utf8(output.stdout).map_err(|error| format!("cannot read Git output: {error}"))
}

fn changed_ranges(diff: &str) -> Result<Vec<[usize; 2]>, String> {
    diff.lines()
        .filter(|line| line.starts_with("@@ "))
        .map(|line| {
            let added = line
                .split_ascii_whitespace()
                .nth(2)
                .and_then(|part| part.strip_prefix('+'))
                .ok_or_else(|| format!("invalid Git hunk header: {line}"))?;
            let (start, count) = added.split_once(',').unwrap_or((added, "1"));
            let start = start
                .parse::<usize>()
                .map_err(|_| format!("invalid Git line number: {line}"))?;
            let count = count
                .parse::<usize>()
                .map_err(|_| format!("invalid Git line count: {line}"))?;
            // A deletion can leave two previously separated lines next to one another.
            Ok(if count == 0 {
                [start.max(1), start + 1]
            } else {
                [start, start + count - 1]
            })
        })
        .collect()
}

#[derive(Serialize)]
struct FileLines<'a> {
    file: &'a str,
    range: [usize; 2],
}

pub(super) fn invocations(
    metadata: &Metadata,
    base: &str,
    passthrough: &[String],
) -> Result<Vec<Vec<String>>, String> {
    let root = Path::new(&metadata.workspace_root);
    let commit = git(
        root,
        &[
            "rev-parse",
            "--verify",
            "--end-of-options",
            &format!("{base}^{{commit}}"),
        ],
    )?;
    let ancestor = git(root, &["merge-base", commit.trim(), "HEAD"])?;
    let ancestor = ancestor.trim();
    let tracked = git(
        root,
        &[
            "diff",
            "--name-only",
            "-z",
            "--diff-filter=ACMT",
            "--no-renames",
            ancestor,
            "--",
            "*.rs",
        ],
    )?;
    let untracked = git(
        root,
        &[
            "ls-files",
            "--others",
            "--exclude-standard",
            "-z",
            "--",
            "*.rs",
        ],
    )?;
    let mut paths = BTreeMap::new();
    paths.extend(tracked.split_terminator('\0').map(|path| (path, true)));
    paths.extend(untracked.split_terminator('\0').map(|path| (path, false)));
    let mut invocations = Vec::new();
    for (relative, tracked) in paths {
        let path = root.join(relative);
        let package = metadata
            .packages
            .iter()
            .filter(|package| {
                metadata.workspace_members.contains(&package.id)
                    && Path::new(&package.manifest_path)
                        .parent()
                        .is_some_and(|dir| path.starts_with(dir))
            })
            .max_by_key(|package| package.manifest_path.len());
        let Some(package) = package else { continue };
        if !std::fs::symlink_metadata(&path)
            .map_err(|error| format!("{relative}: {error}"))?
            .is_file()
        {
            continue;
        }
        let ranges = if tracked {
            let diff = git(
                root,
                &[
                    "--literal-pathspecs",
                    "diff",
                    "--no-ext-diff",
                    "--no-textconv",
                    "--no-renames",
                    "--color=never",
                    "--unified=0",
                    "--inter-hunk-context=0",
                    ancestor,
                    "--",
                    relative,
                ],
            )?;
            changed_ranges(&diff)?
        } else {
            let contents =
                std::fs::read_to_string(&path).map_err(|error| format!("{relative}: {error}"))?;
            vec![[1, contents.lines().count().max(1)]]
        };
        // A target may select a different edition from its package. Use the
        // closest target roots, retaining both editions for shared modules.
        let mut editions = BTreeSet::new();
        let mut nearest = None;
        for target in &package.targets {
            let source = Path::new(&target.src_path);
            let Some(dir) = source.parent().filter(|dir| path.starts_with(dir)) else {
                continue;
            };
            let depth = (source == path, dir.components().count());
            if nearest.is_none_or(|previous| depth > previous) {
                nearest = Some(depth);
                editions.clear();
            }
            if nearest == Some(depth) {
                editions.insert(target.edition.as_str());
            }
        }
        if editions.is_empty() {
            editions.insert(package.edition.as_str());
        }
        let filename = path
            .to_str()
            .ok_or_else(|| format!("non-UTF-8 Rust path: {relative}"))?;
        // Windows limits the size of the complete command. Each serialized
        // range repeats the file name and two line numbers.
        let row_bytes = serde_json::to_string(filename).unwrap().len() + 68;
        let rows_per_call = (ARGV_BUDGET / 2 / row_bytes).max(1);
        for batch in ranges.chunks(rows_per_call) {
            let lines = batch
                .iter()
                .map(|range| FileLines {
                    file: filename,
                    range: *range,
                })
                .collect::<Vec<_>>();
            let json = serde_json::to_string(&lines).map_err(|error| error.to_string())?;
            for edition in &editions {
                let mut args = vec![
                    "rustfmt".into(),
                    "--edition".into(),
                    (*edition).into(),
                    "--check".into(),
                ];
                args.extend_from_slice(passthrough);
                args.extend([
                    "--unstable-features".into(),
                    "--config".into(),
                    "skip_children=true".into(),
                    "--file-lines".into(),
                    json.clone(),
                    filename.into(),
                ]);
                invocations.push(args);
            }
        }
    }
    Ok(invocations)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inherited_drift_is_separate_from_a_changed_line_or_new_file() {
        let root = std::env::temp_dir().join(format!("omega-fmt-changes-{}", std::process::id()));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("Cargo.toml"),
            "[package]\nname='format-fixture'\nversion='0.1.0'\nedition='2024'\n",
        )
        .unwrap();
        let path = root.join("src/lib.rs");
        let old = "pub fn old()->usize{1}\n\npub fn current() -> usize {\n    2\n}\n";
        std::fs::write(&path, old).unwrap();
        git(&root, &["init", "-q"]).unwrap();
        git(&root, &["add", "src/lib.rs", "Cargo.toml"]).unwrap();
        git(
            &root,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.invalid",
                "commit",
                "-qm",
                "baseline",
                "--no-gpg-sign",
            ],
        )
        .unwrap();
        let args = [
            "--check",
            "--base",
            "HEAD",
            "--repository",
            root.to_str().unwrap(),
        ]
        .map(String::from);
        assert!(super::super::run(&args).is_ok());
        std::fs::write(&path, old.replace("    2", "    3")).unwrap();
        assert!(super::super::run(&args).is_ok());
        std::fs::write(&path, old.replace("    2", " 3")).unwrap();
        assert!(super::super::run(&args).is_err());
        std::fs::write(&path, old).unwrap();
        let added = root.join("src/new file.rs");
        std::fs::write(&added, "pub fn added( ) {}\n").unwrap();
        assert!(super::super::run(&args).is_err());
        std::fs::write(&added, "pub fn added() {}\n").unwrap();
        assert!(super::super::run(&args).is_ok());
        std::fs::remove_dir_all(root).unwrap();
    }
}
