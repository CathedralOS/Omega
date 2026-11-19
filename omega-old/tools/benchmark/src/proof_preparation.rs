//! Prepare the proof benchmark's inputs and ordinary comparison plans without
//! a host shell. The existing comparison owner still runs and checks samples.

use crate::plan::{CommandSpec, ExpectedOutput, Measurement, Plan};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const PLAN_USAGE: &str = "proof-plan <source|prepare|prove> <baseline> <candidate> <main.omg> <new-plan.json> [--pairs N] (--target <hosted-target> | --repeats N --work-units N --checksum VALUE)";
const SQUARE_BASE: &str = "9b4898865ecd5789bea4def6c2da078192a4ab82";
const SQUARE_MAIN_SHA: &str = "f069e526d1da8440718195f266437c92345f5d9ace8b8461dad1bbad2968253d";
const SQUARE_BUILD_SHA: &str = "9813449ec4cd31761ff0dfe63aa76b1a050d9985e279f56a80414a02795faa87";
const SQUARE_IDENTITY_BUILD_SHA: &str =
    "eaba73728026898495728dedff5c7005f89949ca70cee61a3f48d1ccc9354d32";

pub(crate) fn run(command: &str, arguments: &[String]) -> Result<(), String> {
    let working_directory = std::env::current_dir().map_err(|error| error.to_string())?;
    match command {
        "proof-plan" => {
            let (plan, destination) = make_plan(arguments, &working_directory)?;
            let encoded = serde_json::to_vec_pretty(&plan).map_err(|error| error.to_string())?;
            write_new(&destination, &encoded)?;
            println!("Created {}", destination.display());
            Ok(())
        }
        "proof-square" => prepare_square(arguments, &working_directory),
        _ => Err("unknown proof benchmark preparation command".into()),
    }
}

fn make_plan(arguments: &[String], working_directory: &Path) -> Result<(Plan, PathBuf), String> {
    if arguments.len() < 5 {
        return Err(PLAN_USAGE.into());
    }
    let mode = arguments[0].as_str();
    if !matches!(mode, "source" | "prepare" | "prove") {
        return Err(PLAN_USAGE.into());
    }
    let (pairs, trailing) = arguments[5..].as_chunks::<2>();
    if !trailing.is_empty() {
        return Err(PLAN_USAGE.into());
    }
    let mut options = BTreeMap::new();
    for [name, value] in pairs {
        if !matches!(
            name.as_str(),
            "--target" | "--repeats" | "--work-units" | "--checksum" | "--pairs"
        ) {
            return Err(format!("unknown proof-plan option: {name}"));
        }
        if options.insert(name.as_str(), value.as_str()).is_some() {
            return Err(format!("duplicate proof-plan option: {name}"));
        }
    }
    let working_directory = working_directory
        .canonicalize()
        .map_err(|error| error.to_string())?;
    let baseline = existing_file(&working_directory.join(&arguments[1]))?;
    let candidate = existing_file(&working_directory.join(&arguments[2]))?;
    let source = existing_file(&working_directory.join(&arguments[3]))?;
    let source_name = path_text(&source)?;
    let source_directory = source.parent().ok_or("source has no parent directory")?;
    let library = working_directory.join("source/library");
    if !library.is_dir() {
        return Err("run proof-plan from the checkout root containing source/library".into());
    }
    let mut environment = BTreeMap::new();
    let (command_arguments, measurement) = if mode == "source" {
        if options
            .keys()
            .any(|name| !matches!(*name, "--target" | "--pairs"))
        {
            return Err(
                "source mode accepts --target and --pairs; timer options require prepare or prove"
                    .into(),
            );
        }
        let target = options
            .get("--target")
            .ok_or("source mode requires an explicit --target")?;
        if !matches!(
            *target,
            "windows_x86_64" | "linux_x86_64" | "linux_arm64" | "macos_arm64"
        ) {
            return Err("--target must name one of the four hosted targets".into());
        }
        environment.insert("OMEGA_LIBRARY_ROOT".into(), path_text(&library)?.into());
        (
            vec![
                source_name.into(),
                "--entry".into(),
                "main".into(),
                "--target".into(),
                (*target).into(),
            ],
            Measurement::Process {
                stdin: String::new(),
                expected: ExpectedOutput {
                    exit_code: 0,
                    stdout: format!(
                        "{source_name}\n  part 0: accepted\n{source_name}\troute=accepted\n"
                    ),
                    stderr: String::new(),
                },
            },
        )
    } else {
        if options.contains_key("--target") {
            return Err(
                "prepare/prove replay is target-neutral and does not accept --target".into(),
            );
        }
        let repeats = positive_option(&options, "--repeats")?;
        let work_units = positive_option(&options, "--work-units")?;
        let checksum = options
            .get("--checksum")
            .filter(|value| !value.is_empty())
            .ok_or("timer mode requires --checksum")?;
        (
            vec![source_name.into(), repeats.to_string(), mode.into()],
            Measurement::ReportedWork {
                stdin: String::new(),
                work_units,
                checksum: (*checksum).into(),
            },
        )
    };
    let pairs = options.get("--pairs").map_or(Ok(8), |value| {
        value
            .parse::<usize>()
            .map_err(|error| format!("invalid --pairs: {error}"))
    })?;
    let plan = Plan {
        schema: "omega-performance-plan/1".into(),
        name: format!("Proof {mode} comparison: {}", source.display()),
        working_directory: working_directory.clone(),
        inputs: vec![source_directory.into(), library],
        baseline: CommandSpec {
            program: baseline,
            arguments: command_arguments.clone(),
            environment: environment.clone(),
        },
        candidate: CommandSpec {
            program: candidate,
            arguments: command_arguments,
            environment,
        },
        warmups: 1,
        pairs,
        timeout_ms: 120_000,
        measurement,
    };
    plan.validate()?;
    Ok((plan, working_directory.join(&arguments[4])))
}

fn positive_option(options: &BTreeMap<&str, &str>, name: &str) -> Result<u64, String> {
    options
        .get(name)
        .and_then(|value| value.parse().ok())
        .filter(|value| *value != 0)
        .ok_or_else(|| format!("timer mode requires positive {name}"))
}

fn existing_file(path: &Path) -> Result<PathBuf, String> {
    if !path.is_file() {
        return Err(format!("expected an existing file: {}", path.display()));
    }
    path.canonicalize()
        .map_err(|error| format!("{}: {error}", path.display()))
}

fn path_text(path: &Path) -> Result<&str, String> {
    path.to_str()
        .ok_or_else(|| format!("benchmark plans require UTF-8 paths: {}", path.display()))
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("cannot create new {}: {error}", path.display()))?;
    output
        .write_all(bytes)
        .map_err(|error| format!("{}: {error}", path.display()))
}

fn prepare_square(arguments: &[String], working_directory: &Path) -> Result<(), String> {
    let identity_build = match arguments {
        [_] => false,
        [_, option] if option == "--identity-build" => true,
        _ => return Err("proof-square <new-directory> [--identity-build]".into()),
    };
    // Validate both pinned inputs before creating any output. A missing commit
    // or changed transform cannot leave a seemingly usable partial workload.
    let main = qualify_main(pinned_source(working_directory, "main.omg")?)?;
    let build = pinned_source(working_directory, "build.omg")?;
    require_digest(&main, SQUARE_MAIN_SHA, "qualified square main")?;
    require_digest(&build, SQUARE_BUILD_SHA, "square build")?;
    let build = if identity_build {
        let identity = identity_optimizations(build)?;
        require_digest(
            &identity,
            SQUARE_IDENTITY_BUILD_SHA,
            "identity square build",
        )?;
        identity
    } else {
        build
    };
    let destination = working_directory.join(&arguments[0]);
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::create_dir(&destination)
        .map_err(|error| format!("cannot create new {}: {error}", destination.display()))?;
    for (name, text) in [("main.omg", main), ("build.omg", build)] {
        let path = destination.join(name);
        write_new(&path, text.as_bytes())?;
        println!("{:x}  {}", Sha256::digest(text.as_bytes()), path.display());
    }
    Ok(())
}

fn pinned_source(working_directory: &Path, name: &str) -> Result<String, String> {
    let object = format!("{SQUARE_BASE}:samples/cli/arithmetic/wrapping_square_sum/{name}");
    let output = Command::new("git")
        .args(["show", &object])
        .current_dir(working_directory)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("cannot read pinned workload through git: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "pinned workload {object} is unavailable; fetch {SQUARE_BASE} before retrying: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let mut text = String::from_utf8(output.stdout)
        .map_err(|error| error.to_string())?
        .replace("\r\n", "\n");
    if !text.ends_with('\n') {
        text.push('\n');
    }
    Ok(text)
}

fn require_digest(text: &str, expected: &str, description: &str) -> Result<(), String> {
    let actual = format!("{:x}", Sha256::digest(text.as_bytes()));
    if actual != expected {
        return Err(format!(
            "{description} SHA-256 changed: expected {expected}, found {actual}"
        ));
    }
    Ok(())
}

fn replace_once(text: String, old: &str, new: &str) -> Result<String, String> {
    if text.matches(old).count() != 1 {
        return Err(format!("pinned workload must contain exactly one {old:?}"));
    }
    Ok(text.replace(old, new))
}

fn qualify_main(mut text: String) -> Result<String, String> {
    for (old, new) in [
        ("loop_(0, 1)", "loop_(0u64 as +Wrapping, 1u64 as +Wrapping)"),
        ("i <= 2000000", "i <= (2000000u64 as +Wrapping)"),
        ("i + 1", "i + (1u64 as +Wrapping)"),
        (
            "acc == 2666668666667000000",
            "acc == (2666668666667000000u64 as +Wrapping)",
        ),
    ] {
        text = replace_once(text, old, new)?;
    }
    Ok(text)
}

fn identity_optimizations(mut text: String) -> Result<String, String> {
    const CALL: &str = "Optimizations::enable(";
    if text.matches(CALL).count() != 6 {
        return Err("pinned square build must contain six optimization selections".into());
    }
    while let Some(start) = text.find(CALL) {
        let end = text[start..]
            .find(';')
            .map(|offset| start + offset + 1)
            .ok_or("pinned optimization selection has no semicolon")?;
        let whitespace_start = text[..start].trim_end_matches(char::is_whitespace).len();
        text.replace_range(whitespace_start..end, "");
    }
    replace_once(
        text,
        "    // The staged Psi-phase promotion records cite this subject's benchmark\n    // rows as measurement evidence; enabling the complete six-rule selection\n    // here lets each rule's `disabled` row measure its marginal cost.\n",
        "    // This gate selects the identity optimization phase.\n",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Directory(PathBuf);
    impl Directory {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let path = std::env::temp_dir().join(format!(
                "omega-proof-plan-{}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&path).unwrap();
            fs::create_dir_all(path.join("source/library")).unwrap();
            fs::create_dir(path.join("subject with spaces")).unwrap();
            for name in [
                "baseline tool",
                "candidate tool",
                "subject with spaces/main.omg",
            ] {
                fs::write(path.join(name), "").unwrap();
            }
            Self(path)
        }
        fn arguments(&self, mode: &str) -> Vec<String> {
            [
                mode,
                "baseline tool",
                "candidate tool",
                "subject with spaces/main.omg",
                "plan.json",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect()
        }
    }
    impl Drop for Directory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn source_plan_retains_exact_target_paths_and_acceptance_oracle() {
        let directory = Directory::new();
        let mut arguments = directory.arguments("source");
        arguments.extend(["--target", "linux_arm64"].map(str::to_owned));
        let (plan, _) = make_plan(&arguments, &directory.0).unwrap();
        assert_eq!(plan.baseline.arguments, plan.candidate.arguments);
        assert_eq!(
            &plan.baseline.arguments[1..],
            ["--entry", "main", "--target", "linux_arm64"]
        );
        assert!(plan.baseline.environment.contains_key("OMEGA_LIBRARY_ROOT"));
        let Measurement::Process { stdin, expected } = plan.measurement else {
            panic!("source process")
        };
        assert!(stdin.is_empty());
        assert_eq!(expected.exit_code, 0);
        assert_eq!(
            expected.stdout,
            format!(
                "{}\n  part 0: accepted\n{}\troute=accepted\n",
                plan.baseline.arguments[0], plan.baseline.arguments[0]
            )
        );
    }

    #[test]
    fn proof_search_plan_keeps_count_checksum_and_mode_without_target() {
        let directory = Directory::new();
        let mut arguments = directory.arguments("prove");
        arguments.extend(
            [
                "--repeats",
                "100",
                "--work-units",
                "1200",
                "--checksum",
                "1200",
            ]
            .map(str::to_owned),
        );
        let (plan, _) = make_plan(&arguments, &directory.0).unwrap();
        assert_eq!(&plan.baseline.arguments[1..], ["100", "prove"]);
        assert!(
            matches!(plan.measurement, Measurement::ReportedWork { work_units: 1200, ref checksum, .. } if checksum == "1200")
        );
        arguments.extend(["--target", "windows_x86_64"].map(str::to_owned));
        assert!(make_plan(&arguments, &directory.0).is_err());
    }

    #[test]
    fn invalid_or_duplicate_options_and_existing_outputs_refuse() {
        let directory = Directory::new();
        let arguments = directory.arguments("source");
        for tail in [
            vec!["--target", "uefi_x86_64"],
            vec!["--target", "linux_x86_64", "--target", "linux_arm64"],
            vec!["--target", "linux_x86_64", "--pairs", "1"],
            vec!["--unknown", "x"],
        ] {
            let mut invalid = arguments.clone();
            invalid.extend(tail.into_iter().map(str::to_owned));
            assert!(make_plan(&invalid, &directory.0).is_err());
        }
        let output = directory.0.join("sentinel");
        fs::write(&output, "preserve").unwrap();
        assert!(write_new(&output, b"replacement").is_err());
        assert_eq!(fs::read_to_string(output).unwrap(), "preserve");
    }

    #[test]
    fn qualifier_transform_refuses_changed_source_and_preserves_oracle() {
        let original = "loop_(0, 1); i <= 2000000; i + 1; acc == 2666668666667000000;";
        assert_eq!(
            qualify_main(original.into()).unwrap(),
            "loop_(0u64 as +Wrapping, 1u64 as +Wrapping); i <= (2000000u64 as +Wrapping); i + (1u64 as +Wrapping); acc == (2666668666667000000u64 as +Wrapping);"
        );
        assert!(qualify_main(original.replace("2000000", "2000001")).is_err());
        assert!(qualify_main(format!("{original} loop_(0, 1)")).is_err());
    }
}
