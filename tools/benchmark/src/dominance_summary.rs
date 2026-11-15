//! Validate the focused graph driver's retained pairs before summarizing them.

use crate::{process::digest, statistics};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

#[derive(Deserialize)]
struct Metadata {
    schema: String,
    pairs: usize,
    warmup_pairs: usize,
    cases: Vec<Case>,
}

#[derive(Deserialize, PartialEq, Eq, PartialOrd, Ord)]
struct Case {
    shape: String,
    blocks: usize,
}

#[derive(Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
enum Role {
    Baseline,
    Candidate,
}

#[derive(Deserialize)]
struct Observation {
    shape: String,
    blocks: usize,
    edges: usize,
    queries: usize,
    checksum: u64,
    role: Role,
    round: usize,
    warmup: bool,
    build_ns: u64,
    query_ns: u64,
    retained_bytes: u64,
    peak_working_set_bytes: u64,
    cpu_ns: u64,
}

#[derive(Deserialize)]
struct Completion {
    observations_sha256: String,
}

const METRICS: [&str; 5] = [
    "build_ns",
    "query_ns",
    "retained_bytes",
    "peak_working_set_bytes",
    "cpu_ns",
];

impl Observation {
    fn metrics(&self) -> [f64; 5] {
        [
            self.build_ns as f64,
            self.query_ns as f64,
            self.retained_bytes as f64,
            self.peak_working_set_bytes as f64,
            self.cpu_ns as f64,
        ]
    }
}

fn json_bytes(bytes: &[u8]) -> &[u8] {
    bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes)
}

fn decode(metadata: &[u8], observations: &[u8], complete: &[u8]) -> Result<Value, String> {
    let metadata: Metadata = serde_json::from_slice(json_bytes(metadata))
        .map_err(|error| format!("invalid graph metadata: {error}"))?;
    let complete: Completion = serde_json::from_slice(json_bytes(complete))
        .map_err(|error| format!("invalid graph completion: {error}"))?;
    if !digest(observations).eq_ignore_ascii_case(&complete.observations_sha256) {
        return Err("graph observation digest does not match completion".into());
    }
    if metadata.schema != "omega-dominance-comparison/1"
        || !(6..=1000).contains(&metadata.pairs)
        || metadata.warmup_pairs != 1
        || metadata.cases.is_empty()
        || metadata.cases.len() > 256
        || metadata.cases.iter().collect::<BTreeSet<_>>().len() != metadata.cases.len()
        || metadata
            .cases
            .iter()
            .any(|case| case.shape.is_empty() || case.blocks < 4)
    {
        return Err("invalid graph schema, pair count or workload roster".into());
    }
    let rows = serde_json::Deserializer::from_slice(observations)
        .into_iter::<Observation>()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("invalid graph observation: {error}"))?;
    let case_count = (metadata.pairs + 1) * 2;
    if rows.len() != metadata.cases.len() * case_count {
        return Err("graph observations do not contain every warmup and pair".into());
    }
    let mut summaries = Vec::with_capacity(metadata.cases.len());
    for (case, rows) in metadata.cases.iter().zip(rows.chunks_exact(case_count)) {
        for (position, row) in rows.iter().enumerate() {
            let warmup = position < 2;
            let round = if warmup { 0 } else { (position - 2) / 2 };
            let first_role = if round.is_multiple_of(2) {
                Role::Baseline
            } else {
                Role::Candidate
            };
            let role = if position.is_multiple_of(2) {
                first_role
            } else if first_role == Role::Baseline {
                Role::Candidate
            } else {
                Role::Baseline
            };
            if row.shape != case.shape
                || row.blocks != case.blocks
                || row.warmup != warmup
                || row.round != round
                || row.role != role
                || row.queries != 100_000
                || row.checksum > row.queries as u64
                || row.checksum != rows[0].checksum
                || row.edges != rows[0].edges
                || row.metrics()[..4].contains(&0.0)
            {
                return Err(format!(
                    "invalid observation or pair order for {} / {} at {position}",
                    case.shape, case.blocks
                ));
            }
        }
        let mut metrics = serde_json::Map::new();
        for (metric, name) in METRICS.iter().enumerate() {
            let mut before = Vec::with_capacity(metadata.pairs);
            let mut after = Vec::with_capacity(metadata.pairs);
            for pair in rows[2..].as_chunks::<2>().0 {
                let (baseline, candidate) = if pair[0].role == Role::Baseline {
                    (&pair[0], &pair[1])
                } else {
                    (&pair[1], &pair[0])
                };
                before.push(baseline.metrics()[metric]);
                after.push(candidate.metrics()[metric]);
            }
            let comparison = statistics::compare(&before, &after)?;
            let mut summary = json!({
                "baseline_median": comparison.baseline.median,
                "candidate_median": comparison.candidate.median,
                "bootstrap_resamples": comparison.bootstrap_resamples,
                "bootstrap_seed": comparison.bootstrap_seed,
            });
            if let Some(percentages) = comparison.paired_percent_change {
                summary["paired_median_change_percent"] = json!(percentages.median);
                summary["paired_percentile_interval_95"] =
                    json!(comparison.paired_percent_change_bootstrap_95);
            } else {
                summary["interval_unavailable"] =
                    json!("baseline includes zero (OS CPU accounting resolution)");
            }
            metrics.insert((*name).into(), summary);
        }
        summaries.push(
            json!({"shape":case.shape,"blocks":case.blocks,"pairs":metadata.pairs,
            "checksum":rows[0].checksum,"metrics":metrics}),
        );
    }
    Ok(Value::Array(summaries))
}

pub fn run(directory: &Path) -> Result<Value, String> {
    let read = |name| {
        let path = directory.join(name);
        let metadata =
            fs::metadata(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        if metadata.len() > 64 * 1024 * 1024 {
            return Err(format!(
                "graph evidence file exceeds 64 MiB: {}",
                path.display()
            ));
        }
        fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))
    };
    decode(
        &read("metadata.json")?,
        &read("observations.jsonl")?,
        &read("complete.json")?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (Value, Vec<Value>) {
        let metadata = json!({"schema":"omega-dominance-comparison/1","pairs":6,"warmup_pairs":1,
            "cases":[{"shape":"chain","blocks":4}]});
        let mut rows = Vec::new();
        for warmup in [true, false] {
            for round in 0usize..if warmup { 1 } else { 6 } {
                let roles = if round.is_multiple_of(2) {
                    ["baseline", "candidate"]
                } else {
                    ["candidate", "baseline"]
                };
                for role in roles {
                    rows.push(
                        json!({"shape":"chain","blocks":4,"edges":3,"queries":100000,
                        "checksum":42,"role":role,"round":round,"warmup":warmup,
                        "build_ns":if role=="baseline" {100} else {50},"query_ns":1000,
                        "retained_bytes":1024,"peak_working_set_bytes":4096,"cpu_ns":0}),
                    );
                }
            }
        }
        (metadata, rows)
    }

    fn analyze(metadata: &Value, rows: &[Value], corrupt_digest: bool) -> Result<Value, String> {
        let raw = rows
            .iter()
            .map(|row| format!("{row}\n"))
            .collect::<String>();
        let complete = json!({"observations_sha256":if corrupt_digest {"bad".into()} else {digest(raw.as_bytes())}});
        decode(
            &serde_json::to_vec(metadata).unwrap(),
            raw.as_bytes(),
            &serde_json::to_vec(&complete).unwrap(),
        )
    }

    #[test]
    fn known_pairs_use_shared_statistics_and_keep_zero_cpu_explicit() {
        let (metadata, rows) = fixture();
        let report = analyze(&metadata, &rows, false).unwrap();
        assert_eq!(report[0]["metrics"]["build_ns"]["baseline_median"], 100.0);
        assert_eq!(
            report[0]["metrics"]["build_ns"]["paired_median_change_percent"],
            -50.0
        );
        assert_eq!(
            report[0]["metrics"]["build_ns"]["paired_percentile_interval_95"],
            json!([-50.0, -50.0])
        );
        assert!(report[0]["metrics"]["cpu_ns"]["interval_unavailable"].is_string());
    }

    #[test]
    fn changed_answers_and_pair_order_are_refused() {
        let (metadata, mut rows) = fixture();
        rows[3]["checksum"] = json!(43);
        assert!(analyze(&metadata, &rows, false).is_err());
        let (_, mut rows) = fixture();
        rows.swap(2, 3);
        assert!(analyze(&metadata, &rows, false).is_err());
        let (_, mut rows) = fixture();
        for row in &mut rows {
            row["checksum"] = json!(100_001);
        }
        assert!(analyze(&metadata, &rows, false).is_err());
    }

    #[test]
    fn missing_resources_and_changed_digest_are_refused() {
        let (metadata, mut rows) = fixture();
        assert!(analyze(&metadata, &rows, true).is_err());
        rows[2]["peak_working_set_bytes"] = Value::Null;
        assert!(analyze(&metadata, &rows, false).is_err());
        let (_, mut rows) = fixture();
        rows[2]["build_ns"] = json!(100.5);
        assert!(analyze(&metadata, &rows, false).is_err());
    }

    #[test]
    fn missing_or_duplicate_workloads_are_refused() {
        let (mut metadata, rows) = fixture();
        metadata["cases"] = json!([{"shape":"chain","blocks":4},{"shape":"chain","blocks":4}]);
        assert!(analyze(&metadata, &rows, false).is_err());
        let (metadata, mut rows) = fixture();
        rows.pop();
        assert!(analyze(&metadata, &rows, false).is_err());
    }
}
