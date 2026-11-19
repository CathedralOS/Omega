//! Collects every run's result line and turns them into a table and one sentence.

use crate::build_setting::BuildSetting;
use measure::ResultLine;

/// How large a difference has to be before we call it one.
///
/// Two builds of the very same source, run through this program, came out within about 1 percent
/// of each other on the lexer, so anything inside 2 percent is treated as no difference. See
/// experiment 12 in the lexer's EXPERIMENTS.md.
const NOISE_FLOOR_PERCENT: f64 = 2.0;

struct RecordedRun {
    version_label: String,
    build_setting: BuildSetting,
    result_line: ResultLine,
}

/// Every run of a comparison, and what they add up to.
pub struct ComparisonReport {
    baseline_label: String,
    candidate_label: String,
    recorded_runs: Vec<RecordedRun>,
}

impl ComparisonReport {
    pub fn new(
        baseline_label: &str,
        candidate_label: &str,
    ) -> Self {
        Self {
            baseline_label: baseline_label.to_string(),
            candidate_label: candidate_label.to_string(),
            recorded_runs: Vec::new(),
        }
    }

    pub fn record_run(
        &mut self,
        version_label: &str,
        build_setting: BuildSetting,
        result_line: ResultLine,
    ) {
        self.recorded_runs.push(RecordedRun {
            version_label: version_label.to_string(),
            build_setting,
            result_line,
        });
    }

    pub fn print(&self) {
        for key in self.get_keys() {
            match ResultLine::is_higher_better(&key) {
                Some(is_higher_better) => self.print_number_table(&key, is_higher_better),
                None => println!("{}\n", self.describe_text_key(&key)),
            }
        }

        println!("{}", self.describe_verdict());
    }

    /// One sentence on the first number the tool reports, which a tool should make its headline.
    pub fn describe_verdict(&self) -> String {
        let Some((primary_key, is_higher_better)) = self
            .get_keys()
            .into_iter()
            .find_map(|key| ResultLine::is_higher_better(&key).map(|is_higher_better| (key, is_higher_better)))
        else {
            return String::from("verdict: the tool reported no number to compare.");
        };

        let change_percents: Vec<f64> = BuildSetting::ALL
            .into_iter()
            .filter_map(|build_setting| self.get_change_percent(&primary_key, build_setting, is_higher_better))
            .collect();

        if change_percents.len() != BuildSetting::ALL.len() {
            return format!("verdict: `{primary_key}` was not reported by every build, so there is nothing to conclude.");
        }

        let smallest_change = change_percents.iter().copied().fold(f64::INFINITY, f64::min);
        let largest_change = change_percents
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        let candidate_label = &self.candidate_label;

        if smallest_change > NOISE_FLOOR_PERCENT {
            format!("verdict on {primary_key}: the {candidate_label} is faster in every build, by {smallest_change:.1}% to {largest_change:.1}%.")
        } else if largest_change < -NOISE_FLOOR_PERCENT {
            format!(
                "verdict on {primary_key}: the {candidate_label} is slower in every build, by {:.1}% to {:.1}%.",
                -largest_change, -smallest_change
            )
        } else {
            format!(
                "verdict on {primary_key}: no difference that every build agrees on. The changes run from {smallest_change:+.1}% to {largest_change:+.1}%, and anything inside {NOISE_FLOOR_PERCENT:.0}% either way is noise."
            )
        }
    }

    /// The keys in the order the tool printed them, taken from the first run that was recorded.
    fn get_keys(&self) -> Vec<String> {
        self.recorded_runs
            .first()
            .map(|recorded_run| {
                recorded_run
                    .result_line
                    .get_entries()
                    .iter()
                    .map(|(entry_key, _)| entry_key.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The best value one version reached under one setting, over all rounds.
    ///
    /// The best and not the average: other work on the machine can only make a run worse, so the
    /// best run is the one closest to what the code can do.
    fn get_best_value(
        &self,
        version_label: &str,
        build_setting: BuildSetting,
        key: &str,
        is_higher_better: bool,
    ) -> Option<f64> {
        self.recorded_runs
            .iter()
            .filter(|recorded_run| recorded_run.version_label == version_label && recorded_run.build_setting == build_setting)
            .filter_map(|recorded_run| recorded_run.result_line.get_value(key)?.parse::<f64>().ok())
            .reduce(|best_value, run_value| {
                if is_higher_better {
                    best_value.max(run_value)
                } else {
                    best_value.min(run_value)
                }
            })
    }

    /// How much faster the candidate is than the baseline under one setting, in percent.
    /// Negative means slower. A time and a rate are both turned into "how many times as fast".
    fn get_change_percent(
        &self,
        key: &str,
        build_setting: BuildSetting,
        is_higher_better: bool,
    ) -> Option<f64> {
        let baseline_value = self.get_best_value(&self.baseline_label, build_setting, key, is_higher_better)?;
        let candidate_value = self.get_best_value(&self.candidate_label, build_setting, key, is_higher_better)?;
        // The candidate's speed as a multiple of the baseline's. For a rate that is one over the
        // other; for a time it is the other way up, since half the time is twice the speed.
        let (numerator, denominator) = if is_higher_better {
            (candidate_value, baseline_value)
        } else {
            (baseline_value, candidate_value)
        };

        if denominator <= 0.0 {
            return None;
        }

        Some((numerator / denominator - 1.0) * 100.0)
    }

    fn print_number_table(
        &self,
        key: &str,
        is_higher_better: bool,
    ) {
        let direction_text = if is_higher_better { "higher is better" } else { "lower is better" };

        println!("{key} ({direction_text}; best of the rounds)");
        println!("  {:<12} {:>12} {:>12}   candidate is", "build", self.baseline_label, self.candidate_label);

        for build_setting in BuildSetting::ALL {
            let baseline_value = self.get_best_value(&self.baseline_label, build_setting, key, is_higher_better);
            let candidate_value = self.get_best_value(&self.candidate_label, build_setting, key, is_higher_better);
            let change_text = match self.get_change_percent(key, build_setting, is_higher_better) {
                Some(change_percent) if change_percent >= 0.0 => format!("{change_percent:.1}% faster"),
                Some(change_percent) => format!("{:.1}% slower", -change_percent),
                None => String::from("not comparable"),
            };

            println!(
                "  {:<12} {:>12} {:>12}   {change_text}",
                build_setting.get_label(),
                Self::format_value(baseline_value),
                Self::format_value(candidate_value)
            );
        }

        println!();
    }

    fn format_value(value: Option<f64>) -> String {
        value.map_or_else(|| String::from("-"), |number| format!("{number:.1}"))
    }

    /// Says whether a text value, such as a fingerprint, was the same in every run of both versions.
    fn describe_text_key(
        &self,
        key: &str,
    ) -> String {
        let baseline_values = self.get_distinct_text_values(&self.baseline_label, key);
        let candidate_values = self.get_distinct_text_values(&self.candidate_label, key);

        if baseline_values.len() == 1 && baseline_values == candidate_values {
            format!("{key}: the same in all {} runs ({})", self.recorded_runs.len(), baseline_values[0])
        } else {
            format!(
                "{key}: DIFFERENT. The {} printed {} and the {} printed {}. The two versions do not produce the same output, so a speed difference between them is not like for like.",
                self.baseline_label,
                baseline_values.join(", "),
                self.candidate_label,
                candidate_values.join(", ")
            )
        }
    }

    fn get_distinct_text_values(
        &self,
        version_label: &str,
        key: &str,
    ) -> Vec<String> {
        let mut distinct_values: Vec<String> = self
            .recorded_runs
            .iter()
            .filter(|recorded_run| recorded_run.version_label == version_label)
            .filter_map(|recorded_run| recorded_run.result_line.get_value(key))
            .map(str::to_string)
            .collect();

        distinct_values.sort();
        distinct_values.dedup();

        distinct_values
    }
}

#[cfg(test)]
mod tests {
    use crate::build_setting::BuildSetting;
    use crate::comparison_report::ComparisonReport;
    use measure::ResultLine;

    /// Records one run per version and setting, with the given headline numbers in setting order.
    fn build_report(
        key: &str,
        baseline_numbers: [f64; 3],
        candidate_numbers: [f64; 3],
        candidate_fingerprint: &str,
    ) -> ComparisonReport {
        let mut comparison_report = ComparisonReport::new("baseline", "candidate");

        for (setting_index, build_setting) in BuildSetting::ALL.into_iter().enumerate() {
            for (version_label, number, fingerprint) in [
                ("baseline", baseline_numbers[setting_index], "aaaa"),
                ("candidate", candidate_numbers[setting_index], candidate_fingerprint),
            ] {
                let mut result_line = ResultLine::new();

                result_line.add_number(key, number);
                result_line.add_text("fingerprint", fingerprint);
                comparison_report.record_run(version_label, build_setting, result_line);
            }
        }

        comparison_report
    }

    #[test]
    fn a_gain_in_every_build_is_called_faster() {
        let comparison_report = build_report("per_file_fastest_mbs", [700.0, 700.0, 700.0], [735.0, 770.0, 742.0], "aaaa");

        assert_eq!(
            comparison_report.describe_verdict(),
            "verdict on per_file_fastest_mbs: the candidate is faster in every build, by 5.0% to 10.0%."
        );
    }

    #[test]
    fn a_loss_in_every_build_is_called_slower() {
        let comparison_report = build_report("per_file_fastest_mbs", [700.0, 700.0, 700.0], [630.0, 665.0, 651.0], "aaaa");

        assert!(
            comparison_report
                .describe_verdict()
                .contains("slower in every build, by 5.0% to 10.0%")
        );
    }

    #[test]
    fn a_gain_that_one_build_does_not_show_is_not_a_gain() {
        let comparison_report = build_report("per_file_fastest_mbs", [700.0, 700.0, 700.0], [760.0, 705.0, 750.0], "aaaa");

        assert!(
            comparison_report
                .describe_verdict()
                .contains("no difference that every build agrees on")
        );
    }

    #[test]
    fn a_shorter_time_counts_as_faster() {
        let comparison_report = build_report("parse_min_ms", [200.0, 200.0, 200.0], [160.0, 180.0, 170.0], "aaaa");

        assert_eq!(
            comparison_report.describe_verdict(),
            "verdict on parse_min_ms: the candidate is faster in every build, by 11.1% to 25.0%."
        );
    }

    #[test]
    fn the_best_round_is_the_one_compared() {
        let mut comparison_report = build_report("per_file_fastest_mbs", [700.0, 700.0, 700.0], [700.0, 700.0, 700.0], "aaaa");
        let mut disturbed_result_line = ResultLine::new();

        // A second round in which something else on the machine slowed the candidate down.
        disturbed_result_line.add_number("per_file_fastest_mbs", 500.0);
        disturbed_result_line.add_text("fingerprint", "aaaa");
        comparison_report.record_run("candidate", BuildSetting::Plain, disturbed_result_line);

        assert_eq!(
            comparison_report.get_change_percent("per_file_fastest_mbs", BuildSetting::Plain, true),
            Some(0.0)
        );
    }

    #[test]
    fn a_fingerprint_that_differs_is_said_plainly() {
        let same_report = build_report("per_file_fastest_mbs", [700.0; 3], [700.0; 3], "aaaa");
        let different_report = build_report("per_file_fastest_mbs", [700.0; 3], [700.0; 3], "bbbb");

        assert_eq!(same_report.describe_text_key("fingerprint"), "fingerprint: the same in all 6 runs (aaaa)");
        assert!(
            different_report
                .describe_text_key("fingerprint")
                .starts_with("fingerprint: DIFFERENT.")
        );
    }
}
