//! Sample native + Terminal Psi gate (`cargo xtask sample-gate`).

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::{execution, producer, repo_root};

const REJECTION_FILES: &[&str] = &[
    "expected_diagnostic.txt",
    "expected_error.txt",
    "expected.txt",
];

/// A program that must type-check and then be refused by native admission.
/// Each non-empty line is a fragment the native build's output must contain.
const NATIVE_REJECTION_FILE: &str = "expected_native_error.txt";

#[derive(Debug, Clone)]
pub struct SampleProject {
    pub directory: PathBuf,
    pub rel_path: String,
    pub entry: Option<PathBuf>,
    pub role: String,
    pub source_inputs: Vec<String>, // Explicit root inventory; absence retains ordinary caller default.
    pub rejection: Vec<String>,
    pub rejection_any: Vec<String>,
    pub rejection_casefold: bool,
    /// Fragments from [`NATIVE_REJECTION_FILE`]; `None` when it is absent.
    pub native_rejection: Option<Vec<String>>,
}

fn host_target() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows_x86_64"
    } else if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") {
            "macos_arm64"
        } else {
            "macos_x86_64"
        }
    } else if cfg!(target_arch = "aarch64") {
        "linux_arm64"
    } else {
        "linux_x86_64"
    }
}

fn strip_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
        } else if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(bytes.len());
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
}

pub fn project_at(directory: &Path, repo: &Path) -> Result<SampleProject, String> {
    let rel_path = directory
        .strip_prefix(repo)
        .unwrap_or(directory)
        .to_string_lossy()
        .replace('\\', "/");
    let meta_path = directory.join("sample.json");
    let meta: serde_json::Value = if meta_path.is_file() {
        let content =
            fs::read_to_string(&meta_path).map_err(|e| format!("{}: {e}", meta_path.display()))?;
        serde_json::from_str(&content).map_err(|e| format!("{}: {e}", meta_path.display()))?
    } else {
        serde_json::json!({})
    };

    let entry_name: Option<String> = if let Some(s) = meta.get("entry").and_then(|v| v.as_str()) {
        Some(s.to_owned())
    } else if directory.join("main.omg").is_file() {
        Some("main.omg".to_owned())
    } else {
        let mut omg_files: Vec<String> = fs::read_dir(directory)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                (name.ends_with(".omg") && name != "build.omg").then_some(name)
            })
            .collect();
        omg_files.sort();
        (omg_files.len() == 1).then(|| omg_files.remove(0))
    };
    let entry = entry_name.map(|n| directory.join(n));

    let build_raw = fs::read_to_string(directory.join("build.omg")).unwrap_or_default();
    let build = strip_comments(&build_raw);
    let dir_name = directory.file_name().and_then(|s| s.to_str()).unwrap_or("");
    let diag_file = REJECTION_FILES
        .iter()
        .map(|n| directory.join(n))
        .find(|p| p.is_file());
    let is_negative = matches!(dir_name, "invalid" | "intended_invalid" | "out_of_bounds")
        || dir_name.starts_with("invalid_")
        || (diag_file.is_some() && rel_path.split('/').any(|p| p == "controls"));

    let inferred = if is_negative {
        "intended-invalid"
    } else if rel_path.starts_with("source/library") || build.contains("Build::package") {
        "library"
    } else if !build.contains("ProgramEntry") {
        "check-only"
    } else if build.contains("Build::application") {
        "program"
    } else {
        "example"
    };

    let raw_role = meta
        .get("role")
        .and_then(|v| v.as_str())
        .unwrap_or(inferred);
    let role = if raw_role == "invalid-control" {
        "intended-invalid".to_owned()
    } else {
        raw_role.to_owned()
    };

    let rejection: Vec<String> =
        if let Some(arr) = meta.get("rejection_contains").and_then(|v| v.as_array()) {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        } else if role == "intended-invalid"
            && let Some(df) = &diag_file
        {
            fragment_lines(&fs::read_to_string(df).unwrap_or_default())
        } else {
            Vec::new()
        };

    let rejection_any: Vec<String> = meta
        .get("rejection_any_contains")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();

    let rejection_casefold = meta
        .get("rejection_casefold")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let source_inputs = match meta.get("source_inputs") {
        None => Vec::new(),
        Some(value) => {
            let paths = value
                .as_array()
                .filter(|paths| !paths.is_empty())
                .ok_or_else(|| {
                    format!(
                        "{}: source_inputs requires a nonempty array of relative files/subtrees",
                        meta_path.display()
                    )
                })?;
            paths
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .filter(|path| !path.is_empty())
                        .map(str::to_owned)
                        .ok_or_else(|| {
                            format!(
                                "{}: source_inputs contains a missing path",
                                meta_path.display()
                            )
                        })
                })
                .collect::<Result<Vec<_>, _>>()?
        }
    };

    let native_rejection = fs::read_to_string(directory.join(NATIVE_REJECTION_FILE))
        .ok()
        .map(|text| fragment_lines(&text));

    Ok(SampleProject {
        directory: directory.to_path_buf(),
        rel_path,
        entry,
        role,
        source_inputs,
        rejection,
        rejection_any,
        rejection_casefold,
        native_rejection,
    })
}

pub fn discover_samples(
    search_root: &Path,
    repo: &Path,
    filters: &[String],
) -> Result<Vec<SampleProject>, String> {
    let mut out = Vec::new();
    let mut stack = vec![search_root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        let mut subdirs = Vec::new();
        let mut has_build = false;
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name == "build.omg" {
                has_build = true;
            }
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false)
                && !matches!(name.as_str(), "build" | ".git" | "target")
            {
                subdirs.push(entry.path());
            }
        }
        subdirs.sort();
        for sub in subdirs.into_iter().rev() {
            stack.push(sub);
        }
        if has_build {
            let rel = dir
                .strip_prefix(repo)
                .unwrap_or(&dir)
                .to_string_lossy()
                .replace('\\', "/");
            if !filters.is_empty() && !filters.iter().any(|f| rel.contains(f.as_str())) {
                continue;
            }
            let proj = project_at(&dir, repo)?;
            if proj.role != "library" {
                out.push(proj);
            }
        }
    }
    out.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    Ok(out)
}

fn accept_pending_decisions(review_bytes: &[u8]) -> Option<Vec<u8>> {
    let text = String::from_utf8_lossy(review_bytes);
    let mut changed = false;
    let mut out = String::with_capacity(text.len());
    for (idx, line) in text.split('\n').enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        let trimmed = line.strip_suffix('\r').unwrap_or(line);
        if let Some(prefix) = trimmed.strip_suffix(" pending")
            && prefix.starts_with("decision row ")
        {
            out.push_str(prefix);
            out.push_str(" accept");
            changed = true;
        } else {
            out.push_str(line);
        }
    }
    changed.then(|| out.into_bytes())
}

fn retain_accepted_inputs(sample_dir: &Path, evidence: &Path) -> Result<(), String> {
    // The selected root lock contains its accepted dependency closure.
    let directory = evidence.join("accepted-inputs");
    fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let mut retained = Vec::new();
    for relative in ["omega.lock", "omega.admissions"] {
        let source = sample_dir.join(relative);
        if !source.is_file() {
            continue;
        }
        let target = directory.join(relative);
        let before = execution::digest(&source)?;
        let metadata = fs::metadata(&source).map_err(|e| e.to_string())?;
        fs::copy(&source, &target).map_err(|e| format!("retain {}: {e}", source.display()))?;
        if execution::digest(&target)? != before || execution::digest(&source)? != before {
            return Err(format!("accepted input changed while retained: {relative}"));
        }
        retained.push(serde_json::json!({"source_path": source, "relative_path": relative, "sha256": before, "bytes": metadata.len(),
            "modified_unix_nanos": metadata.modified().map_err(|e| e.to_string())?.duration_since(std::time::UNIX_EPOCH).map_err(|e| e.to_string())?.as_nanos()}));
    }
    execution::write_json(
        &directory.join("manifest.json"),
        &serde_json::json!({"project": sample_dir, "inputs": retained,
        "capture_scope": "Selected project acceptance after ordinary update/resume; the root lock includes its dependency closure, and copied state grants no authority in another checkout."}),
    )
}

fn with_source_inputs<'command>(
    command: &'command mut Command,
    inputs: &[String],
) -> &'command mut Command {
    for path in inputs {
        command.args(["--build-input", path]);
    }
    command
}

fn settle_review(
    cli: &Path,
    sample_dir: &Path,
    target: &str,
    evidence: &Path,
    source_inputs: &[String],
) -> Result<(), String> {
    let out = execution::output(
        with_source_inputs(
            Command::new(cli)
                .args(["update", "--project"])
                .arg(sample_dir)
                .args(["--target", target])
                .stdin(Stdio::null()),
            source_inputs,
        ),
        evidence,
        "update",
    )?;
    let review_path = sample_dir
        .join("build/package-manager")
        .join(format!("review-{target}.txt"));
    if let Ok(bytes) = fs::read(&review_path)
        && let Some(accepted) = accept_pending_decisions(&bytes)
    {
        fs::write(evidence.join("pending-review.txt"), &bytes).map_err(|e| e.to_string())?;
        fs::write(&review_path, &accepted)
            .map_err(|e| format!("failed writing {}: {e}", review_path.display()))?;
        fs::write(evidence.join("accepted-review.txt"), &accepted).map_err(|e| e.to_string())?;
        execution::write_json(
            &evidence.join("accepted-review.json"),
            &serde_json::json!({"source_path": review_path,
            "sha256": execution::digest(&review_path)?, "modified_unix_nanos": fs::metadata(&review_path).and_then(|m| m.modified()).map_err(|e| e.to_string())?
                .duration_since(std::time::UNIX_EPOCH).map_err(|e| e.to_string())?.as_nanos()}),
        )?;
        let resume = execution::output(
            Command::new(cli)
                .args(["update", "--project"])
                .arg(sample_dir)
                .arg("--resume")
                .stdin(Stdio::null()),
            evidence,
            "resume",
        )?;
        if !resume.status.success() {
            return Err(format!(
                "resume-failed: {}{}",
                String::from_utf8_lossy(&resume.stdout),
                String::from_utf8_lossy(&resume.stderr)
            ));
        }
    } else if !out.status.success() {
        return Err(format!(
            "update failed: {}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    retain_accepted_inputs(sample_dir, evidence)
}

/// The trimmed non-empty lines of a pinned-fragment file.
fn fragment_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

/// A program pinned by [`NATIVE_REJECTION_FILE`] passes only when the
/// checker accepts it and native admission then refuses it with every pinned
/// fragment. An empty pin, a check failure, a native build that succeeds or a
/// missing fragment each fail the sample.
fn verify_native_rejection(
    cli: &Path,
    entry: &Path,
    target: &str,
    fragments: &[String],
    evidence: &Path,
    source_inputs: &[String],
) -> Result<String, String> {
    if fragments.is_empty() {
        return Err(format!("{NATIVE_REJECTION_FILE} pins no fragment"));
    }
    let check = execution::output(
        with_source_inputs(
            Command::new(cli)
                .arg("--check")
                .arg(entry)
                .stdin(Stdio::null()),
            source_inputs,
        ),
        evidence,
        "check",
    )?;
    if !check.status.success() {
        return Err(format!(
            "check-fail before native admission: {}{}",
            String::from_utf8_lossy(&check.stdout),
            String::from_utf8_lossy(&check.stderr)
        ));
    }
    let build = execution::output(
        with_source_inputs(
            Command::new(cli)
                .args(["--target", target])
                .arg(entry)
                .stdin(Stdio::null()),
            source_inputs,
        ),
        evidence,
        "compile",
    )?;
    if build.status.success() {
        return Err("native build succeeded; expected native admission to refuse it".to_owned());
    }
    let output = format!(
        "{}{}",
        String::from_utf8_lossy(&build.stdout),
        String::from_utf8_lossy(&build.stderr)
    );
    match fragments
        .iter()
        .find(|fragment| !output.contains(fragment.as_str()))
    {
        Some(missing) => Err(format!(
            "missing native rejection fragment {missing:?} in: {output}"
        )),
        None => Ok("native-rejected".to_owned()),
    }
}

pub fn parse_header_oracle(entry: &Path) -> (Option<i32>, Option<String>) {
    let text = fs::read_to_string(entry).unwrap_or_default();
    let mut expected_exit = None;
    let mut expected_output = None;
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("//") {
            let comment = rest.trim();
            if let Some(exit_str) = comment.strip_prefix("Expected exit:") {
                let digits = exit_str.trim().trim_end_matches('.');
                if let Ok(code) = digits.parse::<i32>() {
                    expected_exit = Some(code);
                }
            } else if let Some(out_str) = comment.strip_prefix("Expected output contains:") {
                expected_output = Some(out_str.trim().to_owned());
            }
        }
    }
    (expected_exit, expected_output)
}

pub fn bound_targets(sample_dir: &Path) -> BTreeSet<String> {
    let build =
        strip_comments(&fs::read_to_string(sample_dir.join("build.omg")).unwrap_or_default());
    let mut targets = BTreeSet::new();
    for part in build.split("::ProgramEntry") {
        let target: String = part
            .chars()
            .rev()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        if !target.is_empty()
            && target
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_lowercase())
        {
            targets.insert(target);
        }
    }
    targets
}

fn compared_host_command(cli: &Path, target: &str) -> Result<Command, String> {
    let host = host_target();
    if target != host {
        return Err(format!(
            "native/Psi execution requires host target {host}, requested {target}"
        ));
    }
    // An explicit CLI --target requests compilation without execution, even
    // when it names this host. Both engines run only through the host request.
    let mut command = Command::new(cli);
    command.args(["run", "--both"]);
    Ok(command)
}

pub fn verify_sample(
    cli: &Path,
    project: &SampleProject,
    target: &str,
    evidence: &Path,
) -> Result<String, String> {
    let Some(entry) = &project.entry else {
        return Err("missing unambiguous entry".to_owned());
    };
    if project.role == "intended-invalid" {
        // Exit 1 alone also describes unrelated compiler errors. A negative
        // sample needs a diagnostic oracle before its refusal can earn credit.
        if project.rejection.is_empty() && project.rejection_any.is_empty() {
            return Err("intended-invalid sample requires diagnostic expectations".to_owned());
        }
        if project
            .rejection
            .iter()
            .chain(&project.rejection_any)
            .any(|fragment| fragment.trim().is_empty())
        {
            return Err(
                "intended-invalid diagnostic expectations contain a blank fragment".to_owned(),
            );
        }
        let out = execution::output(
            with_source_inputs(
                Command::new(cli)
                    .args(["--check", "--target", target])
                    .arg(entry)
                    .stdin(Stdio::null()),
                &project.source_inputs,
            ),
            evidence,
            "check",
        )?;
        if out.status.code() != Some(1) {
            return Err(format!(
                "expected rejection exit 1, got {:?}",
                out.status.code()
            ));
        }
        let stderr = String::from_utf8_lossy(&out.stderr);
        let haystack = if project.rejection_casefold {
            stderr.to_lowercase()
        } else {
            stderr.into_owned()
        };
        for frag in &project.rejection {
            let needle = if project.rejection_casefold {
                frag.to_lowercase()
            } else {
                frag.clone()
            };
            if !haystack.contains(&needle) {
                return Err(format!(
                    "missing rejection fragment {frag:?} in: {haystack}"
                ));
            }
        }
        if !project.rejection_any.is_empty()
            && !project.rejection_any.iter().any(|frag| {
                let needle = if project.rejection_casefold {
                    frag.to_lowercase()
                } else {
                    frag.clone()
                };
                haystack.contains(&needle)
            })
        {
            return Err(format!(
                "missing any rejection fragment {:?} in: {haystack}",
                project.rejection_any
            ));
        }
        return Ok("rejected".to_owned());
    }

    if project.rel_path.starts_with("samples/uefi/") {
        return Ok("not-host-runnable".to_owned());
    }
    let targets = bound_targets(&project.directory);
    if !targets.is_empty() && !targets.contains(target) {
        return Ok("not-host-runnable".to_owned());
    }

    settle_review(
        cli,
        &project.directory,
        target,
        evidence,
        &project.source_inputs,
    )?;

    if let Some(fragments) = &project.native_rejection {
        return verify_native_rejection(
            cli,
            entry,
            target,
            fragments,
            evidence,
            &project.source_inputs,
        );
    }

    if matches!(project.role.as_str(), "check-only" | "library") {
        let out = execution::output(
            with_source_inputs(
                Command::new(cli)
                    .arg("--check")
                    .arg(entry)
                    .stdin(Stdio::null()),
                &project.source_inputs,
            ),
            evidence,
            "check",
        )?;
        if !out.status.success() {
            return Err(format!(
                "check-fail: {}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        return Ok("checked".to_owned());
    }

    let (expected_exit, expected_output) = parse_header_oracle(entry);
    let input_path = project.directory.join("input.txt");
    let has_input = input_path.is_file();
    if !has_input && expected_exit.is_none() && expected_output.is_none() {
        if project.rel_path.split('/').any(|p| p == "probes") {
            return Ok("no-oracle".to_owned());
        }
        let out = execution::output(
            with_source_inputs(
                Command::new(cli)
                    .args(["--target", target])
                    .arg(entry)
                    .stdin(Stdio::null()),
                &project.source_inputs,
            ),
            evidence,
            "compile",
        )?;
        if !out.status.success() {
            return Err(format!(
                "no-native-build: {}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        return Ok("no-oracle".to_owned());
    }

    let mut cmd = compared_host_command(cli, target)?;
    with_source_inputs(&mut cmd, &project.source_inputs);
    let scratch = project.directory.join("build/gate-run");
    let _ = fs::create_dir_all(&scratch);
    cmd.arg(entry).current_dir(&scratch);
    if has_input {
        let file = fs::File::open(&input_path)
            .map_err(|e| format!("failed opening {}: {e}", input_path.display()))?;
        cmd.stdin(Stdio::from(file));
    } else {
        cmd.stdin(Stdio::null());
    }
    let out = execution::output(&mut cmd, evidence, "run-both")?;
    let stdout = String::from_utf8_lossy(&out.stdout).replace("\r\n", "\n");
    let stderr = String::from_utf8_lossy(&out.stderr).replace("\r\n", "\n");

    let mut native_exit: Option<i32> = None;
    let mut interp_exit: Option<i32> = None;
    for line in stderr.lines() {
        if let Some(rest) = line.strip_prefix("native exit ")
            && let Ok(c) = rest.trim().parse::<i32>()
        {
            native_exit = Some(c);
        } else if let Some(rest) = line.strip_prefix("interp exit: ")
            && let Ok(c) = rest.trim().parse::<i32>()
        {
            interp_exit = Some(c);
        }
    }
    let (Some(n_exit), Some(i_exit)) = (native_exit, interp_exit) else {
        return Err(format!(
            "execution failed (exit {:?}): {stdout}{stderr}",
            out.status.code()
        ));
    };
    if n_exit != i_exit {
        return Err(format!(
            "engine disagreement: native exit {n_exit} != interp exit {i_exit}"
        ));
    }
    // The CLI returns the program's exit on agreement, including authored
    // nonzero exits. Require the CLI result to match the two observed engine exits.
    if out.status.code() != Some(n_exit) {
        return Err(format!(
            "execution failed (exit {:?}, engines {n_exit}): {stdout}{stderr}",
            out.status.code()
        ));
    }
    if let Some(exp) = expected_exit
        && n_exit != exp
    {
        return Err(format!("expected exit {exp}, got {n_exit}"));
    }
    if let Some(exp_out) = expected_output
        && !stdout.contains(&exp_out)
    {
        return Err(format!(
            "expected output containing {exp_out:?}, got {stdout:?}"
        ));
    }
    Ok(format!("pass (native exit {n_exit}, Psi exit {i_exit})"))
}

pub fn run(args: &[String]) -> Result<(), u8> {
    let repo = repo_root();
    let mut filters: Vec<String> = Vec::new();
    let mut facts_only = false;
    let mut programs_only = false;
    let mut cli_override: Option<PathBuf> = None;
    let mut target = host_target().to_owned();

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-h" | "--help" => {
                println!(
                    "usage: cargo xtask sample-gate [--filter <fragments>] [--facts] [--programs-only] [--target <target>] [--cli <path>]"
                );
                return Ok(());
            }
            "--facts" => facts_only = true,
            "--programs-only" => programs_only = true,
            "--filter" => {
                i += 1;
                let val = args.get(i).ok_or_else(|| {
                    eprintln!("sample-gate: --filter requires a value");
                    2u8
                })?;
                filters.extend(
                    val.split(',')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(str::to_owned),
                );
            }
            "--target" => {
                i += 1;
                target = args.get(i).cloned().ok_or_else(|| {
                    eprintln!("sample-gate: --target requires a value");
                    2u8
                })?;
            }
            "--cli" => {
                i += 1;
                let path = args.get(i).ok_or_else(|| {
                    eprintln!("sample-gate: --cli requires a path");
                    2u8
                })?;
                cli_override = Some(PathBuf::from(path));
            }
            other => {
                eprintln!("sample-gate: unknown option `{other}`");
                return Err(2);
            }
        }
        i += 1;
    }

    let search_root = if facts_only {
        repo.join("samples/cli/facts")
    } else {
        repo.join("samples")
    };
    let mut projects = discover_samples(&search_root, &repo, &filters).map_err(|e| {
        eprintln!("sample-gate: {e}");
        2u8
    })?;
    if programs_only {
        projects.retain(|p| {
            !p.rel_path
                .split('/')
                .any(|part| matches!(part, "probes" | "controls"))
                && p.role != "intended-invalid"
        });
    }
    if projects.is_empty() {
        eprintln!("sample-gate: no samples matched filter {:?}", filters);
        return Err(2);
    }

    if cli_override.is_some() {
        eprintln!(
            "sample-gate: --cli cannot authenticate an external executable; use the current-source builder"
        );
        return Err(2);
    }
    let evidence =
        execution::reserve_directory(&repo.join("build/sample-gate-runs")).map_err(|e| {
            eprintln!("sample-gate: {e}");
            4u8
        })?;
    let producer = producer::prepare(&repo, &evidence.join("producer")).map_err(|e| {
        eprintln!("sample-gate: {e}");
        4u8
    })?;
    let cli = producer.cli.as_path();

    let mut failed = 0usize;
    for proj in &projects {
        match verify_sample(cli, proj, &target, &evidence.join(&proj.rel_path)) {
            Ok(state) => {
                println!("{state}\t{}", proj.rel_path);
            }
            Err(reason) => {
                let first_line = reason.lines().next().unwrap_or(&reason);
                eprintln!("FAIL\t{}\t{first_line}", proj.rel_path);
                failed += 1;
            }
        }
    }

    producer.verify().map_err(|e| {
        eprintln!("sample-gate: {e}");
        4u8
    })?;
    if failed > 0 {
        eprintln!(
            "sample-gate: {failed} of {} sample(s) failed",
            projects.len()
        );
        Err(1)
    } else {
        println!(
            "sample-gate: all {} sample(s) passed on {target}",
            projects.len()
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn invalid_sample(directory: &Path) -> SampleProject {
        fs::write(directory.join("main.omg"), "machine main() {}\n").unwrap();
        fs::write(directory.join("build.omg"), "").unwrap();
        fs::write(
            directory.join("sample.json"),
            r#"{"role":"intended-invalid"}"#,
        )
        .unwrap();
        project_at(directory, directory).unwrap()
    }

    #[test]
    fn malformed_source_inventory_cannot_fall_back_to_a_wide_capture() {
        let directory = execution::reserve_directory(&std::env::temp_dir()).unwrap();
        fs::write(directory.join("main.omg"), "machine main() {}\n").unwrap();
        for metadata in [
            r#"{"source_inputs":[]}"#,
            r#"{"source_inputs":"main.omg"}"#,
            r#"{"source_inputs":["main.omg",null]}"#,
            r#"{"source_inputs":[""]}"#,
        ] {
            fs::write(directory.join("sample.json"), metadata).unwrap();
            assert!(
                project_at(&directory, &directory)
                    .unwrap_err()
                    .contains("source_inputs")
            );
        }
        fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn sample_preparation_and_execution_receive_the_same_explicit_inventory() {
        use std::os::unix::fs::PermissionsExt;
        let directory = execution::reserve_directory(&std::env::temp_dir()).unwrap();
        fs::write(
            directory.join("main.omg"),
            "// Expected exit: 0\nmachine main() {}\n",
        )
        .unwrap();
        fs::write(directory.join("build.omg"), format!(
            "machine build(builder: &mut Build) {{ Build::application(builder, \"sample\"); Build::bind(builder, {}::ProgramEntry, main); }}\n", host_target())).unwrap();
        fs::write(
            directory.join("sample.json"),
            r#"{"source_inputs":["main.omg","build.omg"]}"#,
        )
        .unwrap();
        let cli = directory.join("compiler");
        fs::write(
            &cli,
            r#"#!/bin/sh
main=no
build=no
previous=''
for argument do
  if [ "$previous" = '--build-input' ]; then
    case "$argument" in main.omg) main=yes;; build.omg) build=yes;; esac
  fi
  previous="$argument"
done
if [ "$main" != yes ] || [ "$build" != yes ]; then
  echo 'caller inventory was lost' >&2
  exit 91
fi
case "$1" in
  update) exit 0;;
  run) echo 'native exit 0' >&2; echo 'interp exit: 0' >&2; exit 0;;
  *) exit 92;;
esac
"#,
        )
        .unwrap();
        fs::set_permissions(&cli, fs::Permissions::from_mode(0o700)).unwrap();
        let project = project_at(&directory, &directory).unwrap();
        assert!(
            verify_sample(&cli, &project, host_target(), &directory.join("evidence"))
                .unwrap()
                .starts_with("pass")
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn intended_invalid_requires_a_nonblank_diagnostic_oracle_before_execution() {
        let directory = execution::reserve_directory(&std::env::temp_dir()).unwrap();
        let cli = directory.join("missing-compiler");
        let mut sample = invalid_sample(&directory);
        for expected in [None, Some("  \n\t\n")] {
            if let Some(text) = expected {
                fs::write(directory.join("expected.txt"), text).unwrap();
                sample = project_at(&directory, &directory).unwrap();
            }
            let error = verify_sample(&cli, &sample, host_target(), &directory.join("evidence"))
                .unwrap_err();
            assert!(
                error.contains("requires diagnostic expectations"),
                "{error}"
            );
        }
        for meta in [
            r#"{"role":"intended-invalid","rejection_contains":[]}"#,
            r#"{"role":"intended-invalid","rejection_contains":[" "]}"#,
            r#"{"role":"intended-invalid","rejection_any_contains":[""]}"#,
            r#"{"role":"intended-invalid","rejection_contains":["wanted"],"rejection_any_contains":["wanted",""]}"#,
        ] {
            fs::write(directory.join("sample.json"), meta).unwrap();
            sample = project_at(&directory, &directory).unwrap();
            let error = verify_sample(&cli, &sample, host_target(), &directory.join("evidence"))
                .unwrap_err();
            assert!(error.contains("diagnostic expectations"), "{error}");
        }
        assert!(!directory.join("evidence").exists());
        fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn intended_invalid_requires_the_pinned_reason_and_a_normal_rejection() {
        use std::os::unix::fs::PermissionsExt;

        let directory = execution::reserve_directory(&std::env::temp_dir()).unwrap();
        let mut sample = invalid_sample(&directory);
        sample.rejection = vec!["required contract".to_owned()];
        sample.rejection_any = vec!["bound too small".to_owned(), "missing evidence".to_owned()];
        let cli = directory.join("compiler");
        for (name, diagnostic, code, expected) in [
            ("correct", "required contract: missing evidence", 1, None),
            (
                "unrelated",
                "syntax error",
                1,
                Some("missing rejection fragment"),
            ),
            (
                "missing-alternative",
                "required contract",
                1,
                Some("missing any rejection fragment"),
            ),
            (
                "crashed",
                "required contract: missing evidence",
                101,
                Some("expected rejection exit 1"),
            ),
            (
                "accepted",
                "required contract: missing evidence",
                0,
                Some("expected rejection exit 1"),
            ),
        ] {
            fs::write(
                &cli,
                format!("#!/bin/sh\nprintf '%s\\n' '{diagnostic}' >&2\nexit {code}\n"),
            )
            .unwrap();
            fs::set_permissions(&cli, fs::Permissions::from_mode(0o700)).unwrap();
            let result = verify_sample(&cli, &sample, host_target(), &directory.join(name));
            if let Some(fragment) = expected {
                let error = result.unwrap_err();
                assert!(error.contains(fragment), "{error}");
            } else {
                assert_eq!(result.unwrap(), "rejected");
            }
        }
        // An any-of pin can stand alone, and authored case folding still applies.
        sample.rejection.clear();
        sample.rejection_casefold = true;
        fs::write(
            &cli,
            "#!/bin/sh\nprintf 'MISSING EVIDENCE\\n' >&2\nexit 1\n",
        )
        .unwrap();
        assert_eq!(
            verify_sample(&cli, &sample, host_target(), &directory.join("casefold")).unwrap(),
            "rejected"
        );
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn compared_host_request_executes_both_without_compile_only_target() {
        let command = compared_host_command(Path::new("omega"), host_target()).unwrap();
        assert_eq!(command.get_args().collect::<Vec<_>>(), ["run", "--both"]);
    }

    #[test]
    fn compared_host_request_refuses_a_different_target() {
        let other = if host_target() == "linux_x86_64" {
            "windows_x86_64"
        } else {
            "linux_x86_64"
        };
        let error = compared_host_command(Path::new("omega"), other).unwrap_err();
        assert!(error.contains(host_target()));
        assert!(error.contains(other));
    }

    #[test]
    fn rewrites_pending_decisions_to_accept_preserving_lf() {
        let input = b"# header\ndecision row 0123abcd pending\nother line\n";
        let output = accept_pending_decisions(input).expect("should rewrite pending row");
        assert_eq!(
            output,
            b"# header\ndecision row 0123abcd accept\nother line\n"
        );
    }
}
