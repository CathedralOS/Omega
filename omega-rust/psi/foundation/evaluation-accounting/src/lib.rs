/*
We provide deterministic resource accounting for compile-time build evaluations,
combining aggregate session sponsors (`BuildEvaluationSponsor`,
`BuildEvaluationSponsorLimits`), per-invocation accounts and movable RAII
storage leases (`EvaluationInvocationAccount`, `EvaluationStorageLease` in
`invocation`), and versioned usage telemetry records (`BuildEvaluationUsage`,
`BuildEvaluationUsageError`).
*/

/*
Compile-time evaluation across Checked-Tree and Terminal-Psi engines requires
strict, deterministic resource bounds over logical work units, BuildLog bytes,
filesystem operation attempts, live filesystem handles, live storage cells, live
payload bytes, and retained result custody. Separating pure counter transactions
and RAII leases from interpreter state lets us enforce fail-closed pre-site
reservations where any refused charge leaves both aggregate and invocation
accounts unchanged.
*/

/*
Every sponsor and usage record binds an explicit `EvaluationMeasurementIdentity`
so equal numeric schedule markers from different engines can never spend or
substitute for each other's budgets. `BuildEvaluationSponsor::charge_pure_usage`
atomically replays validated pure evaluation costs against a current sponsor
without importing the producer's account authority.
*/

#![forbid(unsafe_code)]

mod invocation;

pub use invocation::*;

use std::sync::{Arc, Mutex, MutexGuard};

pub const BUILD_EVALUATION_SPONSOR_LIMITS_SCHEMA_VERSION: u32 = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub struct BuildEvaluationSponsorLimits {
    work_budget: u64,
    maximum_build_log_bytes: u64,
    maximum_filesystem_operation_attempts: u64,
    maximum_live_filesystem_handles: u64,
    maximum_live_storage_cells: u64,
    maximum_live_payload_bytes: u64,
    maximum_result_storage_cells: u64,
    maximum_result_payload_bytes: u64,
}

impl BuildEvaluationSponsorLimits {
    pub const fn new(
        work_budget: u64,
        maximum_build_log_bytes: u64,
        maximum_filesystem_operation_attempts: u64,
        maximum_live_filesystem_handles: u64,
        maximum_live_storage_cells: u64,
        maximum_live_payload_bytes: u64,
        maximum_result_storage_cells: u64,
        maximum_result_payload_bytes: u64,
    ) -> Option<Self> {
        if work_budget == 0
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
            work_budget,
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

    pub const fn work_budget(self) -> u64 {
        self.work_budget
    }

    pub const fn maximum_build_log_bytes(self) -> u64 {
        self.maximum_build_log_bytes
    }

    pub const fn maximum_filesystem_operation_attempts(self) -> u64 {
        self.maximum_filesystem_operation_attempts
    }

    pub const fn maximum_live_filesystem_handles(self) -> u64 {
        self.maximum_live_filesystem_handles
    }

    pub const fn maximum_live_storage_cells(self) -> u64 {
        self.maximum_live_storage_cells
    }

    pub const fn maximum_live_payload_bytes(self) -> u64 {
        self.maximum_live_payload_bytes
    }

    pub const fn maximum_result_storage_cells(self) -> u64 {
        self.maximum_result_storage_cells
    }

    pub const fn maximum_result_payload_bytes(self) -> u64 {
        self.maximum_result_payload_bytes
    }

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
pub(crate) struct BuildEvaluationSponsorAccount {
    pub(crate) limits: BuildEvaluationSponsorLimits,
    pub(crate) measurement: EvaluationMeasurementIdentity,
    pub(crate) consumed: Mutex<BuildEvaluationSponsorConsumption>,
}

#[derive(Debug, Default)]
pub(crate) struct BuildEvaluationSponsorConsumption {
    pub(crate) work_units: u64,
    pub(crate) build_log_bytes: u64,
    pub(crate) filesystem_operation_attempts: u64,
    pub(crate) live_filesystem_handles: u64,
    pub(crate) peak_live_filesystem_handles: u64,
    pub(crate) live_storage_cells: u64,
    pub(crate) peak_live_storage_cells: u64,
    pub(crate) live_payload_bytes: u64,
    pub(crate) peak_live_payload_bytes: u64,
    pub(crate) result_storage_cells: u64,
    pub(crate) result_payload_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct BuildEvaluationSponsor {
    pub(crate) account: Arc<BuildEvaluationSponsorAccount>,
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

    fn consumption(&self) -> MutexGuard<'_, BuildEvaluationSponsorConsumption> {
        self.account
            .consumed
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn consumed_work_units(&self) -> u64 {
        self.consumption().work_units
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

    pub fn peak_live_cells(&self) -> u64 {
        self.peak_live_storage_cells()
    }

    pub fn peak_live_text_bytes(&self) -> u64 {
        self.peak_live_payload_bytes()
    }

    pub fn charge_work_unit(&self) -> Result<(), String> {
        self.require_checked_tree()?;
        let maximum = self.account.limits.work_budget();
        let mut consumed = self.account.consumed.lock().map_err(|_| {
            "build-evaluation aggregate work sponsor account is unavailable".to_owned()
        })?;
        if consumed.work_units >= maximum {
            return Err(format!(
                "build-evaluation aggregate work sponsor exhausted at {maximum} work units"
            ));
        }
        consumed.work_units += 1;
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

    pub fn charge_pure_usage(
        &self,
        mut usage: BuildEvaluationUsage,
    ) -> Result<BuildEvaluationUsage, String> {
        usage
            .validate()
            .map_err(|reason| format!("invalid cached evaluation usage: {reason:?}"))?;
        if usage.measurement != self.measurement() {
            return Err("cached evaluation measurement differs from its current sponsor".into());
        }
        if usage.filesystem_operation_attempts != 0 {
            return Err("filesystem-observing evaluation usage cannot be charged as Pure".into());
        }
        let limits = self.limits();
        let mut consumed = self.account.consumed.lock().map_err(|_| {
            "build-evaluation cached-usage sponsor account is unavailable".to_owned()
        })?;
        let work = charged_total(
            consumed.work_units,
            usage.work_units,
            limits.work_budget(),
            "aggregate work",
            "work units",
        )?;
        let log = charged_total(
            consumed.build_log_bytes,
            usage.build_log_bytes,
            limits.maximum_build_log_bytes(),
            "aggregate BuildLog",
            "bytes",
        )?;
        let cells = charged_total(
            consumed.result_storage_cells,
            usage.result_storage_cells,
            limits.maximum_result_storage_cells(),
            "result-cell",
            "cells",
        )?;
        let bytes = charged_total(
            consumed.result_payload_bytes,
            usage.result_payload_bytes,
            limits.maximum_result_payload_bytes(),
            "result-text",
            "bytes",
        )?;
        let live_cells = charged_total(
            consumed.live_storage_cells,
            usage.peak_live_storage_cells,
            limits.maximum_live_storage_cells(),
            "live-cell",
            "cells",
        )?;
        let live_bytes = charged_total(
            consumed.live_payload_bytes,
            usage.peak_live_payload_bytes,
            limits.maximum_live_payload_bytes(),
            "live-text",
            "bytes",
        )?;
        usage.sponsor_schema_version = Some(limits.schema_version());
        usage.session_work_budget = Some(limits.work_budget());
        usage.session_build_log_byte_ceiling = Some(limits.maximum_build_log_bytes());
        usage.session_filesystem_attempt_ceiling =
            Some(limits.maximum_filesystem_operation_attempts());
        usage.session_live_filesystem_handle_ceiling =
            Some(limits.maximum_live_filesystem_handles());
        usage.session_live_storage_cell_ceiling = Some(limits.maximum_live_storage_cells());
        usage.session_live_payload_byte_ceiling = Some(limits.maximum_live_payload_bytes());
        usage.session_result_storage_cell_ceiling = Some(limits.maximum_result_storage_cells());
        usage.session_result_payload_byte_ceiling = Some(limits.maximum_result_payload_bytes());
        usage.session_peak_live_filesystem_handles = consumed.peak_live_filesystem_handles;
        usage.session_peak_live_storage_cells = consumed.peak_live_storage_cells.max(live_cells);
        usage.session_peak_live_payload_bytes = consumed.peak_live_payload_bytes.max(live_bytes);
        usage
            .validate()
            .map_err(|reason| format!("invalid current evaluation usage: {reason:?}"))?;
        consumed.work_units = work;
        consumed.build_log_bytes = log;
        consumed.result_storage_cells = cells;
        consumed.result_payload_bytes = bytes;
        consumed.peak_live_storage_cells = usage.session_peak_live_storage_cells;
        consumed.peak_live_payload_bytes = usage.session_peak_live_payload_bytes;
        Ok(usage)
    }
}

fn charged_total(
    current: u64,
    charge: u64,
    maximum: u64,
    resource: &str,
    unit: &str,
) -> Result<u64, String> {
    let candidate = current
        .checked_add(charge)
        .ok_or_else(|| format!("build-evaluation {resource} accounting overflowed"))?;
    if candidate > maximum {
        return Err(format!(
            "build-evaluation {resource} sponsor exhausted at {maximum} {unit}"
        ));
    }
    Ok(candidate)
}

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

#[derive(Debug)]
pub struct BuildEvaluationLiveFilesystemHandleLease {
    sponsor: BuildEvaluationSponsor,
}

impl Drop for BuildEvaluationLiveFilesystemHandleLease {
    fn drop(&mut self) {
        self.sponsor.release_live_filesystem_handle();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub struct BuildEvaluationUsage {
    pub measurement: EvaluationMeasurementIdentity,
    pub usage_schema_version: u32,
    pub step_schedule_marker: u32,
    pub invocation_work_budget: u64, // Deterministic ceiling applied to the single evaluation, not a CPU limit.
    pub sponsor_schema_version: Option<u32>, // Shared sponsor-limit schema for package review; `None` for standalone compilation.
    pub session_work_budget: Option<u64>, // Aggregate deterministic work available to the complete sponsored review session.
    pub session_build_log_byte_ceiling: Option<u64>, // Aggregate compiler-owned BuildLog bytes available to the session.
    pub session_filesystem_attempt_ceiling: Option<u64>, // Aggregate canonical filesystem operation attempts available to the session.
    pub session_live_filesystem_handle_ceiling: Option<u64>, // Maximum concurrent filesystem resources across the session.
    pub session_live_storage_cell_ceiling: Option<u64>, // Maximum concurrent logical storage cells across the session.
    pub session_live_payload_byte_ceiling: Option<u64>, // Maximum concurrent logical bytes in owned payload backings across the session.
    pub session_result_storage_cell_ceiling: Option<u64>, // Aggregate recursive result cells admitted across the session.
    pub session_result_payload_byte_ceiling: Option<u64>, // Aggregate owned result payload bytes admitted across the session.
    pub session_peak_live_filesystem_handles: u64, // Highest concurrent filesystem-handle reservation count observed in the session.
    pub session_peak_live_storage_cells: u64, // Highest concurrent logical storage-cell reservation count observed in the session.
    pub session_peak_live_payload_bytes: u64, // Highest concurrent owned payload-byte reservation count observed in the session.
    pub work_units: u64,                      // Work consumed by this build-machine evaluation.
    pub build_log_bytes: u64,                 // BuildLog bytes emitted by this evaluation.
    pub filesystem_operation_attempts: u64, // Filesystem operation attempts retained by this evaluation.
    pub peak_live_storage_cells: u64, // Maximum concurrent logical storage cells during this evaluation.
    pub peak_live_payload_bytes: u64, // Maximum concurrent logical bytes in owned payload backings during this evaluation.
    pub result_storage_cells: u64,
    pub result_payload_bytes: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildEvaluationUsageError {
    UnsupportedMeasurementIdentity,
    MeasurementSponsorMismatch,
    UnsupportedSponsorSchema,
    ZeroInvocationWorkBudget,
    InvocationWorkBudgetExceeded,
    UnsponsoredSessionUsage,
    ZeroSessionWorkBudget,
    ZeroSessionBuildLogCeiling,
    ZeroSessionFilesystemAttemptCeiling,
    ZeroLiveFilesystemHandleCeiling,
    ZeroLiveStorageCellCeiling,
    ZeroLivePayloadByteCeiling,
    ZeroResultStorageCellCeiling,
    ZeroResultPayloadByteCeiling,
    SessionWorkBudgetExceeded,
    SessionBuildLogExceeded,
    SessionFilesystemAttemptsExceeded,
    LiveFilesystemHandlesExceeded,
    LiveStorageCellsExceeded,
    InvocationStoragePeakExceedsSession,
    LivePayloadBytesExceeded,
    InvocationPayloadPeakExceedsSession,
    ResultStorageCellsExceeded,
    ResultPayloadBytesExceeded,
    IncompleteSponsorIdentity,
}

impl BuildEvaluationUsage {
    pub fn validate(&self) -> Result<(), BuildEvaluationUsageError> {
        if !self
            .measurement
            .is_supported(self.usage_schema_version, self.step_schedule_marker)
        {
            return Err(BuildEvaluationUsageError::UnsupportedMeasurementIdentity);
        }
        if self.invocation_work_budget == 0 {
            return Err(BuildEvaluationUsageError::ZeroInvocationWorkBudget);
        }
        if self.work_units > self.invocation_work_budget {
            return Err(BuildEvaluationUsageError::InvocationWorkBudgetExceeded);
        }
        if self
            .sponsor_schema_version
            .is_some_and(|schema| schema != BUILD_EVALUATION_SPONSOR_LIMITS_SCHEMA_VERSION)
        {
            return Err(BuildEvaluationUsageError::UnsupportedSponsorSchema);
        }
        match (
            self.sponsor_schema_version,
            self.session_work_budget,
            self.session_build_log_byte_ceiling,
            self.session_filesystem_attempt_ceiling,
            self.session_live_filesystem_handle_ceiling,
            self.session_live_storage_cell_ceiling,
            self.session_live_payload_byte_ceiling,
            self.session_result_storage_cell_ceiling,
            self.session_result_payload_byte_ceiling,
        ) {
            (None, None, None, None, None, None, None, None, None) => {
                if self.session_peak_live_filesystem_handles != 0
                    || self.session_peak_live_storage_cells != 0
                    || self.session_peak_live_payload_bytes != 0
                {
                    return Err(BuildEvaluationUsageError::UnsponsoredSessionUsage);
                }
            }
            (Some(_), Some(0), Some(_), Some(_), Some(_), Some(_), Some(_), Some(_), Some(_)) => {
                return Err(BuildEvaluationUsageError::ZeroSessionWorkBudget);
            }
            (Some(_), Some(_), Some(0), Some(_), Some(_), Some(_), Some(_), Some(_), Some(_)) => {
                return Err(BuildEvaluationUsageError::ZeroSessionBuildLogCeiling);
            }
            (Some(_), Some(_), Some(_), Some(0), Some(_), Some(_), Some(_), Some(_), Some(_)) => {
                return Err(BuildEvaluationUsageError::ZeroSessionFilesystemAttemptCeiling);
            }
            (Some(_), Some(_), Some(_), Some(_), Some(0), Some(_), Some(_), Some(_), Some(_)) => {
                return Err(BuildEvaluationUsageError::ZeroLiveFilesystemHandleCeiling);
            }
            (Some(_), Some(_), Some(_), Some(_), Some(_), Some(0), Some(_), Some(_), Some(_)) => {
                return Err(BuildEvaluationUsageError::ZeroLiveStorageCellCeiling);
            }
            (Some(_), Some(_), Some(_), Some(_), Some(_), Some(_), Some(0), Some(_), Some(_)) => {
                return Err(BuildEvaluationUsageError::ZeroLivePayloadByteCeiling);
            }
            (Some(_), Some(_), Some(_), Some(_), Some(_), Some(_), Some(_), Some(0), Some(_)) => {
                return Err(BuildEvaluationUsageError::ZeroResultStorageCellCeiling);
            }
            (Some(_), Some(_), Some(_), Some(_), Some(_), Some(_), Some(_), Some(_), Some(0)) => {
                return Err(BuildEvaluationUsageError::ZeroResultPayloadByteCeiling);
            }
            (
                Some(_),
                Some(session_ceiling),
                Some(build_log_ceiling),
                Some(filesystem_attempt_ceiling),
                Some(live_filesystem_handle_ceiling),
                Some(live_storage_cell_ceiling),
                Some(live_payload_byte_ceiling),
                Some(result_storage_cell_ceiling),
                Some(result_payload_byte_ceiling),
            ) => {
                if self.work_units > session_ceiling {
                    return Err(BuildEvaluationUsageError::SessionWorkBudgetExceeded);
                }
                if self.build_log_bytes > build_log_ceiling {
                    return Err(BuildEvaluationUsageError::SessionBuildLogExceeded);
                }
                if self.filesystem_operation_attempts > filesystem_attempt_ceiling {
                    return Err(BuildEvaluationUsageError::SessionFilesystemAttemptsExceeded);
                }
                if self.session_peak_live_filesystem_handles > live_filesystem_handle_ceiling {
                    return Err(BuildEvaluationUsageError::LiveFilesystemHandlesExceeded);
                }
                if self.session_peak_live_storage_cells > live_storage_cell_ceiling
                    || self.peak_live_storage_cells > live_storage_cell_ceiling
                {
                    return Err(BuildEvaluationUsageError::LiveStorageCellsExceeded);
                }
                if self.peak_live_storage_cells > self.session_peak_live_storage_cells {
                    return Err(BuildEvaluationUsageError::InvocationStoragePeakExceedsSession);
                }
                if self.session_peak_live_payload_bytes > live_payload_byte_ceiling
                    || self.peak_live_payload_bytes > live_payload_byte_ceiling
                {
                    return Err(BuildEvaluationUsageError::LivePayloadBytesExceeded);
                }
                if self.peak_live_payload_bytes > self.session_peak_live_payload_bytes {
                    return Err(BuildEvaluationUsageError::InvocationPayloadPeakExceedsSession);
                }
                if self.result_storage_cells > result_storage_cell_ceiling {
                    return Err(BuildEvaluationUsageError::ResultStorageCellsExceeded);
                }
                if self.result_payload_bytes > result_payload_byte_ceiling {
                    return Err(BuildEvaluationUsageError::ResultPayloadBytesExceeded);
                }
            }
            _ => {
                return Err(BuildEvaluationUsageError::IncompleteSponsorIdentity);
            }
        }
        Ok(())
    }
}
