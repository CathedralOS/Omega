//! Source-neutral snapshot and consistency of measured execution and sponsorship.
//!
//! The issuing engine and host adapters own the represented counters. This is
//! accounting evidence, not execution authority, a portable fuel certificate,
//! or validation of the producing program. Consumers bind it to their own
//! production identity and render errors at their reporting boundary.

use crate::EvaluationMeasurementIdentity;

/// Accounting-only result retaining its exact engine/resource measurement kind.
/// It is not semantic admission or program identity. Legacy tree and Terminal
/// schedules cannot substitute for each other, even when markers are equal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildEvaluationUsage {
    pub measurement: EvaluationMeasurementIdentity,
    pub usage_schema_version: u32,
    pub step_schedule_marker: u32,
    /// Deterministic ceiling applied to the single evaluation, not a CPU limit.
    pub invocation_fuel_ceiling: u64,
    /// Shared sponsor-limit schema for package review. Standalone compilation
    /// has no aggregate sponsor.
    pub sponsor_schema_version: Option<u32>,
    /// Aggregate deterministic fuel available to the complete sponsored
    /// review session. Dependencies cannot alter this value.
    pub session_fuel_ceiling: Option<u64>,
    /// Aggregate compiler-owned BuildLog bytes available to the complete
    /// sponsored review session.
    pub session_build_log_byte_ceiling: Option<u64>,
    /// Aggregate canonical filesystem operation attempts available to the
    /// complete sponsored review session.
    pub session_filesystem_attempt_ceiling: Option<u64>,
    /// Compiler-owned filesystem resources that may be reserved concurrently
    /// across the sponsored review session.
    pub session_live_filesystem_handle_ceiling: Option<u64>,
    /// Logical storage cells under the declared measurement that may be live across the
    /// sponsored review session. This is not a memory-byte ceiling.
    pub session_live_storage_cell_ceiling: Option<u64>,
    /// Logical bytes in concurrently live owned byte payload backings under the declared measurement.
    /// This does not represent Vec capacity or process memory.
    pub session_live_payload_byte_ceiling: Option<u64>,
    /// Aggregate recursive result cells admitted across the sponsored session.
    pub session_result_storage_cell_ceiling: Option<u64>,
    /// Aggregate owned payload bytes admitted across the sponsored session.
    pub session_result_payload_byte_ceiling: Option<u64>,
    /// Highest concurrent reservation count observed in the shared session at
    /// the point this build result was issued.
    pub session_peak_live_filesystem_handles: u64,
    /// Highest concurrent logical storage-cell reservation count observed in the
    /// shared session when this result was issued.
    pub session_peak_live_storage_cells: u64,
    /// Highest live owned payload-byte reservation count observed in the shared
    /// sponsored session when this result was issued.
    pub session_peak_live_payload_bytes: u64,
    /// Fuel consumed by this build-machine evaluation.
    pub fuel_units: u64,
    /// BuildLog bytes emitted by this evaluation.
    pub build_log_bytes: u64,
    /// Filesystem operation attempts retained by this evaluation.
    pub filesystem_operation_attempts: u64,
    /// Maximum logical storage cells under the declared measurement live concurrently in this
    /// evaluation.
    pub peak_live_storage_cells: u64,
    /// Maximum logical bytes in live owned payload backings during this
    /// evaluation.
    pub peak_live_payload_bytes: u64,
    pub result_storage_cells: u64,
    pub result_payload_bytes: u64,
}

/// A retained accounting snapshot is inconsistent with its own declared
/// ceilings or sponsorship. This does not certify the identity of its producer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildEvaluationUsageError {
    UnsupportedMeasurementIdentity,
    MeasurementSponsorMismatch,
    UnsupportedSponsorSchema,
    ZeroInvocationFuelCeiling,
    InvocationFuelExceeded,
    UnsponsoredSessionUsage,
    ZeroSessionFuelCeiling,
    ZeroSessionBuildLogCeiling,
    ZeroSessionFilesystemAttemptCeiling,
    ZeroLiveFilesystemHandleCeiling,
    ZeroLiveStorageCellCeiling,
    ZeroLivePayloadByteCeiling,
    ZeroResultStorageCellCeiling,
    ZeroResultPayloadByteCeiling,
    SessionFuelExceeded,
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
    /// Check the retained counters against their supplied ceilings, preserving
    /// the first violated obligation. Unknown engine/fuel/storage identities
    /// cannot be accepted as a currently understood measurement.
    pub fn validate(&self) -> Result<(), BuildEvaluationUsageError> {
        if !self
            .measurement
            .is_supported(self.usage_schema_version, self.step_schedule_marker)
        {
            return Err(BuildEvaluationUsageError::UnsupportedMeasurementIdentity);
        }
        if self.invocation_fuel_ceiling == 0 {
            return Err(BuildEvaluationUsageError::ZeroInvocationFuelCeiling);
        }
        if self.fuel_units > self.invocation_fuel_ceiling {
            return Err(BuildEvaluationUsageError::InvocationFuelExceeded);
        }
        if self
            .sponsor_schema_version
            .is_some_and(|schema| schema != crate::BUILD_EVALUATION_SPONSOR_LIMITS_SCHEMA_VERSION)
        {
            return Err(BuildEvaluationUsageError::UnsupportedSponsorSchema);
        }
        match (
            self.sponsor_schema_version,
            self.session_fuel_ceiling,
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
                return Err(BuildEvaluationUsageError::ZeroSessionFuelCeiling);
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
                let consumed = self.fuel_units;
                if consumed > session_ceiling {
                    return Err(BuildEvaluationUsageError::SessionFuelExceeded);
                }
                let build_log = self.build_log_bytes;
                if build_log > build_log_ceiling {
                    return Err(BuildEvaluationUsageError::SessionBuildLogExceeded);
                }
                let filesystem_attempts = self.filesystem_operation_attempts;
                if filesystem_attempts > filesystem_attempt_ceiling {
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
                let result_storage_cells = self.result_storage_cells;
                if result_storage_cells > result_storage_cell_ceiling {
                    return Err(BuildEvaluationUsageError::ResultStorageCellsExceeded);
                }
                let result_payload_bytes = self.result_payload_bytes;
                if result_payload_bytes > result_payload_byte_ceiling {
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
