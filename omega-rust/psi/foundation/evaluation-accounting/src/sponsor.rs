#![forbid(unsafe_code)]

//! Compiler-owned aggregate accounting for one build-evaluation session.
//!
//! This account measures deterministic evaluator fuel, retained BuildLog
//! bytes, canonical filesystem operation attempts, concurrently reserved
//! filesystem handles, logical storage cells, owned backing payload bytes,
//! and result custody. Its immutable measurement identity defines the units.
//! It does not claim to bound host CPU time, process memory, the process-wide
//! descriptor table, unmeasured temporary byte payloads, or another process
//! resource.

use crate::{EvaluationAccountingError, EvaluationMeasurementIdentity};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;

pub const BUILD_EVALUATION_SPONSOR_LIMITS_SCHEMA_VERSION: u32 = 7;

/// Immutable limits for one shared build-evaluation resource account.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildEvaluationSponsorLimits {
    maximum_fuel_units: u64,
    maximum_build_log_bytes: u64,
    maximum_filesystem_operation_attempts: u64,
    maximum_live_filesystem_handles: u64,
    maximum_live_storage_cells: u64,
    maximum_live_payload_bytes: u64,
    maximum_result_storage_cells: u64,
    maximum_result_payload_bytes: u64,
}

impl BuildEvaluationSponsorLimits {
    /// Construct the version-7 limit schema.
    pub const fn new(
        maximum_fuel_units: u64,
        maximum_build_log_bytes: u64,
        maximum_filesystem_operation_attempts: u64,
        maximum_live_filesystem_handles: u64,
        maximum_live_storage_cells: u64,
        maximum_live_payload_bytes: u64,
        maximum_result_storage_cells: u64,
        maximum_result_payload_bytes: u64,
    ) -> Option<Self> {
        if maximum_fuel_units == 0
            || maximum_build_log_bytes == 0
            || maximum_filesystem_operation_attempts == 0
            || maximum_live_filesystem_handles == 0
            || maximum_live_storage_cells == 0
            || maximum_live_payload_bytes == 0
            || maximum_result_storage_cells == 0
            || maximum_result_payload_bytes == 0
        {
            return None;
        }
        Some(Self {
            maximum_fuel_units,
            maximum_build_log_bytes,
            maximum_filesystem_operation_attempts,
            maximum_live_filesystem_handles,
            maximum_live_storage_cells,
            maximum_live_payload_bytes,
            maximum_result_storage_cells,
            maximum_result_payload_bytes,
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
    /// counts logical cells defined by the measurement schedule, not allocator bytes.
    pub const fn maximum_live_storage_cells(self) -> u64 {
        self.maximum_live_storage_cells
    }

    /// Maximum bytes held by live owned payload backings under the measurement schedule.
    /// Every retained owned backing counts while it lives, including overlapping
    /// replacement and output-copy buffers. This excludes allocator capacity
    /// and overhead, process memory, and caller scratch outside that schedule.
    pub const fn maximum_live_payload_bytes(self) -> u64 {
        self.maximum_live_payload_bytes
    }

    pub const fn maximum_result_storage_cells(self) -> u64 {
        self.maximum_result_storage_cells
    }

    pub const fn maximum_result_payload_bytes(self) -> u64 {
        self.maximum_result_payload_bytes
    }
}

// Transitional tree evaluator accessors. Counter units always follow the
// containing account's explicit measurement identity.
impl BuildEvaluationSponsorLimits {
    pub const fn maximum_live_cells(self) -> u64 {
        self.maximum_live_storage_cells()
    }
    pub const fn maximum_live_text_bytes(self) -> u64 {
        self.maximum_live_payload_bytes()
    }
    pub const fn maximum_result_cells(self) -> u64 {
        self.maximum_result_storage_cells()
    }
    pub const fn maximum_result_text_bytes(self) -> u64 {
        self.maximum_result_payload_bytes()
    }
}

#[derive(Debug)]
pub(super) struct BuildEvaluationSponsorAccount {
    pub(super) limits: BuildEvaluationSponsorLimits,
    pub(super) measurement: EvaluationMeasurementIdentity,
    pub(super) consumed: Mutex<BuildEvaluationSponsorConsumption>,
}

#[derive(Debug, Default)]
pub(super) struct BuildEvaluationSponsorConsumption {
    pub(super) fuel_units: u64,
    pub(super) build_log_bytes: u64,
    pub(super) filesystem_operation_attempts: u64,
    pub(super) live_filesystem_handles: u64,
    pub(super) peak_live_filesystem_handles: u64,
    pub(super) live_storage_cells: u64,
    pub(super) peak_live_storage_cells: u64,
    pub(super) live_payload_bytes: u64,
    pub(super) peak_live_payload_bytes: u64,
    pub(super) result_storage_cells: u64,
    pub(super) result_payload_bytes: u64,
}

/// Cloneable compiler-side authority for one aggregate resource account.
///
/// Every clone refers to the same atomically synchronized account. Evaluated
/// Omega code cannot inspect this Rust object or its remaining allowance.
#[derive(Debug, Clone)]
pub struct BuildEvaluationSponsor {
    pub(super) account: Arc<BuildEvaluationSponsorAccount>,
}

impl BuildEvaluationSponsor {
    pub fn new(limits: BuildEvaluationSponsorLimits) -> Self {
        Self {
            account: Arc::new(BuildEvaluationSponsorAccount {
                limits,
                measurement: EvaluationMeasurementIdentity::CheckedTree,
                consumed: Mutex::new(BuildEvaluationSponsorConsumption::default()),
            }),
        }
    }

    /// Create an account with explicit engine units. Numeric fuel markers alone
    /// never identify storage units. Legacy `new` always selects CheckedTree.
    pub fn new_with_measurement(
        limits: BuildEvaluationSponsorLimits,
        measurement: EvaluationMeasurementIdentity,
    ) -> Result<Self, EvaluationAccountingError> {
        if !measurement.is_current() {
            return Err(EvaluationAccountingError::UnsupportedMeasurementIdentity);
        }
        Ok(Self {
            account: Arc::new(BuildEvaluationSponsorAccount {
                limits,
                measurement,
                consumed: Mutex::new(BuildEvaluationSponsorConsumption::default()),
            }),
        })
    }

    pub fn measurement(&self) -> EvaluationMeasurementIdentity {
        self.account.measurement
    }

    fn require_checked_tree(&self) -> Result<(), String> {
        if self.measurement() != EvaluationMeasurementIdentity::CheckedTree {
            return Err("legacy tree charge cannot use a different measurement account".into());
        }
        Ok(())
    }

    pub fn limits(&self) -> BuildEvaluationSponsorLimits {
        self.account.limits
    }

    /// Poison-tolerant read of the live consumption counters: a poisoned lock
    /// still carries the recorded usage, so reads unwrap the guard contents.
    fn consumption(&self) -> MutexGuard<'_, BuildEvaluationSponsorConsumption> {
        self.account
            .consumed
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn consumed_fuel_units(&self) -> u64 {
        self.consumption().fuel_units
    }

    pub fn consumed_build_log_bytes(&self) -> u64 {
        self.consumption().build_log_bytes
    }

    pub fn consumed_filesystem_operation_attempts(&self) -> u64 {
        self.consumption().filesystem_operation_attempts
    }

    pub fn live_filesystem_handles(&self) -> u64 {
        self.consumption().live_filesystem_handles
    }

    pub fn peak_live_filesystem_handles(&self) -> u64 {
        self.consumption().peak_live_filesystem_handles
    }

    pub fn live_storage_cells(&self) -> u64 {
        self.consumption().live_storage_cells
    }

    pub fn peak_live_storage_cells(&self) -> u64 {
        self.consumption().peak_live_storage_cells
    }

    pub fn live_payload_bytes(&self) -> u64 {
        self.consumption().live_payload_bytes
    }

    pub fn peak_live_payload_bytes(&self) -> u64 {
        self.consumption().peak_live_payload_bytes
    }

    pub fn consumed_result_storage_cells(&self) -> u64 {
        self.consumption().result_storage_cells
    }

    pub fn consumed_result_payload_bytes(&self) -> u64 {
        self.consumption().result_payload_bytes
    }

    /// Charge one historical checked-tree site. The caller owns site issuance
    /// and order; this counter does not establish a semantic receipt ledger.
    pub fn charge_fuel_unit(&self) -> Result<(), String> {
        self.require_checked_tree()?;
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

    pub fn charge_build_log_bytes(&self, bytes: u64) -> Result<(), String> {
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

    pub fn charge_filesystem_operation_attempt(&self) -> Result<(), String> {
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

    pub fn reserve_live_filesystem_handle(
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

    pub fn charge_result_custody(&self, cells: u64, text_bytes: u64) -> Result<(), String> {
        self.require_checked_tree()?;
        let limits = self.account.limits;
        let mut consumed = self.account.consumed.lock().map_err(|_| {
            "build-evaluation result-custody sponsor account is unavailable".to_owned()
        })?;
        let candidate_cells = consumed
            .result_storage_cells
            .checked_add(cells)
            .ok_or_else(|| "build-evaluation result-cell accounting overflowed".to_owned())?;
        let candidate_text_bytes = consumed
            .result_payload_bytes
            .checked_add(text_bytes)
            .ok_or_else(|| "build-evaluation result-text accounting overflowed".to_owned())?;
        if candidate_cells > limits.maximum_result_storage_cells() {
            return Err(format!(
                "build-evaluation result-cell sponsor exhausted at {} cells",
                limits.maximum_result_storage_cells()
            ));
        }
        if candidate_text_bytes > limits.maximum_result_payload_bytes() {
            return Err(format!(
                "build-evaluation result-text sponsor exhausted at {} bytes",
                limits.maximum_result_payload_bytes()
            ));
        }
        consumed.result_storage_cells = candidate_cells;
        consumed.result_payload_bytes = candidate_text_bytes;
        Ok(())
    }

    pub fn reserve_live_cell(&self) -> Result<BuildEvaluationLiveCellLease, String> {
        self.require_checked_tree()?;
        let maximum = self.account.limits.maximum_live_storage_cells();
        let mut consumed =
            self.account.consumed.lock().map_err(|_| {
                "build-evaluation live-cell sponsor account is unavailable".to_owned()
            })?;
        let Some(candidate) = consumed.live_storage_cells.checked_add(1) else {
            return Err("build-evaluation live-cell accounting overflowed".to_owned());
        };
        if candidate > maximum {
            return Err(format!(
                "build-evaluation live-cell sponsor exhausted at {maximum} cells"
            ));
        }
        consumed.live_storage_cells = candidate;
        consumed.peak_live_storage_cells = consumed.peak_live_storage_cells.max(candidate);
        drop(consumed);
        Ok(BuildEvaluationLiveCellLease {
            sponsor: self.clone(),
        })
    }

    pub fn reserve_live_text_bytes(
        &self,
        bytes: u64,
    ) -> Result<BuildEvaluationLiveTextByteLease, String> {
        self.increase_live_payload_bytes(bytes)?;
        Ok(BuildEvaluationLiveTextByteLease {
            sponsor: self.clone(),
            bytes,
        })
    }

    fn increase_live_payload_bytes(&self, bytes: u64) -> Result<(), String> {
        self.require_checked_tree()?;
        let maximum = self.account.limits.maximum_live_payload_bytes();
        let mut consumed = self.account.consumed.lock().map_err(|_| {
            "build-evaluation live-Text-byte sponsor account is unavailable".to_owned()
        })?;
        let candidate = consumed
            .live_payload_bytes
            .checked_add(bytes)
            .ok_or_else(|| "build-evaluation live-Text-byte accounting overflowed".to_owned())?;
        if candidate > maximum {
            return Err(format!(
                "build-evaluation live-Text-byte sponsor exhausted at {maximum} bytes"
            ));
        }
        consumed.live_payload_bytes = candidate;
        consumed.peak_live_payload_bytes = consumed.peak_live_payload_bytes.max(candidate);
        Ok(())
    }

    fn release_live_filesystem_handle(&self) {
        let mut consumed = self.consumption();
        debug_assert!(consumed.live_filesystem_handles > 0);
        consumed.live_filesystem_handles = consumed.live_filesystem_handles.saturating_sub(1);
    }

    fn release_live_cell(&self) {
        let mut consumed = self.consumption();
        debug_assert!(consumed.live_storage_cells > 0);
        consumed.live_storage_cells = consumed.live_storage_cells.saturating_sub(1);
    }

    fn release_live_payload_bytes(&self, bytes: u64) {
        let mut consumed = self.consumption();
        debug_assert!(consumed.live_payload_bytes >= bytes);
        consumed.live_payload_bytes = consumed.live_payload_bytes.saturating_sub(bytes);
    }
}

impl BuildEvaluationSponsor {
    pub fn live_cells(&self) -> u64 {
        self.live_storage_cells()
    }
    pub fn peak_live_cells(&self) -> u64 {
        self.peak_live_storage_cells()
    }
    pub fn live_text_bytes(&self) -> u64 {
        self.live_payload_bytes()
    }
    pub fn peak_live_text_bytes(&self) -> u64 {
        self.peak_live_payload_bytes()
    }
    pub fn consumed_result_cells(&self) -> u64 {
        self.consumed_result_storage_cells()
    }
    pub fn consumed_result_text_bytes(&self) -> u64 {
        self.consumed_result_payload_bytes()
    }
}

/// One interpreter-cell reservation. Cloning a semantic cell shares this
/// lease; it retires only when the final alias is dropped.
#[derive(Debug)]
pub struct BuildEvaluationLiveCellLease {
    sponsor: BuildEvaluationSponsor,
}

impl Drop for BuildEvaluationLiveCellLease {
    fn drop(&mut self) {
        self.sponsor.release_live_cell();
    }
}

#[derive(Debug)]
pub struct BuildEvaluationLiveTextByteLease {
    sponsor: BuildEvaluationSponsor,
    bytes: u64,
}

impl BuildEvaluationLiveTextByteLease {
    pub fn grow(&mut self, bytes: u64) -> Result<(), String> {
        self.sponsor.increase_live_payload_bytes(bytes)?;
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .expect("sponsor increase rejected byte-count overflow");
        Ok(())
    }

    pub fn shrink(&mut self, bytes: u64) {
        debug_assert!(self.bytes >= bytes);
        self.bytes = self.bytes.saturating_sub(bytes);
        self.sponsor.release_live_payload_bytes(bytes);
    }
}

impl Drop for BuildEvaluationLiveTextByteLease {
    fn drop(&mut self) {
        self.sponsor.release_live_payload_bytes(self.bytes);
    }
}

/// One compiler-owned reservation. Provider failure, explicit close, and
/// evaluator teardown all release it through ordinary Rust ownership.
#[derive(Debug)]
pub struct BuildEvaluationLiveFilesystemHandleLease {
    sponsor: BuildEvaluationSponsor,
}

impl Drop for BuildEvaluationLiveFilesystemHandleLease {
    fn drop(&mut self) {
        self.sponsor.release_live_filesystem_handle();
    }
}
