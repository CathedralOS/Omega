//! `cargo xtask sweep`: run `omega --check` over the repository's programs and
//! group where each one stops.
//!
//! A full sweep checks every tracked `main.omg` under `samples/` and
//! `tests/fixtures/` (about 9,000 programs, about an hour on one workstation).
//! Two smaller selections answer most questions in minutes:
//!
//! - `--quick` keeps every program outside `samples/cli/facts/` and, for each
//!   fact, one positive program and one negative twin. A fact's places nearly
//!   always stop at the same cause, so this shows every cause and its trend at
//!   a fifth of the cost.
//! - `--cause <text> --previous <sweep.tsv>` re-checks only the programs whose
//!   cause in an earlier sweep contains `<text>`: did a fix land, and what
//!   stops them now.
//! - `--changed <rev> [--previous <sweep.tsv>]` checks the change's blast
//!   zone since `<rev>`: programs whose own folder holds a changed file,
//!   programs whose path dependencies (followed to the end) hold a changed
//!   `.omg` file, every program when the toolchain's `core` or `std` changed
//!   (every check reads them), and, when the compiler
//!   changed, the earlier sweep's blocked programs whose error contains a
//!   message on the diff's changed lines plus every program that checks or is
//!   rejected as it should be (a regression guard). `--list` prints the
//!   selection without checking it.
//!
//! Each program's row records where it stopped and the error that stopped it,
//! with names, paths and numbers left out (`cause`) so one cause groups every
//! program it stops. A program under a `fail/` folder or a folder whose name
//! starts with `invalid` should be rejected; every other program should check.
//! Rows are written as tab-separated text, or JSON when `--out` ends in
//! `.json`, and a summary by outcome, stage and cause is printed.
//!
//! `--target <name>` checks for that target rather than the host's, and
//! `--shard K/N` (or `A-B/N`) checks one share of the selection, so one
//! target's sweep can be split across machines. With `--out`, each program
//! is journalled as it finishes and the same command resumes an interrupted
//! run ([`record`]). `merge` joins a run's shards and refuses any set that
//! is not exactly one row per selected program; `fleet` starts, watches and
//! collects such a run on several machines ([`fleet`]).

mod fleet;
mod record;

use record::{Header, Journal, Shard, Status};
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

const FACTS_ROOT: &str = "samples/cli/facts/";
const DEFAULT_TIMEOUT_SECONDS: u64 = 900;

const USAGE: &str = "\
usage: cargo xtask sweep [--quick | --cause <text> --previous <sweep.tsv|sweep.json>
                          | --changed <rev> [--previous <sweep.tsv|sweep.json>]]
                         [--filter <fragment>] [--target <name>] [--shard K/N | A-B/N]
                         [--omega <path>] [--jobs <n>] [--timeout <seconds>]
                         [--out <file.tsv|file.json>] [--list]
       cargo xtask sweep merge --out <file.json> <shard.json>...
       cargo xtask sweep fleet <start|status|collect|restart> ...  (see `sweep fleet --help`)";

#[derive(Debug, Clone, PartialEq, Eq)]
struct Options {
    quick: bool,
    cause: Option<String>,
    changed: Option<String>,
    previous: Option<PathBuf>,
    filter: Option<String>,
    target: Option<String>,
    shard: Shard,
    omega: Option<PathBuf>,
    jobs: usize,
    timeout: Duration,
    out: Option<PathBuf>,
    list: bool,
}

fn parse(args: &[String]) -> Result<Options, String> {
    let mut options = Options {
        quick: false,
        cause: None,
        changed: None,
        previous: None,
        filter: None,
        target: None,
        shard: Shard::WHOLE,
        omega: None,
        jobs: 10,
        timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECONDS),
        out: None,
        list: false,
    };
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        let mut value = |name: &str| {
            args.next()
                .cloned()
                .ok_or_else(|| format!("{name} needs a value\n{USAGE}"))
        };
        match arg.as_str() {
            "--quick" => options.quick = true,
            "--cause" => options.cause = Some(value("--cause")?),
            "--changed" => options.changed = Some(value("--changed")?),
            "--previous" => options.previous = Some(PathBuf::from(value("--previous")?)),
            "--filter" => options.filter = Some(value("--filter")?),
            "--target" => options.target = Some(value("--target")?),
            "--shard" => {
                options.shard = Shard::parse(&value("--shard")?)
                    .map_err(|error| format!("{error}\n{USAGE}"))?;
            }
            "--omega" => options.omega = Some(PathBuf::from(value("--omega")?)),
            "--jobs" => {
                options.jobs = value("--jobs")?
                    .parse()
                    .ok()
                    .filter(|jobs| *jobs > 0)
                    .ok_or_else(|| format!("--jobs needs a positive number\n{USAGE}"))?;
            }
            "--timeout" => {
                let seconds: u64 = value("--timeout")?
                    .parse()
                    .map_err(|_| format!("--timeout needs a number of seconds\n{USAGE}"))?;
                options.timeout = Duration::from_secs(seconds);
            }
            "--out" => options.out = Some(PathBuf::from(value("--out")?)),
            "--list" => options.list = true,
            "-h" | "--help" => return Err(USAGE.to_owned()),
            other => return Err(format!("unknown argument `{other}`\n{USAGE}")),
        }
    }
    if options.cause.is_some() && options.previous.is_none() {
        return Err(format!("--cause needs --previous <sweep.tsv>\n{USAGE}"));
    }
    if options.previous.is_some() && options.cause.is_none() && options.changed.is_none() {
        return Err(format!(
            "--previous goes with --cause or --changed\n{USAGE}"
        ));
    }
    let selections = [
        options.quick,
        options.cause.is_some(),
        options.changed.is_some(),
    ];
    if selections.iter().filter(|chosen| **chosen).count() > 1 {
        return Err(format!(
            "--quick, --cause and --changed select differently; pass one\n{USAGE}"
        ));
    }
    Ok(options)
}

/// Whether a program's path says it should be rejected: it sits under a
/// `fail/` folder or a folder whose name starts with `invalid` (a negative
/// twin). Only `--quick` picks programs by this; a checked row reads the
/// program's own expectations ([`expectation`]).
fn expects_rejection(program: &str) -> bool {
    program
        .split('/')
        .any(|part| part == "fail" || part.starts_with("invalid"))
}

/// The fact a program belongs to, when it is under `samples/cli/facts/`.
fn fact_of(program: &str) -> Option<&str> {
    program.strip_prefix(FACTS_ROOT)?.split('/').next()
}

/// `--quick`: every program outside the facts set, and per fact the first
/// positive and the first negative program in path order.
fn quick_selection(programs: &[String]) -> Vec<String> {
    let mut kept: BTreeMap<(&str, bool), &String> = BTreeMap::new();
    let mut selected = Vec::new();
    for program in programs {
        match fact_of(program) {
            None => selected.push(program.clone()),
            Some(fact) => {
                kept.entry((fact, expects_rejection(program)))
                    .or_insert(program);
            }
        }
    }
    selected.extend(kept.into_values().cloned());
    selected.sort();
    selected
}

/// The programs to check: tracked `main.omg` files under `samples/` and
/// `tests/fixtures/`, as directories relative to the repository root.
fn tracked_programs(root: &Path) -> Result<Vec<String>, String> {
    let output = Command::new("git")
        .args([
            "ls-files",
            "--",
            "samples/*main.omg",
            "tests/fixtures/*main.omg",
        ])
        .current_dir(root)
        .output()
        .map_err(|error| format!("git ls-files: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git ls-files failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let mut programs: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect();
    programs.sort();
    Ok(programs)
}

/// One checked program.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Row {
    program: String,
    exit: i32,
    ms: u128,
    outcome: &'static str,
    stage: &'static str,
    cause: String,
    detail: String,
    /// Its own files say `--check` must refuse it.
    reject: bool,
}

/// The error that stopped a program: the first error indented under the
/// compiler's summary line, else the summary line itself.
fn first_error(output: &str) -> (String, String) {
    let top = output
        .lines()
        .find(|line| line.starts_with("error:") || line.starts_with("cannot compile"))
        .unwrap_or("")
        .to_owned();
    let nested = output
        .lines()
        .find(|line| {
            line.starts_with(char::is_whitespace) && line.trim_start().starts_with("error:")
        })
        .map(|line| {
            line.trim_start()
                .trim_start_matches("error:")
                .trim()
                .to_owned()
        });
    let cause = nested.unwrap_or_else(|| top.trim_start_matches("error:").trim().to_owned());
    let cause = cause
        .strip_prefix("project source discovery failed:")
        .map(|rest| rest.trim().to_owned())
        .unwrap_or(cause);
    (top, cause)
}

/// The error with names, paths and numbers left out.
fn pattern(error: &str) -> String {
    let mut out = String::with_capacity(error.len());
    let mut quoted = false;
    let mut chars = error.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '`' {
            if !quoted {
                out.push_str("`…`");
            }
            quoted = !quoted;
        } else if quoted {
            continue;
        } else if c.is_ascii_digit() {
            while chars.peek().is_some_and(char::is_ascii_digit) {
                chars.next();
            }
            out.push('N');
        } else {
            out.push(c);
        }
    }
    let out = out
        .split("; the checker does not support this construct yet")
        .next()
        .unwrap_or("")
        .split_whitespace()
        .filter(|word| !(word.contains(":\\") || word.starts_with("\\\\?\\")))
        .collect::<Vec<_>>()
        .join(" ");
    out.chars().take(170).collect()
}

/// Where a program stopped, from its summary line and its first error.
fn stage_of(top: &str, cause: &str) -> &'static str {
    let has = |needle: &str| cause.contains(needle);
    if top.contains("project source discovery failed") {
        "Build program"
    } else if has("root binding") && has("is ambiguous") {
        "Package roots"
    } else if (cause.starts_with("import ") && has("does not select"))
        || has("duplicate const")
        || has("ambiguous membership")
        || has("resolves to no declaration")
    {
        "Name resolution"
    } else if cause.starts_with("not supported yet:") {
        "Checker coverage"
    } else if has("no product route yet") {
        "Product route"
    } else if has("expected one of") || has("unexpected token") {
        "Parsing"
    } else if has("cast") || has("domain") || has("does not fit") || has("mismatch") {
        "Typing"
    } else if has("cannot prove")
        || has("unestablished")
        || has("not provably")
        || has("may overflow")
        || has("conflict")
        || has("borrow")
        || has("terminat")
    {
        "Checker verdict"
    } else {
        "Other"
    }
}

/// A row for a program expected to check, as most are; tests use it.
#[cfg(test)]
fn row(program: &str, exit: i32, ms: u128, output: &str) -> Row {
    judged(program, exit, ms, output, output, &Expectation::Check)
}

/// Where a program stopped and whether that is what its own files ask for.
/// A negative program earns "Rejected as expected" only when the refusal is
/// the one it pins, judged as `sample-gate` judges it; a different verdict,
/// or a negative that pins nothing, is "Rejected for another reason".
fn judged(
    program: &str,
    exit: i32,
    ms: u128,
    output: &str,
    diagnostics: &str,
    expected: &Expectation,
) -> Row {
    let reject = matches!(expected, Expectation::Reject(_));
    if exit == 0 {
        return Row {
            program: program.to_owned(),
            exit,
            ms,
            outcome: if reject {
                "Accepted a wrong program"
            } else {
                "Checks"
            },
            stage: "Checks",
            cause: String::new(),
            detail: String::new(),
            reject,
        };
    }
    let (top, cause) = first_error(output);
    let stage = stage_of(&top, &cause);
    let outcome = match expected {
        Expectation::Check => "Blocked",
        Expectation::Reject(project) => {
            let pinned = project.as_ref().map_err(String::clone).and_then(|project| {
                crate::samples::judge_rejection(project, Some(exit), diagnostics)
            });
            match pinned {
                Ok(()) => "Rejected as expected",
                Err(_) if matches!(stage, "Checker verdict" | "Typing" | "Parsing") => {
                    "Rejected for another reason"
                }
                Err(_) => "Blocked",
            }
        }
    };
    Row {
        program: program.to_owned(),
        exit,
        ms,
        outcome,
        stage,
        reject,
        cause: pattern(&cause),
        detail: cause.chars().take(300).collect(),
    }
}

/// `omega --check [--target <name>] <program>` with stdin closed, at idle
/// priority where the host offers one, killed after `timeout`.
fn check(omega: &Path, root: &Path, target: Option<&str>, program: &str, timeout: Duration) -> Row {
    let started = Instant::now();
    let mut command = Command::new(omega);
    command.arg("--check");
    if let Some(target) = target {
        command.arg("--target").arg(target);
    }
    command
        .arg(program)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const IDLE_PRIORITY_CLASS: u32 = 0x0000_0040;
        command.creation_flags(IDLE_PRIORITY_CLASS);
    }
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            let failed = format!("error: cannot start omega: {error}");
            return judged(program, -1, 0, &failed, &failed, &Expectation::Check);
        }
    };
    // Drain both pipes on their own threads so a chatty child never blocks.
    let drain = |pipe: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut text = String::new();
            if let Some(mut pipe) = pipe {
                let _ = pipe.read_to_string(&mut text);
            }
            text
        })
    };
    let stdout = drain(
        child
            .stdout
            .take()
            .map(|pipe| Box::new(pipe) as Box<dyn Read + Send>),
    );
    let stderr = drain(
        child
            .stderr
            .take()
            .map(|pipe| Box::new(pipe) as Box<dyn Read + Send>),
    );
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if started.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(25)),
            Err(_) => break None,
        }
    };
    let mut output = stdout.join().unwrap_or_default();
    let diagnostics = stderr.join().unwrap_or_default();
    output.push_str(&diagnostics);
    let ms = started.elapsed().as_millis();
    let expected = expectation(root, program);
    match status {
        Some(status) => judged(
            program,
            status.code().unwrap_or(-1),
            ms,
            &output,
            &diagnostics,
            &expected,
        ),
        None => {
            let timed_out = format!("error: timed out after {} seconds", timeout.as_secs());
            judged(program, -1, ms, &timed_out, &timed_out, &expected)
        }
    }
}

/// What a program's own files say `--check` must do with it.
#[derive(Debug, Clone)]
enum Expectation {
    Check,
    /// Refuse it for the reason its `expected.txt` or `sample.json` pins; the
    /// error says why its expectations could not be read.
    Reject(Result<Box<crate::samples::SampleProject>, String>),
}

/// The sample gate's reading of the program's folder, so both tools agree on
/// which programs are negative and what each must be refused for.
fn expectation(root: &Path, program: &str) -> Expectation {
    let folder = root.join(record::short(program));
    match crate::samples::project_at(&folder, root) {
        Ok(project) if project.role == "intended-invalid" => {
            Expectation::Reject(Ok(Box::new(project)))
        }
        Ok(_) => Expectation::Check,
        // Unreadable expectations never earn a negative credit.
        Err(error) if expects_rejection(program) => Expectation::Reject(Err(error)),
        Err(_) => Expectation::Check,
    }
}

/// One row of an earlier sweep's tab-separated output.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Previous {
    program: String,
    outcome: String,
    cause: String,
    detail: String,
}

fn read_previous(previous: &Path) -> Result<Vec<Previous>, String> {
    let content =
        fs::read_to_string(previous).map_err(|error| format!("{}: {error}", previous.display()))?;
    if previous
        .extension()
        .is_some_and(|extension| extension == "json")
    {
        let (_, rows) = record::parse_output(&content)
            .map_err(|error| format!("{}: {error}", previous.display()))?;
        return Ok(rows
            .into_iter()
            .map(|row| Previous {
                program: row.program,
                outcome: row.outcome.to_owned(),
                cause: row.cause,
                detail: row.detail,
            })
            .collect());
    }
    Ok(content
        .lines()
        .filter(|line| !line.starts_with('#') && !line.starts_with("program\t"))
        .filter_map(|line| {
            let fields: Vec<&str> = line.split('\t').collect();
            (fields.len() >= 7).then(|| Previous {
                program: fields[0].to_owned(),
                outcome: fields[3].to_owned(),
                cause: fields[5].to_owned(),
                detail: fields[6].to_owned(),
            })
        })
        .collect())
}

/// The programs an earlier sweep recorded with a cause containing `text`.
fn previous_with_cause(previous: &[Previous], text: &str) -> Vec<String> {
    let mut programs: Vec<String> = previous
        .iter()
        .filter(|row| row.cause.contains(text))
        .map(|row| row.program.clone())
        .collect();
    programs.sort();
    programs.dedup();
    programs
}

/// Why a program is in a change's blast zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Reason {
    /// A file in its own folder changed.
    OwnSource,
    /// Its `build.omg` names a path dependency the change touched.
    Dependency,
    /// The toolchain's `core` or `std`, which every check reads, changed.
    Library,
    /// It was blocked by an error message the change touched.
    Message,
    /// It checks or is rejected as it should be; re-checked to catch a
    /// regression.
    Guard,
}

impl Reason {
    fn label(self) -> &'static str {
        match self {
            Self::OwnSource => "a file in its own folder changed",
            Self::Dependency => "a package it depends on, directly or not, changed",
            Self::Library => "the toolchain core or std it reads changed",
            Self::Message => "it was blocked by an error message the change touched",
            Self::Guard => "it checks or is rejected as expected (regression guard)",
        }
    }
}

/// The repository paths changed between `rev` and the working tree.
fn changed_files(root: &Path, rev: &str) -> Result<Vec<String>, String> {
    let output = Command::new("git")
        // Without rename detection a moved file names both its old folder,
        // whose program no longer compiles it, and its new one.
        .args([
            "diff",
            "--name-only",
            "--no-renames",
            &format!("{rev}...HEAD"),
        ])
        .current_dir(root)
        .output()
        .map_err(|error| format!("git diff: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git diff {rev}...HEAD failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect())
}

/// String literals on the changed lines of the compiler sources in the diff,
/// split at format placeholders: the error messages a change touched.
fn changed_messages(root: &Path, rev: &str) -> Result<Vec<String>, String> {
    let output = Command::new("git")
        .args([
            "diff",
            "--unified=0",
            &format!("{rev}...HEAD"),
            "--",
            "*.rs",
        ])
        .current_dir(root)
        .output()
        .map_err(|error| format!("git diff: {error}"))?;
    Ok(messages_in(&String::from_utf8_lossy(&output.stdout)))
}

/// The message fragments on a unified diff's changed lines.
fn messages_in(diff: &str) -> Vec<String> {
    let mut messages = Vec::new();
    for line in diff.lines() {
        let changed = (line.starts_with('-') && !line.starts_with("---"))
            || (line.starts_with('+') && !line.starts_with("+++"));
        if !changed {
            continue;
        }
        for literal in line.split('"').skip(1).step_by(2) {
            for fragment in literal.split(['{', '}']) {
                let fragment = fragment.trim();
                // Short fragments ("x", "N") would match every program.
                if fragment.len() >= 12 && fragment.contains(' ') {
                    messages.push(fragment.to_owned());
                }
            }
        }
    }
    messages.sort();
    messages.dedup();
    messages
}

/// Collapse `a/b/../c` without touching the filesystem.
fn normal(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." if parts.last().is_some_and(|last| *last != "..") => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    parts.join("/")
}

/// The repository paths a program's `build.omg` names as path dependencies
/// (`location: "<relative path>"`), resolved against the program's folder.
fn direct_dependencies(root: &Path, folder: &str) -> Vec<String> {
    let Ok(build) = fs::read_to_string(root.join(folder).join("build.omg")) else {
        return Vec::new();
    };
    build
        .split("location:")
        .skip(1)
        .filter_map(|rest| rest.split('"').nth(1))
        .filter(|location| location.starts_with('.'))
        .map(|location| normal(&format!("{folder}/{location}")))
        .collect()
}

/// The package folders a program's check reads besides its own: its path
/// dependencies, and theirs, to the end. With the program's own folder (every
/// `.omg` beneath a package root is its source, `LogicalSourceInventory` in
/// `package-compilation`) and the toolchain's `core` and `std`, these are the
/// source closure `omega --check` compiles.
fn dependency_roots(root: &Path, program: &str) -> Vec<String> {
    let folder = program.trim_end_matches("main.omg").trim_end_matches('/');
    let mut found: Vec<String> = Vec::new();
    let mut pending = direct_dependencies(root, folder);
    while let Some(dependency) = pending.pop() {
        if found.contains(&dependency) {
            continue;
        }
        pending.extend(direct_dependencies(root, &dependency));
        found.push(dependency);
    }
    found.sort();
    found
}

/// Whether a changed library file is in a library every check reads: the
/// toolchain supplies `core` and `std` to every program, declared or not;
/// any other library arrives only through a declared dependency.
fn toolchain_library(path: &str) -> bool {
    path.starts_with("source/library/core/") || path.starts_with("source/library/std/")
}

/// The blast zone of the change from `rev`: each program the change can
/// affect, with why.
fn blast_zone(
    root: &Path,
    rev: &str,
    programs: &[String],
    previous: Option<&[Previous]>,
) -> Result<BTreeMap<String, Reason>, String> {
    let changed = changed_files(root, rev)?;
    let mut zone: BTreeMap<String, Reason> = BTreeMap::new();
    let mut add = |program: &str, reason: Reason| {
        let entry = zone.entry(program.to_owned()).or_insert(reason);
        *entry = (*entry).min(reason);
    };
    let compiler = changed.iter().any(|path| {
        path.starts_with("omega-rust/")
            || path == "Cargo.toml"
            || path == "Cargo.lock"
            || path == "rust-toolchain.toml"
    });
    let library = changed
        .iter()
        .any(|path| toolchain_library(path) && path.ends_with(".omg"));
    if library {
        for program in programs {
            add(program, Reason::Library);
        }
    }
    let sources: Vec<&String> = changed
        .iter()
        // Only Omega source reaches a check: a README or an expected-output
        // file beside a program changes nothing `omega --check` reads. A
        // changed `core` or `std` file already selected every program.
        .filter(|path| path.ends_with(".omg") && !toolchain_library(path))
        .collect();
    if !sources.is_empty() {
        for program in programs {
            let folder = program.trim_end_matches("main.omg");
            if sources.iter().any(|path| path.starts_with(folder)) {
                add(program, Reason::OwnSource);
                continue;
            }
            let roots = dependency_roots(root, program);
            if sources
                .iter()
                .any(|path| roots.iter().any(|dep| path.starts_with(&format!("{dep}/"))))
            {
                add(program, Reason::Dependency);
            }
        }
    }
    if compiler {
        let previous = previous.ok_or(
            "the change touches the compiler; pass --previous <sweep.tsv> so the zone can name the programs it targets and guard",
        )?;
        let messages = changed_messages(root, rev)?;
        for row in previous {
            if row.outcome != "Blocked" {
                add(&row.program, Reason::Guard);
            } else if messages
                .iter()
                .any(|message| row.detail.contains(message.as_str()))
            {
                add(&row.program, Reason::Message);
            }
        }
    }
    Ok(zone)
}

fn escape_tsv(text: &str) -> String {
    text.replace(['\t', '\n', '\r'], " ")
}

/// Writes rows as JSON when `path` ends in `.json`, else as tab-separated
/// text; both carry the run's header.
fn render_rows(path: &Path, header: &Header, rows: &[Row]) -> Result<(), String> {
    let text = if path
        .extension()
        .is_some_and(|extension| extension == "json")
    {
        let rows: Vec<serde_json::Value> = rows.iter().map(record::row_json).collect();
        serde_json::to_string(&serde_json::json!({
            "commit": header.commit,
            "run": header,
            "programs": rows,
        }))
        .map_err(|error| error.to_string())?
    } else {
        let mut text = format!(
            "# commit {} target {} shard {} selection {} ({} programs)\n\
             program\texit\tms\toutcome\tstage\tcause\tdetail\n",
            header.commit,
            if header.target.is_empty() {
                "host"
            } else {
                &header.target
            },
            header.shard.label(),
            header.fingerprint,
            header.selection,
        );
        for row in rows {
            text.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                row.program,
                row.exit,
                row.ms,
                row.outcome,
                row.stage,
                escape_tsv(&row.cause),
                escape_tsv(&row.detail),
            ));
        }
        text
    };
    fs::write(path, text).map_err(|error| format!("{}: {error}", path.display()))
}

fn summary(rows: &[Row]) -> String {
    let mut text = String::new();
    let count = |key: fn(&Row) -> &str, filter: fn(&Row) -> bool| {
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for row in rows.iter().filter(|row| filter(row)) {
            *counts.entry(key(row).to_owned()).or_default() += 1;
        }
        let mut counts: Vec<_> = counts.into_iter().collect();
        counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        counts
    };
    text.push_str(&format!("programs: {}\n\noutcome:\n", rows.len()));
    for (outcome, n) in count(|row| row.outcome, |_| true) {
        text.push_str(&format!("{n:>7}  {outcome}\n"));
    }
    text.push_str("\nblocked, by stage:\n");
    for (stage, n) in count(|row| row.stage, |row| row.outcome == "Blocked") {
        text.push_str(&format!("{n:>7}  {stage}\n"));
    }
    text.push_str("\nblocked, top causes:\n");
    for (cause, n) in count(|row| &row.cause, |row| row.outcome == "Blocked")
        .into_iter()
        .take(25)
    {
        text.push_str(&format!("{n:>7}  {cause}\n"));
    }
    let elsewhere = count(
        |row| &row.cause,
        |row| row.outcome == "Rejected for another reason",
    );
    if !elsewhere.is_empty() {
        text.push_str("\nnegatives refused for another reason than they pin, top causes:\n");
        for (cause, n) in elsewhere.into_iter().take(15) {
            text.push_str(&format!("{n:>7}  {cause}\n"));
        }
    }
    let wrong: Vec<_> = rows
        .iter()
        .filter(|row| row.outcome == "Accepted a wrong program")
        .collect();
    if !wrong.is_empty() {
        text.push_str("\naccepted though they should be rejected:\n");
        for row in wrong {
            text.push_str(&format!("  {}\n", row.program));
        }
    }
    text
}

pub fn run(args: &[String]) -> Result<(), u8> {
    match args.first().map(String::as_str) {
        Some("merge") => return merge_command(&args[1..]),
        Some("fleet") => return fleet::run(&args[1..]),
        _ => {}
    }
    let options = parse(args).map_err(|message| {
        eprintln!("{message}");
        2
    })?;
    let root = crate::repo_root()?;
    let fail = |message: String| {
        eprintln!("xtask sweep: {message}");
        1
    };
    let omega = options.omega.clone().unwrap_or_else(|| {
        let exe = if cfg!(windows) { "omega.exe" } else { "omega" };
        root.join("target").join("debug").join(exe)
    });
    if !omega.is_file() {
        return Err(fail(format!(
            "no omega binary at {}; build it with `mbx build -p omega` or pass --omega",
            omega.display()
        )));
    }
    let previous = match &options.previous {
        Some(path) => Some(read_previous(path).map_err(fail)?),
        None => None,
    };
    let tracked = tracked_programs(&root).map_err(fail)?;
    let mut programs = if let Some(cause) = &options.cause {
        let previous = previous.as_deref().unwrap_or_default();
        previous_with_cause(previous, cause)
    } else if let Some(rev) = &options.changed {
        let zone = blast_zone(&root, rev, &tracked, previous.as_deref()).map_err(fail)?;
        let mut reasons: BTreeMap<Reason, usize> = BTreeMap::new();
        for reason in zone.values() {
            *reasons.entry(*reason).or_default() += 1;
        }
        eprintln!("xtask sweep: blast zone of {rev}...HEAD:");
        for (reason, n) in &reasons {
            eprintln!("{n:>7}  {}", reason.label());
        }
        // A program the change removed has nothing left to check.
        zone.into_keys()
            .filter(|program| tracked.binary_search(program).is_ok())
            .collect()
    } else {
        tracked
    };
    if options.quick {
        programs = quick_selection(&programs);
    }
    if let Some(filter) = &options.filter {
        programs.retain(|program| program.contains(filter.as_str()));
    }
    let selection = programs.len();
    let fingerprint = record::fingerprint(programs.iter().map(String::as_str));
    let programs: Vec<String> = programs
        .into_iter()
        .enumerate()
        .filter(|(index, _)| options.shard.holds(*index))
        .map(|(_, program)| program)
        .collect();
    if options.list {
        for program in &programs {
            println!("{program}");
        }
        eprintln!(
            "xtask sweep: {} programs selected (shard {} of a selection of {selection}, {fingerprint})",
            programs.len(),
            options.shard.label()
        );
        return Ok(());
    }
    let header = Header {
        commit: head_commit(&root).map_err(fail)?,
        target: options.target.clone().unwrap_or_default(),
        shard: options.shard,
        selection,
        fingerprint,
        total: programs.len(),
    };
    sweep(&options, &root, &omega, &header, &programs).map_err(|message| {
        if let Some(out) = &options.out {
            let _ = record::write_status(
                out,
                &Status {
                    state: "failed".to_owned(),
                    total: header.total,
                    updated: record::now(),
                    detail: message.clone(),
                    ..Status::default()
                },
            );
        }
        fail(message)
    })
}

/// An error that says the run itself is wrong, not the program: every other
/// program would fail the same way.
fn run_wide_refusal(row: &Row) -> Option<String> {
    let run_wide = ["unknown target profile", "cannot start omega"];
    run_wide
        .iter()
        .any(|needle| row.detail.contains(needle))
        .then(|| format!("stopped at {}: {}", record::short(&row.program), row.detail))
}

fn head_commit(root: &Path) -> Result<String, String> {
    let output = Command::new("git")
        .args(["rev-parse", "--short=10", "HEAD"])
        .current_dir(root)
        .output()
        .map_err(|error| format!("git rev-parse: {error}"))?;
    let commit = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if !output.status.success() || commit.is_empty() {
        return Err("git rev-parse HEAD failed".to_owned());
    }
    Ok(commit)
}

/// Checks `programs`, journalling each row when there is an `--out`, and
/// writes the verified output.
fn sweep(
    options: &Options,
    root: &Path,
    omega: &Path,
    header: &Header,
    programs: &[String],
) -> Result<(), String> {
    // Open the journal before checking anything, so an unwritable `--out`
    // fails in the first second rather than after the last program.
    let (journal, earlier) = match &options.out {
        Some(out) => {
            let (journal, earlier) = Journal::open(out, header)?;
            (Some(journal), earlier)
        }
        None => (None, Vec::new()),
    };
    let finished: std::collections::BTreeSet<&str> =
        earlier.iter().map(|row| row.program.as_str()).collect();
    let todo: Vec<&String> = programs
        .iter()
        .filter(|program| !finished.contains(program.as_str()))
        .collect();
    eprintln!(
        "xtask sweep: {} programs at {}{}, shard {}, {} at a time{}",
        header.total,
        header.commit,
        if header.target.is_empty() {
            String::new()
        } else {
            format!(" for {}", header.target)
        },
        header.shard.label(),
        options.jobs,
        if earlier.is_empty() {
            String::new()
        } else {
            format!("; resuming, {} already checked", earlier.len())
        },
    );
    let started = Instant::now();
    let status = |done: usize, state: &str| -> Result<(), String> {
        match &options.out {
            Some(out) => record::write_status(
                out,
                &Status {
                    state: state.to_owned(),
                    done,
                    total: header.total,
                    elapsed: started.elapsed().as_secs(),
                    updated: record::now(),
                    detail: String::new(),
                },
            ),
            None => Ok(()),
        }
    };
    status(earlier.len(), "running")?;

    struct Progress {
        rows: Vec<Row>,
        journal: Option<Journal>,
        error: Option<String>,
    }
    let progress = Mutex::new(Progress {
        rows: earlier,
        journal,
        error: None,
    });
    let next = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for _ in 0..options.jobs.min(todo.len().max(1)) {
            scope.spawn(|| {
                loop {
                    if progress.lock().expect("progress").error.is_some() {
                        break;
                    }
                    let index = next.fetch_add(1, Ordering::SeqCst);
                    let Some(program) = todo.get(index) else {
                        break;
                    };
                    let row = check(
                        omega,
                        root,
                        options.target.as_deref(),
                        program,
                        options.timeout,
                    );
                    let mut progress = progress.lock().expect("progress");
                    // A misspelled `--target` fails every program the same
                    // way; stop rather than record thousands of such rows.
                    if let Some(refusal) = run_wide_refusal(&row) {
                        progress.error.get_or_insert(refusal);
                        break;
                    }
                    let written = match &mut progress.journal {
                        Some(journal) => journal.append(&row),
                        None => Ok(()),
                    };
                    progress.rows.push(row);
                    let done = progress.rows.len();
                    let reported = written.and_then(|()| {
                        if done.is_multiple_of(10) {
                            status(done, "running")
                        } else {
                            Ok(())
                        }
                    });
                    if let Err(error) = reported {
                        progress.error.get_or_insert(error);
                    }
                    if done.is_multiple_of(250) || done == header.total {
                        eprintln!(
                            "xtask sweep: {done}/{} in {}s",
                            header.total,
                            started.elapsed().as_secs()
                        );
                    }
                }
            });
        }
    });
    let Progress {
        mut rows,
        journal,
        error,
    } = progress.into_inner().expect("progress");
    if let Some(error) = error {
        return Err(error);
    }
    rows.sort_by(|a, b| a.program.cmp(&b.program));
    if let Some(out) = &options.out {
        record::write_output(out, header, &rows, render_rows)?;
        if let Some(journal) = journal {
            journal.remove()?;
        }
        status(rows.len(), "finished")?;
    }
    print!("{}", summary(&rows));
    println!(
        "\n{} programs in {}s at {}",
        rows.len(),
        started.elapsed().as_secs(),
        header.commit
    );
    if let Some(out) = &options.out {
        println!("wrote {} rows to {}", rows.len(), out.display());
    }
    Ok(())
}

/// `sweep merge --out <file.json> <shard.json>...`
fn merge_command(args: &[String]) -> Result<(), u8> {
    let usage = "usage: cargo xtask sweep merge --out <file.json> <shard.json>...";
    let mut out = None;
    let mut inputs = Vec::new();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--out" => out = args.next().map(PathBuf::from),
            "-h" | "--help" => {
                eprintln!("{usage}");
                return Err(2);
            }
            other => inputs.push(PathBuf::from(other)),
        }
    }
    let Some(out) = out.filter(|_| !inputs.is_empty()) else {
        eprintln!("{usage}");
        return Err(2);
    };
    let fail = |message: String| {
        eprintln!("xtask sweep merge: {message}");
        1
    };
    let (header, rows) = merge_files(&inputs).map_err(fail)?;
    record::write_output(&out, &header, &rows, render_rows).map_err(fail)?;
    print!("{}", summary(&rows));
    println!(
        "\nmerged {} shards into {} rows at {}; wrote {}",
        inputs.len(),
        rows.len(),
        header.commit,
        out.display()
    );
    Ok(())
}

fn merge_files(inputs: &[PathBuf]) -> Result<(Header, Vec<Row>), String> {
    let mut parts = Vec::with_capacity(inputs.len());
    for path in inputs {
        let text =
            fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
        let (header, rows) =
            record::parse_output(&text).map_err(|error| format!("{}: {error}", path.display()))?;
        parts.push((path.clone(), header, rows));
    }
    record::merge(&parts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn negatives_are_fail_folders_and_invalid_twins() {
        assert!(expects_rejection(
            "tests/fixtures/fail/arithmetic/x/main.omg"
        ));
        assert!(expects_rejection(
            "samples/cli/facts/f/invalid_local/main.omg"
        ));
        assert!(expects_rejection(
            "samples/cli/facts/f/controls/c/invalid/main.omg"
        ));
        assert!(!expects_rejection("samples/cli/facts/f/local/main.omg"));
    }

    #[test]
    fn quick_keeps_one_positive_and_one_negative_per_fact() {
        let programs: Vec<String> = [
            "samples/cli/algorithms/sort/main.omg",
            "samples/cli/facts/a/field/main.omg",
            "samples/cli/facts/a/invalid_field/main.omg",
            "samples/cli/facts/a/invalid_local/main.omg",
            "samples/cli/facts/a/local/main.omg",
            "samples/cli/facts/b/main.omg",
        ]
        .map(str::to_owned)
        .into();
        assert_eq!(
            quick_selection(&programs),
            [
                "samples/cli/algorithms/sort/main.omg",
                "samples/cli/facts/a/field/main.omg",
                "samples/cli/facts/a/invalid_field/main.omg",
                "samples/cli/facts/b/main.omg",
            ]
        );
    }

    #[test]
    fn a_cause_groups_by_its_words_not_its_names() {
        let output = "cannot compile admitted project: checked compilation failed for package `x` with 1 diagnostic(s)\n  error: duplicate const `C1`\n    in package `p`, \\\\?\\C:\\repo\\main.omg:5:7\n";
        let (top, cause) = first_error(output);
        assert_eq!(cause, "duplicate const `C1`");
        assert_eq!(stage_of(&top, &cause), "Name resolution");
        assert_eq!(pattern(&cause), "duplicate const `…`");
        assert_eq!(
            pattern(
                "not supported yet: type float; the checker does not support this construct yet, so the program is refused"
            ),
            "not supported yet: type float"
        );
    }

    #[test]
    fn a_build_failure_is_the_build_stage() {
        let output = "error: project source discovery failed: \n  error: not supported yet: call late-bound selection; the checker does not support this construct yet\n";
        let row = row("samples/cli/x/main.omg", 1, 10, output);
        assert_eq!(row.stage, "Build program");
        assert_eq!(row.cause, "not supported yet: call late-bound selection");
        assert_eq!(row.outcome, "Blocked");
    }

    #[test]
    fn a_negative_earns_credit_only_for_the_refusal_it_pins() {
        let program = "samples/cli/facts/f/invalid_local/main.omg";
        let pinning = |fragments: &[&str]| {
            Expectation::Reject(Ok(Box::new(crate::samples::SampleProject {
                directory: PathBuf::from("samples/cli/facts/f/invalid_local"),
                rel_path: "samples/cli/facts/f/invalid_local".to_owned(),
                entry: Some(PathBuf::from("main.omg")),
                role: "intended-invalid".to_owned(),
                source_inputs: Vec::new(),
                rejection: fragments.iter().map(|text| (*text).to_owned()).collect(),
                rejection_any: Vec::new(),
                rejection_casefold: false,
                native_rejection: None,
            })))
        };
        let overflow = "cannot compile\n  error: exact arithmetic may overflow `u64`\n";
        let judge = |output: &str, exit: i32, expected: &Expectation| {
            judged(program, exit, 10, output, output, expected)
        };

        let pinned = pinning(&["may overflow `u64`"]);
        assert_eq!(judge(overflow, 1, &pinned).outcome, "Rejected as expected");
        assert!(judge(overflow, 1, &pinned).reject);
        assert_eq!(
            judge("compiled", 0, &pinned).outcome,
            "Accepted a wrong program"
        );

        // The twin exists for a borrow error; an earlier overflow proves nothing.
        let borrow = pinning(&["while local borrow `held` is still active"]);
        assert_eq!(
            judge(overflow, 1, &borrow).outcome,
            "Rejected for another reason"
        );
        // A refusal the checker cannot judge yet is still a blocker.
        let unsupported = "cannot compile\n  error: not supported yet: type generic application\n";
        assert_eq!(judge(unsupported, 1, &borrow).outcome, "Blocked");
        // A negative with nothing pinned, or unreadable pins, earns nothing.
        assert_eq!(
            judge(overflow, 1, &pinning(&[])).outcome,
            "Rejected for another reason"
        );
        let unreadable = Expectation::Reject(Err("sample.json: bad".to_owned()));
        assert_eq!(
            judge(overflow, 1, &unreadable).outcome,
            "Rejected for another reason"
        );
        // A crash is not a normal rejection, even when its text matches.
        assert_eq!(
            judge(overflow, 101, &pinned).outcome,
            "Rejected for another reason"
        );
    }

    #[test]
    fn expectations_come_from_the_programs_own_files() {
        let root = crate::repo_root().unwrap();
        let twin = "samples/cli/facts/abi_multi_arg_spill/invalid_call_result/main.omg";
        let Expectation::Reject(Ok(project)) = expectation(&root, twin) else {
            panic!("{twin} pins its refusal in sample.json");
        };
        assert_eq!(project.rejection, ["may overflow `u64`"]);
        let positive = "samples/cli/facts/abi_multi_arg_spill/call_result/main.omg";
        assert!(matches!(expectation(&root, positive), Expectation::Check));
        let fail = "tests/fixtures/fail/arithmetic/exact_divisor_unproven/main.omg";
        let Expectation::Reject(Ok(project)) = expectation(&root, fail) else {
            panic!("{fail} pins its refusal in expected.txt");
        };
        assert!(!project.rejection.is_empty());
    }

    #[test]
    fn selections_take_one_mode_and_the_earlier_sweep_they_need() {
        let args = |list: &[&str]| list.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>();
        assert!(parse(&args(&["--cause", "x"])).is_err());
        assert!(parse(&args(&["--cause", "x", "--previous", "a.tsv"])).is_ok());
        assert!(parse(&args(&["--changed", "origin/main"])).is_ok());
        assert!(parse(&args(&["--changed", "origin/main", "--previous", "a.tsv"])).is_ok());
        assert!(parse(&args(&["--previous", "a.tsv"])).is_err());
        assert!(parse(&args(&["--quick", "--changed", "origin/main"])).is_err());
        assert!(parse(&args(&["--quick"])).is_ok());
    }

    #[test]
    fn a_path_dependency_resolves_from_its_program() {
        assert_eq!(
            normal("samples/cli/facts/f/invalid_local/../../../../../tests/support/x"),
            "tests/support/x"
        );
        assert_eq!(normal("a/./b//c/../d"), "a/b/d");
    }

    #[test]
    fn dependencies_are_followed_to_the_end() {
        // app depends on lib, lib on base, and base back on lib: the closure
        // is both packages, once each.
        let root = std::env::temp_dir().join(format!("sweep-deps-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let build = |folder: &str, location: &str| {
            fs::create_dir_all(root.join(folder)).unwrap();
            fs::write(
                root.join(folder).join("build.omg"),
                format!("Source::Path {{ location: \"{location}\" }}"),
            )
            .unwrap();
        };
        build("samples/app", "../../packages/lib");
        build("packages/lib", "../base");
        build("packages/base", "../lib");
        let roots = dependency_roots(&root, "samples/app/main.omg");
        let _ = fs::remove_dir_all(&root);
        assert_eq!(roots, ["packages/base", "packages/lib"]);
    }

    #[test]
    fn only_core_and_std_reach_every_program() {
        assert!(toolchain_library("source/library/core/layout.omg"));
        assert!(toolchain_library("source/library/std/console.omg"));
        assert!(!toolchain_library("source/library/alloc/vec.omg"));
        assert!(!toolchain_library("samples/cli/facts/f/local/main.omg"));
    }

    #[test]
    fn a_diff_names_the_messages_it_touched() {
        let diff = "\
--- a/x.rs
+++ b/x.rs
@@ -1 +1 @@
-    return unserved(\"call late-bound selection\");
+    return Err(format!(\"no product route yet: provider settlement for `{name}`: no plan\"));
     unchanged(\"a message on an unchanged line\");
+    let short = \"x\";
";
        assert_eq!(
            messages_in(diff),
            [
                "call late-bound selection",
                "no product route yet: provider settlement for `",
            ]
        );
    }

    #[test]
    fn a_misspelled_target_stops_the_run_not_one_program() {
        let refused = row(
            "samples/a/main.omg",
            1,
            1,
            "error: unknown target profile `linx`; expected linux_x86_64
",
        );
        assert!(run_wide_refusal(&refused).unwrap().contains("samples/a"));
        let blocked = row(
            "samples/a/main.omg",
            1,
            1,
            "error: not supported yet: x
",
        );
        assert!(run_wide_refusal(&blocked).is_none());
    }

    #[test]
    fn the_previous_sweep_reads_back_its_own_rows() {
        let rows = [row(
            "samples/cli/x/main.omg",
            1,
            5,
            "error: project source discovery failed: \n  error: not supported yet: call late-bound selection\n",
        )];
        let path = std::env::temp_dir().join(format!("sweep-{}.tsv", std::process::id()));
        let header = Header {
            commit: "abc".to_owned(),
            target: String::new(),
            shard: Shard::WHOLE,
            selection: 1,
            fingerprint: record::fingerprint(["samples/cli/x/main.omg"]),
            total: 1,
        };
        render_rows(&path, &header, &rows).unwrap();
        let read = read_previous(&path).unwrap();
        let _ = fs::remove_file(&path);
        assert_eq!(read.len(), 1);
        assert_eq!(read[0].outcome, "Blocked");
        assert_eq!(
            previous_with_cause(&read, "late-bound"),
            ["samples/cli/x/main.omg"]
        );
    }
}
