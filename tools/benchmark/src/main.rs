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
mod plan;
mod process;
mod statistics;

use std::path::Path;

fn main() {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    if arguments.len() == 1 && (arguments[0] == "--help" || arguments[0] == "-h") {
        println!("omega-bench compare <plan.json> <new-result-directory>");
        return;
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
                report.comparison.paired_percent_change.median,
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
