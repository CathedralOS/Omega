// Reconstructing a snapshot may omit derived indexes. These measurements are
// compared with the live clone's counts, not assumed allocation-equivalent.

use selected_body_allocation_trace as trace;
use sha2::{Digest, Sha256};

mod snapshot_limits {
    include!("snapshot_limits.rs");
}

#[global_allocator]
static ALLOCATOR: trace::Tracking<mimalloc::MiMalloc> = trace::Tracking(mimalloc::MiMalloc);

fn main() -> Result<(), String> {
    let arguments = std::env::args_os().skip(1).collect::<Vec<_>>();
    if arguments.len() != 1 {
        return Err("selected_bodies_replay <capture-directory>".into());
    }
    let directory = std::path::Path::new(&arguments[0]);
    let profile_path = directory.join("profile.json");
    if std::fs::metadata(&profile_path)
        .map_err(|error| error.to_string())?
        .len()
        > 16 * 1024 * 1024
    {
        return Err("profile record exceeds 16 MiB".into());
    }
    let profile: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&profile_path).map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    if profile["status"] != "complete" || profile["backend"] != "selected-bodies" {
        return Err("a completed selected-bodies profile is required".into());
    }
    let configuration = profile["snapshot_limits"]["configuration"]
        .as_str()
        .ok_or("profile has no private snapshot limit identity")?;
    let role = profile["role"].as_str().ok_or("profile has no role")?;
    let executed = profile["executed_plan"][role]["environment"][snapshot_limits::ENVIRONMENT]
        .as_str()
        .unwrap_or(snapshot_limits::DEFAULT);
    if configuration != executed {
        return Err("snapshot limit identity differs from executed configuration".into());
    }
    let [bytes, allocation, elements, depth] = snapshot_limits::parse(configuration)?;
    let limits = compiler_storage::RecordLimits {
        bytes,
        allocation,
        elements,
        depth,
    };
    let log =
        std::fs::read(directory.join("typed/profile.log")).map_err(|error| error.to_string())?;
    if profile["clone_log_sha256"].as_str() != Some(format!("{:x}", Sha256::digest(&log)).as_str())
    {
        return Err("clone log differs from the capture record".into());
    }
    let snapshots = profile["snapshots"]
        .as_array()
        .ok_or("profile has no snapshot roster")?;
    let expected_names = snapshots
        .iter()
        .map(|snapshot| {
            let name = snapshot["file"]
                .as_str()
                .ok_or("snapshot has no file name")?;
            if std::path::Path::new(name).components().count() != 1
                || !name.starts_with("typed-")
                || !name.ends_with(".bin")
            {
                return Err("invalid snapshot name");
            }
            Ok(name)
        })
        .collect::<Result<Vec<_>, &str>>()?;
    let mut paths = std::fs::read_dir(directory.join("typed"))
        .map_err(|error| error.to_string())?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    paths.retain(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("typed-") && name.ends_with(".bin"))
    });
    paths.sort();
    let actual_names = paths
        .iter()
        .map(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .ok_or("snapshot name is not UTF-8")
        })
        .collect::<Result<Vec<_>, _>>()?;
    if actual_names != expected_names {
        return Err("snapshot files differ from the capture roster".into());
    }
    for (path, snapshot) in paths.iter().zip(snapshots) {
        let length = std::fs::metadata(path)
            .map_err(|error| error.to_string())?
            .len();
        if length > limits.bytes as u64 || snapshot["bytes"].as_u64() != Some(length) {
            return Err(format!(
                "{}: snapshot length differs or exceeds captured private limit",
                path.display()
            ));
        }
        let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
        let sha256 = format!("{:x}", Sha256::digest(&bytes));
        if snapshot["sha256"].as_str() != Some(sha256.as_str()) {
            return Err(format!(
                "{}: snapshot digest differs from the capture record",
                path.display()
            ));
        }
        let typed = compiler_storage::decode::<typed_trees::TypedTrees>(&bytes, limits)
            .map_err(|error| format!("{}: {error}", path.display()))?;
        let input = typed;
        let (candidate, counts) = trace::measure(|| std::hint::black_box(input.clone()));
        if input != candidate || input.identity.get() != candidate.identity.get() {
            return Err(format!("{}: clone changes semantic tables", path.display()));
        }
        println!(
            "REPLAY file={} sha256={} operation=reconstructed-owned-payload-clone machines={} expressions={} statements={} allocations={} requested_bytes={} live_delta={} peak_delta={} ns={} arena_clones={} arena_slots={} arena_payload_bytes={} exact_equal=true",
            path.file_name().unwrap().to_string_lossy(),
            sha256,
            input.machines().len(),
            input.expression_table.expression_count(),
            input.statement_table.statement_count(),
            counts.allocations,
            counts.requested_bytes,
            counts.live_delta,
            counts.peak_delta,
            counts.nanoseconds,
            counts.arena_clones,
            counts.arena_slots,
            counts.arena_payload_bytes,
        );
    }
    println!("REPLAY snapshots={} verified_capture=true", snapshots.len());
    Ok(())
}
