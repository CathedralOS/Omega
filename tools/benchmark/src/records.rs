//! Verify retained evidence before showing results or using them as regression gates.

use crate::experiment::{Sample, phase_counters, stage_timings, summarize};
use crate::plan::{ExpectedOutput, Measurement, Plan};
use crate::process::{digest, file_digest};
use serde_json::Value;
use std::fs;
use std::path::{Component, Path};

pub struct VerifiedRun {
    pub report: Value,
    pub plan: Plan,
    pub header: Value,
    pub samples: Vec<Sample>,
    pub identities: Value,
}

fn read(path: &Path) -> Result<Vec<u8>, String> {
    let metadata =
        fs::metadata(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    if metadata.len() > 64 * 1024 * 1024 {
        return Err(format!("evidence file exceeds 64 MiB: {}", path.display()));
    }
    fs::read(path).map_err(|error| error.to_string())
}

pub fn validate(directory: &Path) -> Result<VerifiedRun, String> {
    let encoded = read(&directory.join("journal.jsonl"))?;
    let mut events = Vec::new();
    for event in serde_json::Deserializer::from_slice(&encoded).into_iter::<Value>() {
        events.push(event.map_err(|error| format!("invalid journal: {error}"))?);
        if events.len() > 5000 {
            return Err("too many journal events".into());
        }
    }
    let header = events.first().ok_or("empty journal")?;
    let complete = events.last().ok_or("empty journal")?;
    if header["event"] != "start"
        || header["schema"] != "omega-performance-run/2"
        || complete["event"] != "complete"
    {
        return Err("a completed version-2 comparison journal is required".into());
    }
    let bytes = read(&directory.join("report.json"))?;
    if complete["report_sha256"].as_str() != Some(digest(&bytes).as_str()) {
        return Err("report digest disagrees with journal".into());
    }
    let report: Value = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
    let plan: Plan =
        serde_json::from_value(header["plan"].clone()).map_err(|error| error.to_string())?;
    plan.validate()?;
    if events.iter().any(|event| {
        !matches!(
            event["event"].as_str(),
            Some("start" | "identities" | "compiler_identity" | "sample" | "complete")
        )
    }) {
        return Err("unknown journal event".into());
    }
    if matches!(plan.measurement, Measurement::Compiler { .. }) {
        for role in ["baseline", "candidate"] {
            let profiles: Vec<_> = events
                .iter()
                .filter(|event| event["event"] == "compiler_identity" && event["role"] == role)
                .collect();
            if profiles.len() != 1 {
                return Err("one compiler identity per role required".into());
            }
            let recorded = &profiles[0]["profile"];
            if recorded["schema"] != "omega-compiler-build-profile-v1"
                || !matches!(
                    recorded["opt_level"].as_str(),
                    Some("1" | "2" | "3" | "s" | "z")
                )
            {
                return Err("compiler identity lacks optimized build profile".into());
            }
            let raw: Value =
                serde_json::from_slice(&read(&directory.join(format!("{role}.profile.stdout")))?)
                    .map_err(|error| error.to_string())?;
            if &raw != recorded {
                return Err("compiler profile differs from retained output".into());
            }
            let version = read(&directory.join(format!("{role}.version.stdout")))?;
            if profiles[0]["version"].as_str() != Some(String::from_utf8_lossy(&version).as_ref()) {
                return Err("compiler version differs from retained output".into());
            }
        }
    } else if events
        .iter()
        .any(|event| event["event"] == "compiler_identity")
    {
        return Err("runtime journal contains compiler identities".into());
    }
    let identities: Vec<_> = events
        .iter()
        .filter(|event| event["event"] == "identities")
        .collect();
    if identities.len() != 1 {
        return Err("exactly one executable/input identity record required".into());
    }
    for command in [&plan.baseline, &plan.candidate] {
        if !is_sha256(
            identities[0]["sha256"][command.program.display().to_string()]
                .as_str()
                .unwrap_or(""),
        ) {
            return Err("missing executable identity".into());
        }
    }
    let samples: Vec<Sample> = events
        .iter()
        .filter(|event| event["event"] == "sample")
        .map(|event| {
            serde_json::from_value(event["sample"].clone()).map_err(|error| error.to_string())
        })
        .collect::<Result<_, _>>()?;
    if samples.len() != 2 * (plan.warmups + plan.pairs) {
        return Err("incomplete sample set".into());
    }
    let expected_events = 3
        + samples.len()
        + if matches!(plan.measurement, Measurement::Compiler { .. }) {
            2
        } else {
            0
        };
    if events.len() != expected_events {
        return Err("unexpected, duplicated, or failed journal events".into());
    }
    let mut position = 0;
    let mut artifact_identity = None;
    for warmup in [true, false] {
        for round in 0..if warmup { plan.warmups } else { plan.pairs } {
            for role in if round.is_multiple_of(2) {
                ["baseline", "candidate"]
            } else {
                ["candidate", "baseline"]
            } {
                let sample = &samples[position];
                position += 1;
                if sample.role != role
                    || sample.round != round
                    || sample.warmup != warmup
                    || sample.failure.is_some()
                {
                    return Err("sample roles/order/validity disagree with plan".into());
                }
                let command = if role == "baseline" {
                    &plan.baseline
                } else {
                    &plan.candidate
                };
                let observations = if matches!(plan.measurement, Measurement::Compiler { .. }) {
                    2
                } else {
                    1
                };
                if sample.observations.len() != observations {
                    return Err("missing process or native observation".into());
                }
                let subject = &sample.observations[0];
                if subject.program != command.program.display().to_string()
                    || !subject.arguments.starts_with(&command.arguments)
                    || subject.process_wall_ns == 0
                {
                    return Err("subject command or timing disagrees with plan".into());
                }
                if observations == 1 && subject.arguments != command.arguments {
                    return Err("runtime arguments disagree with plan".into());
                }
                let sample_dir = directory.join(format!(
                    "{}-{round:04}-{role}",
                    if warmup { "warmup" } else { "pair" }
                ));
                let mut captures = Vec::new();
                for (observation, label) in sample.observations.iter().zip(["subject", "native"]) {
                    let stdout = read(&sample_dir.join(format!("{label}.stdout")))?;
                    let stderr = read(&sample_dir.join(format!("{label}.stderr")))?;
                    if observation.stdout_sha256 != digest(&stdout)
                        || observation.stderr_sha256 != digest(&stderr)
                    {
                        return Err("captured stream digest mismatch".into());
                    }
                    captures.push((stdout, stderr));
                }
                let primary = match &plan.measurement {
                    Measurement::Process { expected, .. } => {
                        oracle(expected, subject.exit_code, &captures[0])?;
                        subject.process_wall_ns as f64
                    }
                    Measurement::ReportedWork {
                        work_units,
                        checksum,
                        ..
                    } => {
                        if subject.exit_code != Some(0) || !captures[0].1.is_empty() {
                            return Err("timed-work process oracle failed".into());
                        }
                        let reported: Value = serde_json::from_slice(&captures[0].0)
                            .map_err(|error| error.to_string())?;
                        let elapsed = reported["elapsed_ns"]
                            .as_u64()
                            .ok_or("missing elapsed time")?;
                        if reported["schema"] != "omega-timed-work/1"
                            || reported["work_units"] != *work_units
                            || reported["checksum"] != *checksum
                            || elapsed == 0
                            || elapsed > subject.process_wall_ns
                        {
                            return Err("timed-work count, checksum or timer failed".into());
                        }
                        elapsed as f64 / *work_units as f64
                    }
                    Measurement::Compiler {
                        expected,
                        require_identical_artifacts,
                        target,
                        ..
                    } => {
                        let source = plan
                            .measurement
                            .compiler_source(role)
                            .ok_or("missing compiler source")?;
                        if subject.exit_code != Some(0) {
                            return Err("compiler observation failed".into());
                        }
                        let text = format!(
                            "{}\n{}",
                            String::from_utf8_lossy(&captures[0].0),
                            String::from_utf8_lossy(&captures[0].1)
                        );
                        if sample.compiler_stage_ms != stage_timings(&text)
                            || sample.compiler_counters != phase_counters(&text)
                        {
                            return Err(
                                "compiler attribution disagrees with captured output".into()
                            );
                        }
                        let suffix = &subject.arguments[command.arguments.len()..];
                        if suffix.len() != 6
                            || suffix[0] != "--timings"
                            || suffix[1] != "--build-dir"
                            || suffix[3] != "--target"
                            || suffix[5] != source.display().to_string()
                            || suffix[4] != *target
                        {
                            return Err("compiler product arguments disagree with plan".into());
                        }
                        oracle(expected, sample.observations[1].exit_code, &captures[1])?;
                        let relative = Path::new(
                            sample
                                .artifact_relative_path
                                .as_deref()
                                .ok_or("missing artifact path")?,
                        );
                        if relative
                            .components()
                            .any(|part| !matches!(part, Component::Normal(_)))
                        {
                            return Err("invalid relative artifact path".into());
                        }
                        let native = &sample.observations[1];
                        let published: Vec<_> = text
                            .lines()
                            .filter_map(|line| line.strip_prefix("published native output to "))
                            .collect();
                        let recorded_build = Path::new(&suffix[2]);
                        let published_image = recorded_build
                            .parent()
                            .ok_or("missing recorded sample directory")?
                            .join(relative);
                        if !native.arguments.is_empty()
                            || Path::new(&native.program) != published_image
                            || published.len() != 1
                            || Path::new(published[0]) != Path::new(&native.program)
                            || !published_image.starts_with(recorded_build)
                        {
                            return Err(
                                "native execution does not name the published compiler artifact"
                                    .into(),
                            );
                        }
                        let artifact = sample_dir.join(relative);
                        if sample.artifact_sha256.as_deref()
                            != Some(file_digest(&artifact)?.as_str())
                            || sample.artifact_bytes
                                != Some(
                                    fs::metadata(&artifact)
                                        .map_err(|error| error.to_string())?
                                        .len(),
                                )
                        {
                            return Err("native image identity mismatch".into());
                        }
                        if *require_identical_artifacts {
                            if artifact_identity.is_none() {
                                artifact_identity = sample.artifact_sha256.clone();
                            }
                            if artifact_identity != sample.artifact_sha256 {
                                return Err("required native image equality failed".into());
                            }
                        }
                        subject.process_wall_ns as f64
                    }
                };
                if sample.primary_value != Some(primary) {
                    return Err("primary metric disagrees with observations".into());
                }
            }
        }
    }
    let recomputed =
        serde_json::to_value(summarize(&plan, &samples)?).map_err(|error| error.to_string())?;
    if report != recomputed {
        return Err("summary disagrees with retained paired samples".into());
    }
    Ok(VerifiedRun {
        report,
        plan,
        header: header.clone(),
        samples,
        identities: identities[0]["sha256"].clone(),
    })
}

fn oracle(
    expected: &ExpectedOutput,
    exit: Option<i32>,
    capture: &(Vec<u8>, Vec<u8>),
) -> Result<(), String> {
    if exit != Some(expected.exit_code)
        || capture.0 != expected.stdout.as_bytes()
        || capture.1 != expected.stderr.as_bytes()
    {
        return Err("retained authored result oracle failed".into());
    }
    Ok(())
}
fn is_sha256(digest: &str) -> bool {
    digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// 0: established within budget; 2: inconclusive; 3: established regression.
pub fn regression_exit(report: &Value, maximum_percent: f64) -> Result<u8, String> {
    if !maximum_percent.is_finite() || maximum_percent < 0.0 {
        return Err("regression budget must be finite and nonnegative".into());
    }
    let comparison = &report["comparison"];
    if comparison["pairs"].as_u64().unwrap_or(0) < 6 {
        return Ok(2);
    }
    let Some(interval) = comparison["paired_percent_change_bootstrap_95"].as_array() else {
        return Ok(2);
    };
    let lower = interval
        .first()
        .and_then(Value::as_f64)
        .ok_or("invalid interval")?;
    let upper = interval
        .get(1)
        .and_then(Value::as_f64)
        .ok_or("invalid interval")?;
    if lower > maximum_percent {
        Ok(3)
    } else if upper <= maximum_percent {
        Ok(0)
    } else {
        Ok(2)
    }
}

pub fn markdown(run: &VerifiedRun) -> String {
    let mut text = format!(
        "# {}\n\nPrimary metric: `{}`\n\n| Metric | Baseline median | Candidate median | Paired change |\n| --- | ---: | ---: | ---: |\n",
        run.plan.name,
        run.report["metric"].as_str().unwrap_or("unknown")
    );
    let row = |name: &str, comparison: &Value| {
        format!(
            "| {} | {} | {} | {} |\n",
            name.replace('|', "\\|"),
            comparison["baseline"]["median"],
            comparison["candidate"]["median"],
            comparison["paired_percent_change"]["median"]
                .as_f64()
                .map_or("unavailable".into(), |value| format!("{value:+.2}%"))
        )
    };
    text.push_str(&row("primary", &run.report["comparison"]));
    if let Some(metrics) = run.report["additional_metrics"].as_object() {
        for (name, metric) in metrics {
            if metric["status"] == "measured" {
                text.push_str(&row(name, &metric["comparison"]));
            }
        }
    }
    text.push_str(
        "\nScope and limitations: see report warnings and each observation's resource provider.\n",
    );
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn gates_regressions_without_promoting_uncertain_or_small_runs() {
        let mut report =
            json!({"comparison":{"pairs":12,"paired_percent_change_bootstrap_95":[8.0,12.0]}});
        assert_eq!(regression_exit(&report, 5.0).unwrap(), 3);
        assert_eq!(regression_exit(&report, 10.0).unwrap(), 2);
        assert_eq!(regression_exit(&report, 15.0).unwrap(), 0);
        report["comparison"]["pairs"] = json!(2);
        assert_eq!(regression_exit(&report, 5.0).unwrap(), 2);
        assert!(regression_exit(&report, f64::NAN).is_err());
    }
}
