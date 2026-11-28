//! Draw the measured pairs from a verified journal, in round order.
//! Each CPU measurement belongs to the same subject process as its elapsed time.

use crate::experiment::Sample;
use crate::plan::Measurement;
use crate::records::VerifiedRun;
use crate::statistics;
use std::fmt::Write;

fn cpu_ns(sample: &Sample) -> Option<f64> {
    let resources = &sample.observations.first()?.resources;
    let user = resources["user_cpu_ns"]["value"].as_u64()?;
    let system = resources["system_cpu_ns"]["value"].as_u64()?;
    Some(user as f64 + system as f64)
}

pub(crate) fn render(run: &VerifiedRun) -> Result<String, String> {
    let mut page = String::from(include_str!("pair_chart_header.html"));
    let name = crate::html_text(&run.plan.name);
    write!(
        page,
        "<h1>{name}</h1><p>{} measured pairs. Warmups excluded.</p>",
        run.plan.pairs
    )
    .unwrap();
    page.push_str("<details><summary>Compared executables</summary><dl>");
    for (role, command) in [
        ("Baseline", &run.plan.baseline),
        ("Candidate", &run.plan.candidate),
    ] {
        let path = command.program.display().to_string();
        let hash = run.identities[&path]
            .as_str()
            .ok_or("missing executable identity")?;
        write!(
            page,
            "<dt>{role}</dt><dd><code>{}</code><br>SHA-256 <code>{hash}</code></dd>",
            crate::html_text(&path)
        )
        .unwrap();
    }
    page.push_str("</dl></details>");
    if run.plan.pairs < 6 {
        page.push_str("<p>Fewer than six pairs were measured. The runner classifies this comparison as insufficient.</p>");
    }

    let reported = matches!(run.plan.measurement, Measurement::ReportedWork { .. });
    if reported {
        plot(&mut page, run, "Child-reported work", true, |sample| {
            sample.primary_value
        })?;
    }
    plot(&mut page, run, "Process elapsed time", false, |sample| {
        Some(sample.observations.first()?.process_wall_ns as f64)
    })?;
    plot(&mut page, run, "Process CPU time", false, cpu_ns)?;
    page.push_str("</main></html>");
    Ok(page)
}

fn plot(
    page: &mut String,
    run: &VerifiedRun,
    title: &str,
    per_unit: bool,
    value: impl Fn(&Sample) -> Option<f64>,
) -> Result<(), String> {
    let mut pairs = Vec::with_capacity(run.plan.pairs);
    for pair in run
        .samples
        .as_chunks::<2>()
        .0
        .iter()
        .filter(|pair| !pair[0].warmup)
    {
        let (baseline, candidate) = if pair[0].role == "baseline" {
            (&pair[0], &pair[1])
        } else {
            (&pair[1], &pair[0])
        };
        pairs.push((value(baseline), value(candidate)));
    }
    write!(page, "<section><h2>{title}</h2>").unwrap();
    let maximum = pairs
        .iter()
        .flat_map(|&(a, b)| [a, b])
        .flatten()
        .fold(0.0, f64::max);
    let (divisor, unit) = if per_unit {
        (1.0, "ns/unit")
    } else if maximum >= 1e9 {
        (1e9, "s")
    } else {
        (1e6, "ms")
    };
    let complete: Vec<_> = pairs.iter().filter_map(|&(a, b)| a.zip(b)).collect();
    if complete.len() == pairs.len() {
        let (before, after): (Vec<_>, Vec<_>) = complete.into_iter().unzip();
        let summary = statistics::compare(&before, &after)?;
        let [low, high] = summary.median_difference_bootstrap_95;
        write!(page, "<p>Median {:.3} → {:.3} {unit}. Median paired change {:+.3} {unit} (95% bootstrap interval {:+.3} to {:+.3} {unit}).</p>",
            summary.baseline.median/divisor, summary.candidate.median/divisor,
            summary.paired_candidate_minus_baseline.median/divisor, low/divisor, high/divisor).unwrap();
    } else {
        write!(
            page,
            "<p>Both measurements are available in {} of {} pairs.</p>",
            complete.len(),
            pairs.len()
        )
        .unwrap();
    }
    write!(page, "<table><thead><tr><th>Pair</th><th>Baseline ({unit})</th><th>Candidate ({unit})</th></tr></thead><tbody>").unwrap();
    for (round, (before, after)) in pairs.into_iter().enumerate() {
        write!(page, "<tr><th scope=row>{}</th>", round + 1).unwrap();
        for measured in [before, after] {
            if let Some(n) = measured {
                let width = if maximum > 0.0 {
                    n / maximum * 100.0
                } else {
                    0.0
                };
                write!(
                    page,
                    "<td>{:.3}<i aria-hidden=true style='width:{width:.4}%'></i></td>",
                    n / divisor
                )
                .unwrap();
            } else {
                page.push_str("<td>Unavailable</td>");
            }
        }
        page.push_str("</tr>");
    }
    page.push_str("</tbody></table></section>");
    Ok(())
}
