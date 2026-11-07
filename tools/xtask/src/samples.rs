//! Sample native + Terminal Psi gate (`cargo xtask sample-gate`).

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::repo_root;

const REJECTION_FILES: &[&str] = &[
    "expected_diagnostic.txt",
    "expected_error.txt",
    "expected.txt",
];

#[derive(Debug, Clone)]
pub struct SampleProject {
    pub directory: PathBuf,
    pub rel_path: String,
    pub entry: Option<PathBuf>,
    pub role: String,
    pub rejection: Vec<String>,
    pub rejection_any: Vec<String>,
    pub rejection_casefold: bool,
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
            fs::read_to_string(df)
                .unwrap_or_default()
                .lines()
                .map(str::trim)
                .filter(|l| !l.is_empty())
                .map(str::to_owned)
                .collect()
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

    Ok(SampleProject {
        directory: directory.to_path_buf(),
        rel_path,
        entry,
        role,
        rejection,
        rejection_any,
        rejection_casefold,
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

fn build_omega_cli(repo: &Path) -> Result<PathBuf, u8> {
    let output = Command::new("cargo")
        .args([
            "build",
            "--locked",
            "--message-format=json",
            "-p",
            "omega",
            "--bin",
            "omega",
        ])
        .current_dir(repo)
        .env("CARGO_PROFILE_DEV_OPT_LEVEL", "1")
        .stdin(Stdio::null())
        .output()
        .map_err(|e| {
            eprintln!("sample-gate: failed to invoke cargo build: {e}");
            1u8
        })?;
    if !output.status.success() {
        eprintln!(
            "sample-gate: failed to build omega CLI:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        return Err(1);
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let Ok(val) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if val.get("reason").and_then(|v| v.as_str()) == Some("compiler-artifact")
            && val
                .get("target")
                .and_then(|t| t.get("name"))
                .and_then(|n| n.as_str())
                == Some("omega")
            && let Some(exe) = val.get("executable").and_then(|v| v.as_str())
        {
            return Ok(PathBuf::from(exe));
        }
    }
    eprintln!("sample-gate: cargo produced no omega executable");
    Err(1)
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

fn settle_review(cli: &Path, sample_dir: &Path, target: &str) -> Result<(), String> {
    let out = Command::new(cli)
        .args(["update", "--project"])
        .arg(sample_dir)
        .args(["--target", target])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("failed to spawn omega update: {e}"))?;
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    if combined.contains("unsupported package lock version") {
        let _ = fs::remove_file(sample_dir.join("omega.lock"));
        let _ = Command::new(cli)
            .args(["update", "--project"])
            .arg(sample_dir)
            .args(["--target", target])
            .stdin(Stdio::null())
            .output();
    }
    let review_path = sample_dir
        .join("build/package-manager")
        .join(format!("review-{target}.txt"));
    if let Ok(bytes) = fs::read(&review_path)
        && let Some(accepted) = accept_pending_decisions(&bytes)
    {
        fs::write(&review_path, accepted)
            .map_err(|e| format!("failed writing {}: {e}", review_path.display()))?;
        let resume = Command::new(cli)
            .args(["update", "--project"])
            .arg(sample_dir)
            .arg("--resume")
            .stdin(Stdio::null())
            .output()
            .map_err(|e| format!("failed to spawn omega update --resume: {e}"))?;
        if !resume.status.success() {
            return Err(format!(
                "resume-failed: {}{}",
                String::from_utf8_lossy(&resume.stdout),
                String::from_utf8_lossy(&resume.stderr)
            ));
        }
    }
    Ok(())
}

fn parse_header_oracle(entry: &Path) -> (Option<i32>, Option<String>) {
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

fn bound_targets(sample_dir: &Path) -> BTreeSet<String> {
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

fn verify_sample(cli: &Path, project: &SampleProject, target: &str) -> Result<String, String> {
    let Some(entry) = &project.entry else {
        return Err("missing unambiguous entry".to_owned());
    };
    if project.role == "intended-invalid" {
        let out = Command::new(cli)
            .args(["--check", "--target", target])
            .arg(entry)
            .stdin(Stdio::null())
            .output()
            .map_err(|e| format!("failed to run omega --check: {e}"))?;
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

    settle_review(cli, &project.directory, target)?;

    if matches!(project.role.as_str(), "check-only" | "library") {
        let out = Command::new(cli)
            .arg("--check")
            .arg(entry)
            .stdin(Stdio::null())
            .output()
            .map_err(|e| format!("failed to run omega --check: {e}"))?;
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
        let out = Command::new(cli)
            .args(["--target", target])
            .arg(entry)
            .stdin(Stdio::null())
            .output()
            .map_err(|e| format!("failed to compile sample: {e}"))?;
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
    let out = cmd
        .output()
        .map_err(|e| format!("failed to run omega run --both: {e}"))?;
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

    let cli = match cli_override {
        Some(p) => p,
        None => build_omega_cli(&repo)?,
    };

    let mut failed = 0usize;
    for proj in &projects {
        match verify_sample(&cli, proj, &target) {
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
