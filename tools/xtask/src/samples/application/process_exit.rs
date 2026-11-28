//! Obtain the full native exit status before wait(2) truncates it.

use crate::execution;
use std::ops::Range;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::sync::OnceLock;

pub(in crate::samples) enum ExitEvidence {
    Explicit(i32),
    #[cfg(target_os = "macos")]
    SignedZero,
}

const SITE: &str = "linux-x64-exit-site ";

pub(super) fn run(
    artifact: &Path,
    scratch: &Path,
    stdin: Stdio,
    evidence: &Path,
    report: &str,
) -> Result<(Output, Option<ExitEvidence>), String> {
    #[cfg(target_os = "macos")]
    if report.lines().any(|line| line == "process-exit-possible") {
        return super::super::darwin_exit::run(artifact, scratch, stdin, evidence);
    }
    let mut sites = Vec::new();
    for line in report.lines().filter_map(|line| line.strip_prefix(SITE)) {
        let address = |s: &str| u64::from_str_radix(s.strip_prefix("0x")?, 16).ok();
        let (start, end) = line.split_once("..").ok_or("invalid process exit site")?;
        let start = address(start).ok_or("invalid process exit start address")?;
        let end = address(end).ok_or("invalid process exit end address")?;
        if start >= end {
            return Err("empty process exit site".into());
        }
        sites.push(start..end);
    }
    if !cfg!(all(target_os = "linux", target_arch = "x86_64"))
        || sites.is_empty()
        || !can_trace(evidence)
    {
        return execution::output(
            Command::new(artifact).current_dir(scratch).stdin(stdin),
            evidence,
            "run-native",
        )
        .map(|output| (output, None));
    }
    let trace = evidence.join("native-process.trace");
    let output = execution::output(
        Command::new("strace")
            .args([
                "-f",
                "-q",
                "-i",
                "-e",
                "trace=%process",
                "-e",
                "raw=execve,execveat",
                "-o",
            ])
            .arg(&trace)
            .arg("--")
            .arg(artifact)
            .current_dir(scratch)
            .stdin(stdin),
        evidence,
        "run-native",
    )?;
    let bytes = execution::read_bounded(&trace)?;
    let trace = std::str::from_utf8(&bytes).map_err(|err| err.to_string())?;
    let code = observed_exit(trace, &sites, output.status.code())?;
    Ok((output, code.map(ExitEvidence::Explicit)))
}

fn can_trace(evidence: &Path) -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        execution::run_with_timeout(
            Command::new("strace")
                .args([
                    "-qq",
                    "-e",
                    "trace=none",
                    "-o",
                    "/dev/null",
                    "--",
                    "/bin/true",
                ])
                .stdin(Stdio::null()),
            evidence,
            "probe-strace",
            std::time::Duration::from_secs(10),
        )
        .is_ok_and(|status| status.success())
    })
}

/// Only attribute a single execution of the published image. A later exec
/// can reuse the same addresses. A different thread or child can exit with a
/// different status. Their traces remain available for manual inspection.
fn observed_exit(
    trace: &str,
    sites: &[Range<u64>],
    wait_code: Option<i32>,
) -> Result<Option<i32>, String> {
    fn event(line: &str) -> Option<(&str, &str, &str)> {
        let (pid, rest) = line.split_once(' ')?;
        let (ip, call) = rest.trim_start().strip_prefix('[')?.split_once("] ")?;
        Some((pid, ip, call))
    }
    let mut lines = trace.lines();
    let (pid, _, launch) = lines
        .next()
        .and_then(event)
        .ok_or("missing native launch in strace")?;
    if !launch.starts_with("execve(") || !launch.ends_with("= 0") {
        return Err("strace did not observe the published native program starting".into());
    }
    let (Some(call), Some(end), None) = (
        lines.next().and_then(event),
        lines.next().and_then(event),
        lines.next(),
    ) else {
        return Ok(None);
    };
    let Some(wait_code) = wait_code else {
        return Ok(None);
    };
    if call.0 != pid || end.0 != pid || end.2 != format!("+++ exited with {wait_code} +++") {
        return Ok(None);
    }
    let Some((argument, result)) = call
        .2
        .strip_prefix("exit_group(")
        .and_then(|s| s.split_once(')'))
    else {
        return Ok(None);
    };
    let ip =
        u64::from_str_radix(call.1, 16).map_err(|_| "invalid native exit instruction address")?;
    // strace reports the instruction pointer after the x86-64 syscall.
    if result.trim() != "= ?"
        || !ip
            .checked_sub(1)
            .is_some_and(|byte| sites.iter().any(|site| site.contains(&byte)))
    {
        return Ok(None);
    }
    let code = argument
        .parse::<i32>()
        .map_err(|_| "invalid native exit_group argument")?;
    if code & 255 != wait_code {
        return Err("native exit_group argument disagrees with wait status".into());
    }
    Ok(Some(code))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_an_exit_from_the_published_site_supplies_the_full_signed_status() {
        let sites = [0x401780..0x401790, 0x402800..0x402810];
        let launch = "57 [00007fc234567890] execve(0x1234, 0x2000, 0x3000) = 0\n";
        for code in [-1i32, 0, 70, 326] {
            let wait = code & 255;
            let finish = format!("57 [????????????????] +++ exited with {wait} +++\n");
            let syscall = format!("57 [000000000040178f] exit_group({code}) = ?\n");
            let trace = format!("{launch}{syscall}{finish}");
            assert_eq!(
                observed_exit(&trace, &sites, Some(wait)).unwrap(),
                Some(code)
            );
            assert!(
                observed_exit(&trace, &sites, Some(wait ^ 1))
                    .unwrap()
                    .is_none()
            );
            let elsewhere = trace.replace("40178f", "4023d5");
            assert!(
                observed_exit(&elsewhere, &sites, Some(wait))
                    .unwrap()
                    .is_none()
            );
            let replaced = format!("{launch}{trace}");
            assert!(
                observed_exit(&replaced, &sites, Some(wait))
                    .unwrap()
                    .is_none()
            );
            let child = trace.replace("57 [000000000040178f]", "58 [000000000040178f]");
            assert!(observed_exit(&child, &sites, Some(wait)).unwrap().is_none());
        }
        assert!(observed_exit("", &sites, Some(1)).is_err());
    }
}
