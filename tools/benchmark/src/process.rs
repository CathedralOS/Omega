//! Capture one process with the repository's deadline and process-tree ownership.

use bounded_process::{
    BoundedCaptureBudget, BoundedCaptureLimits, BoundedProcessInput, BoundedProcessLimits,
    BoundedProcessOutput, BoundedProcessPrepared, run_bounded_process,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

#[derive(Debug, Deserialize, Serialize)]
pub struct Observation {
    pub program: String,
    pub arguments: Vec<String>,
    pub process_wall_ns: u64,
    pub exit_code: Option<i32>,
    pub stdout_sha256: String,
    pub stderr_sha256: String,
    pub resources: serde_json::Value,
}

pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn file_digest(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|error| format!("open {}: {error}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|error| format!("read {}: {error}", path.display()))?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn capture(
    program: &Path,
    arguments: &[String],
    environment: &BTreeMap<String, String>,
    directory: &Path,
    stdin: &[u8],
    timeout_ms: u64,
) -> Result<(Observation, BoundedProcessOutput), String> {
    let mut command = Command::new(program);
    command
        .args(arguments)
        .envs(environment)
        .current_dir(directory);
    let gibibyte = 1024 * 1024 * 1024;
    let mut prepared = BoundedProcessPrepared::new(
        command,
        BoundedProcessLimits::new(
            timeout_ms.div_ceil(1000) + 10,
            16 * gibibyte,
            gibibyte,
            1024,
            64,
            8 * gibibyte,
            8 * gibibyte,
        ),
        "omega-bench",
    )
    .map_err(|error| format!("prepare {}: {error}", program.display()))?;
    prepared.observe_resources();
    let started = Instant::now();
    let output = run_bounded_process(
        prepared,
        if stdin.is_empty() {
            BoundedProcessInput::Null
        } else {
            BoundedProcessInput::Bytes(stdin.to_vec())
        },
        BoundedCaptureLimits::new(
            1024 * 1024,
            1024 * 1024,
            Duration::from_millis(timeout_ms),
            Duration::from_secs(2),
            Duration::from_millis(1),
        ),
        BoundedCaptureBudget::new(2 * 1024 * 1024),
    )
    .map_err(|error| format!("execute {}: {error}", program.display()))?;
    let process_wall_ns =
        u64::try_from(started.elapsed().as_nanos()).map_err(|error| error.to_string())?;
    Ok((
        Observation {
            program: program.display().to_string(),
            arguments: arguments.to_vec(),
            process_wall_ns,
            exit_code: output.status.code(),
            stdout_sha256: digest(&output.stdout),
            stderr_sha256: digest(&output.stderr),
            resources: resource_observations(output.resource_usage.as_ref()),
        },
        output,
    ))
}

fn resource_observations(
    usage: Option<&bounded_process::ProcessResourceUsage>,
) -> serde_json::Value {
    use serde_json::json;
    let metric = |value: Option<u64>, unit: &str| match value {
        Some(value) => json!({"status":"measured", "value":value,"unit":unit}),
        None => {
            json!({"status":"unavailable","unit":unit,"reason":usage.and_then(|usage| usage.error.as_deref()).unwrap_or("metric not supplied by this platform provider")})
        }
    };
    json!({"provider":usage.map(|usage| usage.provider),"scope":usage.map(|usage| usage.scope),
        "user_cpu_ns":metric(usage.and_then(|usage|usage.user_cpu_ns),"ns"),
        "system_cpu_ns":metric(usage.and_then(|usage|usage.system_cpu_ns),"ns"),
        "peak_resident_bytes":metric(usage.and_then(|usage|usage.peak_resident_bytes),"bytes"),
        "minor_page_faults":metric(usage.and_then(|usage|usage.minor_page_faults),"count"),
        "major_page_faults":metric(usage.and_then(|usage|usage.major_page_faults),"count"),
        "total_page_faults":metric(usage.and_then(|usage|usage.total_page_faults),"count"),
        "cpu_cycles":metric(usage.and_then(|usage|usage.cpu_cycles),"cycles")})
}
