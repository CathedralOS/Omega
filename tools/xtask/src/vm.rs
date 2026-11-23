//! `cargo xtask vm`: start a heavy job on a LAN VM and watch it while it runs.
//!
//! Heavy builds and test runs go to `omega-linux` and `omega-win` under
//! `~/omega-agent/bin/vm-heavy`, which allows one heavy job per machine. Before
//! this command an agent copied a script over, ran it through `vm-heavy` in a
//! background ssh session, and saw nothing until the whole chain exited; its
//! log sat on the VM where only someone who sshed in and grepped it could see
//! progress. `vm` gives every job a known place and a way to look at it:
//!
//! - `start <host> <label> <script>` copies the script to
//!   `~/omega-agent/jobs/<label>/job.sh` on the VM and starts it detached under
//!   `vm-heavy <label>` at the lowest operating-system priority, with its
//!   output in `~/omega-agent/jobs/<label>/log`. It waits until `vm-heavy`
//!   either takes the machine's lock or refuses, and exits 75 when it refuses,
//!   as `vm-heavy` does. The lock and its refusal rules are `vm-heavy`'s own.
//! - `status <host> [<label>]` reports one job (state, elapsed time, `test ...
//!   ok` and `test ... FAILED` counts, `DIVERGENCE` and `error` lines grouped
//!   by text, and the last lines of output), or, without a label, the lock
//!   holder and every job on the VM.
//! - `follow <host> <label>` streams the log until the job ends, then prints
//!   the status and exits with the job's exit code. Stopping `follow` does not
//!   stop the job.
//!
//! Every remote step is a POSIX shell script sent on the stdin of
//! `ssh <host> sh -s`, so nothing is quoted through the remote login shell
//! (`cmd.exe` on `omega-win`). On Windows the job is started through
//! `Win32_Process.Create`, because Windows OpenSSH ends the processes of a
//! session when it closes.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

const USAGE: &str = "\
usage: cargo xtask vm start <host> <label> <script>
       cargo xtask vm start <host> <label> -- <command> [<arg>...]
       cargo xtask vm status <host> [<label>] [--lines <n>]
       cargo xtask vm follow <host> <label> [--lines <n> | --all]

<host> is an ssh destination such as omega-linux or omega-win. <label> names
the job and its folder, ~/omega-agent/jobs/<label>/ on the VM, which holds
job.sh and its output in log. `start` runs the job under
~/omega-agent/bin/vm-heavy, which refuses (exit 75) while another heavy job
runs there. `status` without a label lists the lock holder and every job.
`follow` streams the log until the job ends and exits with its exit code;
stopping it leaves the job running.";

/// How many lines of output `status` and `follow` show by default.
const DEFAULT_LINES: usize = 10;
/// How long `start` waits for `vm-heavy` to take the lock or refuse.
const START_WAIT_SECONDS: u32 = 60;

/// Shell functions every remote script starts with. `report` prints one job's
/// record: `key value` lines that `parse_reports` reads, and with `full`, a
/// `log` line followed by the whole log.
const REMOTE_PRELUDE: &str = r#"set -u
jobs="$HOME/omega-agent/jobs"
report() {
  d=$1
  printf 'label %s\n' "$(basename "$d")"
  printf 'path %s\n' "$d"
  printf 'now %s\n' "$(date +%s)"
  for f in started running exit; do
    if [ -f "$d/$f" ]; then printf '%s %s\n' "$f" "$(cat "$d/$f")"; fi
  done
  r=$(cat "$d/runner" 2> /dev/null) && [ -n "$r" ] && kill -0 "$r" 2> /dev/null && printf 'alive\n'
  if [ "$2" = full ]; then printf 'log\n'; cat "$d/log" 2> /dev/null; fi
  return 0
}
"#;

/// The job's runner on the VM, written beside `job.sh`. It runs the job under
/// `vm-heavy` and records when it started, when the lock was taken and how it
/// ended; its own pid tells `status` whether the job is still alive.
const RUNNER: &str = r#"#!/bin/sh
# Written by `cargo xtask vm start`: runs job.sh beside it under vm-heavy.
dir=$(cd "$(dirname "$0")" && pwd)
label=$(basename "$dir")
. "$dir/env"
echo $$ > "$dir/runner"
date +%s > "$dir/started"
cd "$HOME"
"$HOME/omega-agent/bin/vm-heavy" "$label" sh -c 'date +%s > "$1/running"; exec "$1/job.sh"' vm-job "$dir" > "$dir/log" 2>&1 < /dev/null
echo "$? $(date +%s)" > "$dir/exit"
"#;

fn sh_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', r"'\''"))
}

/// A label becomes a folder name on both operating systems and appears in
/// shell scripts, so it is kept to letters, digits, `.`, `_` and `-`.
fn validate_label(label: &str) -> Result<(), String> {
    let plain = label
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if label.is_empty() || !plain || label.starts_with(['.', '-']) {
        return Err(format!(
            "label `{label}` must be letters, digits, `.`, `_` or `-`, not starting with `.` or `-`"
        ));
    }
    Ok(())
}

/// The job script as it will run on the VM: LF line endings, a final line
/// break, and `#!/bin/sh` when it names no interpreter.
fn normalize_job(text: &str) -> String {
    let mut job = text.replace("\r\n", "\n");
    if !job.starts_with("#!") {
        job.insert_str(0, "#!/bin/sh\n");
    }
    if !job.ends_with('\n') {
        job.push('\n');
    }
    job
}

/// A job script that runs one command.
fn command_job(command: &[String]) -> String {
    let words: Vec<String> = command.iter().map(|word| sh_quote(word)).collect();
    format!(
        "#!/bin/sh\n# Written by `cargo xtask vm start`.\nexec {}\n",
        words.join(" ")
    )
}

/// A here-document delimiter that no line of `body` equals.
fn delimiter(body: &str) -> String {
    (0usize..)
        .map(|n| format!("OMEGA_VM_JOB_{n}"))
        .find(|candidate| !body.lines().any(|line| line == candidate))
        .expect("a body has finitely many lines")
}

fn heredoc(path: &str, body: &str) -> String {
    let end = delimiter(body);
    format!("cat > {path} <<'{end}'\n{body}{end}\n")
}

/// The remote script for `start`: refuse a label whose job is still alive,
/// write the job, its environment and its runner, launch the runner detached,
/// wait for `vm-heavy` to take the lock or refuse, and report.
fn start_script(label: &str, job: &str, mbx_version: Option<&str>) -> String {
    let mut script = String::from(REMOTE_PRELUDE);
    script.push_str(&format!("label={}\n", sh_quote(label)));
    script.push_str(
        r#"dir="$jobs/$label"
heavy="$HOME/omega-agent/bin/vm-heavy"
if [ ! -x "$heavy" ]; then
  echo "vm: $heavy is not installed; install it from tools/vm_heavy.sh" >&2
  exit 3
fi
r=$(cat "$dir/runner" 2> /dev/null)
if [ -n "$r" ] && [ ! -f "$dir/exit" ] && kill -0 "$r" 2> /dev/null; then
  echo "vm: job $label is still running here; follow it or choose another label" >&2
  exit 4
fi
rm -rf "$dir" && mkdir -p "$dir" || exit 1
"#,
    );
    script.push_str(&heredoc(r#""$dir/job.sh""#, job));
    script.push_str(&heredoc(r#""$dir/run.sh""#, RUNNER));
    // The job finds the pinned mbx the way the VM's hand-written jobs did.
    let mbx = match mbx_version {
        Some(version) => format!("\"$HOME/.cache/omega-mbx-bin/\"{}", sh_quote(version)),
        None => "\"\"".to_owned(),
    };
    script.push_str(&format!("mbx={mbx}\n"));
    script.push_str(&format!(
        r#"# MinGit on Windows has no chmod; MSYS runs a script by its #! line.
if command -v chmod > /dev/null 2>&1; then chmod +x "$dir/job.sh" "$dir/run.sh"; fi
q() {{ printf "'%s'" "$(printf '%s' "$1" | sed "s/'/'\\\\''/g")"; }}
path="$HOME/.cargo/bin:$PATH"
if [ -n "$mbx" ] && [ -d "$mbx" ]; then path="$mbx:$path"; fi
printf 'export HOME=%s\nexport PATH=%s\n' "$(q "$HOME")" "$(q "$path")" > "$dir/env"
case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*)
    exe=$(cd /usr/bin && pwd -W | tr / '\\')\\sh.exe
    run=$(cd "$dir" && pwd -W)/run.sh
    case "$exe$run" in
      *" "*|*"'"*) echo "vm: cannot start $run with $exe: a path holds a space or quote" >&2; exit 1 ;;
    esac
    MSYS2_ARG_CONV_EXCL='*' powershell -NoProfile -NonInteractive -Command "\$s = New-CimInstance -ClassName Win32_ProcessStartup -ClientOnly -Property @{{ PriorityClass = [uint32]64 }}; \$r = Invoke-CimMethod -ClassName Win32_Process -MethodName Create -Arguments @{{ CommandLine = '$exe $run'; ProcessStartupInformation = \$s }}; exit \$r.ReturnValue" < /dev/null > /dev/null || {{ echo "vm: Win32_Process.Create could not start $run" >&2; exit 1; }}
    ;;
  *)
    if command -v setsid > /dev/null 2>&1; then detach=setsid; else detach=nohup; fi
    ( $detach nice -n 19 sh "$dir/run.sh" > /dev/null 2>&1 < /dev/null & )
    ;;
esac
i=0
while [ ! -f "$dir/running" ] && [ ! -f "$dir/exit" ] && [ $i -lt {START_WAIT_SECONDS} ]; do
  sleep 1
  i=$((i + 1))
done
report "$dir" full
"#
    ));
    script
}

/// The remote script for `status`: one job with its log, or every job's
/// record and the lock holder.
fn status_script(label: Option<&str>) -> String {
    let mut script = String::from(REMOTE_PRELUDE);
    match label {
        Some(label) => script.push_str(&format!(
            r#"dir="$jobs"/{quoted}
if [ ! -d "$dir" ]; then echo "vm: no job {label} here (no $dir)" >&2; exit 3; fi
report "$dir" full
"#,
            quoted = sh_quote(label),
        )),
        None => script.push_str(
            r#"lock="$HOME/omega-agent/heavy.lock"
if [ -d "$lock" ]; then printf 'lock %s\n' "$(cat "$lock/owner" 2> /dev/null | tr -d '\r')"; else printf 'lock\n'; fi
for d in "$jobs"/*/; do
  if [ -d "$d" ]; then report "${d%/}" brief; fi
done
"#,
        ),
    }
    script
}

/// The remote script for `follow`: stream the log until the runner ends.
/// `tail -f` stops when its reader goes away, so the loop ends with it.
fn follow_script(label: &str, lines: Option<usize>) -> String {
    let from = lines.map_or_else(|| "+1".to_owned(), |lines| lines.to_string());
    format!(
        r#"{REMOTE_PRELUDE}dir="$jobs"/{quoted}
if [ ! -f "$dir/log" ]; then echo "vm: no job {label} here (no $dir/log)" >&2; exit 3; fi
r=$(cat "$dir/runner" 2> /dev/null)
tail -n {from} -f "$dir/log" &
t=$!
while [ ! -f "$dir/exit" ] && kill -0 "$r" 2> /dev/null && kill -0 "$t" 2> /dev/null; do
  sleep 2
done
sleep 2
kill "$t" 2> /dev/null
wait "$t" 2> /dev/null
exit 0
"#,
        quoted = sh_quote(label),
    )
}

/// One job as its folder on the VM describes it. Times are the VM's clock.
#[derive(Debug, Default, PartialEq, Eq)]
struct Report {
    label: String,
    path: String,
    now: u64,
    started: Option<u64>,
    /// When `vm-heavy` took the lock and started the job.
    running: Option<u64>,
    /// The runner's exit code and when it ended.
    exit: Option<(i32, u64)>,
    alive: bool,
    log: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
enum State {
    /// The runner is alive but `vm-heavy` has not decided yet.
    Starting,
    Running,
    /// `vm-heavy` refused: another heavy job holds the machine.
    Refused,
    Finished(i32),
    /// No exit record and no live runner: the VM restarted or the runner was
    /// killed outright.
    Lost,
}

impl Report {
    fn state(&self) -> State {
        match self.exit {
            Some((75, _)) if self.running.is_none() => State::Refused,
            Some((code, _)) => State::Finished(code),
            None if !self.alive => State::Lost,
            None if self.running.is_some() => State::Running,
            None => State::Starting,
        }
    }

    /// Seconds the job has run, or ran: from the lock (or the runner's start)
    /// to its end or to now.
    fn elapsed(&self) -> Option<u64> {
        let from = self.running.or(self.started)?;
        let to = self.exit.map_or(self.now, |(_, ended)| ended);
        Some(to.saturating_sub(from))
    }
}

fn parse_number(key: &str, value: &str) -> Result<u64, String> {
    value
        .trim()
        .parse()
        .map_err(|_| format!("bad `{key}` value `{value}` from the VM"))
}

/// Reads the records `report` prints, after an optional `lock` line. A record
/// starts at its `label` line; a `log` line makes the rest of the text that
/// record's log.
fn parse_reports(text: &str) -> Result<(Option<String>, Vec<Report>), String> {
    let mut lock = None;
    let mut reports: Vec<Report> = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        let (line, after) = rest.split_once('\n').unwrap_or((rest, ""));
        rest = after;
        let line = line.trim_end_matches('\r');
        let (key, value) = line.split_once(' ').unwrap_or((line, ""));
        if key == "lock" {
            lock = Some(value.trim().to_owned()).filter(|owner| !owner.is_empty());
            continue;
        }
        if key == "label" {
            reports.push(Report {
                label: value.to_owned(),
                ..Report::default()
            });
            continue;
        }
        let Some(report) = reports.last_mut() else {
            return Err(format!("unexpected line `{line}` from the VM"));
        };
        match key {
            "path" => value.clone_into(&mut report.path),
            "now" => report.now = parse_number(key, value)?,
            "started" => report.started = Some(parse_number(key, value)?),
            "running" => report.running = Some(parse_number(key, value)?),
            "exit" => {
                let (code, ended) = value
                    .trim()
                    .split_once(' ')
                    .ok_or_else(|| format!("bad `exit` value `{value}` from the VM"))?;
                let code = code
                    .parse()
                    .map_err(|_| format!("bad exit code `{code}` from the VM"))?;
                report.exit = Some((code, parse_number(key, ended)?));
            }
            "alive" => report.alive = true,
            "log" => {
                report.log = Some(rest.to_owned());
                rest = "";
            }
            _ => return Err(format!("unexpected line `{line}` from the VM")),
        }
    }
    Ok((lock, reports))
}

/// Lines with the same text, counted, in the order each first appeared.
#[derive(Debug, Default, PartialEq, Eq)]
struct Grouped {
    lines: Vec<(String, usize)>,
}

impl Grouped {
    fn add(&mut self, line: &str) {
        match self.lines.iter_mut().find(|(text, _)| text == line) {
            Some((_, count)) => *count += 1,
            None => self.lines.push((line.to_owned(), 1)),
        }
    }

    fn total(&self) -> usize {
        self.lines.iter().map(|(_, count)| count).sum()
    }
}

/// What a job's log says so far.
#[derive(Debug, Default, PartialEq, Eq)]
struct LogSummary {
    passed: usize,
    ignored: usize,
    /// The names of the tests that printed `... FAILED`, in log order.
    failed: Vec<String>,
    divergences: Grouped,
    errors: Grouped,
}

/// The text a terminal would show for one logged line: a carriage return
/// inside a line rewrote it, so only the part after the last one counts.
fn shown(line: &str) -> &str {
    let line = line.trim_end_matches('\r');
    line.rsplit('\r').next().unwrap_or(line)
}

fn summarize(log: &str) -> LogSummary {
    let mut summary = LogSummary::default();
    for line in log.lines().map(shown) {
        if let Some(test) = line.strip_prefix("test ") {
            if test.ends_with(" ... ok") {
                summary.passed += 1;
            } else if let Some(name) = test.strip_suffix(" ... FAILED") {
                summary.failed.push(name.to_owned());
            } else if test.contains(" ... ignored") {
                summary.ignored += 1;
            }
        }
        if line.contains("DIVERGENCE") {
            summary.divergences.add(line.trim());
        }
        if line.starts_with("error:") || line.starts_with("error[") {
            summary.errors.add(line);
        }
    }
    summary
}

fn duration(seconds: u64) -> String {
    let (hours, minutes, seconds) = (seconds / 3600, seconds / 60 % 60, seconds % 60);
    if hours > 0 {
        format!("{hours}h {minutes:02}m {seconds:02}s")
    } else if minutes > 0 {
        format!("{minutes}m {seconds:02}s")
    } else {
        format!("{seconds}s")
    }
}

/// One line saying what the job is doing and for how long.
fn headline(host: &str, report: &Report) -> String {
    let elapsed = report.elapsed().map(duration).unwrap_or_default();
    let state = match report.state() {
        State::Starting => "starting: vm-heavy has not taken the lock yet".to_owned(),
        State::Running => format!("running for {elapsed}"),
        State::Refused => "refused by vm-heavy: another heavy job holds this machine".to_owned(),
        State::Finished(code) => {
            let ago = report
                .exit
                .map(|(_, ended)| duration(report.now.saturating_sub(ended)))
                .unwrap_or_default();
            let outcome = if code == 0 { "finished" } else { "failed" };
            format!("{outcome}, exit {code}, after {elapsed}, {ago} ago")
        }
        State::Lost => format!(
            "lost after {elapsed}: its runner is gone without an exit record (VM restart or kill?)"
        ),
    };
    format!("{host} {}: {state}", report.label)
}

fn render_group(out: &mut String, title: &str, group: &Grouped) {
    if group.lines.is_empty() {
        return;
    }
    out.push_str(&format!("  {title}: {}\n", group.total()));
    for (line, count) in &group.lines {
        if *count == 1 {
            out.push_str(&format!("    {line}\n"));
        } else {
            out.push_str(&format!("    {count}x {line}\n"));
        }
    }
}

/// The full `status` of one job.
fn render(host: &str, report: &Report, lines: usize) -> String {
    let mut out = headline(host, report);
    out.push('\n');
    let log = report.log.as_deref().unwrap_or("");
    let summary = summarize(log);
    out.push_str(&format!(
        "  tests: {} ok, {} FAILED, {} ignored\n",
        summary.passed,
        summary.failed.len(),
        summary.ignored
    ));
    if !summary.failed.is_empty() {
        out.push_str("  FAILED:\n");
        for name in &summary.failed {
            out.push_str(&format!("    {name}\n"));
        }
    }
    render_group(&mut out, "DIVERGENCE lines", &summary.divergences);
    render_group(&mut out, "error lines", &summary.errors);
    let all: Vec<&str> = log.lines().map(shown).collect();
    let tail = &all[all.len().saturating_sub(lines)..];
    if tail.is_empty() {
        out.push_str(&format!(
            "  log: {}/log ({} lines)\n",
            report.path,
            all.len()
        ));
    } else {
        out.push_str(&format!(
            "  last {} of {} lines of {}/log:\n",
            tail.len(),
            all.len(),
            report.path
        ));
    }
    for line in tail {
        out.push_str(&format!("    {line}\n"));
    }
    out
}

/// `status` without a label: the lock holder, then one line per job.
fn render_list(host: &str, lock: Option<&str>, reports: &[Report]) -> String {
    let mut out = match lock {
        Some(owner) => format!("{host}: heavy lock held by {owner}\n"),
        None => format!("{host}: heavy lock free\n"),
    };
    if reports.is_empty() {
        out.push_str("  no jobs in ~/omega-agent/jobs\n");
    }
    for report in reports {
        out.push_str(&format!("  {}\n", headline(host, report)));
    }
    out
}

/// Runs a script on the host through `ssh <host> sh -s`. With `capture`, it
/// returns the script's stdout; otherwise stdout streams to this console.
/// The script's stderr always passes through, so a remote refusal reads as
/// the script wrote it, and a failed script's exit code becomes this
/// command's.
fn remote(host: &str, script: &str, capture: bool) -> Result<String, u8> {
    let mut child = Command::new("ssh")
        .args([
            "-o",
            "BatchMode=yes",
            "-o",
            "ConnectTimeout=15",
            "-o",
            "LogLevel=ERROR",
            host,
            "sh",
            "-s",
        ])
        .stdin(Stdio::piped())
        .stdout(if capture {
            Stdio::piped()
        } else {
            Stdio::inherit()
        })
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|error| fail(format!("cannot start ssh {host}: {error}")))?;
    let mut stdin = child.stdin.take().expect("stdin was piped");
    stdin
        .write_all(script.as_bytes())
        .map_err(|error| fail(format!("{host}: cannot send the script: {error}")))?;
    drop(stdin);
    let output = child
        .wait_with_output()
        .map_err(|error| fail(format!("{host}: ssh failed: {error}")))?;
    match output.status.code() {
        Some(0) => Ok(String::from_utf8_lossy(&output.stdout).into_owned()),
        Some(255) => Err(fail(format!("ssh {host} failed (exit 255)"))),
        code => Err(exit_code(code.unwrap_or(1))),
    }
}

/// A process exit code for this command: the job's own, or 1 when it does
/// not fit.
fn exit_code(code: i32) -> u8 {
    u8::try_from(code)
        .ok()
        .filter(|&code| code != 0)
        .unwrap_or(1)
}

fn one_report(host: &str, text: &str) -> Result<Report, String> {
    let (_, mut reports) = parse_reports(text)?;
    match reports.len() {
        1 => Ok(reports.remove(0)),
        n => Err(format!("{host}: expected one job record, got {n}")),
    }
}

fn mbx_version() -> Option<String> {
    let root = crate::repo_root().ok()?;
    let text = fs::read_to_string(root.join("tools").join("mbx-version")).ok()?;
    Some(text.trim().to_owned()).filter(|version| !version.is_empty())
}

fn start(args: &[String]) -> Result<(), u8> {
    let (host, label, job) = match args {
        [host, label, dash, command @ ..] if dash == "--" && !command.is_empty() => {
            (host, label, command_job(command))
        }
        [host, label, script] if !script.starts_with('-') => {
            let text = fs::read_to_string(Path::new(script))
                .map_err(|error| fail(format!("cannot read {script}: {error}")))?;
            (host, label, normalize_job(&text))
        }
        _ => return Err(usage_error()),
    };
    validate_label(label).map_err(fail)?;
    let script = start_script(label, &job, mbx_version().as_deref());
    let text = remote(host, &script, true)?;
    let report = one_report(host, &text).map_err(fail)?;
    print!("{}", render(host, &report, DEFAULT_LINES));
    match report.state() {
        State::Refused => Err(75),
        State::Finished(0) => Ok(()),
        State::Finished(code) => Err(exit_code(code)),
        State::Lost => Err(1),
        State::Starting | State::Running => {
            println!("  watch it: mbx xtask vm status {host} {label}");
            println!("        or: mbx xtask vm follow {host} {label}");
            Ok(())
        }
    }
}

/// Reads `--lines <n>` (and, for `follow`, `--all`) beside the positional
/// arguments. `None` lines means the whole log.
fn parse_view(args: &[String], allow_all: bool) -> Result<(Vec<&String>, Option<usize>), u8> {
    let mut positional = Vec::new();
    let mut lines = Some(DEFAULT_LINES);
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--lines" => {
                lines = Some(
                    iter.next()
                        .and_then(|n| n.parse().ok())
                        .ok_or_else(usage_error)?,
                );
            }
            "--all" if allow_all => lines = None,
            flag if flag.starts_with('-') => return Err(usage_error()),
            _ => positional.push(arg),
        }
    }
    Ok((positional, lines))
}

fn status(args: &[String]) -> Result<(), u8> {
    let (positional, lines) = parse_view(args, false)?;
    let lines = lines.unwrap_or(DEFAULT_LINES);
    let (host, label) = match positional.as_slice() {
        [host] => (*host, None),
        [host, label] => (*host, Some(label.as_str())),
        _ => return Err(usage_error()),
    };
    if let Some(label) = label {
        validate_label(label).map_err(fail)?;
    }
    let text = remote(host, &status_script(label), true)?;
    let rendered = match label {
        Some(_) => render(host, &one_report(host, &text).map_err(fail)?, lines),
        None => {
            let (lock, reports) = parse_reports(&text).map_err(fail)?;
            render_list(host, lock.as_deref(), &reports)
        }
    };
    crate::write_status(|out| out.write_all(rendered.as_bytes()))
}

fn follow(args: &[String]) -> Result<(), u8> {
    let (positional, lines) = parse_view(args, true)?;
    let [host, label] = positional.as_slice() else {
        return Err(usage_error());
    };
    validate_label(label).map_err(fail)?;
    eprintln!("vm: following {label} on {host}; stopping this leaves the job running");
    remote(host, &follow_script(label, lines), false)?;
    let text = remote(host, &status_script(Some(label)), true)?;
    let report = one_report(host, &text).map_err(fail)?;
    println!();
    print!("{}", render(host, &report, 0));
    match report.state() {
        State::Finished(0) => Ok(()),
        State::Finished(code) => Err(exit_code(code)),
        State::Refused => Err(75),
        // `follow` returns once the runner ends; a runner that is still
        // alive means the ssh session ended first.
        State::Lost | State::Starting | State::Running => Err(1),
    }
}

fn fail(error: String) -> u8 {
    eprintln!("vm: {error}");
    1
}

fn usage_error() -> u8 {
    eprintln!("{USAGE}");
    2
}

pub(crate) fn run(args: &[String]) -> Result<(), u8> {
    match args.first().map(String::as_str) {
        Some("start") => start(&args[1..]),
        Some("status") => status(&args[1..]),
        Some("follow") => follow(&args[1..]),
        Some("-h" | "--help" | "help") => {
            println!("{USAGE}");
            Ok(())
        }
        _ => Err(usage_error()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(text: &str) -> Report {
        one_report("vm", text).expect("one record")
    }

    #[test]
    fn labels_are_plain_folder_names() {
        for good in ["both-output", "xtask_tests.2", "A1"] {
            assert!(validate_label(good).is_ok(), "{good}");
        }
        for bad in ["", "-x", ".hidden", "a b", "a/b", "a'b", "a;b", "é"] {
            assert!(validate_label(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_job_runs_as_written_with_lf_lines_and_an_interpreter() {
        assert_eq!(
            normalize_job("#!/usr/bin/env bash\r\necho hi"),
            "#!/usr/bin/env bash\necho hi\n"
        );
        assert_eq!(normalize_job("echo hi\n"), "#!/bin/sh\necho hi\n");
        assert_eq!(
            command_job(&["sh".into(), "-c".into(), "echo 'it'".into()]),
            "#!/bin/sh\n# Written by `cargo xtask vm start`.\nexec 'sh' '-c' 'echo '\\''it'\\'''\n"
        );
    }

    #[test]
    fn the_heredoc_delimiter_never_ends_the_body_early() {
        assert_eq!(delimiter("echo hi\n"), "OMEGA_VM_JOB_0");
        assert_eq!(
            delimiter("OMEGA_VM_JOB_0\nOMEGA_VM_JOB_1\n"),
            "OMEGA_VM_JOB_2"
        );
        assert_eq!(
            heredoc("\"$f\"", "OMEGA_VM_JOB_0\n"),
            "cat > \"$f\" <<'OMEGA_VM_JOB_1'\nOMEGA_VM_JOB_0\nOMEGA_VM_JOB_1\n"
        );
    }

    #[test]
    fn the_start_script_runs_the_job_under_vm_heavy_and_waits_for_its_verdict() {
        let script = start_script(
            "both-output",
            "#!/bin/sh\necho hi\n",
            Some("1.29.2-omega.2"),
        );
        assert!(script.contains("label='both-output'\n"));
        assert!(script.contains("\"$HOME/omega-agent/bin/vm-heavy\" \"$label\" sh -c"));
        assert!(script.contains("mbx=\"$HOME/.cache/omega-mbx-bin/\"'1.29.2-omega.2'\n"));
        assert!(script.contains("nice -n 19 sh \"$dir/run.sh\""));
        assert!(script.contains("PriorityClass = [uint32]64"));
        assert!(script.contains("[ $i -lt 60 ]"));
        assert!(script.ends_with("report \"$dir\" full\n"));
        assert!(!script.contains('\r'));
    }

    #[test]
    fn records_parse_with_the_log_kept_byte_for_byte() {
        let text = "label both-output\npath /home/j/omega-agent/jobs/both-output\nnow 1000\n\
                    started 100\nrunning 110\nalive\nlog\nfirst\r\nlog\nlabel x\n";
        let report = report(text);
        assert_eq!(report.label, "both-output");
        assert_eq!(report.started, Some(100));
        assert_eq!(report.running, Some(110));
        assert!(report.alive);
        assert_eq!(report.log.as_deref(), Some("first\r\nlog\nlabel x\n"));
        assert_eq!(report.state(), State::Running);
        assert_eq!(report.elapsed(), Some(890));
    }

    #[test]
    fn a_list_reads_the_lock_owner_and_every_record() {
        let text = "lock both-output, since 11-23 07:05\n\
                    label a\npath /j/a\nnow 50\nstarted 1\nrunning 2\nexit 0 40\n\
                    label b\npath /j/b\nnow 50\nstarted 3\nexit 75 4\n";
        let (lock, reports) = parse_reports(text).unwrap();
        assert_eq!(lock.as_deref(), Some("both-output, since 11-23 07:05"));
        assert_eq!(reports.len(), 2);
        assert_eq!(reports[0].state(), State::Finished(0));
        assert_eq!(reports[0].elapsed(), Some(38));
        assert_eq!(reports[1].state(), State::Refused);
        let (lock, reports) = parse_reports("lock\n").unwrap();
        assert_eq!(lock, None);
        assert!(reports.is_empty());
    }

    #[test]
    fn states_follow_the_runner_and_its_exit_record() {
        let starting = Report {
            now: 10,
            started: Some(1),
            alive: true,
            ..Report::default()
        };
        assert_eq!(starting.state(), State::Starting);
        let lost = Report {
            running: Some(2),
            ..Report::default()
        };
        assert_eq!(lost.state(), State::Lost);
        // A job that ran and itself exited 75 failed; vm-heavy did not refuse it.
        let failed = Report {
            running: Some(2),
            exit: Some((75, 9)),
            ..Report::default()
        };
        assert_eq!(failed.state(), State::Finished(75));
        assert!(parse_reports("now 1\n").is_err());
        assert!(parse_reports("label a\nexit zero 1\n").is_err());
        assert!(parse_reports("label a\nsurprise\n").is_err());
    }

    #[test]
    fn the_summary_counts_test_lines_and_groups_divergence_and_error_lines() {
        let log = "\
running 3 tests
test a ... ok
test b ... FAILED
test c ... ignored, slow
test d ... ok\r
progress 10%\rtest e ... ok
DIVERGENCE: native 0 vs interp 1
  DIVERGENCE: native 0 vs interp 1
DIVERGENCE: native 2 vs interp crash Trap
error[E0425]: cannot find value `x`
error: test failed, to rerun pass `--test run`
test result: ok. 2 passed; 1 failed; 1 ignored
";
        let summary = summarize(log);
        assert_eq!(summary.passed, 3);
        assert_eq!(summary.failed, vec!["b".to_owned()]);
        assert_eq!(summary.ignored, 1);
        assert_eq!(
            summary.divergences.lines,
            vec![
                ("DIVERGENCE: native 0 vs interp 1".to_owned(), 2),
                ("DIVERGENCE: native 2 vs interp crash Trap".to_owned(), 1),
            ]
        );
        assert_eq!(summary.errors.total(), 2);
    }

    #[test]
    fn status_shows_counts_groups_and_the_last_lines() {
        let report = Report {
            label: "both-output".into(),
            path: "/j/both-output".into(),
            now: 1300,
            started: Some(100),
            running: Some(110),
            alive: true,
            log: Some(
                "test a ... ok\ntest b ... FAILED\nDIVERGENCE: x\nDIVERGENCE: x\nlast\n".into(),
            ),
            ..Report::default()
        };
        assert_eq!(
            render("omega-linux", &report, 2),
            "\
omega-linux both-output: running for 19m 50s
  tests: 1 ok, 1 FAILED, 0 ignored
  FAILED:
    b
  DIVERGENCE lines: 2
    2x DIVERGENCE: x
  last 2 of 5 lines of /j/both-output/log:
    DIVERGENCE: x
    last
"
        );
        let done = Report {
            exit: Some((101, 1200)),
            alive: false,
            ..report
        };
        assert_eq!(
            headline("omega-linux", &done),
            "omega-linux both-output: failed, exit 101, after 18m 10s, 1m 40s ago"
        );
        assert_eq!(duration(3725), "1h 02m 05s");
        assert_eq!(duration(7), "7s");
        assert_eq!(exit_code(101), 101);
        assert_eq!(exit_code(300), 1);
        assert_eq!(exit_code(-1), 1);
    }

    #[test]
    fn view_options_sit_beside_the_positional_arguments() {
        let args: Vec<String> = ["omega-linux", "job", "--lines", "3"]
            .map(String::from)
            .to_vec();
        let (positional, lines) = parse_view(&args, false).unwrap();
        assert_eq!(positional, vec!["omega-linux", "job"]);
        assert_eq!(lines, Some(3));
        let all: Vec<String> = ["omega-linux", "job", "--all"].map(String::from).to_vec();
        assert_eq!(parse_view(&all, true).unwrap().1, None);
        assert!(parse_view(&all, false).is_err());
    }
}
