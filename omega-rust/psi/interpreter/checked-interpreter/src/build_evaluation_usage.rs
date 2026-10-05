//! Snapshot and consistency of one measured invocation and its sponsor session.
//!
//! The interpreter owns all counters and ceilings represented here. This is
//! accounting evidence, not execution authority, a portable fuel certificate,
//! or validation of the producing program. Consumers bind it to their own
//! production identity and render errors at their reporting boundary.

use crate::{BuildEvaluationSponsor, EvaluationUsage};

/// Accounting-only projection of the transitional typed-tree build evaluator.
/// This is not terminal-Psi fuel and does not participate in `BuildConfig` or
/// program identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildEvaluationUsage {
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
    /// Semantic interpreter cells that may be live concurrently across the
    /// sponsored review session. This is not a memory-byte ceiling.
    pub session_live_cell_ceiling: Option<u64>,
    /// Logical bytes in concurrently live interpreter Text backing buffers.
    /// This does not represent Vec capacity or process memory.
    pub session_live_text_byte_ceiling: Option<u64>,
    /// Aggregate recursive result cells admitted across the sponsored session.
    pub session_result_cell_ceiling: Option<u64>,
    /// Aggregate Text payload bytes admitted across the sponsored session.
    pub session_result_text_byte_ceiling: Option<u64>,
    /// Highest concurrent reservation count observed in the shared session at
    /// the point this build result was issued.
    pub session_peak_live_filesystem_handles: u64,
    /// Highest concurrent semantic-cell reservation count observed in the
    /// shared session when this result was issued.
    pub session_peak_live_cells: u64,
    /// Highest live interpreter Text payload-byte count observed in the shared
    /// sponsored session when this result was issued.
    pub session_peak_live_text_bytes: u64,
    /// Fuel consumed by this build-machine evaluation.
    pub fuel_units: u64,
    /// BuildLog bytes emitted by this evaluation.
    pub build_log_bytes: u64,
    /// Filesystem operation attempts retained by this evaluation.
    pub filesystem_operation_attempts: u64,
    /// Maximum semantic interpreter cells live concurrently in this
    /// evaluation.
    pub peak_live_cells: u64,
    /// Maximum logical bytes in live Text backing buffers during this
    /// evaluation.
    pub peak_live_text_bytes: u64,
    pub result_cells: u64,
    pub result_text_bytes: u64,
}

/// A retained accounting snapshot is inconsistent with its own declared
/// ceilings or sponsorship. This does not certify the identity of its producer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildEvaluationUsageError {
    ZeroInvocationFuelCeiling,
    InvocationFuelExceeded,
    UnsponsoredSessionUsage,
    ZeroSessionFuelCeiling,
    ZeroSessionBuildLogCeiling,
    ZeroSessionFilesystemAttemptCeiling,
    ZeroLiveFilesystemHandleCeiling,
    ZeroLiveCellCeiling,
    ZeroLiveTextByteCeiling,
    ZeroResultCellCeiling,
    ZeroResultTextByteCeiling,
    SessionFuelExceeded,
    SessionBuildLogExceeded,
    SessionFilesystemAttemptsExceeded,
    LiveFilesystemHandlesExceeded,
    LiveCellsExceeded,
    InvocationCellPeakExceedsSession,
    LiveTextBytesExceeded,
    InvocationTextPeakExceedsSession,
    ResultCellsExceeded,
    ResultTextBytesExceeded,
    IncompleteSponsorIdentity,
}

impl BuildEvaluationUsage {
    /// Capture the measured invocation and the sponsor's current peaks at the
    /// caller's publication point. Session context is not invocation identity.
    pub fn capture(usage: EvaluationUsage, sponsor: Option<&BuildEvaluationSponsor>) -> Self {
        Self {
            usage_schema_version: usage.schema().schema_version(),
            step_schedule_marker: usage.schedule().marker(),
            invocation_fuel_ceiling: usage.fuel_ceiling(),
            sponsor_schema_version: sponsor.map(|sponsor| sponsor.limits().schema_version()),
            session_fuel_ceiling: sponsor.map(|sponsor| sponsor.limits().maximum_fuel_units()),
            session_build_log_byte_ceiling: sponsor
                .map(|sponsor| sponsor.limits().maximum_build_log_bytes()),
            session_filesystem_attempt_ceiling: sponsor
                .map(|sponsor| sponsor.limits().maximum_filesystem_operation_attempts()),
            session_live_filesystem_handle_ceiling: sponsor
                .map(|sponsor| sponsor.limits().maximum_live_filesystem_handles()),
            session_live_cell_ceiling: sponsor.map(|sponsor| sponsor.limits().maximum_live_cells()),
            session_live_text_byte_ceiling: sponsor
                .map(|sponsor| sponsor.limits().maximum_live_text_bytes()),
            session_result_cell_ceiling: sponsor
                .map(|sponsor| sponsor.limits().maximum_result_cells()),
            session_result_text_byte_ceiling: sponsor
                .map(|sponsor| sponsor.limits().maximum_result_text_bytes()),
            session_peak_live_filesystem_handles: sponsor
                .map_or(0, BuildEvaluationSponsor::peak_live_filesystem_handles),
            session_peak_live_cells: sponsor.map_or(0, BuildEvaluationSponsor::peak_live_cells),
            session_peak_live_text_bytes: sponsor
                .map_or(0, BuildEvaluationSponsor::peak_live_text_bytes),
            fuel_units: usage.fuel_units(),
            build_log_bytes: usage.build_log_bytes(),
            filesystem_operation_attempts: usage.filesystem_operation_attempts(),
            peak_live_cells: usage.peak_live_cells(),
            peak_live_text_bytes: usage.peak_live_text_bytes(),
            result_cells: usage.result_cells(),
            result_text_bytes: usage.result_text_bytes(),
        }
    }

    /// Check the retained counters against their supplied ceilings, preserving
    /// the first violated obligation. Schema markers are retained identities,
    /// not checked against the current interpreter version by this operation.
    pub fn validate(&self) -> Result<(), BuildEvaluationUsageError> {
        if self.invocation_fuel_ceiling == 0 {
            return Err(BuildEvaluationUsageError::ZeroInvocationFuelCeiling);
        }
        if self.fuel_units > self.invocation_fuel_ceiling {
            return Err(BuildEvaluationUsageError::InvocationFuelExceeded);
        }
        match (
            self.sponsor_schema_version,
            self.session_fuel_ceiling,
            self.session_build_log_byte_ceiling,
            self.session_filesystem_attempt_ceiling,
            self.session_live_filesystem_handle_ceiling,
            self.session_live_cell_ceiling,
            self.session_live_text_byte_ceiling,
            self.session_result_cell_ceiling,
            self.session_result_text_byte_ceiling,
        ) {
            (None, None, None, None, None, None, None, None, None) => {
                if self.session_peak_live_filesystem_handles != 0
                    || self.session_peak_live_cells != 0
                    || self.session_peak_live_text_bytes != 0
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
                return Err(BuildEvaluationUsageError::ZeroLiveCellCeiling);
            }
            (Some(_), Some(_), Some(_), Some(_), Some(_), Some(_), Some(0), Some(_), Some(_)) => {
                return Err(BuildEvaluationUsageError::ZeroLiveTextByteCeiling);
            }
            (Some(_), Some(_), Some(_), Some(_), Some(_), Some(_), Some(_), Some(0), Some(_)) => {
                return Err(BuildEvaluationUsageError::ZeroResultCellCeiling);
            }
            (Some(_), Some(_), Some(_), Some(_), Some(_), Some(_), Some(_), Some(_), Some(0)) => {
                return Err(BuildEvaluationUsageError::ZeroResultTextByteCeiling);
            }
            (
                Some(_),
                Some(session_ceiling),
                Some(build_log_ceiling),
                Some(filesystem_attempt_ceiling),
                Some(live_filesystem_handle_ceiling),
                Some(live_cell_ceiling),
                Some(live_text_byte_ceiling),
                Some(result_cell_ceiling),
                Some(result_text_byte_ceiling),
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
                if self.session_peak_live_cells > live_cell_ceiling
                    || self.peak_live_cells > live_cell_ceiling
                {
                    return Err(BuildEvaluationUsageError::LiveCellsExceeded);
                }
                if self.peak_live_cells > self.session_peak_live_cells {
                    return Err(BuildEvaluationUsageError::InvocationCellPeakExceedsSession);
                }
                if self.session_peak_live_text_bytes > live_text_byte_ceiling
                    || self.peak_live_text_bytes > live_text_byte_ceiling
                {
                    return Err(BuildEvaluationUsageError::LiveTextBytesExceeded);
                }
                if self.peak_live_text_bytes > self.session_peak_live_text_bytes {
                    return Err(BuildEvaluationUsageError::InvocationTextPeakExceedsSession);
                }
                let result_cells = self.result_cells;
                if result_cells > result_cell_ceiling {
                    return Err(BuildEvaluationUsageError::ResultCellsExceeded);
                }
                let result_text_bytes = self.result_text_bytes;
                if result_text_bytes > result_text_byte_ceiling {
                    return Err(BuildEvaluationUsageError::ResultTextBytesExceeded);
                }
            }
            _ => {
                return Err(BuildEvaluationUsageError::IncompleteSponsorIdentity);
            }
        }
        Ok(())
    }
}
