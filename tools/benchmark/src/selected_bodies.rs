//! Retain the uninstrumented producer and two unchanged continuation workloads.

use crate::plan::{CommandSpec, ExpectedOutput, Measurement, Plan};
use crate::process::digest;
use serde_json::json;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

pub(crate) fn prepare_inputs(producer: &Path, destination: &Path) -> Result<(), String> {
    let root = std::env::current_dir().map_err(|error| error.to_string())?;
    let producer = std::fs::canonicalize(producer).map_err(|error| error.to_string())?;
    let producer_sha256 = crate::process::file_digest(&producer)?;
    let fixtures = [
        (
            "scalar",
            "tests/fixtures/run/calls/scalar_contract_route_exit",
        ),
        (
            "borrowed",
            "tests/fixtures/run/ownership/borrowed_root_replacement",
        ),
    ];
    // Read every input before creating the output. The kept fixtures remain
    // the authority; this helper does not transform their source or Build.
    let inputs = fixtures
        .iter()
        .flat_map(|(name, source)| {
            ["main.omg", "build.omg"].map(|file| (*name, source.to_string(), file.to_string()))
        })
        .map(|(name, source, file)| {
            let relative = format!("{source}/{file}");
            let bytes = std::fs::read(root.join(&relative))
                .map_err(|error| format!("read {relative}: {error}"))?;
            Ok((name, relative, file, bytes))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let parent = destination
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    std::fs::create_dir(destination)
        .map_err(|error| format!("create new input directory: {error}"))?;
    for (name, _) in fixtures {
        std::fs::create_dir(destination.join(name)).map_err(|error| error.to_string())?;
    }
    let producer_name = format!("baseline-omega{}", std::env::consts::EXE_SUFFIX);
    let retained_producer = destination.join(&producer_name);
    std::fs::copy(&producer, &retained_producer).map_err(|error| error.to_string())?;
    if crate::process::file_digest(&retained_producer)? != producer_sha256 {
        return Err("retained producer differs from its input".into());
    }
    let mut identities = Vec::new();
    for (name, original, file, bytes) in inputs {
        let copy = format!("{name}/{file}");
        std::fs::write(destination.join(&copy), &bytes).map_err(|error| error.to_string())?;
        identities.push(json!({"source":original,"copy":copy,"sha256":digest(&bytes)}));
    }
    std::fs::write(
        destination.join("inputs.json"),
        serde_json::to_vec_pretty(
            &json!({"schema":"omega-selected-body-inputs/1","inputs":identities,"producer":{"source":producer,"copy":producer_name,"sha256":producer_sha256}}),
        )
        .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    println!("Created {}", destination.display());
    Ok(())
}

pub(crate) fn prepare_plan(
    driver: &Path,
    source: &Path,
    targets: &str,
    destination: &Path,
) -> Result<(), String> {
    write_plan(&capture_plan(driver, source, targets)?, destination)
}

fn capture_plan(driver: &Path, source: &Path, targets: &str) -> Result<Plan, String> {
    let root = std::env::current_dir()
        .and_then(std::fs::canonicalize)
        .map_err(|error| error.to_string())?;
    let driver = std::fs::canonicalize(driver).map_err(|error| error.to_string())?;
    let source = std::fs::canonicalize(source).map_err(|error| error.to_string())?;
    let build = source
        .parent()
        .ok_or("source has no directory")?
        .join("build.omg");
    let library = root.join("source/library");
    if !driver.is_file() || !source.is_file() || !build.is_file() || !library.is_dir() {
        return Err(
            "run from the checkout root with a driver, copied source and adjacent build.omg".into(),
        );
    }
    let names = target_names(targets)?;
    let mut stdout = names
        .iter()
        .map(|name| format!("target={name} outcome=checked\n"))
        .collect::<String>();
    // Publication counts include the Build artifact used by a check request.
    // The retired settled-check counter has no production caller at this base;
    // accepted outcomes below remain the semantic oracle.
    stdout.push_str(&format!("checked_products={}\nwhole_invocation_builds=1 whole_invocation_settled_checks=0 source_completions=1 terminal_productions=1\n", names.len()));
    let arguments = vec![
        "--check".into(),
        "--targets".into(),
        targets.into(),
        source.to_str().ok_or("source path must be UTF-8")?.into(),
        "1".into(),
        "1".into(),
        "1".into(),
        "0".into(),
    ];
    let environment = BTreeMap::from([(
        "OMEGA_LIBRARY_ROOT".into(),
        library.to_str().ok_or("library path must be UTF-8")?.into(),
    )]);
    let command = || CommandSpec {
        program: driver.clone(),
        arguments: arguments.clone(),
        environment: environment.clone(),
    };
    let plan = Plan {
        schema: "omega-performance-plan/1".into(),
        name: "retained selected-body copy attribution".into(),
        working_directory: root,
        inputs: vec![source, build, library],
        baseline: command(),
        candidate: command(),
        warmups: 1,
        pairs: 8,
        timeout_ms: 600_000,
        measurement: Measurement::Process {
            stdin: String::new(),
            expected: ExpectedOutput {
                exit_code: 0,
                stdout,
                stderr: String::new(),
            },
        },
    };
    plan.validate()?;
    Ok(plan)
}

fn target_names(targets: &str) -> Result<Vec<&str>, String> {
    let mut names = Vec::new();
    for name in targets.split(',') {
        if !matches!(
            name,
            "windows_x86_64" | "linux_x86_64" | "linux_arm64" | "macos_arm64"
        ) {
            return Err(format!("unknown hosted target: {name}"));
        }
        if names.contains(&name) {
            return Err(format!("duplicate target: {name}"));
        }
        names.push(name);
    }
    // ExplicitTargetSet publishes in canonical target-name order.
    names.sort_unstable();
    Ok(names)
}

fn write_plan(plan: &Plan, destination: &Path) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(plan).map_err(|error| error.to_string())?;
    if let Some(parent) = destination
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|error| format!("create new plan: {error}"))?;
    output
        .write_all(&bytes)
        .map_err(|error| error.to_string())?;
    println!("Created {}", destination.display());
    Ok(())
}

/// Record the ordinary compiler's successful baseline output once, then freeze
/// it as the oracle for the same two-slot control. The probe is retained apart
/// from any later paired timings and uses the existing bounded process owner.
pub(crate) fn prepare_source_plan(
    compiler: &Path,
    source: &Path,
    targets: &str,
    destination: &Path,
) -> Result<(), String> {
    if destination.exists() {
        return Err("source control plan already exists".into());
    }
    let mut plan = capture_plan(compiler, source, targets)?;
    plan.name = "ordinary selected-source compilation control".into();
    let mut arguments = vec!["--check".into(), "--offline".into()];
    for target in targets.split(',') {
        arguments.extend(["--target".into(), target.into()]);
    }
    arguments.push(
        plan.inputs[0]
            .to_str()
            .ok_or("source path must be UTF-8")?
            .into(),
    );
    plan.baseline.arguments = arguments.clone();
    plan.candidate.arguments = arguments;
    let oracle = destination.with_extension("oracle");
    if let Some(parent) = oracle
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::create_dir(&oracle)
        .map_err(|error| format!("create new oracle directory: {error}"))?;
    let binary_sha = crate::process::file_digest(&plan.baseline.program)?;
    let (observation, output) = crate::process::capture(
        &plan.baseline.program,
        &plan.baseline.arguments,
        &plan.baseline.environment,
        &plan.working_directory,
        &[],
        plan.timeout_ms,
    )?;
    std::fs::write(oracle.join("subject.stdout"), &output.stdout)
        .map_err(|error| error.to_string())?;
    std::fs::write(oracle.join("subject.stderr"), &output.stderr)
        .map_err(|error| error.to_string())?;
    std::fs::write(
        oracle.join("observation.json"),
        serde_json::to_vec_pretty(&observation).map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    std::fs::write(oracle.join("producer.sha256"), &binary_sha)
        .map_err(|error| error.to_string())?;
    if output.status.code() != Some(0) {
        return Err(format!(
            "baseline source control refused; inspect {}",
            oracle.display()
        ));
    }
    if crate::process::file_digest(&plan.baseline.program)? != binary_sha {
        return Err("baseline executable changed during source control".into());
    }
    let lock = plan.inputs[0]
        .parent()
        .ok_or("source has no directory")?
        .join("omega.lock");
    if lock.is_file() {
        plan.inputs.push(lock);
    }
    plan.measurement = Measurement::Process {
        stdin: String::new(),
        expected: ExpectedOutput {
            exit_code: 0,
            stdout: String::from_utf8(output.stdout).map_err(|error| error.to_string())?,
            stderr: String::from_utf8(output.stderr).map_err(|error| error.to_string())?,
        },
    };
    write_plan(&plan, destination)
}

#[cfg(test)]
mod tests {
    use super::target_names;

    #[test]
    fn output_oracle_follows_the_public_target_catalog() {
        assert_eq!(
            target_names("windows_x86_64,macos_arm64,linux_x86_64,linux_arm64").unwrap(),
            [
                "linux_arm64",
                "linux_x86_64",
                "macos_arm64",
                "windows_x86_64"
            ]
        );
        for invalid in ["", "all", "linux_x64", "windows_x86_64,windows_x86_64"] {
            assert!(target_names(invalid).is_err(), "{invalid}");
        }
    }
}
