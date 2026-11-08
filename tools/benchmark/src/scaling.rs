//! Fail when compiling one more machine costs more in a bigger program.
//!
//! Most compiler slowdowns found since October were a per-question walk over
//! the whole program: an owner found by scanning every machine, a state
//! replayed from its first statement, a table rebuilt per query. Each one makes
//! the cost of a machine grow with the number of other machines, so a compile
//! goes quadratic. This command measures that directly. It checks the one-line
//! `Main::main` plus N identical, uncalled machines for each N in `--sizes`,
//! and compares the cost per added machine in the first and last intervals.
//! Every machine is checked in full whether or not anything calls it, so a
//! linear compiler keeps that ratio near 1.
//!
//! Cost is user-space instructions where Linux `perf` exists (stable on a
//! shared host) and user+system CPU otherwise. Run it on a quiet host
//! (`omega-linux`) against a release compiler; it exits 1 when the ratio
//! exceeds `--max-ratio`, so a change can show it did not add a whole-program
//! walk, or show the ratio it removed.

use crate::progress::{git, measure, median};
use serde_json::{Value, json};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

const USAGE: &str = "omega-bench scaling --omega <release-omega> [--shape machines|citations] [--sizes 0,1000,2000] [--runs N] [--max-ratio R]";

#[derive(Clone, Copy)]
enum Shape {
    Machines,
    Citations,
}

pub fn run(arguments: &[String]) -> Result<(), String> {
    let mut omega = None;
    let mut shape = Shape::Machines;
    let mut sizes = None;
    let mut runs = 1_usize;
    let mut max_ratio = 1.3_f64;
    let mut arguments = arguments.iter();
    while let Some(argument) = arguments.next() {
        let value = arguments
            .next()
            .ok_or_else(|| format!("{argument} needs a value\nusage: {USAGE}"))?;
        match argument.as_str() {
            "--omega" => omega = Some(PathBuf::from(value)),
            "--shape" => {
                shape = match value.as_str() {
                    "machines" => Shape::Machines,
                    "citations" => Shape::Citations,
                    _ => return Err("--shape takes machines or citations".into()),
                };
            }
            "--sizes" => {
                sizes = Some(
                    value
                        .split(',')
                        .map(|size| size.trim().parse())
                        .collect::<Result<Vec<usize>, _>>()
                        .map_err(|_| "--sizes takes comma-separated counts")?,
                );
            }
            "--runs" => runs = value.parse().map_err(|_| "--runs takes a count")?,
            "--max-ratio" => max_ratio = value.parse().map_err(|_| "--max-ratio takes a number")?,
            _ => return Err(format!("unknown argument {argument}\nusage: {USAGE}")),
        }
    }
    let sizes = sizes.unwrap_or_else(|| match shape {
        Shape::Machines => vec![0, 1000, 2000],
        Shape::Citations => vec![0, 8, 16],
    });
    let omega = omega.ok_or_else(|| format!("--omega is required\nusage: {USAGE}"))?;
    let omega =
        fs::canonicalize(&omega).map_err(|error| format!("{}: {error}", omega.display()))?;
    if sizes.len() < 3 || sizes.windows(2).any(|pair| pair[0] >= pair[1]) || runs == 0 {
        return Err("--sizes needs at least three increasing counts and --runs at least 1".into());
    }
    let root = PathBuf::from(git(&["rev-parse", "--show-toplevel"])?);
    let scratch = std::env::temp_dir().join(format!("omega-scaling-{}", std::process::id()));

    let mut points = Vec::new();
    for &size in &sizes {
        let project = scratch.join(format!("n{size}"));
        write_program(shape, size, &project)?;
        let main = project.join("main.omg").display().to_string();
        let template = ["--check", "--build-dir", "{build}", main.as_str()];
        let mut costs = Vec::new();
        let mut metric = "instructions_u";
        // One untimed warmup fills the file cache.
        for index in 0..=runs {
            let row = measure(
                &root,
                &omega,
                &format!("n{size}"),
                &template,
                None,
                "run",
                index,
                &scratch,
            )?;
            if row["ok"] != true {
                let _ = fs::remove_dir_all(&scratch);
                return Err(format!("N={size} failed to check: {}", row["stderr_tail"]));
            }
            if index == 0 {
                continue;
            }
            let cost = row["instructions_u"].as_f64().or_else(|| {
                metric = "cpu_s";
                Some(row["user_cpu_s"].as_f64()? + row["sys_cpu_s"].as_f64()?)
            });
            costs.push(cost.ok_or("this platform reports neither instructions nor CPU time")?);
        }
        let cost = median(&mut costs).expect("at least one run");
        eprintln!("scaling: N={size} {metric} {cost:.4e}");
        points.push((size, cost, metric));
    }
    let _ = fs::remove_dir_all(&scratch);

    let marginal =
        |pair: &[(usize, f64, &str)]| (pair[1].1 - pair[0].1) / (pair[1].0 - pair[0].0) as f64;
    let marginals: Vec<f64> = points.windows(2).map(marginal).collect();
    let ratio = marginals[marginals.len() - 1] / marginals[0];
    let mut table = String::new();
    for (pair, cost) in points.windows(2).zip(&marginals) {
        let _ = writeln!(
            table,
            "  N {:>6} -> {:>6}: {cost:.4e} {} per added unit",
            pair[0].0, pair[1].0, pair[0].2
        );
    }
    print!("{table}");
    let report: Value = json!({
        "metric": points[0].2,
        "points": points.iter().map(|(size, cost, _)| json!({"size": size, "cost": cost})).collect::<Vec<_>>(),
        "marginal_per_unit": marginals,
        "ratio": ratio,
        "max_ratio": max_ratio,
    });
    println!("{report}");
    if ratio > max_ratio {
        return Err(format!(
            "per-unit cost grew {ratio:.2}x from the first to the last interval (limit {max_ratio}): \
             some check walks the whole program or state prefix per unit"
        ));
    }
    println!("scaling: per-unit cost ratio {ratio:.2} (limit {max_ratio})");
    Ok(())
}

/// `machines`: the one-line program plus `size` uncalled copies of one machine,
/// exposing checks that scan the whole program per machine.
/// `citations`: one state citing the same lemma `size` times, every citation
/// needing preconditions proved, exposing checks that replay a state's prefix
/// per call (cost per citation then grows with the citations before it).
fn write_program(shape: Shape, size: usize, directory: &Path) -> Result<(), String> {
    fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    let build = "machine build(builder: &mut Build) {\n    Build::application(builder, \"machine_count\");\n    builder.freestanding = true;\n}\n";
    let mut main = String::new();
    match shape {
        Shape::Machines => {
            main.push_str("data Main {\n    marker: u64;\n}\nmachine Main::main(main: &mut Main) {\n    main.marker = 0;\n}\n");
            for index in 1..=size {
                let _ = write!(
                    main,
                    "machine Main::step_{index}(main: &mut Main) {{\n    main.marker = {index};\n}}\n"
                );
            }
        }
        Shape::Citations => {
            main.push_str("machine keep(a: Nat, b: Nat)\nrequires\n    core::Identity<Nat>(saturating_sub(a, b), Nat::Zero)\nensures\n    core::Identity<Nat>(saturating_sub(a, b), Nat::Zero)\n{\n}\n\nmachine chain(a: Nat, b: Nat)\nrequires\n    core::Identity<Nat>(saturating_sub(a, b), Nat::Zero)\n{\n");
            for _ in 0..size {
                main.push_str("    keep(a, b);\n");
            }
            main.push_str("}\n\nmachine main() {\n}\n");
        }
    }
    let write = |name: &str, text: &str| {
        fs::write(directory.join(name), text).map_err(|error| format!("write {name}: {error}"))
    };
    write("build.omg", build)?;
    write("main.omg", &main)
}
