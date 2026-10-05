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
use std::sync::Mutex;


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


    /// Require each issued charge exactly once, in issuance order, for this
    /// exact account. Equal limits never make a foreign receipt acceptable.
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
