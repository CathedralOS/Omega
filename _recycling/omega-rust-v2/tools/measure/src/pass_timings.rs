//! How long each pass of a measurement took, and the three numbers we report from them.

/// The seconds each pass took, in the order they were recorded.
pub struct PassTimings {
    pass_seconds: Vec<f64>,
}

impl PassTimings {
    pub fn new(expected_pass_count: usize) -> Self {
        Self {
            pass_seconds: Vec::with_capacity(expected_pass_count),
        }
    }

    pub fn record_seconds(
        &mut self,
        pass_seconds: f64,
    ) {
        self.pass_seconds.push(pass_seconds);
    }

    pub fn get_pass_count(&self) -> usize {
        self.pass_seconds.len()
    }

    /// The pass least disturbed by anything else on the machine. Zero when nothing was recorded.
    pub fn get_fastest_seconds(&self) -> f64 {
        self.get_sorted_seconds().first().copied().unwrap_or(0.0)
    }

    /// The middle pass, or the later of the two middle ones when the count is even.
    pub fn get_median_seconds(&self) -> f64 {
        let sorted_seconds = self.get_sorted_seconds();

        sorted_seconds
            .get(sorted_seconds.len() / 2)
            .copied()
            .unwrap_or(0.0)
    }

    pub fn get_slowest_seconds(&self) -> f64 {
        self.get_sorted_seconds().last().copied().unwrap_or(0.0)
    }

    // A handful of passes, asked about three times at the end of a run, so sorting a copy each
    // time costs nothing that matters and keeps the recorded order intact.
    fn get_sorted_seconds(&self) -> Vec<f64> {
        let mut sorted_seconds = self.pass_seconds.clone();

        sorted_seconds.sort_by(|left_seconds, right_seconds| left_seconds.total_cmp(right_seconds));

        sorted_seconds
    }
}

/// Megabytes per second, with a megabyte as one million bytes. Zero when no time was measured,
/// so a tool can print the result without checking first.
pub fn megabytes_per_second(
    byte_count: usize,
    elapsed_seconds: f64,
) -> f64 {
    if elapsed_seconds > 0.0 {
        byte_count as f64 / 1_000_000.0 / elapsed_seconds
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use crate::pass_timings::{PassTimings, megabytes_per_second};

    #[test]
    fn reports_fastest_median_and_slowest_whatever_the_order() {
        let mut pass_timings = PassTimings::new(5);

        for pass_seconds in [0.5, 0.2, 0.9, 0.3, 0.4] {
            pass_timings.record_seconds(pass_seconds);
        }

        assert_eq!(pass_timings.get_pass_count(), 5);
        assert_eq!(pass_timings.get_fastest_seconds(), 0.2);
        assert_eq!(pass_timings.get_median_seconds(), 0.4);
        assert_eq!(pass_timings.get_slowest_seconds(), 0.9);
    }

    #[test]
    fn an_empty_set_of_timings_reports_zero() {
        let pass_timings = PassTimings::new(0);

        assert_eq!(pass_timings.get_fastest_seconds(), 0.0);
        assert_eq!(pass_timings.get_median_seconds(), 0.0);
        assert_eq!(pass_timings.get_slowest_seconds(), 0.0);
    }

    #[test]
    fn megabytes_per_second_uses_a_million_bytes_and_survives_zero_time() {
        assert_eq!(megabytes_per_second(2_000_000, 0.5), 4.0);
        assert_eq!(megabytes_per_second(2_000_000, 0.0), 0.0);
    }
}
