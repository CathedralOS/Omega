//! Host preflight (`cargo xtask doctor`): verifies that this machine can build
//! and measure the Omega compiler.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

use crate::repo_root;

const CLAIMS_REF: &str = "refs/coordination/omega-claims/main";
const MBX_MINIMUM: (u32, u32, u32) = (1, 29, 0);
const NEXTEST_MINIMUM: (u32, u32, u32) = (0, 9, 140);

#[derive(Debug, Clone, Serialize)]
pub struct CheckEntry {
    pub name: &'static str,
    pub status: &'static str,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
}

#[derive(Debug, Serialize)]
struct DoctorReport {
    schema: &'static str,
    checks: Vec<CheckEntry>,
    failed: usize,
    warned: usize,
}

fn check(
    name: &'static str,
    status: &'static str,
    detail: impl Into<String>,
    fix: Option<&str>,
) -> CheckEntry {
    CheckEntry {
        name,
        status,
        detail: detail.into(),
        fix: fix.map(str::to_owned),
    }
}

fn run_cmd(program: &str, args: &[&str], cwd: &Path) -> Option<(i32, String, String)> {
    let output = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_HTTP_LOW_SPEED_LIMIT", "100")
        .env("GIT_HTTP_LOW_SPEED_TIME", "20")
        .stdin(Stdio::null())
        .output()
        .ok()?;
    let code = output.status.code().unwrap_or(1);
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    Some((code, stdout, stderr))
}

pub fn toolchain_channel(root: &Path) -> Option<String> {
    let text = fs::read_to_string(root.join("rust-toolchain.toml")).ok()?;
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("channel") {
            let rest = rest.trim_start();
            if let Some(rest) = rest.strip_prefix('=') {
                let value = rest.trim().trim_matches('"');
                if !value.is_empty() {
                    return Some(value.to_owned());
                }
            }
        }
    }
    None
}

fn check_git_repository(root: &Path) -> CheckEntry {
    let Some((code, head, err)) = run_cmd("git", &["rev-parse", "--verify", "HEAD"], root) else {
        return check("git", "fail", "git is missing or failed to start", None);
    };
    if code != 0 || head.is_empty() {
        let reason = if err.is_empty() {
            "unborn branch"
        } else {
            &err
        };
        return check("git", "fail", format!("no commit at HEAD ({reason})"), None);
    }
    let short_head = &head[..head.len().min(12)];
    let detail = format!("HEAD {short_head}");
    let rebase = run_cmd("git", &["config", "--get", "pull.rebase"], root);
    if !matches!(rebase, Some((0, ref val, _)) if val == "true") {
        return check(
            "git",
            "warn",
            format!("{detail}; pull.rebase is not true"),
            Some(
                "git config pull.rebase true — publication is squash-only through the merge queue (tools/landing.md)",
            ),
        );
    }
    check("git", "ok", detail, None)
}

fn check_rust_toolchain(root: &Path) -> CheckEntry {
    let channel = toolchain_channel(root);
    let Some((code, version, err)) = run_cmd("rustc", &["--version"], root) else {
        return check(
            "rustc",
            "fail",
            "rustc missing",
            Some("install rustup; rust-toolchain.toml selects the pinned toolchain"),
        );
    };
    if code != 0 {
        let msg = if err.is_empty() { &version } else { &err };
        let fix = channel
            .as_deref()
            .map(|c| format!("rustup toolchain install {c}"))
            .unwrap_or_else(|| "install the toolchain rust-toolchain.toml names".to_owned());
        return check(
            "rustc",
            "fail",
            format!("rustc --version failed: {msg}"),
            Some(&fix),
        );
    }
    let detail = match &channel {
        Some(c) => format!("{version} (pinned {c})"),
        None => version.clone(),
    };
    if let Some(c) = &channel
        && c.starts_with("nightly")
        && !version.contains("nightly")
    {
        let fix = format!("rustup toolchain install {c}");
        return check(
            "rustc",
            "warn",
            format!("{detail} — pin wants nightly, resolved toolchain is not"),
            Some(&fix),
        );
    }
    check("rustc", "ok", detail, None)
}

fn check_rust_link_probe(root: &Path) -> CheckEntry {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let scratch = std::env::temp_dir().join(format!("omega-doctor-{}-{stamp}", std::process::id()));
    if fs::create_dir_all(&scratch).is_err() {
        return check(
            "rust-link",
            "fail",
            "could not create scratch directory",
            None,
        );
    }
    let result = (|| {
        let source = scratch.join("probe.rs");
        let binary = scratch.join(if cfg!(windows) { "probe.exe" } else { "probe" });
        if fs::write(&source, "fn main() { std::process::exit(7) }\n").is_err() {
            return check("rust-link", "fail", "could not write probe.rs", None);
        }
        let source_str = source.to_string_lossy();
        let binary_str = binary.to_string_lossy();
        let Some((code, out, err)) = run_cmd("rustc", &[&source_str, "-o", &binary_str], root)
        else {
            return check("rust-link", "fail", "rustc probe failed to start", None);
        };
        if code != 0 {
            let text = if err.is_empty() { &out } else { &err };
            let tail = &text[text.len().saturating_sub(400)..];
            return check(
                "rust-link",
                "fail",
                format!("link probe failed: {tail}"),
                None,
            );
        }
        let Some((run_code, _, _)) = run_cmd(&binary_str, &[], &scratch) else {
            return check("rust-link", "fail", "probe binary would not run", None);
        };
        if run_code != 7 {
            return check(
                "rust-link",
                "fail",
                format!("probe binary exited {run_code}, expected 7"),
                None,
            );
        }
        check(
            "rust-link",
            "ok",
            "compiled, linked, and ran a probe with the pinned toolchain",
            None,
        )
    })();
    let _ = fs::remove_dir_all(&scratch);
    result
}

fn mbx_install_fix() -> &'static str {
    if cfg!(windows) {
        r#"mbx is required (direct cargo fallback is forbidden); install/update via PowerShell: `$v = "1.29.2"; Invoke-WebRequest "https://github.com/jdx/mr-boxington/releases/download/v$v/mbx-x86_64-pc-windows-msvc.zip" -OutFile "$env:TEMP\mbx.zip"; Expand-Archive "$env:TEMP\mbx.zip" "$HOME\.cargo\bin" -Force; mbx setup --yes`"#
    } else {
        "mbx is required (direct cargo fallback is forbidden); install/update via `bash tools/agent_setup.sh`"
    }
}

fn check_mbx(root: &Path) -> CheckEntry {
    let Some((0, version, _)) = run_cmd("mbx", &["--version"], root) else {
        return check("mbx", "fail", "mbx not on PATH", Some(mbx_install_fix()));
    };
    let Some(found) = parse_semver_triplet(&version) else {
        return check(
            "mbx",
            "fail",
            format!("could not parse mbx version from `{version}`"),
            Some(mbx_install_fix()),
        );
    };
    if found < MBX_MINIMUM {
        return check(
            "mbx",
            "fail",
            format!(
                "mbx {}.{}.{} < {}.{}.{}",
                found.0, found.1, found.2, MBX_MINIMUM.0, MBX_MINIMUM.1, MBX_MINIMUM.2
            ),
            Some(mbx_install_fix()),
        );
    }
    check("mbx", "ok", version, None)
}

pub fn parse_semver_triplet(text: &str) -> Option<(u32, u32, u32)> {
    for token in text.split_whitespace() {
        let mut parts = token.split('.');
        if let (Some(a), Some(b), Some(c)) = (parts.next(), parts.next(), parts.next()) {
            let c_digits: String = c.chars().take_while(char::is_ascii_digit).collect();
            if let (Ok(maj), Ok(min), Ok(pat)) = (a.parse(), b.parse(), c_digits.parse()) {
                return Some((maj, min, pat));
            }
        }
    }
    None
}

fn check_nextest(root: &Path) -> CheckEntry {
    let Some((0, out, err)) = run_cmd("mbx", &["nextest", "--version"], root) else {
        return check(
            "nextest",
            "fail",
            "mbx nextest unavailable",
            Some("run `bash tools/agent_setup.sh` — needed for `mbx nextest run`"),
        );
    };
    let text = if out.is_empty() { &err } else { &out };
    let Some(found) = parse_semver_triplet(text) else {
        return check("nextest", "fail", "could not parse nextest version", None);
    };
    if found < NEXTEST_MINIMUM {
        return check(
            "nextest",
            "fail",
            format!(
                "nextest {}.{}.{} < {}.{}.{}",
                found.0, found.1, found.2, NEXTEST_MINIMUM.0, NEXTEST_MINIMUM.1, NEXTEST_MINIMUM.2
            ),
            Some("run `bash tools/agent_setup.sh` to update cargo-nextest"),
        );
    }
    check(
        "nextest",
        "ok",
        format!("nextest {}.{}.{}", found.0, found.1, found.2),
        None,
    )
}

fn check_claims_remote(root: &Path) -> CheckEntry {
    let Some((0, url_out, _)) = run_cmd(
        "git",
        &["remote", "get-url", "--push", "--all", "origin"],
        root,
    ) else {
        return check(
            "claims",
            "warn",
            "no push URL for origin",
            Some("claims.py needs exactly one push URL"),
        );
    };
    let urls: Vec<&str> = url_out.lines().filter(|l| !l.trim().is_empty()).collect();
    if urls.len() != 1 {
        return check(
            "claims",
            "warn",
            format!("origin has {} push URLs; claims needs one", urls.len()),
            None,
        );
    }
    match run_cmd("git", &["ls-remote", "--refs", urls[0], CLAIMS_REF], root) {
        Some((0, _, _)) => check("claims", "ok", format!("{CLAIMS_REF} reachable"), None),
        Some((_, _, err)) => {
            let short = &err[..err.len().min(200)];
            check(
                "claims",
                "warn",
                format!("claims ref unreachable: {short}"),
                Some("claims.py status/note/claim will fail until the coordination remote answers"),
            )
        }
        None => check("claims", "warn", "claims ref unreachable: git failed", None),
    }
}

fn check_host() -> CheckEntry {
    let cpus = std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1);
    let detail = format!(
        "{}/{}, {cpus} cpus",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    check("host", "ok", detail, None)
}

pub fn collect_checks(root: &Path) -> Vec<CheckEntry> {
    vec![
        check_git_repository(root),
        check_rust_toolchain(root),
        check_rust_link_probe(root),
        check_mbx(root),
        check_nextest(root),
        check_claims_remote(root),
        check_host(),
    ]
}

pub fn run(args: &[String]) -> Result<(), u8> {
    let mut json_path: Option<PathBuf> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--json" => {
                i += 1;
                let Some(path) = args.get(i) else {
                    eprintln!("doctor: `--json` requires a path");
                    return Err(2);
                };
                json_path = Some(PathBuf::from(path));
            }
            "-h" | "--help" => {
                println!("usage: cargo xtask doctor [--json <path>]");
                return Ok(());
            }
            other => {
                eprintln!("doctor: unknown argument `{other}`");
                return Err(2);
            }
        }
        i += 1;
    }

    let root = repo_root()?;
    let checks = collect_checks(&root);
    let failed = checks.iter().filter(|c| c.status == "fail").count();
    let warned = checks.iter().filter(|c| c.status == "warn").count();

    for entry in &checks {
        let mut line = format!("{:>4}  {}: {}", entry.status, entry.name, entry.detail);
        if let Some(fix) = &entry.fix {
            line.push_str(&format!("  [fix: {fix}]"));
        }
        println!("{line}");
    }
    println!(
        "doctor: {failed} failed, {warned} warnings, {} checks",
        checks.len()
    );

    if let Some(path) = json_path {
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).map_err(|e| {
                eprintln!("doctor: cannot create {}: {e}", parent.display());
                1u8
            })?;
        }
        let report = DoctorReport {
            schema: "omega-doctor-v1",
            checks,
            failed,
            warned,
        };
        let serialized = serde_json::to_string_pretty(&report).map_err(|_| 1u8)?;
        fs::write(&path, format!("{serialized}\n")).map_err(|e| {
            eprintln!("doctor: cannot write {}: {e}", path.display());
            1u8
        })?;
    }

    if failed > 0 { Err(1) } else { Ok(()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_nextest_semver_from_version_output() {
        assert_eq!(
            parse_semver_triplet("cargo-nextest 0.9.140 (2026-01-01)"),
            Some((0, 9, 140))
        );
    }

    #[test]
    fn reads_pinned_toolchain_channel() {
        let channel = toolchain_channel(&repo_root().expect("test runs in the Omega workspace"));
        assert!(channel.is_some());
    }
}
