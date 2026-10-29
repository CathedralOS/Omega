//! Own the comparison lifecycle and its append-only evidence journal.

use crate::plan::{CommandSpec, ExpectedOutput, Measurement, Plan};
use crate::process::{Observation, capture, digest, file_digest};
use crate::statistics::{Comparison, compare as compare_samples};
use bounded_process::BoundedProcessOutput;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize)]
pub struct Report {
    schema: &'static str,
    pub name: String,
    pub metric: &'static str,
    pub comparison: Comparison,
    warnings: Vec<&'static str>,
    pub additional_metrics: BTreeMap<String, Value>,
}

#[derive(Default, Deserialize, Serialize)]
pub(crate) struct Sample {
    pub role: String,
    pub round: usize,
    pub warmup: bool,
    pub primary_value: Option<f64>,
    pub observations: Vec<Observation>,
    pub compiler_stage_ms: BTreeMap<String, f64>,
    pub compiler_counters: BTreeMap<String, u64>,
    pub artifact_sha256: Option<String>,
    pub artifact_bytes: Option<u64>,
    pub artifact_relative_path: Option<String>,
    pub failure: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReportedWork {
    schema: String,
    elapsed_ns: u64,
    work_units: u64,
    checksum: String,
}

struct Journal {
    file: File,
}

impl Journal {
    fn append(&mut self, event: &Value) -> Result<(), String> {
        serde_json::to_writer(&mut self.file, event).map_err(|error| error.to_string())?;
        self.file
            .write_all(b"\n")
            .and_then(|()| self.file.sync_data())
            .map_err(|error| format!("persist experiment journal: {error}"))
    }
}

pub(crate) fn prepare_plan(plan_path: &Path) -> Result<(Plan, PathBuf, String), String> {
    let plan_path = resolve(
        &std::env::current_dir().map_err(|error| error.to_string())?,
        plan_path,
    )?;
    let plan_bytes = fs::read(&plan_path).map_err(|error| error.to_string())?;
    let mut plan: Plan =
        serde_json::from_slice(&plan_bytes).map_err(|error| format!("read plan: {error}"))?;
    plan.validate()?;
    let plan_parent = plan_path.parent().ok_or("plan has no parent directory")?;
    plan.working_directory = resolve(plan_parent, &plan.working_directory)?;
    for command in [&mut plan.baseline, &mut plan.candidate] {
        command.program = resolve(&plan.working_directory, &command.program)?;
        if !command.program.is_file() {
            return Err("program must be a file".into());
        }
    }
    for input in &mut plan.inputs {
        *input = resolve(&plan.working_directory, input)?;
    }
    if let Measurement::Compiler {
        source,
        library_root,
        target,
        ..
    } = &mut plan.measurement
    {
        *source = resolve(&plan.working_directory, source)?;
        *library_root = resolve(&plan.working_directory, library_root)?;
        if !library_root.is_dir() {
            return Err("library_root must be a directory".into());
        }
        let library_path = library_root
            .to_str()
            .ok_or("library_root must be a UTF-8 path")?
            .to_owned();
        for command in [&mut plan.baseline, &mut plan.candidate] {
            if command
                .environment
                .get("OMEGA_LIBRARY_ROOT")
                .is_some_and(|configured| configured != &library_path)
            {
                return Err(
                    "use measurement.library_root to select the common compiler library".into(),
                );
            }
            command
                .environment
                .insert("OMEGA_LIBRARY_ROOT".into(), library_path.clone());
        }
        if !plan.inputs.iter().any(|input| {
            input == library_root || (input.is_dir() && library_root.starts_with(input))
        }) {
            plan.inputs.push(library_root.clone());
        }
        if !source.is_file() {
            return Err("compiler source must be a file".into());
        }
        if !plan
            .inputs
            .iter()
            .any(|input| input == source || (input.is_dir() && source.starts_with(input)))
        {
            return Err("declared inputs must include the compiler source".into());
        }
        if target != host_target()? {
            return Err(format!(
                "native oracle requires the matching host target {}; got {target}",
                host_target()?
            ));
        }
    }
    Ok((plan, plan_path, digest(&plan_bytes)))
}

pub fn compare(plan_path: &Path, output: &Path) -> Result<Report, String> {
    let (plan, plan_path, plan_sha256) = prepare_plan(plan_path)?;
    let parent = output
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent).map_err(|error| format!("create result parent: {error}"))?;
    let parent = parent.canonicalize().map_err(|error| error.to_string())?;
    let output = parent.join(
        output
            .file_name()
            .ok_or("result directory must have a name")?,
    );
    if plan
        .inputs
        .iter()
        .any(|input| input.is_dir() && output.starts_with(input))
    {
        return Err("result directory must be outside declared input directories".into());
    }
    fs::create_dir(&output)
        .map_err(|error| format!("create new result directory {}: {error}", output.display()))?;
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output.join("journal.jsonl"))
        .map_err(|error| error.to_string())?;
    let mut journal = Journal { file };
    journal.append(&json!({"event":"start", "schema":"omega-performance-run/2", "plan":plan,
        "plan_sha256":plan_sha256, "started_unix_seconds":SystemTime::now().duration_since(UNIX_EPOCH)
            .map_err(|error| error.to_string())?.as_secs(),
        "host":{"os":std::env::consts::OS, "architecture":std::env::consts::ARCH,
            "logical_cpus":std::thread::available_parallelism().ok().map(usize::from),
            "processor":std::env::var("PROCESSOR_IDENTIFIER").ok().or_else(||std::fs::read_to_string("/proc/cpuinfo").ok().and_then(|text|text.lines().find_map(|line|line.strip_prefix("model name").and_then(|line|line.split_once(':')).map(|(_,name)|name.trim().to_owned())))),
            "host_id_sha256":host_identifier()},
        "inherited_settings":(["OMEGA_PHASE_PROFILE","OMEGA_REVIEW_TIMINGS","OMEGA_ALLOCATION_PROFILE","RUST_MIN_STACK","RAYON_NUM_THREADS"].into_iter().filter_map(|name|std::env::var(name).ok().map(|value|(name,value))).collect::<BTreeMap<_,_>>()),
        "resource_limits":{"address_space_bytes":17179869184_u64, "process_memory_bytes":8589934592_u64,
            "aggregate_memory_bytes":8589934592_u64, "active_processes":64, "capture_bytes_per_stream":1048576},
        "timing_boundary":"process wall includes startup, capture and cleanup; reported_work is child-instrumented",
        "inherited_environment":"inherited, with plan overlays; secrets are not recorded; no contention isolation"}))?;
    let result = run(&plan, &plan_path, &output, &mut journal);
    match result {
        Ok(report) => {
            let bytes = serde_json::to_vec_pretty(&report).map_err(|error| error.to_string())?;
            write_new(&output.join("report.json"), &bytes)?;
            journal.append(&json!({"event":"complete", "report_sha256":digest(&bytes)}))?;
            Ok(report)
        }
        Err(error) => {
            journal.append(&json!({"event":"failed", "reason":error}))?;
            Err(error)
        }
    }
}

fn run(
    plan: &Plan,
    plan_path: &Path,
    output: &Path,
    journal: &mut Journal,
) -> Result<Report, String> {
    let snapshot = snapshot(plan, plan_path)?;
    journal.append(&json!({"event":"identities", "sha256":snapshot,
        "runner_sha256":file_digest(&std::env::current_exe().map_err(|error| error.to_string())?)?}))?;
    if matches!(plan.measurement, Measurement::Compiler { .. }) {
        for (role, command) in [("baseline", &plan.baseline), ("candidate", &plan.candidate)] {
            let (_, profile) = capture(
                &command.program,
                &["--build-profile".into()],
                &command.environment,
                &plan.working_directory,
                &[],
                plan.timeout_ms,
            )?;
            write_capture(output, &format!("{role}.profile"), &profile)?;
            let profile_value: Value = serde_json::from_slice(&profile.stdout)
                .map_err(|error| format!("{role}: invalid build profile: {error}"))?;
            if !profile.status.success()
                || profile_value["schema"] != "omega-compiler-build-profile-v1"
                || !matches!(
                    profile_value["opt_level"].as_str(),
                    Some("1" | "2" | "3" | "s" | "z")
                )
            {
                return Err(format!(
                    "{role}: optimized compiler build profile required: {profile_value}"
                ));
            }
            let (_, version) = capture(
                &command.program,
                &["--version".into()],
                &command.environment,
                &plan.working_directory,
                &[],
                plan.timeout_ms,
            )?;
            write_capture(output, &format!("{role}.version"), &version)?;
            if !version.status.success() {
                return Err(format!("{role}: compiler --version failed"));
            }
            journal.append(
                &json!({"event":"compiler_identity", "role":role, "profile":profile_value,
                "version":String::from_utf8_lossy(&version.stdout)}),
            )?;
        }
    }
    let mut artifact_identity = None;
    let mut samples = Vec::new();
    for warmup in [true, false] {
        let rounds = if warmup { plan.warmups } else { plan.pairs };
        for round in 0..rounds {
            // Alternating AB/BA pairs gives ABBA across adjacent rounds.
            let roles = if round.is_multiple_of(2) {
                ["baseline", "candidate"]
            } else {
                ["candidate", "baseline"]
            };
            for role in roles {
                if snapshot != self::snapshot(plan, plan_path)? {
                    return Err("declared inputs or executables changed during comparison".into());
                }
                let command = if role == "baseline" {
                    &plan.baseline
                } else {
                    &plan.candidate
                };
                let sample_name = format!(
                    "{}-{round:04}-{role}",
                    if warmup { "warmup" } else { "pair" }
                );
                let sample_dir = output.join(sample_name);
                fs::create_dir(&sample_dir).map_err(|error| error.to_string())?;
                let mut sample = Sample {
                    role: role.into(),
                    round,
                    warmup,
                    ..Sample::default()
                };
                let mut sample_result = measure(plan, command, &sample_dir, &mut sample);
                if sample_result.is_ok()
                    && matches!(
                        plan.measurement,
                        Measurement::Compiler {
                            require_identical_artifacts: true,
                            ..
                        }
                    )
                {
                    if artifact_identity.is_none() {
                        artifact_identity = sample.artifact_sha256.clone();
                    }
                    if artifact_identity != sample.artifact_sha256 {
                        sample_result = Err(
                            "native artifacts differ; require_identical_artifacts was requested"
                                .into(),
                        );
                    }
                }
                match self::snapshot(plan, plan_path) {
                    Ok(current) if current == snapshot => {}
                    Ok(_) => {
                        sample_result =
                            Err("declared inputs or executables changed during comparison".into())
                    }
                    Err(error) => {
                        sample_result = Err(format!("cannot revalidate declared inputs: {error}"))
                    }
                }
                if let Err(error) = &sample_result {
                    sample.failure = Some(error.clone());
                }
                journal.append(&json!({"event":"sample", "sample":sample}))?;
                sample_result?;
                let value = sample
                    .primary_value
                    .ok_or("successful sample has no primary measurement")?;
                samples.push(sample);
                eprintln!(
                    "{} round {} {role}: {value:.3} {}",
                    if warmup { "warmup" } else { "pair" },
                    round + 1,
                    plan.metric()
                );
            }
        }
    }
    summarize(plan, &samples)
}

pub(crate) fn summarize(plan: &Plan, samples: &[Sample]) -> Result<Report, String> {
    let mut baseline = Vec::with_capacity(plan.pairs);
    let mut candidate = Vec::with_capacity(plan.pairs);
    let mut additional_samples: BTreeMap<String, (Vec<f64>, Vec<f64>)> = [
        "user_cpu_ns",
        "system_cpu_ns",
        "peak_resident_bytes",
        "minor_page_faults",
        "major_page_faults",
        "total_page_faults",
        "cpu_cycles",
    ]
    .into_iter()
    .map(|name| (name.into(), (Vec::new(), Vec::new())))
    .collect();
    for sample in samples {
        let role = sample.role.as_str();
        let value = sample.primary_value.ok_or("missing primary metric")?;
        let warmup = sample.warmup;
        if !warmup {
            if role == "baseline" {
                baseline.push(value);
            } else {
                candidate.push(value);
            }
            if let Some(observation) = sample.observations.first()
                && let Some(resources) = observation.resources.as_object()
            {
                for (name, metric) in resources {
                    if let Some(value) = metric.get("value").and_then(Value::as_u64) {
                        let (before, after) = additional_samples.entry(name.clone()).or_default();
                        if role == "baseline" {
                            before.push(value as f64);
                        } else {
                            after.push(value as f64);
                        }
                    }
                }
            }
            for (name, value) in &sample.compiler_stage_ms {
                let (before, after) = additional_samples
                    .entry(format!("stage_ms:{name}"))
                    .or_default();
                if role == "baseline" {
                    before.push(*value);
                } else {
                    after.push(*value);
                }
            }
            for (name, value) in &sample.compiler_counters {
                let (before, after) = additional_samples
                    .entry(format!("compiler_events:{name}"))
                    .or_default();
                if role == "baseline" {
                    before.push(*value as f64);
                } else {
                    after.push(*value as f64);
                }
            }
            if let Some(bytes) = sample.artifact_bytes {
                let (before, after) = additional_samples
                    .entry("native_image_bytes".into())
                    .or_default();
                if role == "baseline" {
                    before.push(bytes as f64);
                } else {
                    after.push(bytes as f64);
                }
            }
        }
    }
    Ok(Report {
        schema: "omega-performance-report/2",
        name: plan.name.clone(),
        metric: plan.metric(),
        comparison: compare_samples(&baseline, &candidate)?,
        warnings: vec![
            "Within-run bootstrap uncertainty does not cover host drift, contention, or workload selection.",
            "Only declared input paths are hashed. Declare dependency/library/lock inputs used by the workload.",
            "Compiler stage times are inclusive and may overlap; do not sum nested rows.",
            "Observed p95/p99 are nearest-rank sample quantiles; small runs do not resolve population tails.",
        ],
        additional_metrics: additional_samples.into_iter().map(|(name, (before, after))| {
            let metric = if before.len() == plan.pairs && after.len() == plan.pairs {
                match compare_samples(&before, &after) { Ok(comparison) => json!({"status":"measured","comparison":comparison}),
                    Err(reason) => json!({"status":"unavailable","reason":reason}) }
            } else { json!({"status":"unavailable","reason":"metric missing from one or more paired observations"}) };
            (name, metric)
        }).collect(),
    })
}

pub(crate) fn measure(
    plan: &Plan,
    command: &CommandSpec,
    output: &Path,
    sample: &mut Sample,
) -> Result<(), String> {
    let mut arguments = command.arguments.clone();
    let build_dir = output.join("build");
    if let Measurement::Compiler { source, target, .. } = &plan.measurement {
        fs::create_dir(&build_dir).map_err(|error| error.to_string())?;
        arguments.extend([
            "--timings".into(),
            "--build-dir".into(),
            build_dir.display().to_string(),
            "--target".into(),
            target.clone(),
            source.display().to_string(),
        ]);
    }
    let stdin = match &plan.measurement {
        Measurement::Process { stdin, .. } | Measurement::ReportedWork { stdin, .. } => {
            stdin.as_bytes()
        }
        Measurement::Compiler { .. } => &[],
    };
    let (observation, captured) = capture(
        &command.program,
        &arguments,
        &command.environment,
        &plan.working_directory,
        stdin,
        plan.timeout_ms,
    )?;
    let process_wall_ns = observation.process_wall_ns;
    sample.observations.push(observation);
    write_capture(output, "subject", &captured)?;
    match &plan.measurement {
        Measurement::Process { expected, .. } => {
            check_output(expected, &captured)?;
            sample.primary_value = Some(process_wall_ns as f64);
        }
        Measurement::ReportedWork {
            work_units,
            checksum,
            ..
        } => {
            if !captured.status.success() || !captured.stderr.is_empty() {
                return Err("reported work must exit 0 with empty stderr".into());
            }
            let reported: ReportedWork = serde_json::from_slice(&captured.stdout)
                .map_err(|error| format!("invalid timed-work report: {error}"))?;
            if reported.schema != "omega-timed-work/1"
                || reported.work_units != *work_units
                || reported.checksum != *checksum
                || reported.elapsed_ns == 0
                || reported.elapsed_ns > process_wall_ns
            {
                return Err(
                    "timed-work schema, count, checksum, or elapsed time disagrees with its oracle"
                        .into(),
                );
            }
            sample.primary_value = Some(reported.elapsed_ns as f64 / *work_units as f64);
        }
        Measurement::Compiler { expected, .. } => {
            if !captured.status.success() {
                return Err(format!(
                    "compile failed: {:?}; see subject.stderr",
                    captured.status.code()
                ));
            }
            let compiler_text = format!(
                "{}\n{}",
                String::from_utf8_lossy(&captured.stdout),
                String::from_utf8_lossy(&captured.stderr)
            );
            sample.compiler_stage_ms = stage_timings(&compiler_text);
            sample.compiler_counters = phase_counters(&compiler_text);
            let published: Vec<_> = compiler_text
                .lines()
                .filter_map(|line| line.strip_prefix("published native output to "))
                .collect();
            if published.len() != 1 {
                return Err("compile must report exactly one published native output".into());
            }
            let artifact = resolve(&plan.working_directory, Path::new(published[0]))?;
            if !artifact.starts_with(
                build_dir
                    .canonicalize()
                    .map_err(|error| error.to_string())?,
            ) || !artifact.is_file()
            {
                return Err(
                    "published executable must be inside this sample's fresh build directory"
                        .into(),
                );
            }
            sample.artifact_sha256 = Some(file_digest(&artifact)?);
            sample.artifact_relative_path = Some(
                artifact
                    .strip_prefix(output)
                    .map_err(|error| error.to_string())?
                    .display()
                    .to_string()
                    .replace('\\', "/"),
            );
            sample.artifact_bytes = Some(
                fs::metadata(&artifact)
                    .map_err(|error| error.to_string())?
                    .len(),
            );
            let (runtime, result) = capture(
                &artifact,
                &[],
                &command.environment,
                &plan.working_directory,
                &[],
                plan.timeout_ms,
            )?;
            sample.observations.push(runtime);
            write_capture(output, "native", &result)?;
            check_output(expected, &result)?;
            sample.primary_value = Some(process_wall_ns as f64);
        }
    }
    Ok(())
}

fn check_output(expected: &ExpectedOutput, captured: &BoundedProcessOutput) -> Result<(), String> {
    if captured.status.code() != Some(expected.exit_code)
        || captured.stdout != expected.stdout.as_bytes()
        || captured.stderr != expected.stderr.as_bytes()
    {
        return Err(format!(
            "output oracle failed: exit {:?} (expected {}), stdout match {}, stderr match {}; inspect captured streams",
            captured.status.code(),
            expected.exit_code,
            captured.stdout == expected.stdout.as_bytes(),
            captured.stderr == expected.stderr.as_bytes()
        ));
    }
    Ok(())
}

pub(crate) fn stage_timings(text: &str) -> BTreeMap<String, f64> {
    let mut stages = BTreeMap::new();
    for line in text.lines() {
        if let Some((duration, label)) = line.trim().split_once(" ms  ")
            && let Ok(duration) = duration.trim().parse::<f64>()
            && duration.is_finite()
            && duration >= 0.0
        {
            let name = label
                .trim()
                .split_once(" x  ")
                .filter(|(count, _)| count.trim().parse::<u64>().is_ok())
                .map_or(label.trim(), |(_, name)| name);
            *stages.entry(name.to_owned()).or_default() += duration;
        }
    }
    stages
}

pub(crate) fn phase_counters(text: &str) -> BTreeMap<String, u64> {
    let mut counters = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        let timed = line
            .split_once(" ms  ")
            .and_then(|(_, label)| label.trim().split_once(" x  "));
        if let Some((count, name)) = timed.or_else(|| line.split_once(" n  "))
            && let Ok(count) = count.trim().parse::<u64>()
        {
            *counters.entry(name.to_owned()).or_default() += count;
        }
    }
    counters
}

pub(crate) fn snapshot(plan: &Plan, plan_path: &Path) -> Result<BTreeMap<String, String>, String> {
    let mut files = BTreeMap::new();
    for input in plan.inputs.iter().map(PathBuf::as_path).chain([
        plan_path,
        plan.baseline.program.as_path(),
        plan.candidate.program.as_path(),
    ]) {
        collect_inputs(input, &mut files)?;
    }
    Ok(files)
}

fn collect_inputs(path: &Path, files: &mut BTreeMap<String, String>) -> Result<(), String> {
    let metadata =
        fs::symlink_metadata(path).map_err(|error| format!("input {}: {error}", path.display()))?;
    if metadata.file_type().is_symlink() {
        return Err(format!(
            "input symlinks must be declared by their resolved paths: {}",
            path.display()
        ));
    }
    if metadata.is_file() {
        files.insert(path.display().to_string(), file_digest(path)?);
    } else if metadata.is_dir() {
        for entry in fs::read_dir(path).map_err(|error| error.to_string())? {
            collect_inputs(&entry.map_err(|error| error.to_string())?.path(), files)?;
        }
    } else {
        return Err(format!(
            "input is not a regular file/directory: {}",
            path.display()
        ));
    }
    Ok(())
}

fn resolve(directory: &Path, path: &Path) -> Result<PathBuf, String> {
    directory
        .join(path)
        .canonicalize()
        .map_err(|error| format!("resolve {}: {error}", directory.join(path).display()))
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|error| format!("create {}: {error}", path.display()))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_data())
        .map_err(|error| error.to_string())
}

fn write_capture(
    directory: &Path,
    label: &str,
    captured: &BoundedProcessOutput,
) -> Result<(), String> {
    write_new(&directory.join(format!("{label}.stdout")), &captured.stdout)?;
    write_new(&directory.join(format!("{label}.stderr")), &captured.stderr)
}

fn host_target() -> Result<&'static str, String> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => Ok("windows_x86_64"),
        ("linux", "x86_64") => Ok("linux_x86_64"),
        ("linux", "aarch64") => Ok("linux_arm64"),
        ("macos", "aarch64") => Ok("macos_arm64"),
        ("macos", "x86_64") => Ok("macos_x86_64"),
        _ => Err("no hosted Omega native target for this measuring host".into()),
    }
}

fn host_identifier() -> Option<String> {
    std::env::var("COMPUTERNAME")
        .ok()
        .filter(|name| !name.trim().is_empty())
        .or_else(|| {
            std::env::var("HOSTNAME")
                .ok()
                .filter(|name| !name.trim().is_empty())
        })
        .or_else(|| {
            std::fs::read_to_string("/etc/machine-id")
                .ok()
                .filter(|name| !name.trim().is_empty())
        })
        .map(|name| digest(name.as_bytes()))
        .or_else(native_host_identifier)
}

#[cfg(unix)]
fn native_host_identifier() -> Option<String> {
    // macOS need not export HOSTNAME and has no /etc/machine-id. Read the
    // kernel's hostname without launching another process or mutating settings.
    let mut hostname = [0_u8; 256];
    // SAFETY: hostname is writable for the supplied length. Do not read beyond
    // the buffer or accept a truncated name without a terminating NUL.
    if unsafe { libc::gethostname(hostname.as_mut_ptr().cast(), hostname.len()) } != 0 {
        return None;
    }
    let length = hostname.iter().position(|byte| *byte == 0)?;
    (length != 0).then(|| digest(&hostname[..length]))
}

#[cfg(not(unix))]
fn native_host_identifier() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::stage_timings;

    #[cfg(unix)]
    #[test]
    fn native_host_identification_does_not_require_shell_environment() {
        let first = super::native_host_identifier().expect("kernel hostname");
        assert_eq!(first.len(), 64);
        assert_eq!(Some(first), super::native_host_identifier());
    }

    #[test]
    fn keeps_each_samples_stage_values_and_accumulates_repeated_labels() {
        let rows = stage_timings(
            "  1.25 ms  Stage 05\n  2.75 ms  Stage 05\n garbage ms  bad\n NaN ms  bad\n",
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows["Stage 05"], 4.0);
    }
}
