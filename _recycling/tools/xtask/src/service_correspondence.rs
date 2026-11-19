//! Compare the same source-correspondence workloads in saved compiler drivers.
//!
//! Alternate adjacent before/after runs, retain every sample and executable
//! hash, and collect allocation observations in separate instrumented drivers.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use bounded_process::{
    BoundedCaptureBudget, BoundedCaptureLimits, BoundedProcessInput, BoundedProcessLimits,
    BoundedProcessPrepared, run_bounded_process,
};
use serde::Serialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const HELP: &str = "usage: mbx xtask measure-service-correspondence --before PATH --after PATH --allocations-before PATH --allocations-after PATH --pairs N --output PATH";
const HEADER: &str = "workload,methods,sample,iterations,ns_per_validation,allocations_per_validation,requested_bytes_per_validation,peak_extra_live_bytes";

#[derive(Clone, Debug, Serialize)]
struct Measurement {
    workload: String,
    methods: usize,
    sample: usize,
    iterations: usize,
    ns_per_validation: f64,
    allocations_per_validation: f64,
    requested_bytes_per_validation: f64,
    peak_extra_live_bytes: u64,
}

#[derive(Serialize)]
struct Observation {
    pair: usize,
    revision: &'static str,
    rows: Vec<Measurement>,
    peak_rss_bytes: Option<u64>,
    resource_usage: Value,
}

fn parse_rows(text: &str) -> Result<Vec<Measurement>, String> {
    let mut lines = text.lines();
    if lines.next() != Some(HEADER) {
        return Err("measurement driver emitted an unexpected CSV header".into());
    }
    let mut rows = Vec::new();
    for line in lines {
        let fields = line.split(',').collect::<Vec<_>>();
        if fields.len() != 8 || fields[0].is_empty() {
            return Err(format!("invalid measurement row: {line}"));
        }
        let integer = |position: usize| {
            fields[position]
                .parse::<usize>()
                .map_err(|error| format!("invalid integer column {position}: {error}"))
        };
        let number = |position: usize| -> Result<f64, String> {
            let value = fields[position]
                .parse::<f64>()
                .map_err(|error| error.to_string())?;
            if !value.is_finite() || value < 0.0 {
                return Err(format!(
                    "invalid nonnegative measurement: {}",
                    fields[position]
                ));
            }
            Ok(value)
        };
        let row = Measurement {
            workload: fields[0].into(),
            methods: integer(1)?,
            sample: integer(2)?,
            iterations: integer(3)?,
            ns_per_validation: number(4)?,
            allocations_per_validation: number(5)?,
            requested_bytes_per_validation: number(6)?,
            peak_extra_live_bytes: fields[7]
                .parse()
                .map_err(|error| format!("invalid heap observation: {error}"))?,
        };
        if row.iterations == 0 || row.ns_per_validation == 0.0 {
            return Err("measurement requires positive iterations and elapsed time".into());
        }
        rows.push(row);
    }
    if rows.is_empty() {
        return Err("measurement driver emitted no rows".into());
    }
    Ok(rows)
}

fn observe(
    binary: &Path,
    pair: usize,
    revision: &'static str,
    allocations: bool,
) -> Result<Observation, String> {
    let mut command = Command::new(binary);
    command.args(["--samples", "1"]);
    if allocations {
        command.args(["--iterations", "1"]);
    }
    let memory = 4 * 1024 * 1024 * 1024;
    let mut prepared = BoundedProcessPrepared::new(
        command,
        BoundedProcessLimits::new(120, memory, 16 * 1024 * 1024, 128, 8, memory, memory),
        "service correspondence measurement",
    )
    .map_err(|error| error.to_string())?;
    prepared.observe_resources();
    let output = run_bounded_process(
        prepared,
        BoundedProcessInput::Null,
        BoundedCaptureLimits::new(
            1024 * 1024,
            1024 * 1024,
            Duration::from_secs(300),
            Duration::from_secs(5),
            Duration::from_millis(5),
        ),
        BoundedCaptureBudget::new(2 * 1024 * 1024),
    )
    .map_err(|error| format!("{}: {error}", binary.display()))?;
    if !output.status.success() {
        return Err(format!(
            "{} exited {:?}: {}",
            binary.display(),
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let text = std::str::from_utf8(&output.stdout).map_err(|error| error.to_string())?;
    let peak_rss_bytes = output
        .resource_usage
        .as_ref()
        .and_then(|usage| usage.peak_resident_bytes);
    let resource_usage = output.resource_usage.as_ref().map(|usage| json!({
        "provider":usage.provider, "scope":usage.scope, "peak_resident_bytes":usage.peak_resident_bytes,
        "user_cpu_ns":usage.user_cpu_ns, "system_cpu_ns":usage.system_cpu_ns,
        "cpu_cycles":usage.cpu_cycles, "error":usage.error,
    })).unwrap_or(Value::Null);
    Ok(Observation {
        pair,
        revision,
        rows: parse_rows(text)?,
        peak_rss_bytes,
        resource_usage,
    })
}

fn same_workloads(left: &[Measurement], right: &[Measurement]) -> Result<(), String> {
    if left.len() != right.len() {
        return Err(format!(
            "workload row counts differ: {} before, {} after",
            left.len(),
            right.len()
        ));
    }
    for (position, (left, right)) in left.iter().zip(right).enumerate() {
        if left.workload != right.workload || left.methods != right.methods {
            return Err(format!(
                "workload {position} differs: {} ({} methods) before, {} ({} methods) after",
                left.workload, left.methods, right.workload, right.methods
            ));
        }
    }
    Ok(())
}

fn median(mut values: Vec<f64>) -> f64 {
    values.sort_unstable_by(f64::total_cmp);
    let middle = values.len() / 2;
    if values.len().is_multiple_of(2) {
        (values[middle - 1] + values[middle]) / 2.0
    } else {
        values[middle]
    }
}

fn digest(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|error| error.to_string())?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn compare(arguments: &[String]) -> Result<(), String> {
    let mut paths: [Option<PathBuf>; 5] = std::array::from_fn(|_| None);
    let mut pairs = 9usize;
    let mut arguments = arguments.iter();
    while let Some(option) = arguments.next() {
        let position = match option.as_str() {
            "--before" => Some(0),
            "--after" => Some(1),
            "--allocations-before" => Some(2),
            "--allocations-after" => Some(3),
            "--output" => Some(4),
            "--pairs" => None,
            other => return Err(format!("unknown option `{other}`; {HELP}")),
        };
        let value = arguments
            .next()
            .ok_or_else(|| format!("{option} requires a value"))?;
        if let Some(position) = position {
            if paths[position].replace(value.into()).is_some() {
                return Err(format!("repeated option {option}"));
            }
        } else {
            pairs = value
                .parse()
                .map_err(|error| format!("invalid pair count: {error}"))?;
        }
    }
    if !(1..=1000).contains(&pairs) {
        return Err("--pairs must be between 1 and 1000".into());
    }
    let mut paths = paths
        .into_iter()
        .map(|path| path.ok_or_else(|| HELP.to_owned()))
        .collect::<Result<Vec<_>, _>>()?;
    for path in &mut paths[..4] {
        *path = path
            .canonicalize()
            .map_err(|error| format!("{}: {error}", path.display()))?;
    }
    let mut observations = Vec::with_capacity(2 * pairs);
    for pair in 0..pairs {
        let order = if pair.is_multiple_of(2) {
            [0, 1]
        } else {
            [1, 0]
        };
        for revision in order {
            observations.push(observe(
                &paths[revision],
                pair,
                ["before", "after"][revision],
                false,
            )?);
        }
    }
    let allocations_before = observe(&paths[2], 0, "before", true)?;
    let allocations_after = observe(&paths[3], 0, "after", true)?;
    let expected = &observations[0].rows;
    for observation in observations
        .iter()
        .chain([&allocations_before, &allocations_after])
    {
        same_workloads(expected, &observation.rows)?;
    }
    let mut summary = Vec::with_capacity(expected.len());
    for (workload_position, expected) in expected.iter().enumerate() {
        let timings = |revision| {
            observations
                .iter()
                .filter(|observation| observation.revision == revision)
                .map(|observation| observation.rows[workload_position].ns_per_validation)
                .collect::<Vec<_>>()
        };
        let before = timings("before");
        let after = timings("after");
        let before_allocations = &allocations_before.rows[workload_position];
        let after_allocations = &allocations_after.rows[workload_position];
        summary.push(json!({
            "workload":expected.workload, "methods":expected.methods,
            "before_median_ns":median(before.clone()), "after_median_ns":median(after.clone()),
            "median_paired_before_over_after":median(before.iter().zip(&after).map(|(before, after)| before / after).collect()),
            "before_allocations":before_allocations.allocations_per_validation, "after_allocations":after_allocations.allocations_per_validation,
            "before_requested_bytes":before_allocations.requested_bytes_per_validation, "after_requested_bytes":after_allocations.requested_bytes_per_validation,
            "before_peak_extra_live_bytes":before_allocations.peak_extra_live_bytes, "after_peak_extra_live_bytes":after_allocations.peak_extra_live_bytes,
        }));
    }
    let mut binaries = serde_json::Map::new();
    for (label, path) in ["before", "after", "allocations_before", "allocations_after"]
        .into_iter()
        .zip(&paths)
    {
        binaries.insert(label.into(), json!({"path":path, "sha256":digest(path)?}));
    }
    crate::execution::write_json(
        &paths[4],
        &json!({
            "schema":"omega.service-correspondence.measurements.v1", "binaries":binaries,
            "pairs":pairs, "runs":observations, "allocations":{"before":allocations_before,"after":allocations_after},
            "summary":summary,
        }),
    )?;
    println!(
        "{}",
        serde_json::to_string_pretty(&summary).map_err(|error| error.to_string())?
    );
    Ok(())
}

pub fn run(arguments: &[String]) -> Result<(), u8> {
    if arguments
        .iter()
        .any(|argument| matches!(argument.as_str(), "--help" | "-h"))
    {
        println!("{HELP}");
        return Ok(());
    }
    compare(arguments).map_err(|error| {
        eprintln!("measure-service-correspondence: {error}");
        1
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_or_nonfinite_measurements_refuse() {
        for row in [
            "one,1,0,1,NaN,0,0,0",
            "one,1,0,0,1,0,0,0",
            "one,1,0,1,1,0,0",
        ] {
            assert!(parse_rows(&format!("{HEADER}\n{row}\n")).is_err());
        }
        assert!(parse_rows(HEADER).is_err());
    }

    #[test]
    fn different_workload_payloads_do_not_form_a_comparison() {
        let before = parse_rows(&format!("{HEADER}\none,1,0,1,2,53,2346,499\n")).unwrap();
        let after = parse_rows(&format!("{HEADER}\none,2,0,1,2,53,2346,499\n")).unwrap();
        assert!(same_workloads(&before, &after).is_err());
        assert_eq!(median(vec![3.0, 1.0, 2.0]), 2.0);
        assert_eq!(median(vec![4.0, 1.0, 3.0, 2.0]), 2.5);
    }
}
