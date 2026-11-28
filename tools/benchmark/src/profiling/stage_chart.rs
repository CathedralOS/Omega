use serde::Serialize;
use std::collections::BTreeMap;
use std::fmt::Write;

#[derive(Serialize)]
pub(super) struct Interval {
    pub start_ms: f64,
    pub end_ms: f64,
}

#[derive(Serialize)]
pub(super) struct Phase<'a> {
    pub phase: &'a str,
    #[serde(flatten)]
    pub interval: Interval,
}

#[derive(Serialize)]
pub(super) struct Timeline<'a> {
    pub compiler_elapsed_ms: f64,
    pub covered_ms: f64,
    pub unattributed_ms: f64,
    pub gaps: Vec<Interval>,
    pub spans: Vec<Phase<'a>>,
}

fn escaped(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn bar(start: f64, end: f64, scale: f64) -> String {
    let percent = |ms: f64| {
        if scale > 0.0 {
            100.0 * (ms / scale)
        } else {
            0.0
        }
    };
    format!(
        "<div class=track aria-hidden=true><i style='margin-left:{:.4}%;width:{:.4}%'></i></div>",
        percent(start),
        percent(end - start)
    )
}

pub(super) fn render(
    name: &str,
    role: &str,
    timeline: &Timeline<'_>,
    stages: &BTreeMap<String, f64>,
) -> String {
    let mut page = String::from(include_str!("stage_chart_header.html"));
    write!(
        page,
        "<h1>{}</h1><p>{} compiler · {:.3} ms elapsed · {:.3} ms covered · {:.3} ms unassigned</p>",
        escaped(name),
        escaped(role),
        timeline.compiler_elapsed_ms,
        timeline.covered_ms,
        timeline.unattributed_ms
    )
    .unwrap();
    page.push_str("<h2>Command timeline</h2><p>All bars share the command clock. Overlapping spans count once toward covered time.</p><table><thead><tr><th>Phase</th><th>Start (ms)</th><th>End (ms)</th><th>Elapsed (ms)</th><th>Position</th></tr></thead><tbody>");
    let mut intervals: Vec<_> = timeline
        .spans
        .iter()
        .map(|s| (s.phase, &s.interval))
        .chain(timeline.gaps.iter().map(|gap| ("Unassigned", gap)))
        .collect();
    intervals.sort_by(|a, b| a.1.start_ms.total_cmp(&b.1.start_ms));
    for (phase, interval) in intervals {
        write!(
            page,
            "<tr><th scope=row>{}</th><td>{:.3}</td><td>{:.3}</td><td>{:.3}</td><td>{}</td></tr>",
            escaped(phase),
            interval.start_ms,
            interval.end_ms,
            interval.end_ms - interval.start_ms,
            bar(
                interval.start_ms,
                interval.end_ms,
                timeline.compiler_elapsed_ms
            )
        )
        .unwrap();
    }
    page.push_str("</tbody></table><h2>Compiler stages</h2><p>Stages can overlap or contain other stages. Each bar is relative to the longest stage. Names include the input and output where the compiler reports them.</p>");
    let mut stages: Vec<_> = stages
        .iter()
        .filter(|(name, _)| name.as_str() != "total elapsed")
        .collect();
    stages.sort_by(|a, b| b.1.total_cmp(a.1));
    if let Some((_, longest)) = stages.first().copied() {
        page.push_str("<table><thead><tr><th>Stage</th><th>Accumulated ms</th><th>Relative duration</th></tr></thead><tbody>");
        for (name, duration) in stages {
            write!(
                page,
                "<tr><th scope=row>{}</th><td>{duration:.3}</td><td>{}</td></tr>",
                escaped(name),
                bar(0.0, *duration, *longest)
            )
            .unwrap();
        }
        page.push_str("</tbody></table>");
    } else {
        page.push_str("<p>The compiler reported no stage durations.</p>");
    }
    page.push_str("<p><a href='profile.json'>Raw measurements and run identities</a> · <a href='subject.stderr'>Compiler timings and measurement counts</a></p></main></html>");
    page
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_zero_duration_profile_keeps_labels_as_text_and_excludes_the_total_from_stages() {
        let report = render(
            "a < b & c",
            "candidate",
            &Timeline {
                compiler_elapsed_ms: 0.0,
                covered_ms: 0.0,
                unattributed_ms: 0.0,
                spans: vec![Phase {
                    phase: "source <phase>",
                    interval: Interval {
                        start_ms: 0.0,
                        end_ms: 0.0,
                    },
                }],
                gaps: vec![],
            },
            &BTreeMap::from([("total elapsed".into(), 0.0), ("source -> psi".into(), 0.0)]),
        );
        assert!(report.contains("a &lt; b &amp; c"));
        assert!(report.contains("source &lt;phase&gt;"));
        assert!(report.contains("source -&gt; psi"));
        assert!(!report.contains("total elapsed"));
        assert!(!report.contains("NaN"));
    }
}
