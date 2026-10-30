//! Paired observations stay paired during resampling; warmups never enter inference.

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Distribution {
    pub median: f64,
    pub mean: f64,
    pub observed_p95: f64,
    pub observed_p99: f64,
    pub minimum: f64,
    pub maximum: f64,
    pub median_absolute_deviation: f64,
}

#[derive(Debug, Serialize)]
pub struct Comparison {
    pub pairs: usize,
    pub baseline: Distribution,
    pub candidate: Distribution,
    pub paired_candidate_minus_baseline: Distribution,
    pub paired_percent_change: Option<Distribution>,
    pub paired_percent_change_bootstrap_95: Option<[f64; 2]>,
    pub median_difference_bootstrap_95: [f64; 2],
    pub bootstrap_resamples: usize,
    pub bootstrap_seed: u64,
    pub interpretation: &'static str,
}

fn median(values: &[f64]) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let middle = sorted.len() / 2;
    if sorted.len().is_multiple_of(2) {
        (sorted[middle - 1] + sorted[middle]) / 2.0
    } else {
        sorted[middle]
    }
}

fn distribution(values: &[f64]) -> Distribution {
    let center = median(values);
    let mut ordered = values.to_vec();
    ordered.sort_by(f64::total_cmp);
    // Nearest-rank empirical quantiles describe these samples only. At small
    // counts both tails are simply the maximum, not population tail estimates.
    let quantile = |percentile: usize| ordered[(ordered.len() * percentile).div_ceil(100) - 1];
    Distribution {
        median: center,
        mean: values.iter().sum::<f64>() / values.len() as f64,
        observed_p95: quantile(95),
        observed_p99: quantile(99),
        minimum: values.iter().copied().fold(f64::INFINITY, f64::min),
        maximum: values.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        median_absolute_deviation: median(
            &values
                .iter()
                .map(|value| (value - center).abs())
                .collect::<Vec<_>>(),
        ),
    }
}

pub fn compare(baseline: &[f64], candidate: &[f64]) -> Result<Comparison, String> {
    if baseline.len() < 2
        || baseline.len() != candidate.len()
        || baseline
            .iter()
            .chain(candidate)
            .any(|value| !value.is_finite() || *value < 0.0)
    {
        return Err(
            "comparison requires matching nonnegative finite samples from at least two pairs"
                .into(),
        );
    }
    let differences: Vec<_> = baseline
        .iter()
        .zip(candidate)
        .map(|(before, after)| after - before)
        .collect();
    let percentages: Vec<_> = baseline
        .iter()
        .zip(candidate)
        .map(|(before, after)| (after / before - 1.0) * 100.0)
        .collect();
    let has_percentages = baseline.iter().all(|value| *value > 0.0)
        && percentages.iter().all(|value| value.is_finite());
    let seed = 0x6f6d_6567_6162_656e_u64;
    let mut random = seed;
    let mut bootstrap = Vec::with_capacity(4000);
    let mut resampled = vec![0.0; differences.len()];
    let mut resampled_percentages = vec![0.0; differences.len()];
    let mut percentage_bootstrap = Vec::with_capacity(4000);
    for _ in 0..4000 {
        for (sample_index, value) in resampled.iter_mut().enumerate() {
            random ^= random << 13;
            random ^= random >> 7;
            random ^= random << 17;
            let pair_index = (random % differences.len() as u64) as usize;
            *value = differences[pair_index];
            if has_percentages {
                resampled_percentages[sample_index] = percentages[pair_index];
            }
        }
        bootstrap.push(median(&resampled));
        if has_percentages {
            percentage_bootstrap.push(median(&resampled_percentages));
        }
    }
    bootstrap.sort_by(f64::total_cmp);
    percentage_bootstrap.sort_by(f64::total_cmp);
    let interval = [bootstrap[100], bootstrap[3899]];
    // A tiny sample's degenerate interval is not strong evidence, even if every
    // observed difference happens to have the same sign.
    let interpretation = if baseline.len() < 6 {
        "insufficient_pairs"
    } else if interval[1] < 0.0 {
        "candidate_lower_in_this_run"
    } else if interval[0] > 0.0 {
        "candidate_higher_in_this_run"
    } else {
        "inconclusive"
    };
    Ok(Comparison {
        pairs: baseline.len(),
        baseline: distribution(baseline),
        candidate: distribution(candidate),
        paired_candidate_minus_baseline: distribution(&differences),
        paired_percent_change: has_percentages.then(|| distribution(&percentages)),
        paired_percent_change_bootstrap_95: has_percentages
            .then(|| [percentage_bootstrap[100], percentage_bootstrap[3899]]),
        median_difference_bootstrap_95: interval,
        bootstrap_resamples: bootstrap.len(),
        bootstrap_seed: seed,
        interpretation,
    })
}

pub fn compare_independent(
    baseline: &[f64],
    candidate: &[f64],
) -> Result<serde_json::Value, String> {
    if baseline.len() < 2
        || candidate.len() < 2
        || baseline
            .iter()
            .chain(candidate)
            .any(|value| !value.is_finite() || *value <= 0.0)
    {
        return Err("independent comparison requires two positive finite observation sets".into());
    }
    let mut random = 0x6869_7374_6f72_7921_u64;
    let mut before = vec![0.0; baseline.len()];
    let mut after = vec![0.0; candidate.len()];
    let mut differences = Vec::with_capacity(4000);
    let mut percentages = Vec::with_capacity(4000);
    let mut pick = |length: usize| {
        random ^= random << 13;
        random ^= random >> 7;
        random ^= random << 17;
        (random % length as u64) as usize
    };
    for _ in 0..4000 {
        for value in &mut before {
            *value = baseline[pick(baseline.len())];
        }
        for value in &mut after {
            *value = candidate[pick(candidate.len())];
        }
        let before = median(&before);
        let after = median(&after);
        differences.push(after - before);
        percentages.push((after / before - 1.0) * 100.0);
    }
    differences.sort_by(f64::total_cmp);
    percentages.sort_by(f64::total_cmp);
    Ok(
        serde_json::json!({"baseline":distribution(baseline),"candidate":distribution(candidate),
        "median_difference_bootstrap_95":[differences[100],differences[3899]],
        "median_percent_change_bootstrap_95":[percentages[100],percentages[3899]],
        "bootstrap_resamples":4000,"bootstrap_seed":0x6869_7374_6f72_7921_u64,
        "minimum_sample_count_met":baseline.len()>=6&&candidate.len()>=6}),
    )
}

#[cfg(test)]
mod tests {
    use super::compare;

    #[test]
    fn empirical_tails_use_nearest_rank_without_interpolation() {
        let values: Vec<_> = (1..=100).map(f64::from).collect();
        let distribution = super::distribution(&values);
        assert_eq!(distribution.mean, 50.5);
        assert_eq!(distribution.observed_p95, 95.0);
        assert_eq!(distribution.observed_p99, 99.0);
        assert_eq!(super::distribution(&[1.0, 2.0]).observed_p95, 2.0);
    }

    #[test]
    fn retains_pairing_under_host_drift() {
        let before = [10.0, 100.0, 20.0, 200.0, 30.0, 300.0];
        let after = before.map(|value| value - 2.0);
        let result = compare(&before, &after).expect("valid samples");
        assert_eq!(result.paired_candidate_minus_baseline.median, -2.0);
        assert_eq!(result.median_difference_bootstrap_95, [-2.0, -2.0]);
        assert_eq!(result.interpretation, "candidate_lower_in_this_run");
    }

    #[test]
    fn refuses_invalid_or_unmatched_samples() {
        assert!(compare(&[1.0], &[1.0]).is_err());
        assert!(compare(&[1.0, 2.0], &[1.0]).is_err());
        assert!(compare(&[1.0, f64::NAN], &[1.0, 2.0]).is_err());
        assert!(compare(&[1.0, 2.0], &[-1.0, 2.0]).is_err());
        let zero = compare(&[0.0, 0.0], &[0.0, 1.0]).unwrap();
        assert!(zero.paired_percent_change.is_none());
    }

    #[test]
    fn equality_and_small_samples_do_not_claim_a_win() {
        assert_eq!(
            compare(&[1.0; 6], &[1.0; 6]).unwrap().interpretation,
            "inconclusive"
        );
        assert_eq!(
            compare(&[10.0; 2], &[1.0; 2]).unwrap().interpretation,
            "insufficient_pairs"
        );
    }
    #[test]
    fn independent_histories_recover_known_distribution_shift() {
        let result = super::compare_independent(&[10.0; 8], &[12.0; 9]).unwrap();
        assert_eq!(
            result["median_difference_bootstrap_95"],
            serde_json::json!([2.0, 2.0])
        );
        let percent = result["median_percent_change_bootstrap_95"][0]
            .as_f64()
            .unwrap();
        assert!((percent - 20.0).abs() < 1e-10);
    }
}
