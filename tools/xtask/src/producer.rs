//! Prepare compiler executables and bind Cargo's effective options to their bytes.

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{Value, json};

use crate::execution;

pub struct Producer {
    pub cli: PathBuf,
    pub executables: Vec<Value>,
}

pub fn host_target() -> &'static str {
    hosted_profile(std::env::consts::OS, std::env::consts::ARCH)
}

fn hosted_profile(os: &str, arch: &str) -> &'static str {
    match (os, arch) {
        ("windows", "x86_64") => "windows_x86_64",
        ("windows", "aarch64") => "windows_arm64",
        ("macos", "aarch64") => "macos_arm64",
        ("macos", "x86_64") => "macos_x86_64",
        ("linux", "aarch64") => "linux_arm64",
        ("linux", "x86_64") => "linux_x86_64",
        _ => "unsupported",
    }
}

fn git_bytes(root: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("git {}: {e}", args.join(" ")))?;
    if !output.status.success() {
        return Err(format!("git {} failed", args.join(" ")));
    }
    Ok(output.stdout)
}

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    Ok(String::from_utf8_lossy(&git_bytes(root, args)?)
        .trim()
        .to_owned())
}

fn dirty_inputs_digest(root: &Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};

    // Porcelain v2 carries the index and HEAD object IDs, modes and raw paths.
    // File bytes distinguish repeated working-tree edits with identical status.
    let entries = git_bytes(
        root,
        &[
            "status",
            "--porcelain=v2",
            "-z",
            "--no-renames",
            "--untracked-files=all",
        ],
    )?;
    let mut digest = Sha256::new();
    for entry in entries.split(|&b| b == 0).filter(|entry| !entry.is_empty()) {
        let fields = match entry[0] {
            b'1' => 9,
            b'u' => 11,
            b'?' => 2,
            other => return Err(format!("unrecognized Git status entry: {other}")),
        };
        let name = entry
            .splitn(fields, |&b| b == b' ')
            .nth(fields - 1)
            .ok_or_else(|| "incomplete Git status entry".to_owned())?;
        #[cfg(unix)]
        let name = {
            use std::os::unix::ffi::OsStrExt;
            std::ffi::OsStr::from_bytes(name)
        };
        #[cfg(not(unix))]
        let name = std::str::from_utf8(name).map_err(|e| format!("invalid Git path: {e}"))?;
        let path = root.join(name);
        digest.update(entry);
        digest.update([0]);
        let metadata = match fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                digest.update(b"deleted");
                continue;
            }
            Err(error) => return Err(format!("{}: {error}", path.display())),
        };
        if metadata.is_file() {
            let mut file = fs::File::open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            let mut content = Sha256::new();
            std::io::copy(&mut file, &mut content)
                .map_err(|e| format!("{}: {e}", path.display()))?;
            digest.update(b"file");
            digest.update(content.finalize());
        } else if metadata.is_symlink() {
            let target = fs::read_link(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            digest.update(b"link");
            digest.update(Sha256::digest(target.as_os_str().as_encoded_bytes()));
        } else if metadata.is_dir() && git(&path, &["rev-parse", "--show-prefix"])?.is_empty() {
            // Git reports submodules and untracked nested repositories as one directory.
            // An unborn nested repository has no revision, but may already have source files.
            let revision = git(&path, &["rev-parse", "--revs-only", "HEAD"])?;
            digest.update(b"repository");
            digest.update(Sha256::digest(revision.as_bytes()));
            digest.update(dirty_inputs_digest(&path)?.as_bytes());
        } else {
            return Err(format!(
                "cannot establish source identity for {}",
                path.display()
            ));
        }
    }
    Ok(format!("{:x}", digest.finalize()))
}

pub fn source_identity(root: &Path) -> Result<Value, String> {
    let changes = git(root, &["status", "--porcelain", "--untracked-files=normal"])?;
    let mut identity = json!({"revision": git(root, &["rev-parse", "HEAD"])?,
        "tree": git(root, &["rev-parse", "HEAD^{tree}"])?,
        "tracked_changes": changes});
    if !changes.is_empty() {
        identity["dirty_inputs_sha256"] = Value::String(dirty_inputs_digest(root)?);
    }
    Ok(identity)
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
    let mut command = build_command(root, arguments);
    let status = run_preparation(&mut command, directory, name)?;
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

fn run_preparation(
    command: &mut Command,
    directory: &Path,
    name: &str,
) -> Result<std::process::ExitStatus, String> {
    // Queuing for the shared host is not compiler failure. Retain elapsed time
    // and the real build status without changing fixture/proof execution limits.
    execution::run_unlimited(command, directory, name)
}

fn build_command(root: &Path, arguments: &[&str]) -> Command {
    let mut command = Command::new("mbx");
    command
        .args(arguments)
        .current_dir(root)
        .stdin(Stdio::null())
        .env("CARGO_PROFILE_DEV_OPT_LEVEL", "1")
        .env("CARGO_PROFILE_TEST_OPT_LEVEL", "1")
        .env("CARGO_UNSTABLE_CHECKSUM_FRESHNESS", "true")
        .env("CARGO_BUILD_FINGERPRINT", "content");
    command
}

/// Build the `omega` CLI and record its bytes with the Cargo options that
/// produced them.
pub fn prepare(root: &Path, directory: &Path) -> Result<Producer, String> {
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
    ]
    .into_iter()
    .map(|key| (key, std::env::var(key).ok()))
    .collect::<std::collections::BTreeMap<_, _>>();
    execution::write_json(
        &directory.join("build-inputs.json"),
        &json!({"host": host_target(), "environment": environment,
        "forced_profile": {"dev": "1", "test": "1"}, "freshness": "checksum/content"}),
    )?;
    let name = "cli-build";
    let arguments = [
        "build",
        "--locked",
        "--message-format=json",
        "-p",
        "omega",
        "--bin",
        "omega",
    ];
    let artifacts = build(root, directory, name, &arguments)?;
    let artifact = artifacts
        .iter()
        .find(|v| v["target"]["name"] == "omega" && v["executable"].is_string())
        .ok_or_else(|| format!("{name} produced no omega"))?;
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
    let producer = Producer {
        cli: PathBuf::from(
            artifact["executable"]
                .as_str()
                .ok_or("missing executable")?,
        ),
        executables: vec![artifact_receipt(artifact)?, artifact_receipt(compiler)?],
    };
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_architecture_selects_the_matching_profile() {
        assert_eq!(hosted_profile("windows", "aarch64"), "windows_arm64");
        assert_eq!(hosted_profile("windows", "x86_64"), "windows_x86_64");
        assert_eq!(hosted_profile("macos", "x86_64"), "macos_x86_64");
        assert_eq!(hosted_profile("linux", "riscv64"), "unsupported");
    }

    #[test]
    fn source_identity_distinguishes_edits_with_the_same_porcelain_status() {
        let root = execution::reserve_directory(
            &std::env::temp_dir().join("omega-producer-source-identity-tests"),
        )
        .unwrap();
        git(&root, &["init", "-q"]).unwrap();
        let tracked = root.join("tracked.omg");
        fs::write(&tracked, "original").unwrap();
        git(&root, &["add", "tracked.omg"]).unwrap();
        git(
            &root,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.invalid",
                "-c",
                "commit.gpgSign=false",
                "commit",
                "-qm",
                "fixture",
            ],
        )
        .unwrap();
        let clean = source_identity(&root).unwrap();
        fs::write(&tracked, "first edit").unwrap();
        let first = source_identity(&root).unwrap();
        fs::write(&tracked, "next edit").unwrap();
        let second = source_identity(&root).unwrap();
        assert_eq!(first["tracked_changes"], second["tracked_changes"]);
        assert_ne!(first, second, "two versions of the same tracked input");
        fs::write(&tracked, "original").unwrap();
        assert_eq!(source_identity(&root).unwrap(), clean);
        let directory = root.join("new inputs");
        fs::create_dir(&directory).unwrap();
        let added = directory.join("a source.omg");
        fs::write(&added, "first").unwrap();
        let first = source_identity(&root).unwrap();
        fs::write(&added, "other").unwrap();
        let second = source_identity(&root).unwrap();
        assert_eq!(first["tracked_changes"], second["tracked_changes"]);
        assert_ne!(first, second, "two versions of the same untracked input");

        git(&directory, &["init", "-q"]).unwrap();
        let first = source_identity(&root).unwrap();
        fs::write(&added, "edited inside nested repository").unwrap();
        assert_ne!(first, source_identity(&root).unwrap());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn preparation_retains_real_status_without_a_wall_clock_cap() {
        let directory = execution::reserve_directory(
            &std::env::temp_dir().join("omega-preparation-budget-tests"),
        )
        .unwrap();
        for (name, mode, code) in [
            ("delayed-build", "delayed", 0),
            ("cli-build", "failure", 23),
        ] {
            let mut command = Command::new(std::env::current_exe().unwrap());
            command
                .args([
                    "--exact",
                    "execution::tests::command_fixture",
                    "--ignored",
                    "--nocapture",
                ])
                .env("OMEGA_XTASK_EXECUTION_TEST_MODE", mode)
                .stdin(Stdio::null());
            assert_eq!(
                run_preparation(&mut command, &directory, name)
                    .unwrap()
                    .code(),
                Some(code)
            );
            let receipt: Value =
                serde_json::from_slice(&fs::read(directory.join(format!("{name}.json"))).unwrap())
                    .unwrap();
            assert!(receipt["timeout_ms"].is_null());
            assert_eq!(receipt["timed_out"], false);
            assert_eq!(receipt["exit_code"], code);
            assert_eq!(receipt["container_cleanup"]["scope"], "owned_process_tree");
            assert_eq!(receipt["container_cleanup"]["reaped"], true);
        }
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
            cli: executable.clone(),
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
