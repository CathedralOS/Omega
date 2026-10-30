/*
   omega-bench compares one fixed workload under two explicit executables. We
   keep the ordering, oracle, declared inputs, and measurement boundary in a
   plan, then retain every warmup and measured observation in a new directory.
   A failed observation remains in the journal and prevents a success report.

   Compiler measurements include process startup, compilation, publication,
   and capture. Native execution is checked separately, outside that timer.
   Runtime process measurements use the same boundary. Sustained kernels may
   instead report an explicitly timed region with a checked checksum and work
   count; those values are labelled child-reported, never process timings.

   Start with README.md's Rust comparison examples. experiment.rs owns the
   prepare -> alternate -> validate -> summarize lifecycle; process.rs reuses
   Omega's bounded child runner. We do not invoke Cargo or settle package
   decisions during measurement. Existing Python records remain readable by
   their original workflow; a paired experiment has a separate schema.
*/

mod experiment;
mod history;
mod plan;
mod process;
mod profiling;
mod records;
mod statistics;
mod suite;

use std::path::Path;

fn main() {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    if arguments.len() == 1 && (arguments[0] == "--help" || arguments[0] == "-h") {
        println!(
            "omega-bench compare <plan.json> <new-result-directory>\nomega-bench suite <suite.json> <new-result-directory>\nomega-bench validate <run-directory>\nomega-bench show <run-directory>\nomega-bench check <run-directory> <maximum-regression-percent>\nomega-bench history <old-run-directory> <new-run-directory>\nomega-bench profile <plan.json> <baseline|candidate> <phase|allocations|wpr|perf-stat|perf-record> <new-result-directory>"
        );
        return;
    }
    let command = arguments.first().and_then(|argument| argument.to_str());
    if arguments.len() == 3 && command == Some("history") {
        match history::compare(Path::new(&arguments[1]), Path::new(&arguments[2])).and_then(
            |report| serde_json::to_string_pretty(&report).map_err(|error| error.to_string()),
        ) {
            Ok(report) => {
                println!("{report}");
                return;
            }
            Err(error) => {
                eprintln!("omega-bench: {error}");
                std::process::exit(1);
            }
        }
    }
    if arguments.len() == 5 && command == Some("profile") {
        let result = arguments[2]
            .to_str()
            .zip(arguments[3].to_str())
            .ok_or("profile arguments must be UTF-8".to_owned())
            .and_then(|(role, backend)| {
                profiling::run(
                    Path::new(&arguments[1]),
                    role,
                    backend,
                    Path::new(&arguments[4]),
                )
            });
        match result {
            Ok(true) => {
                println!("profile complete");
                return;
            }
            Ok(false) => {
                eprintln!("profile unavailable or failed; inspect profile.json");
                std::process::exit(2);
            }
            Err(error) => {
                eprintln!("omega-bench: {error}");
                std::process::exit(1);
            }
        }
    }
    if arguments.len() == 2 && matches!(command, Some("validate" | "show")) {
        match records::validate(Path::new(&arguments[1])) {
            Ok(run) => {
                if command == Some("show") {
                    print!("{}", records::markdown(&run));
                } else {
                    println!("validated {}", run.plan.name);
                }
                return;
            }
            Err(error) => {
                eprintln!("omega-bench: {error}");
                std::process::exit(1);
            }
        }
    }
    if arguments.len() == 3 && command == Some("check") {
        let result = records::validate(Path::new(&arguments[1])).and_then(|run| {
            let budget = arguments[2]
                .to_str()
                .ok_or("invalid regression budget")?
                .parse::<f64>()
                .map_err(|error| error.to_string())?;
            records::regression_exit(&run.report, budget)
        });
        match result {
            Ok(code) => {
                println!(
                    "{}",
                    match code {
                        0 => "within budget",
                        3 => "regression",
                        _ => "inconclusive",
                    }
                );
                std::process::exit(i32::from(code));
            }
            Err(error) => {
                eprintln!("omega-bench: {error}");
                std::process::exit(1);
            }
        }
    }
    if arguments.len() == 3 && command == Some("suite") {
        match suite::run(Path::new(&arguments[1]), Path::new(&arguments[2])) {
            Ok(()) => return,
            Err(error) => {
                eprintln!("omega-bench: {error}");
                std::process::exit(1);
            }
        }
    }
    if arguments.len() != 3 || arguments[0] != "compare" {
        eprintln!("usage: omega-bench compare <plan.json> <new-result-directory>");
        std::process::exit(2);
    }
    match experiment::compare(Path::new(&arguments[1]), Path::new(&arguments[2])) {
        Ok(report) => {
            let (scale, unit) = if report.metric == "reported_work_ns_per_unit" {
                (1.0, "ns/unit")
            } else {
                (1_000_000.0, "ms")
            };
            println!("{}: {}", report.name, report.metric);
            println!(
                "baseline median {:.3} {unit}; candidate median {:.3} {unit}",
                report.comparison.baseline.median / scale,
                report.comparison.candidate.median / scale
            );
            println!(
                "paired change {:+.2}%; difference 95% bootstrap interval [{:.3}, {:.3}] {unit}: {}",
                report
                    .comparison
                    .paired_percent_change
                    .as_ref()
                    .map_or(0.0, |change| change.median),
                report.comparison.median_difference_bootstrap_95[0] / scale,
                report.comparison.median_difference_bootstrap_95[1] / scale,
                report.comparison.interpretation
            );
            println!("records: {}", Path::new(&arguments[2]).display());
        }
        Err(error) => {
            eprintln!("omega-bench: {error}");
            std::process::exit(1);
        }
    }
}
