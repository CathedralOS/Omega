//! Count the lines of the checking stages a program runs (`cargo xtask stage-coverage`).
//!
//! The plan to replace the checker turns on how much of Psi 05, validation and
//! Psi 06 a program actually executes. This command answers that on demand: it
//! builds the `omega` CLI with LLVM source-based coverage, runs each named
//! program under it, and reports the lines whose counter is non-zero, per stage.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::repo_root;

/// The stages being measured, each named by the source directory that owns it.
/// A line belongs to a stage when its source path lies under that directory.
const STAGES: [(&str, &str); 3] = [
    (
        "Psi 05",
        "omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/",
    ),
    ("validation", "omega-rust/psi/compilation/validation/src/"),
    (
        "Psi 06",
        "omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/",
    ),
];

const DEFAULT_OMEGA_ARGUMENTS: [&str; 3] = ["run", "--engine", "terminal"];

const USAGE: &str = "\
usage: cargo xtask stage-coverage [options] <program>... [-- <omega arguments>]

Build a coverage-instrumented `omega` and report, for each program, how many
lines of Psi 05, validation and Psi 06 it runs.

  <program>            A `main.omg`, or a fixture directory holding one
  --list <file>        Also read programs from <file>, one path per line
  --json <path>        Also write the report as JSON
  --target-dir <dir>   Instrumented build directory (default: target/stage-coverage)
  --build-only         Build the instrumented compiler and stop
  -- <arguments>       Run `omega <arguments> <main.omg>` instead of
                       `omega run --engine terminal <main.omg>`";

/// Lines per stage, in `STAGES` order.
type StageLines = [usize; STAGES.len()];

struct Options {
    programs: Vec<PathBuf>,
    json: Option<PathBuf>,
    target_dir: PathBuf,
    build_only: bool,
    omega_arguments: Vec<String>,
}

struct LlvmTools {
    profdata: PathBuf,
    cov: PathBuf,
}

struct ProgramResult {
    program: PathBuf,
    /// The compiler's exit code; `None` when a signal ended it.
    exit_code: Option<i32>,
    run: StageLines,
    profile: PathBuf,
}

fn parse_options(args: &[String], root: &Path) -> Result<Option<Options>, String> {
    let mut options = Options {
        programs: Vec::new(),
        json: None,
        target_dir: root.join("target/stage-coverage"),
        build_only: false,
        omega_arguments: DEFAULT_OMEGA_ARGUMENTS.map(str::to_owned).to_vec(),
    };
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let mut value = |flag: &str| {
            iter.next()
                .cloned()
                .ok_or_else(|| format!("`{flag}` needs a value"))
        };
        match arg.as_str() {
            "-h" | "--help" => return Ok(None),
            "--build-only" => options.build_only = true,
            "--json" => options.json = Some(PathBuf::from(value("--json")?)),
            "--target-dir" => {
                let directory = PathBuf::from(value("--target-dir")?);
                // The build runs from the repository root, so a relative
                // directory must mean the same place there as it does here.
                options.target_dir = std::path::absolute(&directory)
                    .map_err(|e| format!("resolve {}: {e}", directory.display()))?;
            }
            "--list" => {
                let list = PathBuf::from(value("--list")?);
                let text = fs::read_to_string(&list)
                    .map_err(|e| format!("read {}: {e}", list.display()))?;
                options.programs.extend(
                    text.lines()
                        .map(str::trim)
                        .filter(|line| !line.is_empty() && !line.starts_with('#'))
                        .map(PathBuf::from),
                );
            }
            "--" => {
                options.omega_arguments = iter.cloned().collect();
                if options.omega_arguments.is_empty() {
                    return Err("`--` needs the omega arguments to run".to_owned());
                }
                break;
            }
            flag if flag.starts_with('-') => return Err(format!("unknown argument `{flag}`")),
            program => options.programs.push(PathBuf::from(program)),
        }
    }
    if options.programs.is_empty() && !options.build_only {
        return Err("name at least one program, or pass `--build-only`".to_owned());
    }
    for program in &mut options.programs {
        if program.is_dir() {
            program.push("main.omg");
        }
        if !program.is_file() {
            return Err(format!("no program at {}", program.display()));
        }
        *program = std::path::absolute(&*program)
            .map_err(|e| format!("resolve {}: {e}", program.display()))?;
    }
    Ok(Some(options))
}

fn captured(command: &mut Command, what: &str) -> Result<Vec<u8>, String> {
    let output = command
        .stdin(Stdio::null())
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| format!("start {what}: {e}"))?;
    if !output.status.success() {
        return Err(format!("{what} failed ({})", output.status));
    }
    Ok(output.stdout)
}

/// The profile format changes between LLVM releases, so the tools must be the
/// ones shipped with the pinned toolchain, not whichever `llvm-cov` is on PATH.
fn llvm_tools(root: &Path) -> Result<LlvmTools, String> {
    let rustc = |argument: &str| {
        captured(
            Command::new("rustc").arg(argument).current_dir(root),
            "rustc",
        )
        .map(|stdout| String::from_utf8_lossy(&stdout).into_owned())
    };
    let sysroot = rustc("--print=sysroot")?;
    let version = rustc("-vV")?;
    let host = version
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .ok_or("rustc -vV names no host")?;
    let bin = Path::new(sysroot.trim())
        .join("lib/rustlib")
        .join(host)
        .join("bin");
    let tool = |name: &str| {
        let path = bin.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
        if path.is_file() {
            Ok(path)
        } else {
            Err(format!(
                "{} is missing; install it with `rustup component add llvm-tools`",
                path.display()
            ))
        }
    };
    Ok(LlvmTools {
        profdata: tool("llvm-profdata")?,
        cov: tool("llvm-cov")?,
    })
}

fn build_instrumented(root: &Path, target_dir: &Path) -> Result<PathBuf, String> {
    let status = Command::new("mbx")
        .args(["build", "-p", "omega"])
        .current_dir(root)
        .stdin(Stdio::null())
        .env("RUSTFLAGS", "-C instrument-coverage")
        .env("CARGO_PROFILE_DEV_OPT_LEVEL", "1")
        .env("CARGO_TARGET_DIR", target_dir)
        .status()
        .map_err(|e| format!("start mbx: {e}"))?;
    if !status.success() {
        return Err(format!("instrumented build failed ({status})"));
    }
    let binary = target_dir
        .join("debug")
        .join(format!("omega{}", std::env::consts::EXE_SUFFIX));
    if !binary.is_file() {
        return Err(format!("build left no {}", binary.display()));
    }
    Ok(binary)
}

/// Count, per stage, the instrumented lines and the lines run in an lcov
/// report. `SF:` names the source file of the `DA:<line>,<count>` records that
/// follow it; a line is run when its count is not zero.
fn count_lines(lcov: &str) -> (StageLines, StageLines) {
    let mut instrumented = [0; STAGES.len()];
    let mut run = [0; STAGES.len()];
    let mut stage = None;
    for line in lcov.lines() {
        if let Some(source) = line.strip_prefix("SF:") {
            let source = source.replace('\\', "/");
            stage = STAGES
                .iter()
                .position(|(_, directory)| source.contains(directory));
        } else if let (Some(stage), Some(record)) = (stage, line.strip_prefix("DA:")) {
            instrumented[stage] += 1;
            let count = record.split(',').nth(1).unwrap_or("0");
            if count.bytes().any(|digit| digit != b'0') {
                run[stage] += 1;
            }
        }
    }
    (instrumented, run)
}

fn merge_profiles(tools: &LlvmTools, inputs: &[PathBuf], output: &Path) -> Result<(), String> {
    captured(
        Command::new(&tools.profdata)
            .args(["merge", "-sparse"])
            .args(inputs)
            .arg("-o")
            .arg(output),
        "llvm-profdata merge",
    )
    .map(drop)
}

fn stage_lines(
    tools: &LlvmTools,
    root: &Path,
    binary: &Path,
    profile: &Path,
) -> Result<(StageLines, StageLines), String> {
    let mut command = Command::new(&tools.cov);
    command
        .args(["export", "-format=lcov", "-instr-profile"])
        .arg(profile)
        .arg(binary);
    // Naming the stage directories keeps the export to the files being counted.
    for (_, directory) in STAGES {
        command.arg(root.join(directory));
    }
    let lcov = captured(&mut command, "llvm-cov export")?;
    Ok(count_lines(&String::from_utf8_lossy(&lcov)))
}

fn run_program(
    tools: &LlvmTools,
    binary: &Path,
    omega_arguments: &[String],
    program: &Path,
    directory: &Path,
) -> Result<(Option<i32>, PathBuf), String> {
    // Start from an empty directory so an earlier run's counters are not merged in.
    if directory.exists() {
        fs::remove_dir_all(directory).map_err(|e| format!("clear {}: {e}", directory.display()))?;
    }
    fs::create_dir_all(directory).map_err(|e| format!("create {}: {e}", directory.display()))?;
    let log = |name: &str| {
        fs::File::create(directory.join(name))
            .map(Stdio::from)
            .map_err(|e| format!("create {name}: {e}"))
    };
    // `%p` gives every process its own raw profile, so a compiler that starts
    // another instrumented process loses neither set of counters.
    let status = Command::new(binary)
        .args(omega_arguments)
        .arg(program)
        .current_dir(directory)
        .stdin(Stdio::null())
        .stdout(log("omega.stdout")?)
        .stderr(log("omega.stderr")?)
        .env("LLVM_PROFILE_FILE", directory.join("%p.profraw"))
        .status()
        .map_err(|e| format!("start {}: {e}", binary.display()))?;
    let mut raw: Vec<PathBuf> = fs::read_dir(directory)
        .map_err(|e| format!("read {}: {e}", directory.display()))?
        .filter_map(|entry| Some(entry.ok()?.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "profraw"))
        .collect();
    raw.sort();
    if raw.is_empty() {
        return Err(format!(
            "{} wrote no profile; see {}",
            program.display(),
            directory.display()
        ));
    }
    let profile = directory.join("program.profdata");
    merge_profiles(tools, &raw, &profile)?;
    for path in raw {
        let _ = fs::remove_file(path);
    }
    Ok((status.code(), profile))
}

fn report_row(label: &str, exit: &str, lines: StageLines) -> String {
    let total: usize = lines.iter().sum();
    format!(
        "| {label} | {exit} | {} | {} | {} | {total} |",
        lines[0], lines[1], lines[2]
    )
}

fn stage_json(lines: StageLines) -> serde_json::Value {
    let mut object = serde_json::Map::new();
    for ((name, _), count) in STAGES.iter().zip(lines) {
        object.insert((*name).to_owned(), count.into());
    }
    object.insert("total".to_owned(), lines.iter().sum::<usize>().into());
    object.into()
}

fn measure(root: &Path, options: &Options) -> Result<(), String> {
    let tools = llvm_tools(root)?;
    let binary = build_instrumented(root, &options.target_dir)?;
    if options.build_only {
        println!("stage-coverage: built {}", binary.display());
        return Ok(());
    }
    let profiles = options.target_dir.join("stage-coverage-profiles");
    let display = |program: &Path| {
        program
            .strip_prefix(root)
            .unwrap_or(program)
            .display()
            .to_string()
            .replace('\\', "/")
    };
    let mut instrumented = [0; STAGES.len()];
    let mut results = Vec::with_capacity(options.programs.len());
    for (index, program) in options.programs.iter().enumerate() {
        eprintln!(
            "stage-coverage: [{}/{}] {}",
            index + 1,
            options.programs.len(),
            display(program)
        );
        let (exit_code, profile) = run_program(
            &tools,
            &binary,
            &options.omega_arguments,
            program,
            &profiles.join(index.to_string()),
        )?;
        let (all, run) = stage_lines(&tools, root, &binary, &profile)?;
        instrumented = all;
        results.push(ProgramResult {
            program: program.clone(),
            exit_code,
            run,
            profile,
        });
    }
    if instrumented.iter().sum::<usize>() == 0 {
        return Err("the build holds no instrumented line of the three stages".to_owned());
    }
    // A line run by several programs counts once, so the list's total comes
    // from the merged profile and not from adding the rows.
    let union = if results.len() > 1 {
        let merged = profiles.join("all.profdata");
        let inputs: Vec<PathBuf> = results.iter().map(|r| r.profile.clone()).collect();
        merge_profiles(&tools, &inputs, &merged)?;
        Some(stage_lines(&tools, root, &binary, &merged)?.1)
    } else {
        None
    };

    let stamp = crate::corpus::current_stamp(root);
    let command = format!("omega {}", options.omega_arguments.join(" "));
    println!("stage-coverage: `{command} <program>` at {stamp}; lines with a non-zero counter\n");
    println!("| Lines | Exit | Psi 05 | validation | Psi 06 | All three |");
    println!("| --- | ---: | ---: | ---: | ---: | ---: |");
    println!("{}", report_row("Instrumented", "", instrumented));
    let exit_text = |code: Option<i32>| code.map_or("signal".to_owned(), |code| code.to_string());
    for result in &results {
        let label = format!("Run by `{}`", display(&result.program));
        println!(
            "{}",
            report_row(&label, &exit_text(result.exit_code), result.run)
        );
    }
    if let Some(union) = union {
        println!("{}", report_row("Run by any program", "", union));
    }
    println!(
        "\nCompiler output for each program is under {}",
        profiles.display()
    );

    if let Some(path) = &options.json {
        let report = serde_json::json!({
            "revision": stamp,
            "command": command,
            "stages": STAGES.iter().map(|(name, directory)| serde_json::json!({"name": name, "directory": directory})).collect::<Vec<_>>(),
            "instrumented": stage_json(instrumented),
            "programs": results.iter().map(|result| serde_json::json!({
                "program": display(&result.program),
                "exit_code": result.exit_code,
                "run": stage_json(result.run),
            })).collect::<Vec<_>>(),
            "run_by_any_program": union.map(stage_json),
        });
        crate::execution::write_json(path, &report)?;
    }
    Ok(())
}

pub fn run(args: &[String]) -> Result<(), u8> {
    let root = repo_root();
    let options = match parse_options(args, &root) {
        Ok(Some(options)) => options,
        Ok(None) => {
            println!("{USAGE}");
            return Ok(());
        }
        Err(error) => {
            eprintln!("stage-coverage: {error}\n\n{USAGE}");
            return Err(2);
        }
    };
    measure(&root, &options).map_err(|error| {
        eprintln!("stage-coverage: {error}");
        1
    })
}

#[cfg(test)]
mod tests {
    use super::count_lines;

    #[test]
    fn counts_run_lines_by_stage_and_ignores_other_sources() {
        let lcov = "\
SF:/repo/omega-rust/psi/compilation/pipeline/05_typed-trees-to-checked-trees/src/a.rs
DA:1,3
DA:2,0
end_of_record
SF:/repo/omega-rust/psi/compilation/validation/src/b.rs
DA:7,1
end_of_record
SF:/repo/omega-rust/psi/compilation/validation/terminal-installation/src/c.rs
DA:9,5
end_of_record
SF:/repo/omega-rust/psi/compilation/pipeline/06_checked-trees-to-lowered-psi/src/d.rs
DA:4,0
DA:5,120
end_of_record
";
        let (instrumented, run) = count_lines(lcov);
        assert_eq!(instrumented, [2, 1, 2]);
        assert_eq!(run, [1, 1, 1]);
    }
}
