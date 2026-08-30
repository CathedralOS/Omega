#![forbid(unsafe_code)]

//! Compiler-owned aggregate accounting for one build-evaluation session.
//!
//! This account measures deterministic evaluator fuel, retained BuildLog
//! bytes, canonical filesystem operation attempts, concurrently reserved
//! filesystem handles, interpreter value cells, and successful result custody.
//! It does not claim to bound host CPU time, process memory, the process-wide
//! descriptor table, temporary byte payloads, or another process resource.

use std::sync::Arc;
use std::sync::Mutex;

pub const BUILD_EVALUATION_SPONSOR_LIMITS_SCHEMA_VERSION: u32 = 6;

/// Immutable limits for one shared build-evaluation resource account.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildEvaluationSponsorLimits {
    maximum_fuel_units: u64,
    maximum_build_log_bytes: u64,
    maximum_filesystem_operation_attempts: u64,
    maximum_live_filesystem_handles: u64,
    maximum_live_cells: u64,
    maximum_result_cells: u64,
    maximum_result_text_bytes: u64,
}

impl BuildEvaluationSponsorLimits {
    /// Construct the version-6 limit schema.
    pub const fn new(
        maximum_fuel_units: u64,
        maximum_build_log_bytes: u64,
        maximum_filesystem_operation_attempts: u64,
        maximum_live_filesystem_handles: u64,
        maximum_live_cells: u64,
        maximum_result_cells: u64,
        maximum_result_text_bytes: u64,
    ) -> Option<Self> {
        if maximum_fuel_units == 0
            || maximum_build_log_bytes == 0
            || maximum_filesystem_operation_attempts == 0
            || maximum_live_filesystem_handles == 0
            || maximum_live_cells == 0
            || maximum_result_cells == 0
            || maximum_result_text_bytes == 0
        {
            return None;
        }
        Some(Self {
            maximum_fuel_units,
            maximum_build_log_bytes,
            maximum_filesystem_operation_attempts,
            maximum_live_filesystem_handles,
            maximum_live_cells,
            maximum_result_cells,
            maximum_result_text_bytes,
        })
    }

    pub const fn schema_version(self) -> u32 {
        BUILD_EVALUATION_SPONSOR_LIMITS_SCHEMA_VERSION
    }

    pub const fn maximum_fuel_units(self) -> u64 {
        self.maximum_fuel_units
    }

    /// Aggregate bytes emitted through the compiler-owned BuildLog facet.
    /// This does not claim to bound temporary evaluator data.
    pub const fn maximum_build_log_bytes(self) -> u64 {
        self.maximum_build_log_bytes
    }

    /// Aggregate canonical filesystem calls whose attempt rows may be
    /// retained. This is a vector/cardinality bound, not memory containment.
    pub const fn maximum_filesystem_operation_attempts(self) -> u64 {
        self.maximum_filesystem_operation_attempts
    }

    /// Maximum compiler-reserved filesystem resources live concurrently.
    /// This is not a claim about unrelated descriptors in the host process.
    pub const fn maximum_live_filesystem_handles(self) -> u64 {
        self.maximum_live_filesystem_handles
    }

    /// Maximum interpreter storage-cell allocations live concurrently. This
    /// counts semantic cells, not allocator bytes or resident memory.
    pub const fn maximum_live_cells(self) -> u64 {
        self.maximum_live_cells
    }

    pub const fn maximum_result_cells(self) -> u64 {
        self.maximum_result_cells
    }

    pub const fn maximum_result_text_bytes(self) -> u64 {
        self.maximum_result_text_bytes
    }
}

#[derive(Debug)]
struct BuildEvaluationSponsorAccount {
    limits: BuildEvaluationSponsorLimits,
    consumed: Mutex<BuildEvaluationSponsorConsumption>,
}

#[derive(Debug, Default)]
struct BuildEvaluationSponsorConsumption {
    fuel_units: u64,
    build_log_bytes: u64,
    filesystem_operation_attempts: u64,
    live_filesystem_handles: u64,
    peak_live_filesystem_handles: u64,
    live_cells: u64,
    peak_live_cells: u64,
    result_cells: u64,
    result_text_bytes: u64,
}

/// Cloneable compiler-side authority for one aggregate resource account.
///
/// Every clone refers to the same atomically synchronized account. Evaluated
/// Omega code cannot inspect this Rust object or its remaining allowance.
#[derive(Debug, Clone)]
pub struct BuildEvaluationSponsor {
    account: Arc<BuildEvaluationSponsorAccount>,
}

impl BuildEvaluationSponsor {
    pub fn new(limits: BuildEvaluationSponsorLimits) -> Self {
        Self {
            account: Arc::new(BuildEvaluationSponsorAccount {
                limits,
                consumed: Mutex::new(BuildEvaluationSponsorConsumption::default()),
            }),
        }
    }

    pub fn limits(&self) -> BuildEvaluationSponsorLimits {
        self.account.limits
    }

    pub fn consumed_fuel_units(&self) -> u64 {
        match self.account.consumed.lock() {
            Ok(consumed) => consumed.fuel_units,
            Err(poisoned) => poisoned.into_inner().fuel_units,
        }
    }

    pub fn consumed_build_log_bytes(&self) -> u64 {
        match self.account.consumed.lock() {
            Ok(consumed) => consumed.build_log_bytes,
            Err(poisoned) => poisoned.into_inner().build_log_bytes,
        }
    }

    pub fn consumed_filesystem_operation_attempts(&self) -> u64 {
        match self.account.consumed.lock() {
            Ok(consumed) => consumed.filesystem_operation_attempts,
            Err(poisoned) => poisoned.into_inner().filesystem_operation_attempts,
        }
    }

    pub fn live_filesystem_handles(&self) -> u64 {
        match self.account.consumed.lock() {
            Ok(consumed) => consumed.live_filesystem_handles,
            Err(poisoned) => poisoned.into_inner().live_filesystem_handles,
        }
    }

    pub fn peak_live_filesystem_handles(&self) -> u64 {
        match self.account.consumed.lock() {
            Ok(consumed) => consumed.peak_live_filesystem_handles,
            Err(poisoned) => poisoned.into_inner().peak_live_filesystem_handles,
        }
    }

    pub fn live_cells(&self) -> u64 {
        match self.account.consumed.lock() {
            Ok(consumed) => consumed.live_cells,
            Err(poisoned) => poisoned.into_inner().live_cells,
        }
    }

    pub fn peak_live_cells(&self) -> u64 {
        match self.account.consumed.lock() {
            Ok(consumed) => consumed.peak_live_cells,
            Err(poisoned) => poisoned.into_inner().peak_live_cells,
        }
    }

    pub fn consumed_result_cells(&self) -> u64 {
        match self.account.consumed.lock() {
            Ok(consumed) => consumed.result_cells,
            Err(poisoned) => poisoned.into_inner().result_cells,
        }
    }

    pub fn consumed_result_text_bytes(&self) -> u64 {
        match self.account.consumed.lock() {
            Ok(consumed) => consumed.result_text_bytes,
            Err(poisoned) => poisoned.into_inner().result_text_bytes,
        }
    }

    pub(crate) fn charge_fuel_unit(&self) -> Result<(), String> {
        let maximum = self.account.limits.maximum_fuel_units();
        let mut consumed = self.account.consumed.lock().map_err(|_| {
            "build-evaluation aggregate fuel sponsor account is unavailable".to_owned()
        })?;
        if consumed.fuel_units >= maximum {
            return Err(format!(
                "build-evaluation aggregate fuel sponsor exhausted at {maximum} fuel units"
            ));
        }
        consumed.fuel_units += 1;
        Ok(())
    }

    pub(crate) fn charge_build_log_bytes(&self, bytes: u64) -> Result<(), String> {
        let maximum = self.account.limits.maximum_build_log_bytes();
        let mut consumed = self.account.consumed.lock().map_err(|_| {
            "build-evaluation aggregate BuildLog sponsor account is unavailable".to_owned()
        })?;
        let Some(candidate) = consumed.build_log_bytes.checked_add(bytes) else {
            return Err("build-evaluation aggregate BuildLog accounting overflowed".to_owned());
        };
        if candidate > maximum {
            return Err(format!(
                "build-evaluation aggregate BuildLog sponsor exhausted at {maximum} bytes"
            ));
        }
        consumed.build_log_bytes = candidate;
        Ok(())
    }

    pub(crate) fn charge_filesystem_operation_attempt(&self) -> Result<(), String> {
        let maximum = self.account.limits.maximum_filesystem_operation_attempts();
        let mut consumed = self.account.consumed.lock().map_err(|_| {
            "build-evaluation aggregate filesystem-attempt sponsor account is unavailable"
                .to_owned()
        })?;
        if consumed.filesystem_operation_attempts >= maximum {
            return Err(format!(
                "build-evaluation aggregate filesystem-attempt sponsor exhausted at {maximum} attempts"
            ));
        }
        consumed.filesystem_operation_attempts += 1;
        Ok(())
    }

    pub(crate) fn reserve_live_filesystem_handle(
        &self,
    ) -> Result<BuildEvaluationLiveFilesystemHandleLease, String> {
        let maximum = self.account.limits.maximum_live_filesystem_handles();
        let mut consumed = self.account.consumed.lock().map_err(|_| {
            "build-evaluation live-filesystem-handle sponsor account is unavailable".to_owned()
        })?;
        let Some(candidate) = consumed.live_filesystem_handles.checked_add(1) else {
            return Err("build-evaluation live-filesystem-handle accounting overflowed".to_owned());
        };
        if candidate > maximum {
            return Err(format!(
                "build-evaluation live-filesystem-handle sponsor exhausted at {maximum} handles"
            ));
        }
        consumed.live_filesystem_handles = candidate;
        consumed.peak_live_filesystem_handles =
            consumed.peak_live_filesystem_handles.max(candidate);
        drop(consumed);
        Ok(BuildEvaluationLiveFilesystemHandleLease {
            sponsor: self.clone(),
        })
    }

    pub(crate) fn charge_result_custody(&self, cells: u64, text_bytes: u64) -> Result<(), String> {
        let limits = self.account.limits;
        let mut consumed = self.account.consumed.lock().map_err(|_| {
            "build-evaluation result-custody sponsor account is unavailable".to_owned()
        })?;
        let candidate_cells = consumed
            .result_cells
            .checked_add(cells)
            .ok_or_else(|| "build-evaluation result-cell accounting overflowed".to_owned())?;
        let candidate_text_bytes = consumed
            .result_text_bytes
            .checked_add(text_bytes)
            .ok_or_else(|| "build-evaluation result-text accounting overflowed".to_owned())?;
        if candidate_cells > limits.maximum_result_cells() {
            return Err(format!(
                "build-evaluation result-cell sponsor exhausted at {} cells",
                limits.maximum_result_cells()
            ));
        }
        if candidate_text_bytes > limits.maximum_result_text_bytes() {
            return Err(format!(
                "build-evaluation result-text sponsor exhausted at {} bytes",
                limits.maximum_result_text_bytes()
            ));
        }
        consumed.result_cells = candidate_cells;
        consumed.result_text_bytes = candidate_text_bytes;
        Ok(())
    }

    pub(crate) fn reserve_live_cell(&self) -> Result<BuildEvaluationLiveCellLease, String> {
        let maximum = self.account.limits.maximum_live_cells();
        let mut consumed =
            self.account.consumed.lock().map_err(|_| {
                "build-evaluation live-cell sponsor account is unavailable".to_owned()
            })?;
        let Some(candidate) = consumed.live_cells.checked_add(1) else {
            return Err("build-evaluation live-cell accounting overflowed".to_owned());
        };
        if candidate > maximum {
            return Err(format!(
                "build-evaluation live-cell sponsor exhausted at {maximum} cells"
            ));
        }
        consumed.live_cells = candidate;
        consumed.peak_live_cells = consumed.peak_live_cells.max(candidate);
        drop(consumed);
        Ok(BuildEvaluationLiveCellLease {
            sponsor: self.clone(),
        })
    }

    fn release_live_filesystem_handle(&self) {
        let mut consumed = match self.account.consumed.lock() {
            Ok(consumed) => consumed,
            Err(poisoned) => poisoned.into_inner(),
        };
        debug_assert!(consumed.live_filesystem_handles > 0);
        consumed.live_filesystem_handles = consumed.live_filesystem_handles.saturating_sub(1);
    }

    fn release_live_cell(&self) {
        let mut consumed = match self.account.consumed.lock() {
            Ok(consumed) => consumed,
            Err(poisoned) => poisoned.into_inner(),
        };
        debug_assert!(consumed.live_cells > 0);
        consumed.live_cells = consumed.live_cells.saturating_sub(1);
    }
}

/// One interpreter-cell reservation. Cloning a semantic cell shares this
/// lease; it retires only when the final alias is dropped.
#[derive(Debug)]
pub(crate) struct BuildEvaluationLiveCellLease {
    sponsor: BuildEvaluationSponsor,
}

impl Drop for BuildEvaluationLiveCellLease {
    fn drop(&mut self) {
        self.sponsor.release_live_cell();
    }
}

/// One compiler-owned reservation. Provider failure, explicit close, and
/// evaluator teardown all release it through ordinary Rust ownership.
#[derive(Debug)]
pub(crate) struct BuildEvaluationLiveFilesystemHandleLease {
    sponsor: BuildEvaluationSponsor,
}

impl Drop for BuildEvaluationLiveFilesystemHandleLease {
    fn drop(&mut self) {
        self.sponsor.release_live_filesystem_handle();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_reject_zero_and_expose_version_six_schema() {
        assert_eq!(
            BuildEvaluationSponsorLimits::new(0, 9, 11, 13, 14, 15, 17),
            None
        );
        assert_eq!(
            BuildEvaluationSponsorLimits::new(7, 0, 11, 13, 14, 15, 17),
            None
        );
        assert_eq!(
            BuildEvaluationSponsorLimits::new(7, 9, 0, 13, 14, 15, 17),
            None
        );
        assert_eq!(
            BuildEvaluationSponsorLimits::new(7, 9, 11, 0, 14, 15, 17),
            None
        );
        assert_eq!(
            BuildEvaluationSponsorLimits::new(7, 9, 11, 13, 0, 15, 17),
            None
        );
        assert_eq!(
            BuildEvaluationSponsorLimits::new(7, 9, 11, 13, 14, 0, 17),
            None
        );
        assert_eq!(
            BuildEvaluationSponsorLimits::new(7, 9, 11, 13, 14, 15, 0),
            None
        );
        let limits =
            BuildEvaluationSponsorLimits::new(7, 9, 11, 13, 14, 15, 17).expect("nonzero limits");
        assert_eq!(limits.schema_version(), 6);
        assert_eq!(limits.maximum_fuel_units(), 7);
        assert_eq!(limits.maximum_build_log_bytes(), 9);
        assert_eq!(limits.maximum_filesystem_operation_attempts(), 11);
        assert_eq!(limits.maximum_live_filesystem_handles(), 13);
        assert_eq!(limits.maximum_live_cells(), 14);
        assert_eq!(limits.maximum_result_cells(), 15);
        assert_eq!(limits.maximum_result_text_bytes(), 17);
    }

    #[test]
    fn clones_share_exact_aggregate_exhaustion() {
        let sponsor = BuildEvaluationSponsor::new(
            BuildEvaluationSponsorLimits::new(2, 3, 4, 5, 8, 6, 7).expect("nonzero limits"),
        );
        let clone = sponsor.clone();

        sponsor.charge_fuel_unit().expect("first unit");
        clone.charge_fuel_unit().expect("second unit");
        assert_eq!(sponsor.consumed_fuel_units(), 2);
        assert_eq!(
            clone.charge_fuel_unit().expect_err("account is exhausted"),
            "build-evaluation aggregate fuel sponsor exhausted at 2 fuel units"
        );
        assert_eq!(clone.consumed_fuel_units(), 2);
    }

    #[test]
    fn clones_share_exact_aggregate_build_log_exhaustion() {
        let sponsor = BuildEvaluationSponsor::new(
            BuildEvaluationSponsorLimits::new(2, 3, 4, 5, 8, 6, 7).expect("nonzero limits"),
        );
        let clone = sponsor.clone();

        sponsor.charge_build_log_bytes(1).expect("first byte");
        clone.charge_build_log_bytes(2).expect("remaining bytes");
        assert_eq!(sponsor.consumed_build_log_bytes(), 3);
        assert_eq!(
            clone
                .charge_build_log_bytes(1)
                .expect_err("BuildLog account is exhausted"),
            "build-evaluation aggregate BuildLog sponsor exhausted at 3 bytes"
        );
        assert_eq!(clone.consumed_build_log_bytes(), 3);
    }

    #[test]
    fn clones_share_exact_aggregate_filesystem_attempt_exhaustion() {
        let sponsor = BuildEvaluationSponsor::new(
            BuildEvaluationSponsorLimits::new(2, 3, 2, 5, 8, 6, 7).expect("nonzero limits"),
        );
        let clone = sponsor.clone();

        sponsor
            .charge_filesystem_operation_attempt()
            .expect("first attempt");
        clone
            .charge_filesystem_operation_attempt()
            .expect("second attempt");
        assert_eq!(sponsor.consumed_filesystem_operation_attempts(), 2);
        assert_eq!(
            clone
                .charge_filesystem_operation_attempt()
                .expect_err("attempt account is exhausted"),
            "build-evaluation aggregate filesystem-attempt sponsor exhausted at 2 attempts"
        );
        assert_eq!(clone.consumed_filesystem_operation_attempts(), 2);
    }

    #[test]
    fn live_filesystem_handle_leases_bound_concurrency_and_release_on_drop() {
        let sponsor = BuildEvaluationSponsor::new(
            BuildEvaluationSponsorLimits::new(2, 3, 4, 2, 8, 6, 7).expect("nonzero limits"),
        );
        let first = sponsor
            .reserve_live_filesystem_handle()
            .expect("first reservation");
        let second = sponsor
            .reserve_live_filesystem_handle()
            .expect("second reservation");
        assert_eq!(sponsor.live_filesystem_handles(), 2);
        assert_eq!(sponsor.peak_live_filesystem_handles(), 2);
        assert_eq!(
            sponsor
                .reserve_live_filesystem_handle()
                .expect_err("concurrent account is exhausted"),
            "build-evaluation live-filesystem-handle sponsor exhausted at 2 handles"
        );
        drop(first);
        assert_eq!(sponsor.live_filesystem_handles(), 1);
        let replacement = sponsor
            .reserve_live_filesystem_handle()
            .expect("released capacity is reusable");
        assert_eq!(sponsor.peak_live_filesystem_handles(), 2);
        drop((second, replacement));
        assert_eq!(sponsor.live_filesystem_handles(), 0);
    }

    #[test]
    fn live_cell_leases_bound_concurrency_and_release_on_final_drop() {
        let sponsor = BuildEvaluationSponsor::new(
            BuildEvaluationSponsorLimits::new(2, 3, 4, 5, 2, 6, 7).expect("nonzero limits"),
        );
        let first = sponsor.reserve_live_cell().expect("first reservation");
        let second = sponsor.reserve_live_cell().expect("second reservation");
        assert_eq!(sponsor.live_cells(), 2);
        assert_eq!(sponsor.peak_live_cells(), 2);
        assert_eq!(
            sponsor
                .reserve_live_cell()
                .expect_err("concurrent account is exhausted"),
            "build-evaluation live-cell sponsor exhausted at 2 cells"
        );
        drop(first);
        let replacement = sponsor
            .reserve_live_cell()
            .expect("released capacity is reusable");
        assert_eq!(sponsor.peak_live_cells(), 2);
        drop((second, replacement));
        assert_eq!(sponsor.live_cells(), 0);
    }

    #[test]
    fn result_custody_charges_cells_and_text_atomically() {
        let sponsor = BuildEvaluationSponsor::new(
            BuildEvaluationSponsorLimits::new(2, 3, 4, 5, 8, 2, 3).expect("nonzero limits"),
        );
        sponsor.charge_result_custody(1, 2).expect("first result");
        assert!(sponsor.charge_result_custody(2, 0).is_err());
        assert!(sponsor.charge_result_custody(1, 2).is_err());
        assert_eq!(sponsor.consumed_result_cells(), 1);
        assert_eq!(sponsor.consumed_result_text_bytes(), 2);
        sponsor
            .charge_result_custody(1, 1)
            .expect("both exact remainders");
        assert_eq!(sponsor.consumed_result_cells(), 2);
        assert_eq!(sponsor.consumed_result_text_bytes(), 3);
    }

    #[test]
    fn observation_recovers_a_poisoned_account_while_charging_fails_closed() {
        let sponsor = BuildEvaluationSponsor::new(
            BuildEvaluationSponsorLimits::new(2, 3, 4, 5, 8, 6, 7).expect("nonzero limits"),
        );
        let poisoning_clone = sponsor.clone();
        let _ = std::thread::spawn(move || {
            let _guard = poisoning_clone
                .account
                .consumed
                .lock()
                .expect("fresh account");
            panic!("poison account");
        })
        .join();

        assert_eq!(sponsor.consumed_fuel_units(), 0);
        assert_eq!(
            sponsor
                .charge_fuel_unit()
                .expect_err("a poisoned account must fail closed"),
            "build-evaluation aggregate fuel sponsor account is unavailable"
        );
    }
}
