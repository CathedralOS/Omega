//! Immutable measurement, limit and usage contracts for accepted-code execution.
//! No live sponsor, account, lock or storage lease crosses through these records.
#![forbid(unsafe_code)]
use semantic_vocabulary::WorkScheduleIdentity;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvaluationResource {
    Work,
    LiveStorage,
    ResultCustody,
    BuildLog,
    FilesystemAttempts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvaluationAccountingError {
    UnsupportedMeasurementIdentity,
    MeasurementSponsorMismatch,
    ZeroInvocationWorkBudget,
    AccountUnavailable,
    CounterOverflow(EvaluationResource),
    BudgetExhausted(EvaluationResource),
    LeaseReleaseExceedsReservation,
    LeaseAccountMismatch,
}

impl std::fmt::Display for EvaluationAccountingError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedMeasurementIdentity => {
                formatter.write_str("unsupported evaluation measurement identity")
            }
            Self::MeasurementSponsorMismatch => {
                formatter.write_str("invocation measurement differs from its aggregate sponsor")
            }
            Self::ZeroInvocationWorkBudget => {
                formatter.write_str("evaluation invocation work budget is zero")
            }
            Self::AccountUnavailable => formatter.write_str("evaluation account is unavailable"),
            Self::CounterOverflow(resource) => {
                write!(formatter, "evaluation {resource:?} accounting overflowed")
            }
            Self::BudgetExhausted(resource) => {
                write!(formatter, "evaluation {resource:?} budget exhausted")
            }
            Self::LeaseReleaseExceedsReservation => {
                formatter.write_str("lease release exceeds held storage")
            }
            Self::LeaseAccountMismatch => {
                formatter.write_str("storage leases belong to different invocations")
            }
        }
    }
}
impl std::error::Error for EvaluationAccountingError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
pub struct TerminalStorageScheduleIdentity(u32);

impl TerminalStorageScheduleIdentity {
    pub const CURRENT: Self = Self(1);
    pub const fn new(marker: u32) -> Option<Self> {
        if marker == 0 {
            None
        } else {
            Some(Self(marker))
        }
    }
    pub const fn marker(self) -> u32 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
pub enum EvaluationMeasurementIdentity {
    TerminalPsi {
        work_schedule: WorkScheduleIdentity,
        storage_schedule: TerminalStorageScheduleIdentity,
    },
}

impl EvaluationMeasurementIdentity {
    pub fn current() -> Self {
        Self::TerminalPsi {
            work_schedule: WorkScheduleIdentity::new(1).expect("schedule marker 1 is nonzero"),
            storage_schedule: TerminalStorageScheduleIdentity::CURRENT,
        }
    }

    pub const fn is_current(self) -> bool {
        match self {
            Self::TerminalPsi {
                work_schedule,
                storage_schedule,
            } => work_schedule.marker() == 1 && storage_schedule.marker() == 1,
        }
    }

    pub const fn is_supported(self, schema: u32, schedule: u32) -> bool {
        match self {
            Self::TerminalPsi {
                work_schedule,
                storage_schedule,
            } => {
                schema == 8
                    && schedule == work_schedule.marker()
                    && work_schedule.marker() == 1
                    && storage_schedule.marker() == 1
            }
        }
    }
}

pub const BUILD_EVALUATION_SPONSOR_LIMITS_SCHEMA_VERSION: u32 = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, record_storage::Record)]
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
