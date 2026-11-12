//! Prepare compiler executables and bind Cargo's effective options to their bytes.

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{Value, json};

use crate::execution;

pub struct Producer {
    pub runner: Option<PathBuf>,
    pub cli: Option<PathBuf>,
    pub executables: Vec<Value>,
}

pub fn host_target() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => "windows_x86_64",
        ("macos", "aarch64") => "macos_arm64",
        ("linux", "aarch64") => "linux_arm64",
        ("linux", "x86_64") => "linux_x86_64",
        _ => "unsupported",
    }
}

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("git {}: {e}", args.join(" ")))?;
    if !output.status.success() {
        return Err(format!("git {} failed", args.join(" ")));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

pub fn source_identity(root: &Path) -> Result<Value, String> {
    Ok(json!({"revision": git(root, &["rev-parse", "HEAD"])?,
        "tree": git(root, &["rev-parse", "HEAD^{tree}"])?,
        "tracked_changes": git(root, &["status", "--porcelain", "--untracked-files=normal"])?}))
}

fn optimized(profile: &Value, flags: &Value) -> Result<(), String> {
    let level = profile["opt_level"]
        .as_str()
        .ok_or("missing Cargo opt_level")?;
    if !matches!(level, "1" | "2" | "3" | "s" | "z") {
        return Err(format!("unoptimized Cargo profile: {level}"));
    }
    let flags = flags.as_array().ok_or("missing fingerprint rustflags")?;
    let strings = flags
        .iter()
        .map(|v| v.as_str().ok_or("invalid rustflag"))
        .collect::<Result<Vec<_>, _>>()?;
    for (index, flag) in strings.iter().enumerate() {
        let option = if matches!(*flag, "-C" | "--codegen") {
            strings.get(index + 1).copied().unwrap_or("")
        } else {
            flag.strip_prefix("-C")
                .or_else(|| flag.strip_prefix("--codegen="))
                .unwrap_or(flag)
        };
        if flag.starts_with('@') {
            return Err("response-file rustflags cannot be authenticated".to_owned());
        }
        if let Some(level) = option.strip_prefix("opt-level=")
            && !matches!(level, "1" | "2" | "3" | "s" | "z")
        {
            return Err(format!("unoptimized effective rustflag: {option}"));
        }
    }
    Ok(())
}

fn unit_fingerprints(directory: &Path, name: &str) -> Result<Vec<PathBuf>, String> {
    let mut paths = Vec::new();
    let suffix = format!("-{name}.json");
    for entry in
        fs::read_dir(directory).map_err(|e| format!("read {}: {e}", directory.display()))?
    {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.ends_with(&suffix))
        {
            paths.push(path);
        }
    }
    Ok(paths)
}

fn artifact_fingerprints(
    file: &Path,
    name: &str,
    digest: &str,
    binary: bool,
) -> Result<Vec<PathBuf>, String> {
    let directory = file.parent().ok_or("artifact has no parent")?;
    // mbx keeps each exact unit's outputs beside its own fingerprint folder.
    if directory.file_name().is_some_and(|value| value == "out") {
        return unit_fingerprints(
            &directory
                .parent()
                .ok_or("unit output has no parent")?
                .join("fingerprint"),
            name,
        );
    }
    let profile = if directory.file_name().is_some_and(|value| value == "deps") {
        directory.parent().ok_or("deps has no parent")?
    } else {
        directory
    };
    let mut outputs = std::collections::BTreeMap::new();
    let cargo = profile.join(".fingerprint");
    if cargo.is_dir() {
        for entry in fs::read_dir(&cargo).map_err(|e| e.to_string())? {
            let unit = entry.map_err(|e| e.to_string())?.path();
            let Some((_, hash)) = unit
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n.rsplit_once('-'))
            else {
                continue;
            };
            for candidate in [
                format!("lib{name}-{hash}.rlib"),
                format!("{name}-{hash}{}", std::env::consts::EXE_SUFFIX),
            ] {
                let output = profile.join("deps").join(candidate);
                if output.is_file() && execution::digest(&output)? == digest {
                    outputs.insert(output, unit_fingerprints(&unit, name)?);
                }
            }
        }
    }
    // An unhashed CLI is a copy of one exact internal output. Identify that
    // output by bytes in either layout; multiple matching old units refuse.
    if binary && directory.file_name().is_none_or(|value| value != "deps") {
        let builds = profile.join("build");
        if builds.is_dir() {
            for package in fs::read_dir(builds).map_err(|e| e.to_string())? {
                let package = package.map_err(|e| e.to_string())?.path();
                if !package.is_dir() {
                    continue;
                }
                for unit in fs::read_dir(package).map_err(|e| e.to_string())? {
                    let unit = unit.map_err(|e| e.to_string())?.path();
                    let output = unit
                        .join("out")
                        .join(file.file_name().ok_or("binary has no name")?);
                    if output.is_file() && execution::digest(&output)? == digest {
                        outputs.insert(output, unit_fingerprints(&unit.join("fingerprint"), name)?);
                    }
                }
            }
        }
    }
    if outputs.len() != 1 {
        return Err(format!(
            "{} does not identify one exact compiler output",
            file.display()
        ));
    }
    Ok(outputs
        .into_values()
        .next()
        .ok_or("matching output disappeared")?)
}

fn artifact_receipt(artifact: &Value) -> Result<Value, String> {
    let executable = artifact["executable"].as_str().map(PathBuf::from);
    let files = artifact["filenames"]
        .as_array()
        .ok_or("artifact has no filenames")?;
    let file = executable
        .clone()
        .or_else(|| {
            files
                .iter()
                .filter_map(Value::as_str)
                .find(|p| p.ends_with(".rlib"))
                .map(PathBuf::from)
        })
        .ok_or("artifact has no compiler image")?;
    if !files
        .iter()
        .filter_map(Value::as_str)
        .any(|name| Path::new(name) == file.as_path())
    {
        return Err("compiler executable is absent from its Cargo artifact filenames".to_owned());
    }
    let digest = execution::digest(&file)?;
    let name = artifact["target"]["name"]
        .as_str()
        .ok_or("artifact has no target name")?
        .replace('-', "_");
    let paths = artifact_fingerprints(&file, &name, &digest, executable.is_some())?;
    if paths.len() != 1 {
        return Err(format!(
            "{} does not identify one exact compiler fingerprint",
            file.display()
        ));
    }
    let path = &paths[0];
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let fingerprint: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    optimized(&artifact["profile"], &fingerprint["rustflags"])?;
    let contents = String::from_utf8(bytes).map_err(|e| e.to_string())?;
    Ok(
        json!({"path": file, "sha256": digest, "cargo_artifact": artifact, "fingerprints": [{"path": path,
        "sha256": execution::digest(path)?, "contents": contents, "rustflags": fingerprint["rustflags"]}]}),
    )
}

fn build(
    root: &Path,
    directory: &Path,
    name: &str,
    arguments: &[&str],
) -> Result<Vec<Value>, String> {
    let mut command = Command::new("mbx");
    command
        .args(arguments)
        .current_dir(root)
        .stdin(Stdio::null())
        .env("CARGO_PROFILE_DEV_OPT_LEVEL", "1")
        .env("CARGO_PROFILE_TEST_OPT_LEVEL", "1")
        .env("CARGO_UNSTABLE_CHECKSUM_FRESHNESS", "true")
        .env("CARGO_BUILD_FINGERPRINT", "content");
    let status = execution::run(&mut command, directory, name)?;
    if !status.success() {
        return Err(format!("{name} failed; see retained build stderr"));
    }
    let file =
        fs::File::open(directory.join(format!("{name}.stdout"))).map_err(|e| e.to_string())?;
    let mut artifacts = Vec::new();
    for line in BufReader::new(file).lines() {
        let line = line.map_err(|e| e.to_string())?;
        if let Ok(value) = serde_json::from_str::<Value>(&line)
            && value["reason"] == "compiler-artifact"
            && (value["executable"].is_string() || value["target"]["name"] == "compiler")
        {
            artifacts.push(value);
        }
    }
    Ok(artifacts)
}

pub fn prepare(
    root: &Path,
    directory: &Path,
    need_runner: bool,
    need_cli: bool,
) -> Result<Producer, String> {
    fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    let before = source_identity(root)?;
    execution::write_json(&directory.join("source-before.json"), &before)?;
    for (name, args) in [("rustc", vec!["-vV"]), ("mbx", vec!["--version"])] {
        let status = execution::run(
            Command::new(name)
                .args(args)
                .current_dir(root)
                .stdin(Stdio::null()),
            directory,
            name,
        )?;
        if !status.success() {
            return Err(format!("{name} identity command failed"));
        }
    }
    let environment = [
        "CARGO_BUILD_JOBS",
        "CARGO_TARGET_DIR",
        "CARGO_INCREMENTAL",
        "RUSTFLAGS",
        "CARGO_ENCODED_RUSTFLAGS",
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
        "RUST_MIN_STACK",
        "OMEGA_CORPUS_JOBS",
        "OMEGA_CORPUS_FIXTURE_SECS",
        "OMEGA_CORPUS_EXECUTE_SECS",
    ]
    .into_iter()
    .map(|key| (key, std::env::var(key).ok()))
    .collect::<std::collections::BTreeMap<_, _>>();
    execution::write_json(
        &directory.join("build-inputs.json"),
        &json!({"host": host_target(), "environment": environment,
        "forced_profile": {"dev": "1", "test": "1"}, "freshness": "checksum/content"}),
    )?;
    let mut producer = Producer {
        runner: None,
        cli: None,
        executables: Vec::new(),
    };
    for (needed, name, args) in [
        (
            need_runner,
            "runner-build",
            vec![
                "test",
                "--locked",
                "--no-run",
                "--message-format=json",
                "-p",
                "compiler",
                "--test",
                "corpus_runner",
            ],
        ),
        (
            need_cli,
            "cli-build",
            vec![
                "build",
                "--locked",
                "--message-format=json",
                "-p",
                "omega",
                "--bin",
                "omega",
            ],
        ),
    ] {
        if !needed {
            continue;
        }
        let artifacts = build(root, directory, name, &args)?;
        let target = if name == "runner-build" {
            "corpus_runner"
        } else {
            "omega"
        };
        let artifact = artifacts
            .iter()
            .find(|v| v["target"]["name"] == target && v["executable"].is_string())
            .ok_or_else(|| format!("{name} produced no {target}"))?;
        let receipt = artifact_receipt(artifact)?;
        let path = PathBuf::from(
            artifact["executable"]
                .as_str()
                .ok_or("missing executable")?,
        );
        if name == "runner-build" {
            producer.runner = Some(path);
        } else {
            producer.cli = Some(path);
        }
        producer.executables.push(receipt);
        let compiler = artifacts
            .iter()
            .find(|v| {
                v["target"]["name"] == "compiler"
                    && v["filenames"].as_array().is_some_and(|v| {
                        v.iter()
                            .filter_map(Value::as_str)
                            .any(|p| p.ends_with(".rlib"))
                    })
            })
            .ok_or("build lacks compiler library profile")?;
        producer.executables.push(artifact_receipt(compiler)?);
    }
    let after = source_identity(root)?;
    execution::write_json(&directory.join("source-after.json"), &after)?;
    if before != after {
        return Err("source changed during producer preparation".to_owned());
    }
    execution::write_json(
        &directory.join("producer.json"),
        &json!({"source": before, "host": host_target(), "executables": producer.executables}),
    )?;
    producer.verify()?;
    Ok(producer)
}

impl Producer {
    pub fn verify(&self) -> Result<(), String> {
        for record in &self.executables {
            let path = Path::new(record["path"].as_str().ok_or("producer path missing")?);
            if execution::digest(path)? != record["sha256"] {
                return Err(format!("producer changed: {}", path.display()));
            }
        }
        Ok(())
    }
}

/// The runner's file name in a directory written by [`export_runner`].
pub const EXPORTED_RUNNER: &str = "corpus_runner";
/// The receipt `prepare` wrote for that runner, copied beside it unchanged.
pub const EXPORTED_RECEIPT: &str = "producer.json";

fn runner_record(receipt: &Value) -> Result<&Value, String> {
    receipt["executables"]
        .as_array()
        .and_then(|records| {
            records.iter().find(|record| {
                record["cargo_artifact"]["target"]["name"] == "corpus_runner"
                    && record["cargo_artifact"]["executable"].is_string()
            })
        })
        .ok_or_else(|| "receipt records no corpus runner".to_owned())
}

/// Copy the runner `prepare` just built, and the receipt that names its
/// source commit and bytes, into `out` so another job can run it without
/// compiling. `receipts` is the directory `prepare` wrote into.
pub fn export_runner(producer: &Producer, receipts: &Path, out: &Path) -> Result<(), String> {
    let runner = producer
        .runner
        .as_deref()
        .ok_or("producer built no runner")?;
    fs::create_dir_all(out).map_err(|e| format!("create {}: {e}", out.display()))?;
    let copy = out.join(EXPORTED_RUNNER);
    // `fs::copy` keeps the executable bit.
    fs::copy(runner, &copy).map_err(|e| format!("copy {}: {e}", runner.display()))?;
    for name in [EXPORTED_RECEIPT, "build-inputs.json"] {
        fs::copy(receipts.join(name), out.join(name)).map_err(|e| format!("copy {name}: {e}"))?;
    }
    let receipt = read_receipt(out)?;
    if execution::digest(&copy)? != runner_record(&receipt)?["sha256"] {
        return Err("exported runner differs from the one its receipt records".to_owned());
    }
    producer.verify()
}

fn read_receipt(directory: &Path) -> Result<Value, String> {
    let path = directory.join(EXPORTED_RECEIPT);
    let bytes = fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))
}

/// The commit an exported runner's receipt says it was built from.
pub fn prebuilt_revision(directory: &Path) -> Result<String, String> {
    read_receipt(directory)?["source"]["revision"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| "receipt names no source revision".to_owned())
}

/// Whether a runner built elsewhere may judge this checkout.
///
/// `receipt` is the `producer.json` its build wrote, `checkout` is
/// [`source_identity`] of the tree whose fixtures it would run, and
/// `runner_digest` is the digest of the runner file actually present. The
/// runner is accepted only when it was built on this host kind from a clean
/// tree at the same commit and tree as the checkout, the checkout is clean
/// too, and the file is byte-for-byte the one the build recorded. This is the
/// identity `prepare` records and nothing more; it trusts the receipt's
/// origin, so use it only for a runner handed over inside one workflow run.
pub fn check_prebuilt(
    receipt: &Value,
    checkout: &Value,
    runner_digest: &str,
) -> Result<(), String> {
    if receipt["host"] != host_target() {
        return Err(format!(
            "runner was built for {}, not {}",
            receipt["host"],
            host_target()
        ));
    }
    let built = &receipt["source"];
    for key in ["revision", "tree"] {
        let (Some(built), Some(here)) = (built[key].as_str(), checkout[key].as_str()) else {
            return Err(format!("runner receipt or checkout names no source {key}"));
        };
        if built != here {
            return Err(format!(
                "runner was built from {key} {built}, but the checkout is at {here}"
            ));
        }
    }
    if built["tracked_changes"] != "" {
        return Err("runner was built from a tree with uncommitted changes".to_owned());
    }
    if checkout["tracked_changes"] != "" {
        return Err(
            "the checkout has uncommitted changes, so a runner built elsewhere cannot match it"
                .to_owned(),
        );
    }
    if runner_record(receipt)?["sha256"] != runner_digest {
        return Err("runner bytes differ from the ones its receipt records".to_owned());
    }
    Ok(())
}

/// Load a runner [`export_runner`] wrote, instead of building one, after
/// [`check_prebuilt`] accepts it for the checkout at `root`. The receipt is
/// kept in `evidence`, and the returned producer's `verify` re-reads the
/// runner's bytes exactly as it does for a runner built here.
pub fn load_prebuilt(root: &Path, directory: &Path, evidence: &Path) -> Result<Producer, String> {
    let directory = fs::canonicalize(directory)
        .map_err(|e| format!("prebuilt runner {}: {e}", directory.display()))?;
    let receipt = read_receipt(&directory)?;
    let runner = directory.join(EXPORTED_RUNNER);
    let digest = execution::digest(&runner)?;
    let checkout = source_identity(root)?;
    check_prebuilt(&receipt, &checkout, &digest)
        .map_err(|e| format!("prebuilt runner {}: {e}", directory.display()))?;
    fs::create_dir_all(evidence).map_err(|e| e.to_string())?;
    execution::write_json(&evidence.join("source-before.json"), &checkout)?;
    execution::write_json(
        &evidence.join("producer.json"),
        &json!({"prebuilt": directory, "receipt": receipt}),
    )?;
    Ok(Producer {
        runner: Some(runner.clone()),
        cli: None,
        executables: vec![json!({"path": runner, "sha256": digest})],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prebuilt_runner_is_accepted_only_for_its_own_clean_commit_and_bytes() {
        let source = |revision: &str, tree: &str, changes: &str| json!({"revision": revision, "tree": tree, "tracked_changes": changes});
        let receipt = |source: Value| {
            json!({"source": source, "host": host_target(), "executables": [
            {"path": "/t/libcompiler.rlib", "sha256": "lib", "cargo_artifact": {"target": {"name": "compiler"}, "executable": null}},
            {"path": "/t/corpus_runner-1", "sha256": "abc", "cargo_artifact": {"target": {"name": "corpus_runner"}, "executable": "/t/corpus_runner-1"}}]})
        };
        let built = receipt(source("r1", "t1", ""));
        let here = source("r1", "t1", "");
        assert!(check_prebuilt(&built, &here, "abc").is_ok());
        // Another commit, another tree, or other bytes: refused.
        assert!(check_prebuilt(&built, &source("r2", "t1", ""), "abc").is_err());
        assert!(check_prebuilt(&built, &source("r1", "t2", ""), "abc").is_err());
        assert!(check_prebuilt(&built, &here, "lib").is_err());
        // A dirty tree on either side cannot be matched by commit.
        assert!(check_prebuilt(&built, &source("r1", "t1", " M x.rs"), "abc").is_err());
        assert!(check_prebuilt(&receipt(source("r1", "t1", "?? x")), &here, "abc").is_err());
        // A receipt for another host, with no runner, or with no identity.
        let mut foreign = built.clone();
        foreign["host"] = json!("elsewhere");
        assert!(check_prebuilt(&foreign, &here, "abc").is_err());
        let mut no_runner = built.clone();
        no_runner["executables"] = json!([]);
        assert!(check_prebuilt(&no_runner, &here, "abc").is_err());
        assert!(check_prebuilt(&json!({"host": host_target()}), &json!({}), "abc").is_err());
    }

    #[test]
    fn exported_runner_keeps_its_mode_and_missing_or_changed_files_refuse_to_load() {
        let directory = std::env::temp_dir().join(format!(
            "omega-prebuilt-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let receipts = directory.join("producer");
        let out = directory.join("runner");
        fs::create_dir_all(&receipts).unwrap();
        let built = directory.join("corpus_runner-abcd");
        fs::write(&built, b"runner image").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&built, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let digest = execution::digest(&built).unwrap();
        let record = json!({"path": built, "sha256": digest,
            "cargo_artifact": {"target": {"name": "corpus_runner"}, "executable": built}});
        execution::write_json(
            &receipts.join(EXPORTED_RECEIPT),
            &json!({"source": {"revision": "r1", "tree": "t1", "tracked_changes": ""},
                "host": host_target(), "executables": [record.clone()]}),
        )
        .unwrap();
        fs::write(receipts.join("build-inputs.json"), b"{}").unwrap();
        let producer = Producer {
            runner: Some(built.clone()),
            cli: None,
            executables: vec![record],
        };
        export_runner(&producer, &receipts, &out).unwrap();
        assert_eq!(prebuilt_revision(&out).unwrap(), "r1");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(out.join(EXPORTED_RUNNER))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o111, 0o111);
        }
        // This checkout is not at revision `r1`, so the load refuses; so does
        // a directory with no receipt or no runner.
        let root = crate::repo_root();
        let evidence = directory.join("evidence");
        let refusal = load_prebuilt(&root, &out, &evidence).err().unwrap();
        assert!(refusal.contains("revision"), "{refusal}");
        assert!(load_prebuilt(&root, &directory.join("absent"), &evidence).is_err());
        fs::remove_file(out.join(EXPORTED_RUNNER)).unwrap();
        assert!(load_prebuilt(&root, &out, &evidence).is_err());
        fs::remove_file(out.join(EXPORTED_RECEIPT)).unwrap();
        assert!(prebuilt_revision(&out).is_err());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn receipt_binds_matching_artifact_flags_and_detects_changed_bytes() {
        let directory = std::env::temp_dir().join(format!(
            "omega-producer-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let profile = directory.join("debug");
        let fingerprint = profile.join(".fingerprint/omega-abcd/bin-omega.json");
        let dependency = profile
            .join("deps")
            .join(format!("omega-abcd{}", std::env::consts::EXE_SUFFIX));
        let executable = profile.join(format!("omega{}", std::env::consts::EXE_SUFFIX));
        fs::create_dir_all(fingerprint.parent().unwrap()).unwrap();
        fs::create_dir_all(dependency.parent().unwrap()).unwrap();
        fs::write(&dependency, b"original image").unwrap();
        fs::write(&executable, b"original image").unwrap();
        fs::write(&fingerprint, br#"{"rustflags":["--codegen=opt-level=0"]}"#).unwrap();
        let artifact = json!({"executable": executable, "filenames": [executable], "target": {"name": "omega"}, "profile": {"opt_level": "1"}});
        assert!(artifact_receipt(&artifact).is_err());
        fs::write(
            &fingerprint,
            br#"{"rustflags":["-C","target-cpu=x86-64-v3"]}"#,
        )
        .unwrap();
        let record = artifact_receipt(&artifact).unwrap();
        let producer = Producer {
            runner: None,
            cli: Some(executable.clone()),
            executables: vec![record],
        };
        assert!(producer.verify().is_ok());
        fs::write(&executable, b"replacement image").unwrap();
        assert!(producer.verify().is_err());
        assert!(artifact_receipt(&artifact).is_err());
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn authenticates_mbx_units_and_refuses_ambiguous_unhashed_outputs() {
        let root = std::env::temp_dir().join(format!(
            "omega-mbx-profile-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let profile = root.join("debug");
        let unit = profile.join("build/omega/abcd");
        fs::create_dir_all(unit.join("out")).unwrap();
        fs::create_dir_all(unit.join("fingerprint")).unwrap();
        let executable = profile.join(format!("omega{}", std::env::consts::EXE_SUFFIX));
        let internal = unit.join("out").join(executable.file_name().unwrap());
        fs::write(&executable, b"same compiler bytes").unwrap();
        fs::write(&internal, b"same compiler bytes").unwrap();
        fs::write(
            unit.join("fingerprint/bin-omega.json"),
            br#"{"rustflags":[]}"#,
        )
        .unwrap();
        let artifact = json!({"executable": executable, "filenames": [executable], "target": {"name": "omega"}, "profile": {"opt_level": "1"}});
        assert!(artifact_receipt(&artifact).is_ok());
        let direct = json!({"executable": internal, "filenames": [internal], "target": {"name": "omega"}, "profile": {"opt_level": "1"}});
        assert!(artifact_receipt(&direct).is_ok());
        let library = unit.join("out/libcompiler-abcd.rlib");
        fs::write(&library, b"library image").unwrap();
        fs::write(
            unit.join("fingerprint/lib-compiler.json"),
            br#"{"rustflags":[]}"#,
        )
        .unwrap();
        assert!(artifact_receipt(&json!({"executable": null, "filenames": [library], "target": {"name": "compiler"}, "profile": {"opt_level": "1"}})).is_ok());
        let other = profile.join("build/omega/another");
        fs::create_dir_all(other.join("out")).unwrap();
        fs::create_dir_all(other.join("fingerprint")).unwrap();
        fs::copy(
            &internal,
            other.join("out").join(executable.file_name().unwrap()),
        )
        .unwrap();
        fs::copy(
            unit.join("fingerprint/bin-omega.json"),
            other.join("fingerprint/bin-omega.json"),
        )
        .unwrap();
        assert!(artifact_receipt(&artifact).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn effective_flags_cannot_override_optimized_profile() {
        let profile = json!({"opt_level": "1"});
        assert!(optimized(&profile, &json!(["-C", "target-cpu=x86-64-v3"])).is_ok());
        assert!(optimized(&profile, &json!(["-C", "opt-level=0"])).is_err());
        assert!(optimized(&profile, &json!(["-Copt-level=0"])).is_err());
        assert!(optimized(&profile, &json!(["--codegen", "opt-level=0"])).is_err());
        assert!(optimized(&profile, &json!(["--codegen=opt-level=0"])).is_err());
        assert!(optimized(&profile, &json!(["@hidden-options"])).is_err());
        assert!(optimized(&json!({"opt_level": "0"}), &json!([])).is_err());
    }
}
