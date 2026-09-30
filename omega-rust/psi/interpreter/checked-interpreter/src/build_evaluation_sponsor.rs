#![forbid(unsafe_code)]

//! Compiler-owned aggregate accounting for one build-evaluation session.
//!
//! This account measures deterministic evaluator fuel, retained BuildLog
//! bytes, canonical filesystem operation attempts, concurrently reserved
//! filesystem handles, interpreter value cells, live interpreter Text payload
//! bytes, and successful result custody.
//! It does not claim to bound host CPU time, process memory, the process-wide
//! descriptor table, non-Text temporary byte payloads, or another process
//! resource.

use std::sync::Arc;
use std::sync::{Mutex, Weak};

use crate::{
    BuildEvaluationUsage, CURRENT_EVALUATION_STEP_SCHEDULE, CURRENT_EVALUATION_USAGE_SCHEMA,
};

pub const BUILD_EVALUATION_SPONSOR_LIMITS_SCHEMA_VERSION: u32 = 7;

/// Immutable limits for one shared build-evaluation resource account.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildEvaluationSponsorLimits {
    maximum_fuel_units: u64,
    maximum_build_log_bytes: u64,
    maximum_filesystem_operation_attempts: u64,
    maximum_live_filesystem_handles: u64,
    maximum_live_cells: u64,
    maximum_live_text_bytes: u64,
    maximum_result_cells: u64,
    maximum_result_text_bytes: u64,
}

impl BuildEvaluationSponsorLimits {
    /// Construct the version-7 limit schema.
    pub const fn new(
        maximum_fuel_units: u64,
        maximum_build_log_bytes: u64,
        maximum_filesystem_operation_attempts: u64,
        maximum_live_filesystem_handles: u64,
        maximum_live_cells: u64,
        maximum_live_text_bytes: u64,
        maximum_result_cells: u64,
        maximum_result_text_bytes: u64,
    ) -> Option<Self> {
        if maximum_fuel_units == 0
            || maximum_build_log_bytes == 0
            || maximum_filesystem_operation_attempts == 0
            || maximum_live_filesystem_handles == 0
            || maximum_live_cells == 0
            || maximum_live_text_bytes == 0
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
            maximum_live_text_bytes,
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

    /// Maximum logical bytes held by live interpreter Text backing buffers.
    /// This excludes Vec capacity, temporary copies, allocator overhead, and
    /// resident memory.
    pub const fn maximum_live_text_bytes(self) -> u64 {
        self.maximum_live_text_bytes
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
    live_text_bytes: u64,
    peak_live_text_bytes: u64,
    result_cells: u64,
    result_text_bytes: u64,
    retained_consumptions: u64,
}

/// Cloneable compiler-side authority for one aggregate resource account.
///
/// Every clone refers to the same atomically synchronized account. Evaluated
/// Omega code cannot inspect this Rust object or its remaining allowance.
#[derive(Debug, Clone)]
pub struct BuildEvaluationSponsor {
    account: Arc<BuildEvaluationSponsorAccount>,
}

/// One current-account policy charge for consuming an already-completed Pure Build.
/// The producing usage remains unchanged. This receipt is not an execution,
/// live allocation, observed peak, or reusable permission to consume again.
#[derive(Debug, Clone)]
pub struct BuildEvaluationConsumptionReceipt {
    account: Weak<BuildEvaluationSponsorAccount>,
    occurrence: u64,
    usage: BuildEvaluationUsage,
}

impl BuildEvaluationConsumptionReceipt {
    pub const fn producing_usage(&self) -> BuildEvaluationUsage {
        self.usage
    }
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

    pub fn live_text_bytes(&self) -> u64 {
        match self.account.consumed.lock() {
            Ok(consumed) => consumed.live_text_bytes,
            Err(poisoned) => poisoned.into_inner().live_text_bytes,
        }
    }

    pub fn peak_live_text_bytes(&self) -> u64 {
        match self.account.consumed.lock() {
            Ok(consumed) => consumed.peak_live_text_bytes,
            Err(poisoned) => poisoned.into_inner().peak_live_text_bytes,
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

    /// Reserve one retained Pure occurrence under this account's current policy.
    /// All cumulative accounts change atomically or none do. Retained invocation
    /// peaks are eligibility bounds only: consuming a result creates no live
    /// evaluator cells and cannot manufacture this session's observed peaks.
    pub fn charge_retained_pure_usage(
        &self,
        usage: BuildEvaluationUsage,
    ) -> Result<BuildEvaluationConsumptionReceipt, String> {
        usage
            .validate()
            .map_err(|error| format!("retained Build usage is inconsistent: {error:?}"))?;
        if usage.usage_schema_version != CURRENT_EVALUATION_USAGE_SCHEMA.schema_version()
            || usage.step_schedule_marker != CURRENT_EVALUATION_STEP_SCHEDULE.marker()
            || usage.filesystem_operation_attempts != 0
        {
            return Err("retained Pure Build usage has unsupported telemetry".to_owned());
        }
        let limits = self.account.limits;
        let mut consumed = self.account.consumed.lock().map_err(|_| {
            "build-evaluation retained-consumption account is unavailable".to_owned()
        })?;
        if consumed.live_cells != 0
            || consumed.live_text_bytes != 0
            || consumed.live_filesystem_handles != 0
        {
            return Err("retained Pure Build requires a quiescent evaluation account".to_owned());
        }
        if usage.peak_live_cells > limits.maximum_live_cells()
            || usage.peak_live_text_bytes > limits.maximum_live_text_bytes()
        {
            return Err("retained Pure Build exceeds current live-resource provisions".to_owned());
        }
        let reserve = |current: u64, additional: u64, maximum: u64, name: &str| {
            current
                .checked_add(additional)
                .filter(|total| *total <= maximum)
                .ok_or_else(|| format!("retained Pure Build exceeds current {name} provision"))
        };
        let fuel_units = reserve(
            consumed.fuel_units,
            usage.fuel_units,
            limits.maximum_fuel_units(),
            "fuel",
        )?;
        let build_log_bytes = reserve(
            consumed.build_log_bytes,
            usage.build_log_bytes,
            limits.maximum_build_log_bytes(),
            "BuildLog",
        )?;
        let result_cells = reserve(
            consumed.result_cells,
            usage.result_cells,
            limits.maximum_result_cells(),
            "result-cell",
        )?;
        let result_text_bytes = reserve(
            consumed.result_text_bytes,
            usage.result_text_bytes,
            limits.maximum_result_text_bytes(),
            "result-Text",
        )?;
        let occurrence = consumed.retained_consumptions;
        let next = occurrence
            .checked_add(1)
            .ok_or_else(|| "retained Build occurrence accounting overflowed".to_owned())?;
        consumed.fuel_units = fuel_units;
        consumed.build_log_bytes = build_log_bytes;
        consumed.result_cells = result_cells;
        consumed.result_text_bytes = result_text_bytes;
        consumed.retained_consumptions = next;
        Ok(BuildEvaluationConsumptionReceipt {
            account: Arc::downgrade(&self.account),
            occurrence,
            usage,
        })
    }

    /// Require each issued charge exactly once, in issuance order, for this
    /// exact account. Equal limits never make a foreign receipt acceptable.
    pub fn verify_retained_consumptions(
        &self,
        receipts: &[BuildEvaluationConsumptionReceipt],
    ) -> Result<(), String> {
        let consumed = self.account.consumed.lock().map_err(|_| {
            "build-evaluation retained-consumption account is unavailable".to_owned()
        })?;
        let account = Arc::downgrade(&self.account);
        if receipts.len() as u64 != consumed.retained_consumptions
            || receipts.iter().enumerate().any(|(position, receipt)| {
                !Weak::ptr_eq(&account, &receipt.account) || receipt.occurrence != position as u64
            })
        {
            return Err(
                "retained Build charges are missing, duplicated, or belong to another account"
                    .to_owned(),
            );
        }
        Ok(())
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

    pub(crate) fn reserve_live_text_bytes(
        &self,
        bytes: u64,
    ) -> Result<BuildEvaluationLiveTextByteLease, String> {
        self.increase_live_text_bytes(bytes)?;
        Ok(BuildEvaluationLiveTextByteLease {
            sponsor: self.clone(),
            bytes,
        })
    }

    fn increase_live_text_bytes(&self, bytes: u64) -> Result<(), String> {
        let maximum = self.account.limits.maximum_live_text_bytes();
        let mut consumed = self.account.consumed.lock().map_err(|_| {
            "build-evaluation live-Text-byte sponsor account is unavailable".to_owned()
        })?;
        let candidate = consumed
            .live_text_bytes
            .checked_add(bytes)
            .ok_or_else(|| "build-evaluation live-Text-byte accounting overflowed".to_owned())?;
        if candidate > maximum {
            return Err(format!(
                "build-evaluation live-Text-byte sponsor exhausted at {maximum} bytes"
            ));
        }
        consumed.live_text_bytes = candidate;
        consumed.peak_live_text_bytes = consumed.peak_live_text_bytes.max(candidate);
        Ok(())
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

    fn release_live_text_bytes(&self, bytes: u64) {
        let mut consumed = match self.account.consumed.lock() {
            Ok(consumed) => consumed,
            Err(poisoned) => poisoned.into_inner(),
        };
        debug_assert!(consumed.live_text_bytes >= bytes);
        consumed.live_text_bytes = consumed.live_text_bytes.saturating_sub(bytes);
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

#[derive(Debug)]
pub(crate) struct BuildEvaluationLiveTextByteLease {
    sponsor: BuildEvaluationSponsor,
    bytes: u64,
}

impl BuildEvaluationLiveTextByteLease {
    pub(crate) fn grow(&mut self, bytes: u64) -> Result<(), String> {
        self.sponsor.increase_live_text_bytes(bytes)?;
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .expect("sponsor increase rejected byte-count overflow");
        Ok(())
    }

    pub(crate) fn shrink(&mut self, bytes: u64) {
        debug_assert!(self.bytes >= bytes);
        self.bytes = self.bytes.saturating_sub(bytes);
        self.sponsor.release_live_text_bytes(bytes);
    }
}

impl Drop for BuildEvaluationLiveTextByteLease {
    fn drop(&mut self) {
        self.sponsor.release_live_text_bytes(self.bytes);
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
    use super::{BuildEvaluationSponsor, BuildEvaluationSponsorLimits};

    #[test]
    fn limits_reject_zero_and_expose_version_seven_schema() {
        assert_eq!(
            BuildEvaluationSponsorLimits::new(0, 9, 11, 13, 14, 16, 15, 17),
            None
        );
        assert_eq!(
            BuildEvaluationSponsorLimits::new(7, 0, 11, 13, 14, 16, 15, 17),
            None
        );
        assert_eq!(
            BuildEvaluationSponsorLimits::new(7, 9, 0, 13, 14, 16, 15, 17),
            None
        );
        assert_eq!(
            BuildEvaluationSponsorLimits::new(7, 9, 11, 0, 14, 16, 15, 17),
            None
        );
        assert_eq!(
            BuildEvaluationSponsorLimits::new(7, 9, 11, 13, 0, 16, 15, 17),
            None
        );
        assert_eq!(
            BuildEvaluationSponsorLimits::new(7, 9, 11, 13, 14, 0, 15, 17),
            None
        );
        assert_eq!(
            BuildEvaluationSponsorLimits::new(7, 9, 11, 13, 14, 16, 0, 17),
            None
        );
        assert_eq!(
            BuildEvaluationSponsorLimits::new(7, 9, 11, 13, 14, 16, 15, 0),
            None
        );
        let limits = BuildEvaluationSponsorLimits::new(7, 9, 11, 13, 14, 16, 15, 17)
            .expect("nonzero limits");
        assert_eq!(limits.schema_version(), 7);
        assert_eq!(limits.maximum_fuel_units(), 7);
        assert_eq!(limits.maximum_build_log_bytes(), 9);
        assert_eq!(limits.maximum_filesystem_operation_attempts(), 11);
        assert_eq!(limits.maximum_live_filesystem_handles(), 13);
        assert_eq!(limits.maximum_live_cells(), 14);
        assert_eq!(limits.maximum_live_text_bytes(), 16);
        assert_eq!(limits.maximum_result_cells(), 15);
        assert_eq!(limits.maximum_result_text_bytes(), 17);
    }

    #[test]
    fn clones_share_exact_aggregate_exhaustion() {
        let sponsor = BuildEvaluationSponsor::new(
            BuildEvaluationSponsorLimits::new(2, 3, 4, 5, 8, 9, 6, 7).expect("nonzero limits"),
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
            BuildEvaluationSponsorLimits::new(2, 3, 4, 5, 8, 9, 6, 7).expect("nonzero limits"),
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
            BuildEvaluationSponsorLimits::new(2, 3, 2, 5, 8, 9, 6, 7).expect("nonzero limits"),
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
    fn live_cell_leases_bound_concurrency_and_release_on_final_drop() {
        let sponsor = BuildEvaluationSponsor::new(
            BuildEvaluationSponsorLimits::new(2, 3, 4, 5, 2, 9, 6, 7).expect("nonzero limits"),
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
    fn live_text_byte_leases_bound_logical_bytes_and_release_on_drop() {
        let sponsor = BuildEvaluationSponsor::new(
            BuildEvaluationSponsorLimits::new(2, 3, 4, 5, 8, 5, 6, 7).expect("nonzero limits"),
        );
        let first = sponsor
            .reserve_live_text_bytes(3)
            .expect("first reservation");
        let second = sponsor
            .reserve_live_text_bytes(2)
            .expect("second reservation");
        assert_eq!(sponsor.live_text_bytes(), 5);
        assert_eq!(sponsor.peak_live_text_bytes(), 5);
        assert!(sponsor.reserve_live_text_bytes(1).is_err());
        drop(first);
        assert_eq!(sponsor.live_text_bytes(), 2);
        let replacement = sponsor
            .reserve_live_text_bytes(3)
            .expect("released bytes are reusable");
        drop((second, replacement));
        assert_eq!(sponsor.live_text_bytes(), 0);
    }

    #[test]
    fn result_custody_charges_cells_and_text_atomically() {
        let sponsor = BuildEvaluationSponsor::new(
            BuildEvaluationSponsorLimits::new(2, 3, 4, 5, 8, 9, 2, 3).expect("nonzero limits"),
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
            BuildEvaluationSponsorLimits::new(2, 3, 4, 5, 8, 9, 6, 7).expect("nonzero limits"),
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
    fn retained_usage() -> crate::BuildEvaluationUsage {
        crate::BuildEvaluationUsage::capture(
            crate::EvaluationUsage {
                fuel_units: 6,
                build_log_bytes: 3,
                result_cells: 4,
                result_text_bytes: 5,
                peak_live_cells: 7,
                peak_live_text_bytes: 8,
                ..crate::EvaluationUsage::empty(100)
            },
            None,
        )
    }

    fn retention_sponsor(fuel: u64) -> BuildEvaluationSponsor {
        BuildEvaluationSponsor::new(
            BuildEvaluationSponsorLimits::new(fuel, 20, 20, 20, 20, 20, 20, 20).unwrap(),
        )
    }

    #[test]
    fn retained_pure_charges_keep_independent_pass_budgets_and_original_usage() {
        let usage = retained_usage();
        let first = retention_sponsor(10);
        let second = retention_sponsor(10);
        let first_receipt = first.charge_retained_pure_usage(usage).unwrap();
        let second_receipt = second.charge_retained_pure_usage(usage).unwrap();
        assert_eq!(first_receipt.producing_usage(), usage);
        assert_eq!(second_receipt.producing_usage(), usage);
        for sponsor in [&first, &second] {
            assert_eq!(sponsor.consumed_fuel_units(), 6);
            assert_eq!(sponsor.consumed_build_log_bytes(), 3);
            assert_eq!(sponsor.consumed_result_cells(), 4);
            assert_eq!(sponsor.consumed_result_text_bytes(), 5);
            assert_eq!(sponsor.live_cells(), 0);
            assert_eq!(sponsor.peak_live_cells(), 0);
            assert_eq!(sponsor.live_text_bytes(), 0);
            assert_eq!(sponsor.peak_live_text_bytes(), 0);
        }
        first
            .verify_retained_consumptions(&[first_receipt])
            .unwrap();
        second
            .verify_retained_consumptions(&[second_receipt])
            .unwrap();
    }

    #[test]
    fn retained_pure_charge_is_atomic_and_reduces_later_execution_allowance() {
        let sponsor = retention_sponsor(10);
        for _ in 0..5 {
            sponsor.charge_fuel_unit().unwrap();
        }
        assert!(
            sponsor
                .charge_retained_pure_usage(retained_usage())
                .is_err()
        );
        assert_eq!(sponsor.consumed_fuel_units(), 5);
        assert_eq!(sponsor.consumed_build_log_bytes(), 0);
        assert_eq!(sponsor.consumed_result_cells(), 0);
        sponsor.verify_retained_consumptions(&[]).unwrap();
        let sponsor = retention_sponsor(10);
        let receipt = sponsor
            .charge_retained_pure_usage(retained_usage())
            .unwrap();
        for _ in 0..4 {
            sponsor.charge_fuel_unit().unwrap();
        }
        assert!(sponsor.charge_fuel_unit().is_err());
        assert!(
            sponsor
                .charge_retained_pure_usage(retained_usage())
                .is_err()
        );
        assert_eq!(sponsor.consumed_fuel_units(), 10);
        sponsor.verify_retained_consumptions(&[receipt]).unwrap();
    }

    #[test]
    fn retained_pure_charge_rejects_every_lower_provision_without_mutation() {
        let limits = [
            [5, 20, 20, 20, 20, 20, 20, 20],
            [20, 2, 20, 20, 20, 20, 20, 20],
            [20, 20, 20, 20, 6, 20, 20, 20],
            [20, 20, 20, 20, 20, 7, 20, 20],
            [20, 20, 20, 20, 20, 20, 3, 20],
            [20, 20, 20, 20, 20, 20, 20, 4],
        ];
        for [
            fuel,
            log,
            attempts,
            handles,
            cells,
            text,
            results,
            result_text,
        ] in limits
        {
            let sponsor = BuildEvaluationSponsor::new(
                BuildEvaluationSponsorLimits::new(
                    fuel,
                    log,
                    attempts,
                    handles,
                    cells,
                    text,
                    results,
                    result_text,
                )
                .unwrap(),
            );
            assert!(
                sponsor
                    .charge_retained_pure_usage(retained_usage())
                    .is_err()
            );
            assert_eq!(sponsor.consumed_fuel_units(), 0);
            assert_eq!(sponsor.consumed_build_log_bytes(), 0);
            assert_eq!(sponsor.consumed_result_cells(), 0);
            assert_eq!(sponsor.consumed_result_text_bytes(), 0);
            sponsor.verify_retained_consumptions(&[]).unwrap();
        }
    }

    #[test]
    fn retained_pure_receipts_reject_foreign_duplicate_and_missing_occurrences() {
        let first = retention_sponsor(20);
        let second = retention_sponsor(20);
        let receipt = first.charge_retained_pure_usage(retained_usage()).unwrap();
        let other = first.charge_retained_pure_usage(retained_usage()).unwrap();
        let foreign = second.charge_retained_pure_usage(retained_usage()).unwrap();
        assert!(first.verify_retained_consumptions(&[]).is_err());
        assert!(
            first
                .verify_retained_consumptions(&[receipt.clone(), receipt.clone()])
                .is_err()
        );
        assert!(
            first
                .verify_retained_consumptions(&[foreign, other.clone()])
                .is_err()
        );
        assert!(
            first
                .verify_retained_consumptions(&[other.clone(), receipt.clone()])
                .is_err()
        );
        first
            .verify_retained_consumptions(&[receipt, other])
            .unwrap();
    }

    #[test]
    fn retained_pure_usage_rejects_stale_filesystem_and_live_resource_states() {
        for mutate in [
            |usage: &mut crate::BuildEvaluationUsage| usage.usage_schema_version += 1,
            |usage: &mut crate::BuildEvaluationUsage| usage.step_schedule_marker += 1,
            |usage: &mut crate::BuildEvaluationUsage| usage.filesystem_operation_attempts = 1,
            |usage: &mut crate::BuildEvaluationUsage| usage.invocation_fuel_ceiling = 0,
        ] {
            let sponsor = retention_sponsor(20);
            let mut usage = retained_usage();
            mutate(&mut usage);
            assert!(sponsor.charge_retained_pure_usage(usage).is_err());
            assert_eq!(sponsor.consumed_fuel_units(), 0);
        }
        let sponsor = retention_sponsor(20);
        let cell = sponsor.reserve_live_cell().unwrap();
        assert!(
            sponsor
                .charge_retained_pure_usage(retained_usage())
                .is_err()
        );
        drop(cell);
        let text = sponsor.reserve_live_text_bytes(1).unwrap();
        assert!(
            sponsor
                .charge_retained_pure_usage(retained_usage())
                .is_err()
        );
        drop(text);
        let handle = sponsor.reserve_live_filesystem_handle().unwrap();
        assert!(
            sponsor
                .charge_retained_pure_usage(retained_usage())
                .is_err()
        );
        drop(handle);
        let receipt = sponsor
            .charge_retained_pure_usage(retained_usage())
            .unwrap();
        assert_eq!(sponsor.peak_live_cells(), 1);
        assert_eq!(sponsor.peak_live_text_bytes(), 1);
        assert_eq!(sponsor.peak_live_filesystem_handles(), 1);
        sponsor.verify_retained_consumptions(&[receipt]).unwrap();
    }
    #[test]
    fn retained_pure_cumulative_overflow_is_atomic() {
        for mutate in [
            |usage: &mut crate::BuildEvaluationUsage| {
                usage.fuel_units = u64::MAX;
                usage.invocation_fuel_ceiling = u64::MAX;
            },
            |usage: &mut crate::BuildEvaluationUsage| usage.build_log_bytes = u64::MAX,
            |usage: &mut crate::BuildEvaluationUsage| usage.result_cells = u64::MAX,
            |usage: &mut crate::BuildEvaluationUsage| usage.result_text_bytes = u64::MAX,
        ] {
            let sponsor = BuildEvaluationSponsor::new(
                BuildEvaluationSponsorLimits::new(
                    u64::MAX,
                    u64::MAX,
                    u64::MAX,
                    u64::MAX,
                    u64::MAX,
                    u64::MAX,
                    u64::MAX,
                    u64::MAX,
                )
                .unwrap(),
            );
            let receipt = sponsor
                .charge_retained_pure_usage(retained_usage())
                .unwrap();
            let mut huge = retained_usage();
            mutate(&mut huge);
            assert!(sponsor.charge_retained_pure_usage(huge).is_err());
            assert_eq!(sponsor.consumed_fuel_units(), 6);
            assert_eq!(sponsor.consumed_build_log_bytes(), 3);
            assert_eq!(sponsor.consumed_result_cells(), 4);
            assert_eq!(sponsor.consumed_result_text_bytes(), 5);
            sponsor.verify_retained_consumptions(&[receipt]).unwrap();
        }
    }
}
