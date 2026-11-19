/*
    compare-builds: is this version of a layer faster than that one?

        cargo run --release -p compare-builds -- --package omega-lexer --tool lex_throughput -- <directories>

    Run from the repository root. It compares the working tree against the
    last commit, or against whatever `--baseline-ref` or `--baseline-dir`
    names, and ends with one sentence saying which is faster and by how much,
    or that there is no difference it can stand behind.

    Why this is a program and not a habit. Timing two builds by hand went wrong
    three ways on the lexer in one day, and each way produced a number that
    looked fine:

      1. Placement. Two builds of the same logic differed by up to 9 percent
         because of where the hot loop landed in the binary. A change was
         credited with 4 percent that was mostly this.
      2. A shared build folder. Cargo decided three different versions were
         already built and handed back the first one's binary for all of them.
      3. A busy machine. Another build running alongside pulled the medians of
         whole rounds down by 15 percent.

    So the procedure is fixed here, where it cannot be skipped:

      - Each version is built three ways (see `build_setting.rs`), and a
        difference only counts if all three show it.
      - Every build gets a folder of its own, and the binaries are compared
        byte for byte before anything is timed.
      - Runs are interleaved, so that drift in the machine's load falls on both
        versions alike, and the best run of each is what gets compared.
      - Anything the tools print that is not a number, such as a fingerprint of
        the output, is checked to be the same in every run.
*/

/*
    What it needs from a layer.

    A measurement tool, declared as an example of the layer's package, that
    takes its workload from its arguments and ends by printing a RESULT line as
    the `measure` crate defines it. That is all. This program knows nothing
    about lexing or parsing; it reads the keys the tool chose and learns from
    each key's ending whether a bigger value is better.

    The baseline has to print a RESULT line too, so the oldest commit a layer
    can be compared against is the one where its tool first did.

    What it leaves behind. Builds go under `target/compare-builds/`, which git
    ignores. A baseline taken from a commit is checked out there as a git
    worktree for the length of the run and removed at the end. If a run is
    killed, the next run removes what the last one left.
*/

#![forbid(unsafe_code)]

mod baseline_checkout;
mod build_setting;
mod compare_arguments;
mod comparison_report;
mod tool_builder;
mod tool_runner;

use crate::baseline_checkout::BaselineCheckout;
use crate::build_setting::BuildSetting;
use crate::compare_arguments::{BaselineSource, CompareArguments, USAGE};
use crate::comparison_report::ComparisonReport;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const BASELINE_LABEL: &str = "baseline";
const CANDIDATE_LABEL: &str = "candidate";

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let compare_arguments = match CompareArguments::parse(&arguments) {
        Ok(compare_arguments) => compare_arguments,
        Err(argument_error) => {
            eprintln!("{argument_error}\n{USAGE}");

            return ExitCode::from(2);
        }
    };

    match run_comparison(&compare_arguments) {
        Ok(()) => ExitCode::SUCCESS,
        Err(comparison_error) => {
            eprintln!("compare-builds stopped: {comparison_error}");

            ExitCode::FAILURE
        }
    }
}

fn run_comparison(compare_arguments: &CompareArguments) -> Result<(), String> {
    let candidate_directory = std::env::current_dir().map_err(|io_error| format!("Cannot read the current directory: {io_error}."))?;

    if !candidate_directory.join("Cargo.toml").is_file() {
        return Err(String::from("Run this from the repository root, where the workspace's Cargo.toml is."));
    }

    let scratch_directory = candidate_directory.join("target").join("compare-builds");

    std::fs::create_dir_all(&scratch_directory).map_err(|io_error| format!("Cannot create {}: {io_error}.", scratch_directory.display()))?;

    // The checkout is removed when this goes out of scope, on every way out of the function.
    let baseline_checkout;
    let (baseline_directory, baseline_description) = match compare_arguments.get_baseline_source() {
        BaselineSource::GitReference(git_reference) => {
            baseline_checkout = BaselineCheckout::create(&candidate_directory, git_reference, &scratch_directory.join("baseline_source"))?;

            (baseline_checkout.get_directory().to_path_buf(), format!("commit {git_reference}"))
        }
        BaselineSource::Directory(baseline_directory) => (baseline_directory.clone(), baseline_directory.display().to_string()),
    };

    println!(
        "comparing {} of {}: {baseline_description} (baseline) against the working tree (candidate)",
        compare_arguments.get_tool_name(),
        compare_arguments.get_package_name()
    );

    let versions = [
        (BASELINE_LABEL, baseline_directory.as_path()),
        (CANDIDATE_LABEL, candidate_directory.as_path()),
    ];
    let mut built_tools: Vec<(&str, BuildSetting, PathBuf)> = Vec::new();

    for (version_label, workspace_directory) in versions {
        for build_setting in BuildSetting::ALL {
            println!("building {version_label}, {}", build_setting.get_label());

            let target_directory = scratch_directory.join(format!("{version_label}_{}", build_setting.get_label()));

            // The baseline can be a different commit or directory on every run, with file names
            // and timestamps Cargo would mistake for the last one's. Starting its build folders
            // empty is the only way to be sure the binary is this baseline's. The candidate is
            // always the working tree, which Cargo tracks correctly, so its folders are kept.
            if version_label == BASELINE_LABEL && target_directory.exists() {
                std::fs::remove_dir_all(&target_directory).map_err(|io_error| format!("Cannot clear {}: {io_error}.", target_directory.display()))?;
            }

            let executable_path = tool_builder::build_tool(
                workspace_directory,
                compare_arguments.get_package_name(),
                compare_arguments.get_tool_name(),
                build_setting,
                &target_directory,
            )?;

            built_tools.push((version_label, build_setting, executable_path));
        }
    }

    let build_notes = check_binaries_differ(&built_tools)?;
    let mut comparison_report = ComparisonReport::new(BASELINE_LABEL, CANDIDATE_LABEL);

    for round_number in 1..=compare_arguments.get_round_count() {
        println!("round {round_number} of {}", compare_arguments.get_round_count());

        // Setting outside, version inside: the two runs being compared are always next to each other.
        for build_setting in BuildSetting::ALL {
            for (version_label, tool_build_setting, executable_path) in &built_tools {
                if *tool_build_setting != build_setting {
                    continue;
                }

                let result_line = tool_runner::run_tool(executable_path, compare_arguments.get_tool_arguments())?;

                comparison_report.record_run(version_label, build_setting, result_line);
            }
        }
    }

    println!();

    for build_note in build_notes {
        println!("note: {build_note}");
    }

    comparison_report.print();

    Ok(())
}

/// Confirms the three builds of each version are three different binaries, and says so if the two
/// versions built to the same one.
///
/// A setting that changed nothing would be measured as if it were a second opinion, and identical
/// binaries for both versions means there is nothing to compare. Both are worth knowing before
/// reading any number.
fn check_binaries_differ(built_tools: &[(&str, BuildSetting, PathBuf)]) -> Result<Vec<String>, String> {
    let mut binaries: Vec<(&str, BuildSetting, Vec<u8>)> = Vec::new();

    for (version_label, build_setting, executable_path) in built_tools {
        binaries.push((version_label, *build_setting, read_binary(executable_path)?));
    }

    let mut build_notes = Vec::new();

    for (first_index, (first_label, first_setting, first_bytes)) in binaries.iter().enumerate() {
        for (second_label, second_setting, second_bytes) in &binaries[first_index + 1..] {
            if first_bytes != second_bytes {
                continue;
            }

            if first_label == second_label {
                return Err(format!(
                    "The {} and {} builds of the {first_label} are the same binary, so one of the settings did nothing.",
                    first_setting.get_label(),
                    second_setting.get_label()
                ));
            }

            if first_setting == second_setting {
                build_notes.push(format!(
                    "the {} builds of the two versions are the same binary, byte for byte.",
                    first_setting.get_label()
                ));
            }
        }
    }

    Ok(build_notes)
}

fn read_binary(executable_path: &Path) -> Result<Vec<u8>, String> {
    std::fs::read(executable_path).map_err(|io_error| format!("Cannot read {}: {io_error}.", executable_path.display()))
}
