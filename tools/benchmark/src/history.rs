//! Compare compatible completed runs without pretending their samples were paired.

use crate::plan::Plan;
use crate::{records, statistics};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::Path;

pub fn compare(before: &Path, after: &Path) -> Result<Value, String> {
    let before = records::validate(before)?;
    let after = records::validate(after)?;
    if before.header["host"] != after.header["host"]
        || !before.header["host"]["host_id_sha256"].is_string()
    {
        return Err("historical comparisons require the same identified host".into());
    }
    if before.header["inherited_settings"] != after.header["inherited_settings"] {
        return Err("historical inherited measurement settings changed".into());
    }
    if logical_plan(&before.plan)? != logical_plan(&after.plan)? {
        return Err("historical workload, oracle, arguments, or settings changed".into());
    }
    if input_closure(&before)? != input_closure(&after)? {
        return Err("historical declared workload/library input bytes changed".into());
    }
    let baseline: Vec<_> = before
        .samples
        .iter()
        .filter(|sample| !sample.warmup && sample.role == "candidate")
        .filter_map(|sample| sample.primary_value)
        .collect();
    let candidate: Vec<_> = after
        .samples
        .iter()
        .filter(|sample| !sample.warmup && sample.role == "candidate")
        .filter_map(|sample| sample.primary_value)
        .collect();
    Ok(
        json!({"schema":"omega-performance-history/1","comparison_kind":"unpaired_independent_runs",
        "metric":before.report["metric"],"baseline_observations":baseline.len(),"candidate_observations":candidate.len(),
        "comparison":statistics::compare_independent(&baseline,&candidate)?,
        "warning":"Historical runs are not interleaved; their interval does not account for host drift or autocorrelation. Use fresh paired comparisons to establish a regression."}),
    )
}

fn logical_plan(plan: &Plan) -> Result<Value, String> {
    let mut measurement =
        serde_json::to_value(&plan.measurement).map_err(|error| error.to_string())?;
    for name in ["source", "candidate_source", "library_root"] {
        if let Some(path) = measurement.get_mut(name) {
            let original = std::path::Path::new(path.as_str().ok_or("invalid measurement path")?);
            *path = json!(
                original
                    .strip_prefix(&plan.working_directory)
                    .unwrap_or(original)
                    .display()
                    .to_string()
            );
        }
    }
    let mut environment = plan.candidate.environment.clone();
    environment.remove("OMEGA_LIBRARY_ROOT");
    Ok(
        json!({"measurement":measurement,"arguments":plan.candidate.arguments,"environment":environment,"timeout_ms":plan.timeout_ms,"warmups":plan.warmups}),
    )
}

fn input_closure(run: &records::VerifiedRun) -> Result<BTreeMap<String, Value>, String> {
    let mut closure = BTreeMap::new();
    for (input_index, input) in run.plan.inputs.iter().enumerate() {
        for (path, digest) in run
            .identities
            .as_object()
            .ok_or("missing input identity map")?
        {
            let path = std::path::Path::new(path);
            if path == input {
                closure.insert(format!("{input_index}/file"), digest.clone());
            } else if let Ok(relative) = path.strip_prefix(input) {
                closure.insert(
                    format!("{input_index}/{}", relative.display()),
                    digest.clone(),
                );
            }
        }
    }
    Ok(closure)
}
