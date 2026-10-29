//! Paired observations stay paired during resampling; warmups never enter inference.

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Distribution {
    pub median: f64,
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
    pub paired_percent_change: Distribution,
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
    Distribution {
        median: center,
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
            .any(|value| !value.is_finite() || *value <= 0.0)
    {
        return Err(
            "comparison requires matching positive finite samples from at least two pairs".into(),
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
    let seed = 0x6f6d_6567_6162_656e_u64;
    let mut random = seed;
    let mut bootstrap = Vec::with_capacity(4000);
    let mut resampled = vec![0.0; differences.len()];
    for _ in 0..4000 {
        for value in &mut resampled {
            random ^= random << 13;
            random ^= random >> 7;
            random ^= random << 17;
            *value = differences[(random % differences.len() as u64) as usize];
        }
        bootstrap.push(median(&resampled));
    }
    bootstrap.sort_by(f64::total_cmp);
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
        paired_percent_change: distribution(&percentages),
        median_difference_bootstrap_95: interval,
        bootstrap_resamples: bootstrap.len(),
        bootstrap_seed: seed,
        interpretation,
    })
}

#[cfg(test)]
mod tests {
    use super::compare;

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
        assert!(compare(&[1.0, 2.0], &[0.0, 2.0]).is_err());
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
}
