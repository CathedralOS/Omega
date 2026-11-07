//! Run bounded wrong-branch controls with the qualification's unchanged producer.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use serde_json::json;

use crate::{corpus, execution, producer::Producer};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Replacement {
    before: String,
    after: String,
    occurrences: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ControlledSource {
    path: String,
    good_sha256: String,
    weak_sha256: String,
    strong_sha256: String,
    good_to_weak: Vec<Replacement>,
    weak_to_strong: Vec<Replacement>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Plan {
    schema: u32,
    fixtures: [String; 3],
    files: [ControlledSource; 6],
}

struct Snapshot {
    relative: String,
    path: PathBuf,
    original: Vec<u8>,
    weak: Vec<u8>,
    strong: Vec<u8>,
    sha256: String,
    weak_sha256: String,
    strong_sha256: String,
    permissions: fs::Permissions,
    accessed: SystemTime,
    modified: SystemTime,
}

pub(super) struct Replay {
    root: PathBuf,
    baseline_revision: serde_json::Value,
    directory: PathBuf,
    fixtures: Vec<String>,
    files: Vec<Snapshot>,
    active: bool,
    restored: bool,
    restore_attempts: usize,
}

fn replaced(input: &[u8], replacements: &[Replacement]) -> Result<Vec<u8>, String> {
    let mut text = std::str::from_utf8(input)
        .map_err(|e| e.to_string())?
        .to_owned();
    if replacements.is_empty() {
        return Err("control phase has no source substitutions".to_owned());
    }
    for replacement in replacements {
        if replacement.before.is_empty()
            || replacement.occurrences == 0
            || replacement.occurrences > 6
            || text.matches(&replacement.before).count() != replacement.occurrences
        {
            return Err(
                "control substitution does not match its exact occurrence count".to_owned(),
            );
        }
        text = text.replace(&replacement.before, &replacement.after);
    }
    Ok(text.into_bytes())
}

fn nanos(time: SystemTime) -> Result<u128, String> {
    time.duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .map_err(|e| e.to_string())
}

fn mode(metadata: &fs::Metadata) -> serde_json::Value {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        json!({"unix_mode": metadata.mode()})
    }
    #[cfg(not(unix))]
    {
        json!({"readonly": metadata.permissions().readonly()})
    }
}

impl Replay {
    pub(super) fn prepare(
        root: &Path,
        plan_path: &Path,
        directory: &Path,
        runtime: &[String],
        target: &str,
    ) -> Result<Self, String> {
        let plan_bytes =
            fs::read(plan_path).map_err(|e| format!("read runtime control plan: {e}"))?;
        let plan: Plan = serde_json::from_slice(&plan_bytes)
            .map_err(|e| format!("runtime control plan: {e}"))?;
        if plan.schema != 1 {
            return Err("unknown runtime control plan schema".to_owned());
        }
        let fixtures: BTreeSet<_> = plan.fixtures.iter().cloned().collect();
        if fixtures.len() != 3 || !fixtures.iter().all(|fixture| runtime.contains(fixture)) {
            return Err("all three control fixtures must be distinct and included in the good runtime selection".to_owned());
        }
        for fixture in &fixtures {
            let source = root.join("tests/omega").join(fixture);
            if !fixture.starts_with("pass/")
                || super::expected_exit(&source, target, false)? != "0"
                || super::expected_exit(&source, target, true)? != "0"
            {
                return Err(
                    "wrong-branch controls require the unchanged authored exit 0".to_owned(),
                );
            }
        }
        fs::create_dir_all(directory).map_err(|e| e.to_string())?;
        fs::write(directory.join("plan.json"), &plan_bytes).map_err(|e| e.to_string())?;
        let canonical = fs::canonicalize(root).map_err(|e| e.to_string())?;
        let mut seen = BTreeSet::new();
        let mut files = Vec::new();
        let mut manifest = Vec::new();
        for item in plan.files {
            let relative = Path::new(&item.path);
            if !seen.insert(item.path.clone())
                || relative.file_name().is_none_or(|name| name != "main.omg")
                || relative
                    .components()
                    .any(|component| !matches!(component, Component::Normal(_)))
                || !(item.path.starts_with("samples/cli/facts/")
                    || item.path.starts_with("tests/omega/pass/"))
            {
                return Err(format!(
                    "control path is outside the declared existing main sources: {}",
                    item.path
                ));
            }
            let path = root.join(relative);
            let metadata = fs::symlink_metadata(&path)
                .map_err(|e| format!("control source {}: {e}", path.display()))?;
            if !metadata.file_type().is_file()
                || metadata.permissions().readonly()
                || fs::canonicalize(&path).map_err(|e| e.to_string())? != canonical.join(relative)
            {
                return Err(format!(
                    "control source must be a writable ordinary file without path aliases: {}",
                    item.path
                ));
            }
            let original = fs::read(&path).map_err(|e| e.to_string())?;
            let weak = replaced(&original, &item.good_to_weak)?;
            let strong = replaced(&weak, &item.weak_to_strong)?;
            for (phase, bytes, expected) in [
                ("good", &original, &item.good_sha256),
                ("weak", &weak, &item.weak_sha256),
                ("strong", &strong, &item.strong_sha256),
            ] {
                let retained = directory.join("sources").join(phase).join(relative);
                fs::create_dir_all(retained.parent().ok_or("source backup has no parent")?)
                    .map_err(|e| e.to_string())?;
                fs::write(&retained, bytes).map_err(|e| e.to_string())?;
                if execution::digest(&retained)? != *expected {
                    return Err(format!("{phase} source hash differs for {}", item.path));
                }
            }
            if execution::digest(&path)? != item.good_sha256 {
                return Err(format!(
                    "good source changed during prevalidation: {}",
                    item.path
                ));
            }
            manifest.push(json!({"path": item.path, "good_sha256": item.good_sha256, "weak_sha256": item.weak_sha256,
                "strong_sha256": item.strong_sha256, "mode": mode(&metadata), "accessed_unix_nanos": nanos(metadata.accessed().map_err(|e| e.to_string())?)?,
                "modified_unix_nanos": nanos(metadata.modified().map_err(|e| e.to_string())?)?}));
            files.push(Snapshot {
                relative: item.path,
                path,
                original,
                weak,
                strong,
                sha256: item.good_sha256,
                weak_sha256: item.weak_sha256,
                strong_sha256: item.strong_sha256,
                permissions: metadata.permissions(),
                accessed: metadata.accessed().map_err(|e| e.to_string())?,
                modified: metadata.modified().map_err(|e| e.to_string())?,
            });
        }
        let source_paths: BTreeSet<_> = files
            .iter()
            .filter(|file| file.relative.starts_with("tests/omega/"))
            .map(|file| file.relative.clone())
            .collect();
        let expected: BTreeSet<_> = fixtures
            .iter()
            .map(|fixture| format!("tests/omega/{fixture}/main.omg"))
            .collect();
        if source_paths != expected
            || files
                .iter()
                .filter(|file| file.relative.starts_with("samples/cli/facts/"))
                .count()
                != 3
        {
            return Err("control sources must contain exactly the three corpus entries and their three fact mirrors".to_owned());
        }
        for fixture in &expected {
            let entry = files
                .iter()
                .find(|file| &file.relative == fixture)
                .ok_or("control source missing")?;
            if files
                .iter()
                .filter(|file| {
                    file.relative.starts_with("samples/")
                        && file.original == entry.original
                        && file.weak == entry.weak
                        && file.strong == entry.strong
                })
                .count()
                != 1
            {
                return Err(format!(
                    "control mirror does not match all three source phases: {fixture}"
                ));
            }
        }
        execution::write_json(
            &directory.join("source-backup.json"),
            &json!({"source": crate::producer::source_identity(root)?, "plan_sha256": execution::digest(&directory.join("plan.json"))?, "files": manifest,
            "recovery": "Original bytes are under sources/good. Restore the six declared paths with their recorded permissions and accessed/modified times."}),
        )?;
        Ok(Self {
            root: root.to_path_buf(),
            baseline_revision: crate::producer::source_identity(root)?["revision"].clone(),
            directory: directory.to_path_buf(),
            fixtures: fixtures.into_iter().collect(),
            files,
            active: false,
            restored: false,
            restore_attempts: 0,
        })
    }

    fn source_manifest(&self, phase: &str, point: &str) -> Result<(), String> {
        let current = crate::producer::source_identity(&self.root)?;
        if current["revision"] != self.baseline_revision
            || current["tracked_changes"]
                .as_str()
                .is_none_or(|text| text.lines().any(|line| line.starts_with("??")))
        {
            return Err("source revision or untracked inputs changed during controls".to_owned());
        }
        let mut sources = Vec::new();
        for file in &self.files {
            let actual = execution::digest(&file.path)?;
            let expected = if phase == "strong" {
                &file.strong_sha256
            } else {
                &file.weak_sha256
            };
            sources.push(
                json!({"path": file.relative, "sha256": actual, "expected_sha256": expected}),
            );
            if actual != *expected {
                return Err(format!(
                    "{phase} source changed at {point}: {}",
                    file.relative
                ));
            }
        }
        let output = Command::new("git")
            .args(["diff", "--name-only", "-z", "HEAD"])
            .current_dir(&self.root)
            .stdin(Stdio::null())
            .output()
            .map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err("could not authenticate changed control paths".to_owned());
        }
        let changed: BTreeSet<_> = output
            .stdout
            .split(|byte| *byte == 0)
            .filter(|path| !path.is_empty())
            .map(|path| {
                std::str::from_utf8(path)
                    .map(str::to_owned)
                    .map_err(|e| e.to_string())
            })
            .collect::<Result<_, _>>()?;
        if changed
            != self
                .files
                .iter()
                .map(|file| file.relative.clone())
                .collect()
        {
            return Err("source changes exceed the exact six control inputs".to_owned());
        }
        execution::write_json(
            &self.directory.join(format!("{phase}-{point}-inputs.json")),
            &json!({"source": crate::producer::source_identity(&self.root)?, "files": sources}),
        )
    }

    pub(super) fn run(&mut self, producer: &Producer, target: &str) -> Result<(), String> {
        let result = (|| {
            for (phase, strong) in [("weak", false), ("strong", true)] {
                producer.verify()?;
                self.active = true;
                for file in &self.files {
                    fs::write(&file.path, if strong { &file.strong } else { &file.weak })
                        .map_err(|e| e.to_string())?;
                }
                self.source_manifest(phase, "before")?;
                let text = corpus::observe(
                    &self.root,
                    producer
                        .runner
                        .as_deref()
                        .ok_or("runtime controls need the prepared runner")?,
                    &self.fixtures.join(","),
                    "",
                    Some(target),
                    true,
                    false,
                    &self.directory.join(phase),
                )?;
                let rows = corpus::parse_records(&text)?;
                super::validate_roster(&self.fixtures, rows.iter().map(|row| row.fixture.clone()))?;
                execution::write_json(&self.directory.join(format!("{phase}-rows.json")), &rows)?;
                producer.verify()?;
                self.source_manifest(phase, "after")?;
                for row in &rows {
                    if strong {
                        if !strong_trap(row, target)
                            || super::validate_record(row, &self.root, target, true).is_ok()
                        {
                            return Err(format!(
                                "{} did not check/build and reach both intended Traps under its unchanged exit-0 oracle",
                                row.fixture
                            ));
                        }
                    } else {
                        super::validate_record(row, &self.root, target, true)?;
                    }
                }
            }
            Ok(())
        })();
        let restored = self.restore();
        match (result, restored) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(error), Ok(())) => Err(error),
            (Ok(()), Err(error)) => Err(error),
            (Err(error), Err(restore)) => Err(format!(
                "{error}; source restoration also failed: {restore}"
            )),
        }
    }

    fn restore(&mut self) -> Result<(), String> {
        if !self.active || self.restored {
            return Ok(());
        }
        self.restore_attempts += 1;
        let mut errors = Vec::new();
        for file in &self.files {
            let result = (|| {
                if file.path.exists() {
                    fs::set_permissions(&file.path, file.permissions.clone())
                        .map_err(|e| e.to_string())?;
                }
                fs::write(&file.path, &file.original).map_err(|e| e.to_string())?;
                if execution::digest(&file.path)? != file.sha256 {
                    return Err("restored source hash differs".to_owned());
                }
                fs::File::options()
                    .write(true)
                    .open(&file.path)
                    .map_err(|e| e.to_string())?
                    .set_times(
                        fs::FileTimes::new()
                            .set_accessed(file.accessed)
                            .set_modified(file.modified),
                    )
                    .map_err(|e| e.to_string())?;
                fs::set_permissions(&file.path, file.permissions.clone())
                    .map_err(|e| e.to_string())?;
                Ok(())
            })();
            if let Err(error) = result {
                errors.push(format!("{}: {error}", file.relative));
            }
        }
        execution::write_json(
            &self
                .directory
                .join(format!("restore-attempt-{}.json", self.restore_attempts)),
            &json!({"restored": errors.is_empty(), "errors": errors}),
        )?;
        self.restored = errors.is_empty();
        if self.restored {
            Ok(())
        } else {
            Err(format!(
                "{}; original source backup remains at {}",
                errors.join("; "),
                self.directory.display()
            ))
        }
    }
}

impl Drop for Replay {
    fn drop(&mut self) {
        if let Err(error) = self.restore() {
            eprintln!("runtime controls: {error}");
        }
    }
}

fn strong_trap(row: &corpus::CorpusRecord, target: &str) -> bool {
    if row.status != "built" || !row.diagnostics.is_empty() || row.expected_satisfied == Some(false)
    {
        return false;
    }
    let mut facts = std::collections::BTreeMap::new();
    for (key, value) in &row.facts {
        if facts.insert(key.as_str(), value.as_str()).is_some() {
            return false;
        }
    }
    let native = match target {
        "windows_x86_64" => facts.get("exit") == Some(&"-1073741795"),
        "linux_x86_64" => facts.get("exit") == Some(&"signal") && facts.get("signal") == Some(&"4"),
        "macos_arm64" | "linux_arm64" => {
            facts.get("exit") == Some(&"signal") && facts.get("signal") == Some(&"5")
        }
        _ => false,
    };
    native
        && facts.get("interpreter") == Some(&"trapped")
        && !facts.contains_key("interpreter-exit")
        && facts.get("interpreter-engine") != Some(&"checked-tree")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn substitutions_refuse_missing_or_extra_source_occurrences() {
        let edit = Replacement {
            before: "good".to_owned(),
            after: "weak".to_owned(),
            occurrences: 1,
        };
        assert_eq!(replaced(b"good", &[edit]).unwrap(), b"weak");
        let edit = || Replacement {
            before: "good".to_owned(),
            after: "weak".to_owned(),
            occurrences: 1,
        };
        assert!(replaced(b"other", &[edit()]).is_err());
        assert!(replaced(b"good good", &[edit()]).is_err());
    }
    #[test]
    fn ordinary_errors_and_unwinding_restore_original_source_and_times() {
        let directory = std::env::temp_dir().join(format!(
            "omega-control-restore-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("main.omg");
        fs::write(&path, b"original").unwrap();
        let metadata = fs::metadata(&path).unwrap();
        let sha256 = execution::digest(&path).unwrap();
        let make = || Replay {
            root: directory.clone(),
            baseline_revision: json!("test"),
            directory: directory.clone(),
            fixtures: Vec::new(),
            files: vec![Snapshot {
                relative: "main.omg".to_owned(),
                path: path.clone(),
                original: b"original".to_vec(),
                weak: b"weak".to_vec(),
                strong: b"strong".to_vec(),
                sha256: sha256.clone(),
                weak_sha256: String::new(),
                strong_sha256: String::new(),
                permissions: metadata.permissions(),
                accessed: metadata.accessed().unwrap(),
                modified: metadata.modified().unwrap(),
            }],
            active: true,
            restored: false,
            restore_attempts: 0,
        };
        let mut ordinary = make();
        fs::write(&path, b"weak").unwrap();
        ordinary.restore().unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().modified().unwrap(),
            metadata.modified().unwrap()
        );
        assert_eq!(fs::read(&path).unwrap(), b"original");
        drop(ordinary);
        fs::remove_file(directory.join("restore-attempt-1.json")).unwrap();
        let caught = std::panic::catch_unwind(|| {
            let _restore = make();
            fs::write(&path, b"strong").unwrap();
            panic!("controlled unwind");
        });
        assert!(caught.is_err());
        assert_eq!(fs::read(&path).unwrap(), b"original");
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn arbitrary_nonzero_or_interpreter_decline_cannot_satisfy_trap_control() {
        let row = |facts: &str| {
            corpus::parse_records(&format!("pass/a/b built 1ms {facts}\n"))
                .unwrap()
                .remove(0)
        };
        assert!(strong_trap(
            &row("exit:signal signal:4 interpreter:trapped"),
            "linux_x86_64"
        ));
        assert!(!strong_trap(
            &row("exit:signal signal:11 interpreter:trapped"),
            "linux_x86_64"
        ));
        assert!(!strong_trap(
            &row("exit:signal signal:4 interpreter:declined"),
            "linux_x86_64"
        ));
        assert!(!strong_trap(
            &row("exit:1 interpreter:trapped"),
            "linux_x86_64"
        ));
    }
}
