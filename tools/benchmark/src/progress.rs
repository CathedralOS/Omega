//! Record one commit's compiler speed on this host.
//!
//! The owner runs `omega-bench progress` from a checkout on each test machine
//! so speed can be charted per main commit; `cargo xtask sweep` measures how
//! many programs pass. The workload set is frozen in `WORKLOADS` and versioned
//! by `BENCHMARK_SET`: change either and bump the set version, because a
//! dashboard only compares rows of one set.
//! Every process observation becomes one row; summaries are derived from rows.

use crate::process::{capture, file_digest};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub const SCHEMA: &str = "omega-progress-record/1";
pub const BENCHMARK_SET: &str = "omega-progress-set/1";

/// `{build}` is replaced by a fresh empty build directory for each run.
/// `clean` names a directory removed before each run so `omega run` cannot
/// reuse the previous run's products. `accept` names a project whose package
/// review is settled once, untimed, before its runs (see `accept_review`).
struct Workload {
    name: &'static str,
    arguments: &'static [&'static str],
    clean: Option<&'static str>,
    accept: Option<&'static str>,
}

const WORKLOADS: &[Workload] = &[
    Workload {
        name: "one_line_check",
        arguments: &[
            "--check",
            "--build-dir",
            "{build}",
            "tools/benchmark/workloads/one_line/main.omg",
        ],
        clean: None,
        accept: None,
    },
    Workload {
        name: "addr_algebra_check",
        arguments: &[
            "--check",
            "--build-dir",
            "{build}",
            "tests/fixtures/pass/arithmetic/addr_algebra/main.omg",
        ],
        clean: None,
        accept: None,
    },
    Workload {
        name: "medium_sample_check",
        arguments: &[
            "--check",
            "--build-dir",
            "{build}",
            "samples/cli/arithmetic/prime_counter/main.omg",
        ],
        clean: None,
        accept: Some("samples/cli/arithmetic/prime_counter"),
    },
    Workload {
        name: "heavy_proof_check",
        arguments: &[
            "--check",
            "--build-dir",
            "{build}",
            "tests/fixtures/pass/proofs/runtime_inductive_gauss_sum_exit/main.omg",
        ],
        clean: None,
        accept: Some("tests/fixtures/pass/proofs/runtime_inductive_gauss_sum_exit"),
    },
    Workload {
        name: "native_run_both",
        arguments: &[
            "run",
            "--both",
            "samples/cli/arithmetic/wrapping_square_sum/main.omg",
        ],
        clean: Some("samples/cli/arithmetic/wrapping_square_sum/build"),
        accept: None,
    },
];

struct Options {
    omega: Option<PathBuf>,
    results: PathBuf,
    runs: usize,
    aa_runs: usize,
    only: Option<String>,
}

const USAGE: &str = "omega-bench progress [--omega <release-omega>] [--results <dir>] [--runs N] [--aa-runs N] [--only <workload>]";

pub fn run(arguments: &[String]) -> Result<(), String> {
    let options = parse_options(arguments)?;
    let root = git(&["rev-parse", "--show-toplevel"])?;
    let root = PathBuf::from(root);
    let mut compiler_build = Value::Null;
    let omega = match &options.omega {
        Some(path) => path.clone(),
        None => {
            let (path, observation) = build_release_compiler(&root)?;
            compiler_build = observation;
            path
        }
    };
    let omega =
        fs::canonicalize(&omega).map_err(|error| format!("{}: {error}", omega.display()))?;
    let version = Command::new(&omega)
        .arg("--version")
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("{} --version: {error}", omega.display()))?;
    let version = String::from_utf8_lossy(&version.stdout).trim().to_owned();
    let recorded = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_secs();
    let commit = git(&["rev-parse", "HEAD"])?;
    let target = host_target();
    let host = Command::new("hostname")
        .stdin(Stdio::null())
        .output()
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .unwrap_or_default();

    let scratch = std::env::temp_dir().join(format!("omega-progress-{}", std::process::id()));
    let mut rows = Vec::new();
    let mut preparations = BTreeMap::new();
    for workload in WORKLOADS.iter().filter(|workload| {
        options
            .only
            .as_deref()
            .is_none_or(|only| only == workload.name)
    }) {
        if let Some(project) = workload.accept {
            let started = Instant::now();
            accept_review(&omega, &root, project, &target)?;
            preparations.insert(
                workload.name,
                json!({"project": project, "wall_s": started.elapsed().as_secs_f64()}),
            );
        }
        // One warmup fills the file cache, then alternating A/A runs of the same
        // binary estimate the host's noise before the measured runs.
        let plan = std::iter::once("warmup")
            .chain(
                (0..options.aa_runs * 2).map(|index| if index % 2 == 0 { "aa_a" } else { "aa_b" }),
            )
            .chain(std::iter::repeat_n("measured", options.runs));
        for (index, phase) in plan.enumerate() {
            let row = measure(
                &root,
                &omega,
                workload.name,
                workload.arguments,
                workload.clean,
                phase,
                index,
                &scratch,
            )?;
            eprintln!(
                "progress: {} {phase} {:.3}s exit {}",
                workload.name, row["wall_s"], row["exit_code"]
            );
            rows.push(row);
        }
    }
    let _ = fs::remove_dir_all(&scratch);
    let summary = summarize(&rows);
    let record = json!({
        "schema": SCHEMA,
        "benchmark_set": BENCHMARK_SET,
        "key": {
            "commit": commit,
            "dirty": !git(&["status", "--porcelain"])?.is_empty(),
            "source_trees": {
                "omega-rust": git(&["rev-parse", "HEAD:omega-rust"])?,
                "source": git(&["rev-parse", "HEAD:source"])?,
            },
            "host": host,
            "target": target,
            "build_profile": "release",
            "compiler_version": version,
            "recorded_utc": utc_timestamp(recorded),
        },
        "host_detail": {
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "logical_cpus": std::thread::available_parallelism().ok().map(usize::from),
        },
        "compiler": {
            "path": omega.display().to_string(),
            "sha256": file_digest(&omega)?,
            "build": compiler_build,
        },
        "settings": {"warmups": 1, "aa_runs_per_side": options.aa_runs, "runs": options.runs,
            "address_space_bytes": crate::process::address_space_limit()},
        "preparations": preparations,
        "rows": rows,
        "summary": summary,
    });
    validate_record(&record)?;
    fs::create_dir_all(&options.results).map_err(|error| error.to_string())?;
    let file = options.results.join(format!(
        "{}__{}__{}.json",
        &commit[..12.min(commit.len())],
        target,
        utc_timestamp(recorded).replace(':', "")
    ));
    fs::write(
        &file,
        serde_json::to_string_pretty(&record).map_err(|error| error.to_string())? + "\n",
    )
    .map_err(|error| format!("write {}: {error}", file.display()))?;
    println!("{}", one_line_summary(&record, &file));
    let failed = record["rows"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|row| row["ok"] != true)
        .count();
    if failed > 0 {
        return Err(format!("{failed} run(s) failed; the record keeps them"));
    }
    Ok(())
}

fn parse_options(arguments: &[String]) -> Result<Options, String> {
    let mut options = Options {
        omega: None,
        results: PathBuf::from("build/progress-records"),
        runs: 5,
        aa_runs: 3,
        only: None,
    };
    let mut arguments = arguments.iter();
    while let Some(argument) = arguments.next() {
        let mut value = || {
            arguments
                .next()
                .cloned()
                .ok_or_else(|| format!("{argument} needs a value\nusage: {USAGE}"))
        };
        match argument.as_str() {
            "--omega" => options.omega = Some(PathBuf::from(value()?)),
            "--results" => options.results = PathBuf::from(value()?),
            "--runs" => options.runs = value()?.parse().map_err(|_| "--runs takes a count")?,
            "--aa-runs" => {
                options.aa_runs = value()?.parse().map_err(|_| "--aa-runs takes a count")?;
            }
            "--only" => options.only = Some(value()?),
            _ => return Err(format!("unknown argument {argument}\nusage: {USAGE}")),
        }
    }
    if options.runs == 0 {
        return Err("--runs must be at least 1".into());
    }
    if let Some(only) = &options.only
        && !WORKLOADS.iter().any(|workload| workload.name == only)
    {
        return Err(format!("unknown workload {only}"));
    }
    Ok(options)
}

pub(crate) fn git(arguments: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .args(arguments)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("git {}: {error}", arguments.join(" ")))?;
    if !output.status.success() {
        return Err(format!(
            "git {}: {}",
            arguments.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn host_target() -> String {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => "linux_x86_64".into(),
        ("linux", "aarch64") => "linux_arm64".into(),
        ("macos", "aarch64") => "macos_arm64".into(),
        ("windows", "x86_64") => "windows_x86_64".into(),
        (os, arch) => format!("{os}_{arch}"),
    }
}

/// Build the release compiler with mbx, which the repository requires for every
/// Rust build.
fn build_release_compiler(root: &Path) -> Result<(PathBuf, Value), String> {
    let arguments = ["build", "--release", "-p", "omega"];
    let started = Instant::now();
    let status = Command::new("mbx")
        .args(arguments)
        .current_dir(root)
        .stdin(Stdio::null())
        .status()
        .map_err(|error| format!("mbx build: {error}"))?;
    if !status.success() {
        return Err(format!("mbx build --release -p omega failed: {status}"));
    }
    let target =
        std::env::var_os("CARGO_TARGET_DIR").map_or_else(|| root.join("target"), PathBuf::from);
    let path = target
        .join("release")
        .join(format!("omega{}", std::env::consts::EXE_SUFFIX));
    Ok((
        path,
        json!({"command": format!("mbx {}", arguments.join(" ")), "wall_s": started.elapsed().as_secs_f64()}),
    ))
}

/// Settle a std-dependent project's package review for this compiler: drop any
/// lock from another compiler, review, accept every pending decision, resume.
/// This is project acceptance so the workload can compile, not a source audit.
/// The review document is LF-only and byte-sensitive, so only the token changes.
fn accept_review(omega: &Path, root: &Path, project: &str, target: &str) -> Result<(), String> {
    let directory = root.join(project);
    let _ = fs::remove_file(directory.join("omega.lock"));
    let update = |extra: &[&str]| -> Result<String, String> {
        let output = Command::new(omega)
            .args(["packages", "update", "--project"])
            .arg(&directory)
            .args(extra)
            .current_dir(root)
            .stdin(Stdio::null())
            .output()
            .map_err(|error| format!("omega packages update {project}: {error}"))?;
        let text = String::from_utf8_lossy(&output.stdout).into_owned()
            + &String::from_utf8_lossy(&output.stderr);
        Ok(if output.status.success() {
            String::new()
        } else {
            text
        })
    };
    update(&["--target", target])?;
    let review = directory.join(format!("build/package-manager/review-{target}.txt"));
    let text = fs::read(&review).unwrap_or_default();
    let mut accepted = Vec::with_capacity(text.len());
    for line in text.split_inclusive(|byte| *byte == b'\n') {
        match line.strip_suffix(b" pending\n") {
            Some(head) if head.starts_with(b"decision ") => {
                accepted.extend_from_slice(head);
                accepted.extend_from_slice(b" accept\n");
            }
            _ => accepted.extend_from_slice(line),
        }
    }
    if accepted != text {
        fs::write(&review, accepted).map_err(|error| format!("{}: {error}", review.display()))?;
        let failure = update(&["--resume"])?;
        if !failure.is_empty() {
            return Err(format!("accept {project}: {failure}"));
        }
    }
    Ok(())
}

/// One process row. `template` may name `{build}`, replaced by a fresh empty
/// build directory; `clean` is removed under `root` before the run.
pub(crate) fn measure(
    root: &Path,
    omega: &Path,
    name: &str,
    template: &[&str],
    clean: Option<&str>,
    phase: &str,
    index: usize,
    scratch: &Path,
) -> Result<Value, String> {
    let build = scratch.join(format!("{name}-{index}"));
    let _ = fs::remove_dir_all(&build);
    fs::create_dir_all(&build).map_err(|error| error.to_string())?;
    if let Some(clean) = clean {
        let _ = fs::remove_dir_all(root.join(clean));
    }
    let arguments: Vec<String> = template
        .iter()
        .map(|argument| argument.replace("{build}", &build.display().to_string()))
        .collect();
    let environment = BTreeMap::new();
    // Wall and CPU time swing with other work on a shared VM; user-space
    // instruction counts barely move, so Linux runs go through `perf stat`
    // when it is installed. perf itself then sits in the timed process tree.
    let counters = build.with_extension("perf.csv");
    let perf = cfg!(target_os = "linux")
        .then(crate::profiling::find_perf)
        .flatten();
    let (program, arguments) = match &perf {
        Some(perf) => {
            let mut wrapped = vec![
                "stat".to_owned(),
                "-e".to_owned(),
                "instructions:u".to_owned(),
                "-x,".to_owned(),
                "-o".to_owned(),
                counters.display().to_string(),
                "--".to_owned(),
                omega.display().to_string(),
            ];
            wrapped.extend(arguments);
            (perf.as_path(), wrapped)
        }
        None => (omega, arguments),
    };
    let load_average_start = load_average();
    // Empty stdin is the null device: omega must never wait on an open console.
    let (observation, output) =
        capture(program, &arguments, &environment, root, b"", 30 * 60 * 1000)?;
    let load_average_end = load_average();
    let instructions_u = fs::read_to_string(&counters)
        .ok()
        .and_then(|text| parse_perf_instructions(&text));
    let _ = fs::remove_file(&counters);
    let _ = fs::remove_dir_all(&build);
    let resource = |name: &str| observation.resources[name]["value"].as_u64();
    let seconds = |nanos: Option<u64>| nanos.map(|nanos| nanos as f64 / 1e9);
    Ok(json!({
        "workload": name,
        "phase": phase,
        "index": index,
        "arguments": template,
        "exit_code": observation.exit_code,
        "ok": observation.exit_code == Some(0),
        "wall_s": observation.process_wall_ns as f64 / 1e9,
        "user_cpu_s": seconds(resource("user_cpu_ns")),
        "sys_cpu_s": seconds(resource("system_cpu_ns")),
        "peak_rss_bytes": resource("peak_resident_bytes"),
        "instructions_u": instructions_u,
        "load_average_start": load_average_start,
        "load_average_end": load_average_end,
        "stderr_tail": (observation.exit_code != Some(0)).then(|| {
            let text = String::from_utf8_lossy(&output.stderr);
            text.lines().rev().take(5).collect::<Vec<_>>().join("\n")
        }),
    }))
}

/// `perf stat -x,` writes `<count>,<unit>,instructions:u,...`; an unsupported
/// or uncounted event writes `<not supported>` or `<not counted>` instead.
fn parse_perf_instructions(text: &str) -> Option<u64> {
    text.lines()
        .map(|line| line.split(',').collect::<Vec<_>>())
        .find(|fields| {
            fields
                .get(2)
                .is_some_and(|event| event.starts_with("instructions"))
        })
        .and_then(|fields| fields[0].trim().parse().ok())
}

/// The 1, 5 and 15 minute load averages, so a run on a busy host can be
/// flagged; `null` on Windows, which has no load average.
fn load_average() -> Option<[f64; 3]> {
    let text = fs::read_to_string("/proc/loadavg").ok().or_else(|| {
        let output = Command::new("sysctl")
            .args(["-n", "vm.loadavg"])
            .stdin(Stdio::null())
            .output()
            .ok()?;
        output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
    })?;
    let mut values = text
        .split_whitespace()
        .filter_map(|field| field.parse::<f64>().ok());
    Some([values.next()?, values.next()?, values.next()?])
}

pub(crate) fn median(values: &mut [f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    Some(if values.len().is_multiple_of(2) {
        (values[middle - 1] + values[middle]) / 2.0
    } else {
        values[middle]
    })
}

fn summarize(rows: &[Value]) -> Value {
    let mut names: Vec<&str> = rows
        .iter()
        .filter_map(|row| row["workload"].as_str())
        .collect();
    names.dedup();
    let collect = |name: &str, phase: &str, field: &dyn Fn(&Value) -> Option<f64>| -> Vec<f64> {
        rows.iter()
            .filter(|row| row["workload"] == name && row["phase"] == phase)
            .filter_map(field)
            .collect()
    };
    let wall = |row: &Value| row["wall_s"].as_f64();
    let cpu = |row: &Value| Some(row["user_cpu_s"].as_f64()? + row["sys_cpu_s"].as_f64()?);
    let rss = |row: &Value| row["peak_rss_bytes"].as_f64();
    let instructions = |row: &Value| row["instructions_u"].as_f64();
    let spread = |values: &mut Vec<f64>| {
        let center = median(values)?;
        Some((values.last()? - values.first()?) / center * 100.0)
    };
    Value::Array(
        names
            .into_iter()
            .map(|name| {
                let mut measured = collect(name, "measured", &wall);
                let aa_spread = |field: &dyn Fn(&Value) -> Option<f64>| {
                    let a = median(&mut collect(name, "aa_a", field))?;
                    let b = median(&mut collect(name, "aa_b", field))?;
                    Some((a - b).abs() / ((a + b) / 2.0) * 100.0)
                };
                let workload_rows: Vec<&Value> =
                    rows.iter().filter(|row| row["workload"] == name).collect();
                json!({
                    "workload": name,
                    "all_ok": rows.iter().filter(|row| row["workload"] == name).all(|row| row["ok"] == true),
                    "runs": measured.len(),
                    "median_wall_s": median(&mut measured),
                    "measured_range_pct": spread(&mut measured),
                    "median_cpu_s": median(&mut collect(name, "measured", &cpu)),
                    "median_peak_rss_bytes": median(&mut collect(name, "measured", &rss)),
                    "aa_spread_pct": aa_spread(&wall),
                    "median_instructions_u": median(&mut collect(name, "measured", &instructions)),
                    "aa_instructions_spread_pct": aa_spread(&instructions),
                    "load_average_start": workload_rows.first().map(|row| &row["load_average_start"]),
                    "load_average_end": workload_rows.last().map(|row| &row["load_average_end"]),
                })
            })
            .collect(),
    )
}

/// The documented row fields every consumer may rely on; refuse to publish drift.
pub fn validate_record(record: &Value) -> Result<(), String> {
    if record["schema"] != SCHEMA || record["benchmark_set"] != BENCHMARK_SET {
        return Err("record schema or benchmark set mismatch".into());
    }
    for key in [
        "commit",
        "host",
        "target",
        "build_profile",
        "compiler_version",
        "recorded_utc",
    ] {
        if !record["key"][key].is_string() {
            return Err(format!("key.{key} must be a string"));
        }
    }
    for row in record["rows"].as_array().ok_or("rows must be an array")? {
        for field in ["workload", "phase"] {
            if !row[field].is_string() {
                return Err(format!("row.{field} must be a string"));
            }
        }
        if !row["wall_s"].is_f64() || !row["ok"].is_boolean() {
            return Err("row needs wall_s and ok".into());
        }
        for field in [
            "user_cpu_s",
            "sys_cpu_s",
            "peak_rss_bytes",
            "instructions_u",
            "exit_code",
        ] {
            if !(row[field].is_number() || row[field].is_null()) {
                return Err(format!("row.{field} must be a number or null"));
            }
        }
        for field in ["load_average_start", "load_average_end"] {
            let three_numbers = row[field]
                .as_array()
                .is_some_and(|values| values.len() == 3 && values.iter().all(Value::is_number));
            if !(three_numbers || row[field].is_null()) {
                return Err(format!("row.{field} must be three numbers or null"));
            }
        }
    }
    Ok(())
}

fn one_line_summary(record: &Value, file: &Path) -> String {
    let parts: Vec<String> = record["summary"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|row| {
            format!(
                "{} {:.2}s{}",
                row["workload"].as_str().unwrap_or_default(),
                row["median_wall_s"].as_f64().unwrap_or(f64::NAN),
                if row["all_ok"] == true { "" } else { " FAILED" }
            )
        })
        .collect();
    format!(
        "progress {} {}: {} -> {}",
        record["key"]["target"].as_str().unwrap_or_default(),
        &record["key"]["commit"].as_str().unwrap_or_default()[..12],
        parts.join(", "),
        file.display()
    )
}

/// `YYYY-MM-DDTHH:MM:SSZ` from Unix seconds (Howard Hinnant's civil-from-days).
fn utc_timestamp(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64;
    let second_of_day = seconds % 86_400;
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        second_of_day / 3600,
        second_of_day / 60 % 60,
        second_of_day % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perf_instructions_parses_count_and_rejects_unsupported() {
        assert_eq!(
            parse_perf_instructions(
                "
# started
123456789,,instructions:u,1000,100.00,,
"
            ),
            Some(123_456_789)
        );
        assert_eq!(
            parse_perf_instructions(
                "<not supported>,,instructions:u,0,100.00,,
"
            ),
            None
        );
    }

    #[test]
    fn record_schema_accepts_rows_and_refuses_drift() {
        let row = json!({"workload": "one_line_check", "phase": "measured", "index": 0,
            "exit_code": 0, "ok": true, "wall_s": 0.5, "user_cpu_s": 0.4, "sys_cpu_s": 0.1,
            "peak_rss_bytes": 1024, "instructions_u": 1000, "load_average_start": [1.0, 2.0, 3.0],
            "load_average_end": null});
        let mut record = json!({"schema": SCHEMA, "benchmark_set": BENCHMARK_SET,
            "key": {"commit": "abc", "host": "h", "target": "linux_x86_64", "build_profile": "release",
                "compiler_version": "omega 0", "recorded_utc": utc_timestamp(0)},
            "rows": [row]});
        assert_eq!(validate_record(&record), Ok(()));
        assert_eq!(record["key"]["recorded_utc"], "1970-01-01T00:00:00Z");
        assert_eq!(utc_timestamp(1_793_836_800), "2026-11-05T00:00:00Z");
        let summary = summarize(record["rows"].as_array().unwrap());
        assert_eq!(summary[0]["median_wall_s"], 0.5);
        assert_eq!(summary[0]["median_cpu_s"], 0.5);
        assert_eq!(summary[0]["median_instructions_u"], 1000.0);
        record["rows"][0]["load_average_end"] = json!([1.0]);
        assert!(validate_record(&record).is_err());
        record["rows"][0]["load_average_end"] = json!(null);
        record["rows"][0]["wall_s"] = json!("fast");
        assert!(validate_record(&record).is_err());
    }
}
