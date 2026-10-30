/*
   Profiling is a separate, correctness-checked execution, not a timing sample
   mixed into an A/B experiment. The same plan and measurement owner execute
   the subject. Phase/allocation observations and external profiler artifacts
   retain their source/binary identities. Windows recordings use a unique WPR
   instance; no command touches the user's default recording or system settings.
*/
use crate::plan::Measurement;
use crate::{experiment, process};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

struct OwnedWpr {
    instance: String,
    directory: PathBuf,
    working_directory: PathBuf,
    closed: bool,
}
impl Drop for OwnedWpr {
    fn drop(&mut self) {
        if self.closed {
            return;
        }
        let result = process::capture(
            Path::new("wpr.exe"),
            &[
                "-cancel".into(),
                "-instancename".into(),
                self.instance.clone(),
            ],
            &BTreeMap::new(),
            &self.working_directory,
            &[],
            30000,
        );
        let message = match result {
            Ok((_, output)) => format!(
                "exit {:?}\n{}\n{}",
                output.status.code(),
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ),
            Err(error) => error,
        };
        let _ = std::fs::write(self.directory.join("wpr-owned-cleanup.txt"), message);
    }
}

pub fn run(plan_path: &Path, role: &str, backend: &str, directory: &Path) -> Result<bool, String> {
    if !matches!(role, "baseline" | "candidate") {
        return Err("profile role must be baseline or candidate".into());
    }
    if !matches!(
        backend,
        "phase" | "allocations" | "wpr" | "perf-stat" | "perf-record"
    ) {
        return Err(
            "profile backend must be phase, allocations, wpr, perf-stat or perf-record".into(),
        );
    }
    let (mut plan, path, plan_sha256) = experiment::prepare_plan(plan_path)?;
    let directory_parent = directory
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(directory_parent).map_err(|error| error.to_string())?;
    let directory = directory_parent
        .canonicalize()
        .map_err(|error| error.to_string())?
        .join(
            directory
                .file_name()
                .ok_or("profile directory needs a name")?,
        );
    if plan
        .inputs
        .iter()
        .any(|input| input.is_dir() && directory.starts_with(input))
    {
        return Err("profile output must be outside declared input directories".into());
    }
    std::fs::create_dir(&directory)
        .map_err(|error| format!("create new profile directory: {error}"))?;
    let identities = experiment::snapshot(&plan, &path)?;
    let mut result = json!({"schema":"omega-performance-profile/1","backend":backend,"role":role,"plan_sha256":plan_sha256,"plan":plan,
        "identities":identities,"status":"started","timing_use":"instrumented attribution; do not compare these durations with uninstrumented runs"});
    save(&directory, &result)?;
    let control_directory = plan.working_directory.clone();
    let control = |arguments: &[String], label: &str| -> Result<bool, String> {
        let (observation, output) = process::capture(
            Path::new("wpr.exe"),
            arguments,
            &BTreeMap::new(),
            &control_directory,
            &[],
            30000,
        )?;
        std::fs::write(directory.join(format!("{label}.stdout")), &output.stdout)
            .map_err(|error| error.to_string())?;
        std::fs::write(directory.join(format!("{label}.stderr")), &output.stderr)
            .map_err(|error| error.to_string())?;
        std::fs::write(
            directory.join(format!("{label}.json")),
            serde_json::to_vec_pretty(&observation).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        Ok(output.status.success())
    };
    let mut wpr_instance = None;
    let mut wpr_cleanup = None;
    let mut capability_failure = None;
    match backend {
        "phase" if !matches!(plan.measurement, Measurement::Compiler { .. }) => {
            capability_failure = Some("phase attribution requires a compiler plan".into())
        }
        "wpr" => {
            if cfg!(windows) {
                let instance = format!(
                    "OmegaBench-{}-{}",
                    std::process::id(),
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map_err(|error| error.to_string())?
                        .as_nanos()
                );
                match control(&["-start".into(),"CPU".into(),"-filemode".into(),"-recordtempto".into(),directory.display().to_string(),"-instancename".into(),instance.clone()],"wpr-start"){
                    Ok(true)=>{wpr_cleanup=Some(OwnedWpr{instance:instance.clone(),directory:directory.clone(),working_directory:control_directory.clone(),closed:false});result["wpr_instance"]=json!(instance);save(&directory,&result)?;wpr_instance=Some(instance);},
                    Ok(false)=>capability_failure=Some("WPR could not start; inspect wpr-start streams for permissions or provider conflicts".into()),
                    Err(reason)=>capability_failure=Some(reason),
                }
            } else {
                capability_failure = Some("WPR requires Windows".into());
            }
        }
        "perf-stat" | "perf-record" if !cfg!(target_os = "linux") => {
            capability_failure =
                Some("perf capture requires Linux and a permitted perf installation".into())
        }
        _ => {}
    }
    if cfg!(target_os = "linux")
        && matches!(backend, "perf-stat" | "perf-record")
        && find_perf().is_none()
    {
        capability_failure = Some("perf was not found at /usr/bin/perf or /bin/perf".into());
    }
    if let Some(reason) = capability_failure {
        result["status"] = json!("unavailable");
        result["reason"] = json!(reason);
        save(&directory, &result)?;
        return Ok(false);
    }
    let command = if role == "baseline" {
        &mut plan.baseline
    } else {
        &mut plan.candidate
    };
    match backend {
        "phase" => {
            command
                .environment
                .insert("OMEGA_PHASE_PROFILE".into(), "1".into());
        }
        "allocations" => {
            command.environment.insert(
                "OMEGA_ALLOCATION_PROFILE".into(),
                directory.join("allocations.json").display().to_string(),
            );
        }
        "perf-stat" | "perf-record" => {
            let profiler = find_perf().ok_or("perf was not found at /usr/bin/perf or /bin/perf")?;
            result["profiler_sha256"] = json!(process::file_digest(&profiler)?);
            let mut arguments = if backend == "perf-stat" {
                vec![
                    "stat".into(),
                    "-j".into(),
                    "-o".into(),
                    directory.join("counters.jsonl").display().to_string(),
                    "-e".into(),
                    "cycles,instructions,branches,branch-misses,cache-misses,page-faults".into(),
                    "--".into(),
                ]
            } else {
                vec![
                    "record".into(),
                    "--call-graph".into(),
                    "dwarf".into(),
                    "-o".into(),
                    directory.join("perf.data").display().to_string(),
                    "--".into(),
                ]
            };
            arguments.push(command.program.display().to_string());
            arguments.extend(command.arguments.clone());
            command.program = profiler;
            command.arguments = arguments;
        }
        _ => {}
    }
    result["executed_plan"] = serde_json::to_value(&plan).map_err(|error| error.to_string())?;
    let mut sample = experiment::Sample {
        role: role.into(),
        ..Default::default()
    };
    let command = if role == "baseline" {
        &plan.baseline
    } else {
        &plan.candidate
    };
    let mut measured = experiment::measure(&plan, command, &directory, &mut sample);
    if let Some(instance) = wpr_instance {
        result["wpr_instance"] = json!(instance);
        match control(
            &[
                "-stop".into(),
                directory.join("cpu.etl").display().to_string(),
                "-skipPdbGen".into(),
                "-instancename".into(),
                instance.clone(),
            ],
            "wpr-stop",
        ) {
            Ok(true) => {
                if let Some(guard) = &mut wpr_cleanup {
                    guard.closed = true;
                }
            }
            stopped => {
                let cancelled = control(
                    &["-cancel".into(), "-instancename".into(), instance],
                    "wpr-owned-cancel",
                );
                if matches!(cancelled, Ok(true))
                    && let Some(guard) = &mut wpr_cleanup
                {
                    guard.closed = true;
                }
                measured = Err(format!(
                    "owned WPR instance did not save cleanly: {stopped:?}; cancellation: {cancelled:?}"
                ));
            }
        }
    }
    if let Err(error) = &measured {
        sample.failure = Some(error.clone());
    }
    result["sample"] = serde_json::to_value(&sample).map_err(|error| error.to_string())?;
    if let Err(reason) = measured {
        result["status"] = json!("failed");
        result["reason"] = json!(reason);
        save(&directory, &result)?;
        return Ok(false);
    }
    let extracted = (|| -> Result<(), String> {
        match backend {
            "allocations" => {
                let allocation_path = directory.join("allocations.json");
                if !allocation_path.exists() {
                    result["status"] = json!("unavailable");
                    result["reason"] = json!(
                        "subject did not emit allocation observations; build with the allocation-observation feature"
                    );
                    return Ok(());
                }
                let allocation: Value = serde_json::from_slice(
                    &std::fs::read(allocation_path).map_err(|error| error.to_string())?,
                )
                .map_err(|error| error.to_string())?;
                validate_allocations(&allocation)?;
                result["allocations"] = allocation;
            }
            "phase" => {
                if sample.compiler_stage_ms.is_empty() || sample.compiler_counters.is_empty() {
                    return Err("compiler did not emit phase scopes and counters".into());
                }
            }
            "perf-stat" => {
                let bytes = std::fs::read(directory.join("counters.jsonl"))
                    .map_err(|error| error.to_string())?;
                result["counters"] = Value::Array(parse_perf_stat(&bytes)?);
            }
            "perf-record" | "wpr" => {
                let path = directory.join(if backend == "wpr" {
                    "cpu.etl"
                } else {
                    "perf.data"
                });
                if std::fs::metadata(&path)
                    .map_err(|error| error.to_string())?
                    .len()
                    == 0
                {
                    return Err("profiler produced an empty trace".into());
                }
                result["trace_sha256"] = json!(process::file_digest(&path)?);
            }
            _ => {}
        }
        // The original plan still owns the subject identities; an external wrapper
        // is recorded separately and must not replace those source observations.
        let (original, _, _) = experiment::prepare_plan(&path)?;
        if identities != experiment::snapshot(&original, &path)? {
            return Err("profile inputs or subject executable changed".into());
        }
        Ok(())
    })();
    if let Err(reason) = extracted {
        result["status"] = json!("failed");
        result["reason"] = json!(reason);
    } else if result["status"] != "unavailable" {
        result["status"] = json!("complete");
    }
    save(&directory, &result)?;
    Ok(result["status"] == "complete")
}

fn save(directory: &Path, result: &Value) -> Result<(), String> {
    std::fs::write(
        directory.join("profile.json"),
        serde_json::to_vec_pretty(result).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())
}
fn find_perf() -> Option<PathBuf> {
    ["/usr/bin/perf", "/bin/perf"]
        .into_iter()
        .map(PathBuf::from)
        .find(|path| path.is_file())
}

fn validate_allocations(value: &Value) -> Result<(), String> {
    if value["schema"] != "omega-allocation-observation/1" {
        return Err("invalid allocation observation schema".into());
    }
    for name in [
        "allocations",
        "reallocations",
        "deallocations",
        "requested_bytes",
        "live_bytes",
        "peak_live_bytes",
        "failures",
    ] {
        if value[name].as_u64().is_none() {
            return Err(format!("allocation observation missing {name}"));
        }
    }
    if value["live_bytes"].as_u64() > value["peak_live_bytes"].as_u64()
        || value["peak_live_bytes"].as_u64() > value["requested_bytes"].as_u64()
    {
        return Err("allocation byte invariants disagree".into());
    }
    Ok(())
}

fn parse_perf_stat(bytes: &[u8]) -> Result<Vec<Value>, String> {
    let mut counters = Vec::new();
    for row in serde_json::Deserializer::from_slice(bytes).into_iter::<Value>() {
        let row = row.map_err(|error| format!("invalid perf stat JSON: {error}"))?;
        let name = row["event"]
            .as_str()
            .ok_or("perf counter has no event name")?;
        let raw = row["counter-value"]
            .as_str()
            .ok_or("perf counter has no value")?;
        let counter = match raw.parse::<f64>() {
            Ok(value) if value.is_finite() && value >= 0.0 => {
                json!({"event":name,"status":"measured","value":value,"unit":row["unit"],"runtime":row["event-runtime"],"percent_running":row["pcnt-running"]})
            }
            _ => json!({"event":name,"status":"unavailable","reason":raw}),
        };
        counters.push(counter);
    }
    if counters.is_empty() {
        return Err("perf stat returned no counters".into());
    }
    Ok(counters)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hardware_counter_refusals_are_not_zero_counts() {
        let rows=parse_perf_stat(br#"{"event":"cycles","counter-value":"42","unit":"","pcnt-running":100}{"event":"instructions","counter-value":"<not supported>","unit":""}"#).unwrap();
        assert_eq!(rows[0]["value"], 42.0);
        assert_eq!(rows[1]["status"], "unavailable");
        assert!(rows[1].get("value").is_none());
        assert!(parse_perf_stat(b"garbage").is_err());
    }
}
