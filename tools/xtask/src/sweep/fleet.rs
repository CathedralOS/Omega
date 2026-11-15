//! `cargo xtask sweep fleet`: split one target's sweep across several
//! machines, watch it, and bring the verified whole back.
//!
//! A full sweep takes about an hour on one machine. On 2026-11-14 one was run
//! by hand across four machines and lost most of an hour to mistakes a
//! launcher should never make: a wrapper swallowed one host's output files,
//! a scheduled task that only ran while its owner was signed in never
//! started, a hand-written job script wrote every shard to the same broken
//! path, and two hosts each checked all 9,097 programs for their own target
//! rather than sharing one target's work. `fleet` replaces those steps:
//!
//! - `start` checks out one pushed commit on every host, builds there, and
//!   runs `xtask sweep --target <t> --shard <units>` with each host's share
//!   weighted by its speed. The job scripts are generated here, from one
//!   template per shell, never typed per host. `start` waits until every job
//!   has written its first status and names any host that did not.
//! - `status [--watch <seconds>]` reads each host's status file: state,
//!   progress, rate and time left, and flags a job whose status stopped
//!   changing.
//! - `collect` copies every shard back and merges them, refusing a set that is
//!   not exactly one row per selected program.
//! - `restart` re-launches unfinished shards; each resumes from its journal.
//!
//! Hosts live in a file owned by the machine's user, by default
//! `~/omega-agent/fleet.toml` (`--config` or `OMEGA_FLEET` to override):
//!
//! ```toml
//! [[host]]
//! name = "pc"                 # this machine when `ssh` is absent
//! os = "windows"              # windows | unix
//! checkout = "C:/Users/me/omega-agent/fleet"  # a clone used only by fleet
//! results = "C:/Users/me/omega-agent/fleet-results"
//! jobs = 12
//! weight = 3                  # share of the work, relative to other hosts
//! env = { MBX_SCHEDULER_CPUS = "12" }
//!
//! [[host]]
//! name = "linux"
//! ssh = "omega-linux"
//! os = "unix"
//! checkout = "/home/me/omega-agent/fleet"
//! results = "/home/me/omega-agent/fleet-results"
//! lock = "/home/me/omega-agent/heavy.lock"   # the VM's one-heavy-job lock
//! jobs = 10
//! weight = 2
//! ```
//!
//! A job takes the host's `lock` (the same directory `vm-heavy` uses) for its
//! whole build and sweep, and waits, reporting `waiting`, while another job
//! holds it. Each job runs at the lowest operating-system priority. Runs are
//! recorded under `~/omega-agent/fleet-runs/` so `status`, `collect` and
//! `restart` need no arguments for the latest run.

use super::record::{self, Header, Status};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const USAGE: &str = "\
usage: cargo xtask sweep fleet start --target <name> [--commit <rev>] [--hosts <a,b,...>]
                                     [--quick] [--filter <fragment>] [--changed <rev>]
                                     [--timeout <seconds>] [--config <fleet.toml>]
       cargo xtask sweep fleet status [<run>] [--watch <seconds>]
       cargo xtask sweep fleet collect [<run>] [--out <file.json>]
       cargo xtask sweep fleet restart [<run>] [--host <name>]...

<run> defaults to the latest run started from this machine. Hosts are read from
~/omega-agent/fleet.toml (or --config, or $OMEGA_FLEET); see the module docs in
tools/xtask/src/sweep/fleet.rs for its form.";

/// How long `start` waits for every job's first status.
const START_GRACE: Duration = Duration::from_secs(120);
/// A running job whose status is older than this is reported as stale.
const STALE_SECONDS: u64 = 15 * 60;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
enum Os {
    Windows,
    Unix,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct HostConfig {
    name: String,
    /// The `ssh` destination; absent for this machine.
    #[serde(default)]
    ssh: Option<String>,
    os: Os,
    checkout: String,
    results: String,
    #[serde(default)]
    lock: Option<String>,
    jobs: usize,
    #[serde(default = "one")]
    weight: usize,
    #[serde(default)]
    env: std::collections::BTreeMap<String, String>,
}

fn one() -> usize {
    1
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FleetConfig {
    host: Vec<HostConfig>,
}

/// One host's part of a run.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Assignment {
    host: HostConfig,
    shard: record::Shard,
    /// The shard's output on the host; its journal and status sit beside it.
    out: String,
    /// The run's folder on the host: job script, logs and output.
    dir: String,
}

/// A run as `start` recorded it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct Run {
    id: String,
    commit: String,
    target: String,
    /// The selection flags every host's sweep was given.
    selection: Vec<String>,
    origin: String,
    assignments: Vec<Assignment>,
}

fn home() -> Result<PathBuf, String> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .map(PathBuf::from)
        .ok_or_else(|| "no home folder in the environment".to_owned())
}

fn runs_dir() -> Result<PathBuf, String> {
    Ok(home()?.join("omega-agent").join("fleet-runs"))
}

fn load_config(path: Option<&Path>) -> Result<Vec<HostConfig>, String> {
    let path = match path {
        Some(path) => path.to_owned(),
        None => match std::env::var_os("OMEGA_FLEET") {
            Some(path) => PathBuf::from(path),
            None => home()?.join("omega-agent").join("fleet.toml"),
        },
    };
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("{}: {error} (see `sweep fleet --help`)", path.display()))?;
    let config: FleetConfig =
        toml::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))?;
    validate_hosts(&config.host).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(config.host)
}

fn validate_hosts(hosts: &[HostConfig]) -> Result<(), String> {
    if hosts.is_empty() {
        return Err("no [[host]] entries".to_owned());
    }
    for (index, host) in hosts.iter().enumerate() {
        if hosts[..index].iter().any(|other| other.name == host.name) {
            return Err(format!("host `{}` is named twice", host.name));
        }
        if host.jobs == 0 || host.weight == 0 {
            return Err(format!(
                "host `{}` needs positive jobs and weight",
                host.name
            ));
        }
        let bad_name = |text: &str| {
            text.is_empty()
                || !text
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        };
        if bad_name(&host.name) || host.env.keys().any(|key| bad_name(key)) {
            return Err(format!(
                "host `{}`: names and env keys use letters, digits, `_` and `-`",
                host.name
            ));
        }
    }
    Ok(())
}

/// Splits `count` units between hosts by weight, in host order.
fn split(hosts: &[HostConfig]) -> Vec<record::Shard> {
    let count = hosts.iter().map(|host| host.weight).sum();
    let mut first = 1;
    hosts
        .iter()
        .map(|host| {
            let shard = record::Shard {
                first,
                last: first + host.weight - 1,
                count,
            };
            first += host.weight;
            shard
        })
        .collect()
}

fn join(os: &Os, base: &str, name: &str) -> String {
    let separator = match os {
        Os::Windows => '\\',
        Os::Unix => '/',
    };
    let base = match os {
        Os::Windows => base.replace('/', "\\"),
        Os::Unix => base.to_owned(),
    };
    format!("{}{separator}{name}", base.trim_end_matches(['/', '\\']))
}

fn windows_path(path: &str) -> String {
    path.replace('/', "\\")
}

fn sh_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

fn ps_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

/// Quotes one argument for `cmd.exe`, refusing what it cannot carry.
fn cmd_quote(text: &str) -> Result<String, String> {
    if text
        .chars()
        .any(|c| matches!(c, '"' | '%' | '^' | '&' | '|' | '<' | '>' | '!'))
    {
        return Err(format!(
            "`{text}` holds a character cmd.exe would reinterpret"
        ));
    }
    Ok(format!("\"{text}\""))
}

/// The sweep arguments a job runs with, shared by both script templates.
fn sweep_args(run: &Run, assignment: &Assignment) -> Vec<String> {
    let mut args = run.selection.clone();
    args.extend([
        "--target".to_owned(),
        run.target.clone(),
        "--shard".to_owned(),
        assignment.shard.label(),
        "--jobs".to_owned(),
        assignment.host.jobs.to_string(),
        "--out".to_owned(),
        assignment.out.clone(),
    ]);
    args
}

/// The job script for a POSIX host.
fn unix_job(run: &Run, assignment: &Assignment) -> String {
    let host = &assignment.host;
    let q = |text: &str| sh_quote(text);
    let mut script = format!(
        "#!/bin/sh\n\
         # Written by `cargo xtask sweep fleet` for run {id}; `fleet restart` runs it again.\n\
         set -u\n\
         export PATH=\"$HOME/.cargo/bin:$PATH\"\n\
         out={out}\n\
         dir={dir}\n\
         st() {{\n\
         \x20 detail=$(printf '%s' \"$2\" | tr -d '\"\\\\' | tr '\\n' ' ')\n\
         \x20 printf '{{\"state\":\"%s\",\"detail\":\"%s\",\"updated\":%s}}\\n' \"$1\" \"$detail\" \"$(date +%s)\" > \"$out.status.tmp\" && mv \"$out.status.tmp\" \"$out.status\"\n\
         }}\n",
        id = run.id,
        out = q(&assignment.out),
        dir = q(&assignment.dir),
    );
    if let Some(lock) = &host.lock {
        script.push_str(&format!(
            "lock={lock}\n\
             while ! mkdir \"$lock\" 2>/dev/null; do\n\
             \x20 st waiting \"lock held by $(cat \"$lock/owner\" 2>/dev/null)\"\n\
             \x20 sleep 60\n\
             done\n\
             echo \"sweep fleet {id}, since $(date '+%m-%d %H:%M')\" > \"$lock/owner\"\n\
             trap 'rm -rf \"$lock\"' EXIT\n\
             trap 'exit 130' HUP INT TERM\n",
            lock = q(lock),
            id = run.id,
        ));
    }
    script.push_str(&format!(
        "if [ ! -e {checkout}/.git ]; then\n\
         \x20 st fetching \"cloning {origin}\"\n\
         \x20 git clone -q {origin_q} {checkout} || {{ st failed \"cannot clone {origin} into {checkout_plain}\"; exit 1; }}\n\
         fi\n\
         cd {checkout} || {{ st failed \"no checkout at {checkout_plain}\"; exit 1; }}\n\
         st fetching \"\"\n\
         if [ -n \"$(git status --porcelain --untracked-files=no)\" ]; then st failed \"{checkout_plain} has local changes; fleet only uses a clean clone\"; exit 1; fi\n\
         git fetch -q origin {commit} && git checkout -q --detach {commit} || {{ st failed \"cannot check out {commit}\"; exit 1; }}\n",
        checkout = q(&host.checkout),
        checkout_plain = host.checkout,
        origin = run.origin,
        origin_q = q(&run.origin),
        commit = run.commit,
    ));
    for (key, value) in &host.env {
        script.push_str(&format!("export {key}={}\n", q(value)));
    }
    let args: Vec<String> = sweep_args(run, assignment)
        .iter()
        .map(|arg| q(arg))
        .collect();
    script.push_str(&format!(
        "export CARGO_TARGET_DIR=\"$PWD/target\" CARGO_PROFILE_DEV_OPT_LEVEL=1\n\
         st building \"\"\n\
         nice -n 19 mbx build -p omega -p xtask > \"$dir/build.log\" 2>&1 < /dev/null || {{ st failed \"build failed; see $dir/build.log\"; exit 1; }}\n\
         nice -n 19 target/debug/xtask sweep {args} --omega target/debug/omega > \"$dir/sweep.log\" 2>&1 < /dev/null\n\
         code=$?\n\
         if [ $code -ne 0 ] && ! grep -q '\"state\":\"failed\"' \"$out.status\" 2>/dev/null; then st failed \"sweep exited $code; see $dir/sweep.log\"; fi\n\
         exit $code\n",
        args = args.join(" "),
    ));
    script
}

/// The job script for a Windows host, in PowerShell. Native commands run
/// through `cmd /c` so their output is redirected byte for byte.
fn windows_job(run: &Run, assignment: &Assignment) -> Result<String, String> {
    let host = &assignment.host;
    let q = |text: &str| ps_quote(text);
    let checkout = windows_path(&host.checkout);
    let dir = windows_path(&assignment.dir);
    let mut script = format!(
        "# Written by `cargo xtask sweep fleet` for run {id}; `fleet restart` runs it again.\n\
         $out = {out}\n\
         $dir = {dir}\n\
         function St($state, $detail) {{\n\
         \x20 $json = @{{ state = $state; detail = \"$detail\"; updated = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds() }} | ConvertTo-Json -Compress\n\
         \x20 [IO.File]::WriteAllText(\"$out.status.tmp\", $json + \"`n\")\n\
         \x20 Move-Item -Force \"$out.status.tmp\" \"$out.status\"\n\
         }}\n",
        id = run.id,
        out = q(&windows_path(&assignment.out)),
        dir = q(&dir),
    );
    let locked = host.lock.is_some();
    if let Some(lock) = &host.lock {
        script.push_str(&format!(
            "$lock = {lock}\n\
             while ($true) {{\n\
             \x20 try {{ New-Item -ItemType Directory $lock -ErrorAction Stop | Out-Null; break }}\n\
             \x20 catch {{ St 'waiting' (\"lock held by \" + (Get-Content \"$lock\\owner\" -Raw -ErrorAction SilentlyContinue)); Start-Sleep 60 }}\n\
             }}\n\
             Set-Content \"$lock\\owner\" \"sweep fleet {id}, since $(Get-Date -Format 'MM-dd HH:mm')\"\n",
            lock = q(&windows_path(lock)),
            id = run.id,
        ));
    }
    let mut args = Vec::new();
    for arg in sweep_args(run, assignment) {
        args.push(cmd_quote(&windows_path_if_out(&arg, assignment))?);
    }
    // Inside a PowerShell double-quoted string, `"` is written `""`... the
    // cmd line is built as a single-quoted PowerShell string instead.
    let build_line = format!(
        "mbx build -p omega -p xtask > {} 2>&1 < NUL",
        cmd_quote(&format!("{dir}\\build.log"))?
    );
    let sweep_line = format!(
        "target\\debug\\xtask.exe sweep {} --omega target\\debug\\omega.exe > {} 2>&1 < NUL",
        args.join(" "),
        cmd_quote(&format!("{dir}\\sweep.log"))?
    );
    let mut body = format!(
        "if (-not (Test-Path {checkout_git})) {{\n\
         \x20 St 'fetching' {cloning}\n\
         \x20 cmd /c {clone}\n\
         \x20 if ($LASTEXITCODE) {{ St 'failed' {clone_failed}; exit 1 }}\n\
         }}\n\
         Set-Location {checkout}\n\
         St 'fetching' ''\n\
         if (git status --porcelain --untracked-files=no) {{ St 'failed' {dirty}; exit 1 }}\n\
         git fetch -q origin {commit}\n\
         if ($LASTEXITCODE) {{ St 'failed' {no_fetch}; exit 1 }}\n\
         git checkout -q --detach {commit}\n\
         if ($LASTEXITCODE) {{ St 'failed' {no_commit}; exit 1 }}\n",
        checkout_git = q(&format!("{checkout}\\.git")),
        checkout = q(&checkout),
        cloning = q(&format!("cloning {}", run.origin)),
        clone = q(&format!(
            "git clone -q {} {}",
            cmd_quote(&run.origin)?,
            cmd_quote(&checkout)?
        )),
        clone_failed = q(&format!("cannot clone {} into {checkout}", run.origin)),
        dirty = q(&format!(
            "{checkout} has local changes; fleet only uses a clean clone"
        )),
        commit = run.commit,
        no_commit = q(&format!("cannot check out {}", run.commit)),
        no_fetch = q(&format!("cannot fetch {} from origin", run.commit)),
    );
    for (key, value) in &host.env {
        body.push_str(&format!("$env:{key} = {}\n", q(value)));
    }
    body.push_str(&format!(
        "$env:CARGO_TARGET_DIR = \"$PWD\\target\"\n\
         $env:CARGO_PROFILE_DEV_OPT_LEVEL = '1'\n\
         St 'building' ''\n\
         cmd /c {build}\n\
         if ($LASTEXITCODE) {{ St 'failed' \"build failed; see $dir\\build.log\"; exit 1 }}\n\
         cmd /c {sweep}\n\
         $code = $LASTEXITCODE\n\
         if ($code -and -not (Select-String -Quiet -SimpleMatch '\"state\":\"failed\"' \"$out.status\")) {{ St 'failed' \"sweep exited $code; see $dir\\sweep.log\" }}\n\
         exit $code\n",
        build = q(&build_line),
        sweep = q(&sweep_line),
    ));
    if locked {
        script.push_str("try {\n");
        script.push_str(&body);
        script.push_str(
            "} finally { Remove-Item -Recurse -Force $lock -ErrorAction SilentlyContinue }\n",
        );
    } else {
        script.push_str(&body);
    }
    Ok(script)
}

fn windows_path_if_out(arg: &str, assignment: &Assignment) -> String {
    if arg == assignment.out {
        windows_path(arg)
    } else {
        arg.to_owned()
    }
}

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for (position, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            if position <= chunk.len() {
                out.push(BASE64[(n >> shift) as usize & 63] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// A command for the host's shell, with optional bytes on its stdin.
fn host_command(host: &HostConfig, script: &str) -> Command {
    let (program, args): (&str, Vec<String>) = match host.os {
        Os::Unix => ("sh", vec!["-c".to_owned(), script.to_owned()]),
        Os::Windows => {
            let utf16: Vec<u8> = script
                .encode_utf16()
                .flat_map(|unit| unit.to_le_bytes())
                .collect();
            (
                "powershell",
                vec![
                    "-NoProfile".to_owned(),
                    "-NonInteractive".to_owned(),
                    "-EncodedCommand".to_owned(),
                    base64(&utf16),
                ],
            )
        }
    };
    match &host.ssh {
        None => {
            let mut command = Command::new(program);
            command.args(&args);
            command
        }
        Some(destination) => {
            let mut command = Command::new("ssh");
            command.args([
                "-o",
                "BatchMode=yes",
                "-o",
                "ConnectTimeout=15",
                "-o",
                "LogLevel=ERROR",
                destination,
            ]);
            // ssh joins its arguments with spaces for the remote shell.
            let remote = match host.os {
                Os::Unix => sh_quote(script),
                Os::Windows => format!("{program} {}", args.join(" ")),
            };
            match host.os {
                Os::Unix => command.arg(format!("sh -c {remote}")),
                Os::Windows => command.arg(remote),
            };
            command
        }
    }
}

/// Runs `script` on the host and returns its stdout.
fn on_host(host: &HostConfig, script: &str, stdin: Option<&[u8]>) -> Result<Vec<u8>, String> {
    let mut command = host_command(host, script);
    command
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| format!("{}: cannot start: {error}", host.name))?;
    if let (Some(bytes), Some(mut pipe)) = (stdin, child.stdin.take()) {
        pipe.write_all(bytes)
            .map_err(|error| format!("{}: {error}", host.name))?;
    }
    let output = child
        .wait_with_output()
        .map_err(|error| format!("{}: {error}", host.name))?;
    if !output.status.success() {
        return Err(format!(
            "{}: exit {}: {}",
            host.name,
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(output.stdout)
}

/// Reads a file on the host byte for byte; `None` when it does not exist.
fn read_on_host(host: &HostConfig, path: &str) -> Result<Option<Vec<u8>>, String> {
    let script = match host.os {
        Os::Unix => format!(
            "if [ -f {p} ]; then printf Y; cat {p}; else printf N; fi",
            p = sh_quote(path)
        ),
        Os::Windows => format!(
            "$p = {p}; $o = [Console]::OpenStandardOutput()\n\
             if (Test-Path $p) {{ $o.WriteByte(89); $b = [IO.File]::ReadAllBytes($p); $o.Write($b, 0, $b.Length) }} else {{ $o.WriteByte(78) }}\n\
             $o.Flush()",
            p = ps_quote(&windows_path(path))
        ),
    };
    let bytes = on_host(host, &script, None)?;
    match bytes.split_first() {
        Some((b'Y', rest)) => Ok(Some(rest.to_vec())),
        Some((b'N', _)) => Ok(None),
        _ => Err(format!("{}: unexpected reply reading {path}", host.name)),
    }
}

/// Writes the job script on the host and starts it detached at the lowest
/// priority, so it outlives this command and the ssh session.
fn launch(run: &Run, assignment: &Assignment) -> Result<(), String> {
    let host = &assignment.host;
    match host.os {
        Os::Unix => {
            let job = format!("{}/job.sh", assignment.dir);
            let script = format!(
                "mkdir -p {dir} && cat > {job} && rm -f {status} && \
                 (nohup nice -n 19 sh {job} > {log} 2>&1 < /dev/null &)",
                dir = sh_quote(&assignment.dir),
                job = sh_quote(&job),
                status = sh_quote(&format!("{}.status", assignment.out)),
                log = sh_quote(&format!("{}/job.log", assignment.dir)),
            );
            on_host(host, &script, Some(unix_job(run, assignment).as_bytes()))?;
        }
        Os::Windows => {
            let dir = windows_path(&assignment.dir);
            let job = format!("{dir}\\job.ps1");
            let log = format!("{dir}\\job.log");
            let command_line = format!(
                "cmd /c \"powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File \"{job}\" > \"{log}\" 2>&1\""
            );
            // Win32_Process.Create starts the job outside this session; Idle
            // priority (64) is inherited by every process the job starts.
            let script = format!(
                "$ErrorActionPreference = 'Stop'\n\
                 New-Item -ItemType Directory -Force {dir} | Out-Null\n\
                 [IO.File]::WriteAllText({job}, [Console]::In.ReadToEnd())\n\
                 Remove-Item -Force -ErrorAction SilentlyContinue {status}\n\
                 $startup = New-CimInstance -ClassName Win32_ProcessStartup -ClientOnly -Property @{{ PriorityClass = [uint32]64 }}\n\
                 $result = Invoke-CimMethod -ClassName Win32_Process -MethodName Create -Arguments @{{ CommandLine = {line}; ProcessStartupInformation = $startup }}\n\
                 if ($result.ReturnValue -ne 0) {{ throw \"Win32_Process.Create returned $($result.ReturnValue)\" }}",
                dir = ps_quote(&dir),
                job = ps_quote(&job),
                status = ps_quote(&format!("{}.status", windows_path(&assignment.out))),
                line = ps_quote(&command_line),
            );
            on_host(
                host,
                &script,
                Some(windows_job(run, assignment)?.as_bytes()),
            )?;
        }
    }
    Ok(())
}

fn read_status(assignment: &Assignment) -> Result<Option<Status>, String> {
    let path = format!("{}.status", assignment.out);
    match read_on_host(&assignment.host, &path)? {
        None => Ok(None),
        Some(bytes) => {
            let text = String::from_utf8_lossy(&bytes);
            let text = text.trim_start_matches('\u{feff}').trim();
            serde_json::from_str(text)
                .map(Some)
                .map_err(|error| format!("{}: bad status `{text}`: {error}", assignment.host.name))
        }
    }
}

fn settled(status: &Status) -> bool {
    matches!(status.state.as_str(), "finished" | "failed")
}

/// One host's line in `status`.
fn describe(assignment: &Assignment, status: &Result<Option<Status>, String>) -> String {
    let name = &assignment.host.name;
    let shard = assignment.shard.label();
    match status {
        Err(error) => format!("{name:<8} {shard:<8} unreachable: {error}"),
        Ok(None) => format!("{name:<8} {shard:<8} not started (no status file)"),
        Ok(Some(status)) => {
            let age = record::now().saturating_sub(status.updated);
            let mut line = format!("{name:<8} {shard:<8} {}", status.state);
            if status.total > 0 {
                line.push_str(&format!(" {}/{}", status.done, status.total));
                if status.state == "running" && status.done > 0 && status.elapsed > 0 {
                    let rate = status.done as f64 / status.elapsed as f64;
                    let left = (status.total - status.done) as f64 / rate;
                    line.push_str(&format!(
                        ", {:.1}/s, about {} min left",
                        rate,
                        (left / 60.0).ceil() as u64
                    ));
                }
            }
            if !status.detail.is_empty() {
                // A lock owner written on Windows ends in a line break.
                let detail: Vec<&str> = status.detail.split_whitespace().collect();
                line.push_str(&format!(": {}", detail.join(" ")));
            }
            if !settled(status) && status.updated > 0 && age > STALE_SECONDS {
                line.push_str(&format!(
                    "  STALE: no update for {} min; see {}",
                    age / 60,
                    assignment.dir
                ));
            }
            line
        }
    }
}

/// `YYYYMMDD-HHMM` in UTC.
fn stamp(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64;
    let minutes = (seconds % 86_400) / 60;
    // Civil-from-days (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}{month:02}{day:02}-{:02}{:02}",
        minutes / 60,
        minutes % 60
    )
}

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("git {}: {error}", args.join(" ")))?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// The full commit `rev` names here, or, for a branch this clone does not
/// fetch (`origin/work/x`), the commit origin has under that name.
fn resolve_commit(root: &Path, rev: &str) -> Result<String, String> {
    if let Ok(commit) = git(
        root,
        &["rev-parse", "--verify", &format!("{rev}^{{commit}}")],
    ) {
        return Ok(commit);
    }
    let branch = rev.strip_prefix("origin/").unwrap_or(rev);
    let listed = git(
        root,
        &["ls-remote", "origin", &format!("refs/heads/{branch}")],
    )?;
    listed
        .split_whitespace()
        .next()
        .filter(|commit| commit.len() == 40)
        .map(str::to_owned)
        .ok_or_else(|| format!("`{rev}` names no commit here and no branch on origin"))
}

fn save_run(run: &Run) -> Result<PathBuf, String> {
    let dir = runs_dir()?;
    fs::create_dir_all(&dir).map_err(|error| format!("{}: {error}", dir.display()))?;
    let path = dir.join(format!("{}.json", run.id));
    let text = serde_json::to_string_pretty(run).map_err(|error| error.to_string())?;
    fs::write(&path, text).map_err(|error| format!("{}: {error}", path.display()))?;
    Ok(path)
}

fn load_run(id: Option<&str>) -> Result<Run, String> {
    let dir = runs_dir()?;
    let path = match id {
        Some(id) => dir.join(format!("{}.json", id.trim_end_matches(".json"))),
        None => {
            let mut runs: Vec<PathBuf> = fs::read_dir(&dir)
                .map_err(|error| format!("{}: {error}; no run started yet", dir.display()))?
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| {
                    path.extension()
                        .is_some_and(|extension| extension == "json")
                })
                .collect();
            // Ids start with their UTC start time, so the name orders them.
            runs.sort();
            runs.pop()
                .ok_or_else(|| format!("no run recorded in {}", dir.display()))?
        }
    };
    let text = fs::read_to_string(&path).map_err(|error| format!("{}: {error}", path.display()))?;
    serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))
}

#[derive(Debug)]
struct StartOptions {
    target: String,
    commit: String,
    hosts: Option<Vec<String>>,
    selection: Vec<String>,
    config: Option<PathBuf>,
}

fn parse_start(args: &[String]) -> Result<StartOptions, String> {
    let mut options = StartOptions {
        target: String::new(),
        commit: "origin/main".to_owned(),
        hosts: None,
        selection: Vec::new(),
        config: None,
    };
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        let mut value = |name: &str| {
            args.next()
                .cloned()
                .ok_or_else(|| format!("{name} needs a value\n{USAGE}"))
        };
        match arg.as_str() {
            "--target" => options.target = value("--target")?,
            "--commit" => options.commit = value("--commit")?,
            "--hosts" => {
                options.hosts = Some(
                    value("--hosts")?
                        .split(',')
                        .map(str::trim)
                        .filter(|name| !name.is_empty())
                        .map(str::to_owned)
                        .collect(),
                );
            }
            "--config" => options.config = Some(PathBuf::from(value("--config")?)),
            "--quick" => options.selection.push("--quick".to_owned()),
            flag @ ("--filter" | "--changed" | "--timeout") => {
                let given = value(flag)?;
                options.selection.extend([flag.to_owned(), given]);
            }
            other => {
                return Err(format!(
                    "`{other}` is not a fleet start option (`--cause`/`--previous` need a \
                     local earlier sweep; run those with plain `sweep`)\n{USAGE}"
                ));
            }
        }
    }
    if options.target.is_empty() {
        return Err(format!(
            "fleet start needs --target: every host must check for the same target\n{USAGE}"
        ));
    }
    Ok(options)
}

fn start(args: &[String]) -> Result<(), String> {
    let options = parse_start(args)?;
    let mut hosts = load_config(options.config.as_deref())?;
    if let Some(chosen) = &options.hosts {
        for name in chosen {
            if !hosts.iter().any(|host| &host.name == name) {
                return Err(format!("no host `{name}` in the fleet configuration"));
            }
        }
        hosts.retain(|host| chosen.contains(&host.name));
    }
    let root = crate::repository::current()?;
    git(&root, &["fetch", "-q", "origin"])?;
    let commit = resolve_commit(&root, &options.commit)?;
    // Every job fetches exactly this commit from origin; fetching it here
    // first proves they can, before any host is touched.
    git(&root, &["fetch", "-q", "origin", &commit]).map_err(|_| {
        format!("origin does not have {commit}; push it first so every host can fetch it")
    })?;
    let origin = git(&root, &["remote", "get-url", "origin"])?;
    let id = format!(
        "{}-{}-{}",
        stamp(record::now()),
        &commit[..10],
        options.target
    );
    let shards = split(&hosts);
    let assignments: Vec<Assignment> = hosts
        .into_iter()
        .zip(shards)
        .map(|(host, shard)| {
            let dir = join(&host.os, &host.results, &id);
            let out = join(
                &host.os,
                &dir,
                &format!(
                    "shard-{}-{}-of-{}.json",
                    shard.first, shard.last, shard.count
                ),
            );
            Assignment {
                host,
                shard,
                out,
                dir,
            }
        })
        .collect();
    let run = Run {
        id,
        commit,
        target: options.target,
        selection: options.selection,
        origin,
        assignments,
    };
    let record = save_run(&run)?;
    eprintln!(
        "fleet: run {} checks {} at {} on {} hosts; recorded in {}",
        run.id,
        run.target,
        &run.commit[..10],
        run.assignments.len(),
        record.display()
    );
    launch_all(&run, &run.assignments)
}

/// Launches each assignment, then waits for each job's first status so a job
/// that never started is named now, not discovered an hour later.
fn launch_all(run: &Run, assignments: &[Assignment]) -> Result<(), String> {
    let mut failed = Vec::new();
    let mut launched = Vec::new();
    for assignment in assignments {
        match launch(run, assignment) {
            Ok(()) => {
                eprintln!(
                    "fleet: {} launched shard {}",
                    assignment.host.name,
                    assignment.shard.label()
                );
                launched.push(assignment);
            }
            Err(error) => {
                eprintln!("fleet: {} did not launch: {error}", assignment.host.name);
                failed.push(assignment.host.name.clone());
            }
        }
    }
    let started = Instant::now();
    let mut waiting = launched;
    while !waiting.is_empty() && started.elapsed() < START_GRACE {
        std::thread::sleep(Duration::from_secs(3));
        waiting.retain(|assignment| match read_status(assignment) {
            Ok(Some(status)) => {
                eprintln!("fleet: {}", describe(assignment, &Ok(Some(status.clone()))));
                if status.state == "failed" {
                    failed.push(assignment.host.name.clone());
                }
                false
            }
            _ => true,
        });
    }
    for assignment in waiting {
        eprintln!(
            "fleet: {} wrote no status within {}s; see {}",
            assignment.host.name,
            START_GRACE.as_secs(),
            join(&assignment.host.os, &assignment.dir, "job.log")
        );
        failed.push(assignment.host.name.clone());
    }
    if failed.is_empty() {
        eprintln!(
            "fleet: every job is under way; `cargo xtask sweep fleet status {} --watch 300` reports progress",
            run.id
        );
        Ok(())
    } else {
        Err(format!(
            "not started on: {}; fix the cause and run `cargo xtask sweep fleet restart {}`",
            failed.join(", "),
            run.id
        ))
    }
}

fn status(args: &[String]) -> Result<(), String> {
    let mut id = None;
    let mut watch = None;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--watch" => {
                watch = Some(
                    args.next()
                        .and_then(|seconds| seconds.parse::<u64>().ok())
                        .filter(|seconds| *seconds > 0)
                        .ok_or_else(|| format!("--watch needs a number of seconds\n{USAGE}"))?,
                );
            }
            other if !other.starts_with('-') && id.is_none() => id = Some(other.to_owned()),
            other => return Err(format!("unknown argument `{other}`\n{USAGE}")),
        }
    }
    let run = load_run(id.as_deref())?;
    loop {
        let statuses: Vec<_> = run.assignments.iter().map(read_status).collect();
        let (mut done, mut total) = (0, 0);
        let mut all_settled = true;
        let mut lines = Vec::new();
        for (assignment, status) in run.assignments.iter().zip(&statuses) {
            if let Ok(Some(status)) = status {
                done += status.done;
                total += status.total;
                all_settled &= settled(status);
            } else {
                all_settled = false;
            }
            lines.push(describe(assignment, status));
        }
        let clock = stamp(record::now());
        println!(
            "fleet {} at {} UTC: {done}/{} checked",
            run.id,
            &clock[9..],
            if total == 0 {
                "?".to_owned()
            } else {
                total.to_string()
            }
        );
        for line in lines {
            println!("  {line}");
        }
        let _ = std::io::stdout().flush();
        match watch {
            Some(seconds) if !all_settled => std::thread::sleep(Duration::from_secs(seconds)),
            _ => break,
        }
    }
    Ok(())
}

fn collect(args: &[String]) -> Result<(), String> {
    let mut id = None;
    let mut out = None;
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--out" => out = args.next().map(PathBuf::from),
            other if !other.starts_with('-') && id.is_none() => id = Some(other.to_owned()),
            other => return Err(format!("unknown argument `{other}`\n{USAGE}")),
        }
    }
    let run = load_run(id.as_deref())?;
    let local = runs_dir()?.join(&run.id);
    fs::create_dir_all(&local).map_err(|error| format!("{}: {error}", local.display()))?;
    let mut parts = Vec::new();
    let mut problems = Vec::new();
    for assignment in &run.assignments {
        let name = &assignment.host.name;
        match read_on_host(&assignment.host, &assignment.out) {
            Ok(Some(bytes)) => {
                let path = local.join(format!(
                    "shard-{}-{}-of-{}.json",
                    assignment.shard.first, assignment.shard.last, assignment.shard.count
                ));
                fs::write(&path, &bytes).map_err(|error| format!("{}: {error}", path.display()))?;
                let text = String::from_utf8_lossy(&bytes);
                match record::parse_output(&text) {
                    Ok((header, rows)) => {
                        if let Err(error) = matches_run(&run, assignment, &header) {
                            problems.push(format!("{name}: {error}"));
                        } else {
                            parts.push((path, header, rows));
                        }
                    }
                    Err(error) => problems.push(format!("{name}: {}: {error}", path.display())),
                }
            }
            Ok(None) => {
                let state = describe(assignment, &read_status(assignment));
                problems.push(format!("{name}: no output yet ({})", state.trim()));
            }
            Err(error) => problems.push(error),
        }
    }
    if !problems.is_empty() {
        return Err(format!(
            "cannot collect run {}:\n  {}",
            run.id,
            problems.join("\n  ")
        ));
    }
    let (header, rows) = record::merge(&parts)?;
    // Beside the shards, never over the run record `<id>.json`.
    let out = out.unwrap_or_else(|| local.join("merged.json"));
    record::write_output(&out, &header, &rows, super::render_rows)?;
    print!("{}", super::summary(&rows));
    println!(
        "\nfleet: merged {} shards of run {} into {} rows for {} at {}; wrote {}",
        parts.len(),
        run.id,
        rows.len(),
        header.target,
        header.commit,
        out.display()
    );
    Ok(())
}

/// A collected shard has to be the one this run launched there.
fn matches_run(run: &Run, assignment: &Assignment, header: &Header) -> Result<(), String> {
    if !run.commit.starts_with(&header.commit) || header.commit.is_empty() {
        return Err(format!(
            "output is for commit {}, the run is at {}",
            header.commit, run.commit
        ));
    }
    if header.target != run.target {
        return Err(format!(
            "output is for target `{}`, the run checks `{}`",
            header.target, run.target
        ));
    }
    if header.shard != assignment.shard {
        return Err(format!(
            "output is shard {}, the host was given {}",
            header.shard.label(),
            assignment.shard.label()
        ));
    }
    Ok(())
}

fn restart(args: &[String]) -> Result<(), String> {
    let mut id = None;
    let mut only = Vec::new();
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--host" => only.push(
                args.next()
                    .cloned()
                    .ok_or_else(|| format!("--host needs a name\n{USAGE}"))?,
            ),
            other if !other.starts_with('-') && id.is_none() => id = Some(other.to_owned()),
            other => return Err(format!("unknown argument `{other}`\n{USAGE}")),
        }
    }
    let run = load_run(id.as_deref())?;
    for name in &only {
        if !run
            .assignments
            .iter()
            .any(|assignment| &assignment.host.name == name)
        {
            return Err(format!("run {} has no host `{name}`", run.id));
        }
    }
    let mut chosen = Vec::new();
    for assignment in &run.assignments {
        if !only.is_empty() && !only.contains(&assignment.host.name) {
            continue;
        }
        match read_status(assignment) {
            Ok(Some(status)) if status.state == "finished" => {
                eprintln!("fleet: {} already finished", assignment.host.name);
            }
            Ok(Some(status))
                if !settled(&status)
                    && record::now().saturating_sub(status.updated) <= STALE_SECONDS =>
            {
                eprintln!(
                    "fleet: {} is still {} (updated {}s ago); not restarted",
                    assignment.host.name,
                    status.state,
                    record::now().saturating_sub(status.updated)
                );
            }
            _ => chosen.push(assignment.clone()),
        }
    }
    if chosen.is_empty() {
        eprintln!("fleet: nothing to restart in run {}", run.id);
        return Ok(());
    }
    launch_all(&run, &chosen)
}

pub(super) fn run(args: &[String]) -> Result<(), u8> {
    let result = match args.first().map(String::as_str) {
        Some("start") => start(&args[1..]),
        Some("status") => status(&args[1..]),
        Some("collect") => collect(&args[1..]),
        Some("restart") => restart(&args[1..]),
        _ => {
            eprintln!("{USAGE}");
            return Err(2);
        }
    };
    result.map_err(|message| {
        eprintln!("xtask sweep fleet: {message}");
        1
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host(name: &str, os: Os, weight: usize) -> HostConfig {
        HostConfig {
            name: name.to_owned(),
            ssh: Some(format!("{name}-ssh")),
            os,
            checkout: "/home/me/omega-agent/fleet".to_owned(),
            results: "/home/me/omega-agent/fleet-results".to_owned(),
            lock: Some("/home/me/omega-agent/heavy.lock".to_owned()),
            jobs: 10,
            weight,
            env: [("MBX_CACHE_DIR".to_owned(), "/home/me/cache".to_owned())]
                .into_iter()
                .collect(),
        }
    }

    fn sample_run(os: Os) -> (Run, Assignment) {
        let mut host = host("vm", os.clone(), 1);
        if os == Os::Windows {
            host.checkout = "C:/Users/me/omega-agent/fleet".to_owned();
            host.lock = Some("C:/Users/me/omega-agent/heavy.lock".to_owned());
        }
        let assignment = Assignment {
            shard: record::Shard::parse("2-3/5").unwrap(),
            out: join(&os, "/r/run-1", "shard-2-3-of-5.json"),
            dir: join(&os, "/r", "run-1"),
            host,
        };
        let run = Run {
            id: "run-1".to_owned(),
            commit: "0123456789abcdef".to_owned(),
            target: "windows_x86_64".to_owned(),
            selection: vec!["--filter".to_owned(), "samples/cli/".to_owned()],
            origin: "https://github.com/example/omega.git".to_owned(),
            assignments: Vec::new(),
        };
        (run, assignment)
    }

    #[test]
    fn weights_split_every_unit_once_in_host_order() {
        let hosts = [
            host("a", Os::Unix, 3),
            host("b", Os::Windows, 1),
            host("c", Os::Unix, 2),
        ];
        let labels: Vec<String> = split(&hosts).iter().map(record::Shard::label).collect();
        assert_eq!(labels, ["1-3/6", "4/6", "5-6/6"]);
    }

    #[test]
    fn a_configuration_names_each_host_once() {
        let twice = [host("a", Os::Unix, 1), host("a", Os::Unix, 1)];
        assert!(validate_hosts(&twice).unwrap_err().contains("named twice"));
        let mut bad = host("a", Os::Unix, 1);
        bad.env.insert("A;rm".to_owned(), String::new());
        assert!(validate_hosts(&[bad]).is_err());
        assert!(validate_hosts(&[]).is_err());
        let text = "[[host]]\nname = \"pc\"\nos = \"windows\"\ncheckout = \"C:/c\"\n\
                    results = \"C:/r\"\njobs = 4\nsurprise = 1\n";
        assert!(toml::from_str::<FleetConfig>(text).is_err());
    }

    #[test]
    fn the_unix_job_takes_the_lock_and_runs_its_own_shard() {
        let (run, assignment) = sample_run(Os::Unix);
        let job = unix_job(&run, &assignment);
        assert!(job.contains("while ! mkdir \"$lock\""));
        assert!(job.contains("trap 'rm -rf \"$lock\"' EXIT"));
        assert!(job.contains("git checkout -q --detach 0123456789abcdef"));
        assert!(job.contains("export MBX_CACHE_DIR='/home/me/cache'"));
        assert!(job.contains(
            "'--filter' 'samples/cli/' '--target' 'windows_x86_64' '--shard' '2-3/5' \
             '--jobs' '10' '--out' '/r/run-1/shard-2-3-of-5.json'"
        ));
        assert!(job.contains("st building"));
    }

    #[test]
    fn the_windows_job_quotes_for_both_shells_and_frees_its_lock() {
        let (run, assignment) = sample_run(Os::Windows);
        let job = windows_job(&run, &assignment).unwrap();
        assert!(job.contains("$out = '\\r\\run-1\\shard-2-3-of-5.json'"));
        assert!(job.contains("New-Item -ItemType Directory $lock"));
        assert!(job.contains("finally { Remove-Item -Recurse -Force $lock"));
        assert!(job.contains("$env:MBX_CACHE_DIR = '/home/me/cache'"));
        assert!(job.contains(
            "\"--shard\" \"2-3/5\" \"--jobs\" \"10\" \"--out\" \"\\r\\run-1\\shard-2-3-of-5.json\""
        ));
        let mut hostile = run.clone();
        hostile.selection = vec!["--filter".to_owned(), "a&b".to_owned()];
        assert!(windows_job(&hostile, &assignment).is_err());
    }

    #[test]
    fn quoting_survives_the_characters_each_shell_treats_specially() {
        assert_eq!(sh_quote("it's"), r"'it'\''s'");
        assert_eq!(ps_quote("it's"), "'it''s'");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"M"), "TQ==");
    }

    #[test]
    fn a_stalled_job_is_called_stale() {
        let (_, assignment) = sample_run(Os::Unix);
        let stale = Status {
            state: "running".to_owned(),
            done: 10,
            total: 100,
            elapsed: 100,
            updated: record::now() - STALE_SECONDS - 60,
            detail: String::new(),
        };
        assert!(describe(&assignment, &Ok(Some(stale))).contains("STALE"));
        let finished = Status {
            state: "finished".to_owned(),
            done: 100,
            total: 100,
            updated: 1,
            ..Status::default()
        };
        assert!(!describe(&assignment, &Ok(Some(finished))).contains("STALE"));
        assert!(describe(&assignment, &Ok(None)).contains("not started"));
    }

    #[test]
    fn stamps_are_utc_minutes() {
        assert_eq!(stamp(0), "19700101-0000");
        // 2026-11-14 16:51 UTC.
        assert_eq!(stamp(1_794_675_060), "20261114-1651");
    }

    #[test]
    fn start_needs_one_target_and_refuses_local_only_selections() {
        assert!(parse_start(&[]).unwrap_err().contains("needs --target"));
        let previous = ["--target", "linux_x86_64", "--previous", "x.tsv"].map(String::from);
        assert!(parse_start(&previous).is_err());
        let ok = [
            "--target",
            "linux_x86_64",
            "--quick",
            "--filter",
            "samples/",
        ]
        .map(String::from);
        let options = parse_start(&ok).unwrap();
        assert_eq!(options.selection, ["--quick", "--filter", "samples/"]);
    }
}
