/*
We manage per-invocation resource accounts (`EvaluationInvocationAccount`),
engine measurement identities (`EvaluationMeasurementIdentity`,
`TerminalStorageScheduleIdentity`), and movable RAII storage leases
(`EvaluationStorageLease`). Every two-account transaction locks the aggregate
sponsor before the invocation account so concurrent invocations never deadlock,
and any failed charge leaves both counters untouched.
*/

use crate::{BuildEvaluationSponsor, BuildEvaluationSponsorConsumption, BuildEvaluationUsage};
use semantic_vocabulary::WorkScheduleIdentity;
use std::sync::{Arc, Mutex, MutexGuard};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
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

#[derive(Debug, Default)]
struct InvocationConsumption {
    work_units: u64,
    build_log_bytes: u64,
    filesystem_operation_attempts: u64,
    live_storage_cells: u64,
    peak_live_storage_cells: u64,
    live_payload_bytes: u64,
    peak_live_payload_bytes: u64,
    result_storage_cells: u64,
    result_payload_bytes: u64,
}

#[derive(Debug)]
struct InvocationAccount {
    sponsor: BuildEvaluationSponsor,
    measurement: EvaluationMeasurementIdentity,
    work_budget: u64,
    consumed: Mutex<InvocationConsumption>,
}

#[derive(Debug, Clone)]
pub struct EvaluationInvocationAccount {
    account: Arc<InvocationAccount>,
}

impl BuildEvaluationSponsor {
    pub fn owns_usage(&self, usage: &EvaluationInvocationUsage) -> bool {
        Arc::ptr_eq(&self.account, &usage.account.sponsor.account)
    }

    pub fn begin_invocation(
        &self,
        measurement: EvaluationMeasurementIdentity,
        work_budget: u64,
    ) -> Result<EvaluationInvocationAccount, EvaluationAccountingError> {
        if work_budget == 0 {
            return Err(EvaluationAccountingError::ZeroInvocationWorkBudget);
        }
        if !measurement.is_current() {
            return Err(EvaluationAccountingError::UnsupportedMeasurementIdentity);
        }
        if measurement != self.measurement() {
            return Err(EvaluationAccountingError::MeasurementSponsorMismatch);
        }
        Ok(EvaluationInvocationAccount {
            account: Arc::new(InvocationAccount {
                sponsor: self.clone(),
                measurement,
                work_budget,
                consumed: Mutex::new(InvocationConsumption::default()),
            }),
        })
    }
}

impl InvocationAccount {
    fn consumption(
        &self,
    ) -> Result<
        (
            MutexGuard<'_, BuildEvaluationSponsorConsumption>,
            MutexGuard<'_, InvocationConsumption>,
        ),
        EvaluationAccountingError,
    > {
        let shared = self
            .sponsor
            .account
            .consumed
            .lock()
            .map_err(|_| EvaluationAccountingError::AccountUnavailable)?;
        let local = self
            .consumed
            .lock()
            .map_err(|_| EvaluationAccountingError::AccountUnavailable)?;
        Ok((shared, local))
    }
}

impl EvaluationInvocationAccount {
    pub fn same_invocation(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.account, &other.account)
    }

    pub fn measurement(&self) -> EvaluationMeasurementIdentity {
        self.account.measurement
    }

    pub fn charge_work(&self, units: u64) -> Result<(), EvaluationAccountingError> {
        let (mut shared, mut local) = self.account.consumption()?;
        let aggregate = shared.work_units.checked_add(units).ok_or(
            EvaluationAccountingError::CounterOverflow(EvaluationResource::Work),
        )?;
        let invocation = local.work_units.checked_add(units).ok_or(
            EvaluationAccountingError::CounterOverflow(EvaluationResource::Work),
        )?;
        if aggregate > self.account.sponsor.limits().work_budget()
            || invocation > self.account.work_budget
        {
            return Err(EvaluationAccountingError::BudgetExhausted(
                EvaluationResource::Work,
            ));
        }
        shared.work_units = aggregate;
        local.work_units = invocation;
        Ok(())
    }

    pub fn charge_build_log_bytes(&self, bytes: u64) -> Result<(), EvaluationAccountingError> {
        let (mut shared, mut local) = self.account.consumption()?;
        let aggregate = shared.build_log_bytes.checked_add(bytes).ok_or(
            EvaluationAccountingError::CounterOverflow(EvaluationResource::BuildLog),
        )?;
        let invocation = local.build_log_bytes.checked_add(bytes).ok_or(
            EvaluationAccountingError::CounterOverflow(EvaluationResource::BuildLog),
        )?;
        if aggregate > self.account.sponsor.limits().maximum_build_log_bytes() {
            return Err(EvaluationAccountingError::BudgetExhausted(
                EvaluationResource::BuildLog,
            ));
        }
        shared.build_log_bytes = aggregate;
        local.build_log_bytes = invocation;
        Ok(())
    }

    pub fn charge_filesystem_operation_attempt(&self) -> Result<(), EvaluationAccountingError> {
        let (mut shared, mut local) = self.account.consumption()?;
        let aggregate = shared.filesystem_operation_attempts.checked_add(1).ok_or(
            EvaluationAccountingError::CounterOverflow(EvaluationResource::FilesystemAttempts),
        )?;
        let invocation = local.filesystem_operation_attempts.checked_add(1).ok_or(
            EvaluationAccountingError::CounterOverflow(EvaluationResource::FilesystemAttempts),
        )?;
        if aggregate
            > self
                .account
                .sponsor
                .limits()
                .maximum_filesystem_operation_attempts()
        {
            return Err(EvaluationAccountingError::BudgetExhausted(
                EvaluationResource::FilesystemAttempts,
            ));
        }
        shared.filesystem_operation_attempts = aggregate;
        local.filesystem_operation_attempts = invocation;
        Ok(())
    }

    pub fn reserve_storage(
        &self,
        cells: u64,
        payload_bytes: u64,
    ) -> Result<EvaluationStorageLease, EvaluationAccountingError> {
        self.reserve_site(0, cells, payload_bytes)
    }

    pub fn reserve_site(
        &self,
        work_units: u64,
        cells: u64,
        payload_bytes: u64,
    ) -> Result<EvaluationStorageLease, EvaluationAccountingError> {
        self.increase_site(work_units, cells, payload_bytes)?;
        Ok(EvaluationStorageLease {
            account: self.clone(),
            cells,
            payload_bytes,
        })
    }

    fn increase_site(
        &self,
        work_units: u64,
        cells: u64,
        payload_bytes: u64,
    ) -> Result<(), EvaluationAccountingError> {
        let (mut shared, mut local) = self.account.consumption()?;
        let limits = self.account.sponsor.limits();
        let aggregate_work = shared.work_units.checked_add(work_units).ok_or(
            EvaluationAccountingError::CounterOverflow(EvaluationResource::Work),
        )?;
        let invocation_work = local.work_units.checked_add(work_units).ok_or(
            EvaluationAccountingError::CounterOverflow(EvaluationResource::Work),
        )?;
        if aggregate_work > limits.work_budget() || invocation_work > self.account.work_budget {
            return Err(EvaluationAccountingError::BudgetExhausted(
                EvaluationResource::Work,
            ));
        }
        let aggregate_cells = shared.live_storage_cells.checked_add(cells).ok_or(
            EvaluationAccountingError::CounterOverflow(EvaluationResource::LiveStorage),
        )?;
        let aggregate_bytes = shared.live_payload_bytes.checked_add(payload_bytes).ok_or(
            EvaluationAccountingError::CounterOverflow(EvaluationResource::LiveStorage),
        )?;
        let invocation_cells = local.live_storage_cells.checked_add(cells).ok_or(
            EvaluationAccountingError::CounterOverflow(EvaluationResource::LiveStorage),
        )?;
        let invocation_bytes = local.live_payload_bytes.checked_add(payload_bytes).ok_or(
            EvaluationAccountingError::CounterOverflow(EvaluationResource::LiveStorage),
        )?;
        if aggregate_cells > limits.maximum_live_storage_cells()
            || invocation_cells > limits.maximum_live_storage_cells()
            || aggregate_bytes > limits.maximum_live_payload_bytes()
            || invocation_bytes > limits.maximum_live_payload_bytes()
        {
            return Err(EvaluationAccountingError::BudgetExhausted(
                EvaluationResource::LiveStorage,
            ));
        }
        shared.work_units = aggregate_work;
        local.work_units = invocation_work;
        shared.live_storage_cells = aggregate_cells;
        shared.live_payload_bytes = aggregate_bytes;
        shared.peak_live_storage_cells = shared.peak_live_storage_cells.max(aggregate_cells);
        shared.peak_live_payload_bytes = shared.peak_live_payload_bytes.max(aggregate_bytes);
        local.live_storage_cells = invocation_cells;
        local.live_payload_bytes = invocation_bytes;
        local.peak_live_storage_cells = local.peak_live_storage_cells.max(invocation_cells);
        local.peak_live_payload_bytes = local.peak_live_payload_bytes.max(invocation_bytes);
        Ok(())
    }

    fn release_storage(&self, cells: u64, payload_bytes: u64) {
        let mut shared = self
            .account
            .sponsor
            .account
            .consumed
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let mut local = self
            .account
            .consumed
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        debug_assert!(
            shared.live_storage_cells >= cells
                && shared.live_payload_bytes >= payload_bytes
                && local.live_storage_cells >= cells
                && local.live_payload_bytes >= payload_bytes
        );
        shared.live_storage_cells -= cells;
        shared.live_payload_bytes -= payload_bytes;
        local.live_storage_cells -= cells;
        local.live_payload_bytes -= payload_bytes;
    }

    pub fn charge_result_custody(
        &self,
        cells: u64,
        payload_bytes: u64,
    ) -> Result<(), EvaluationAccountingError> {
        let (mut shared, mut local) = self.account.consumption()?;
        let aggregate_cells = shared.result_storage_cells.checked_add(cells).ok_or(
            EvaluationAccountingError::CounterOverflow(EvaluationResource::ResultCustody),
        )?;
        let aggregate_bytes = shared
            .result_payload_bytes
            .checked_add(payload_bytes)
            .ok_or(EvaluationAccountingError::CounterOverflow(
                EvaluationResource::ResultCustody,
            ))?;
        let invocation_cells = local.result_storage_cells.checked_add(cells).ok_or(
            EvaluationAccountingError::CounterOverflow(EvaluationResource::ResultCustody),
        )?;
        let invocation_bytes = local
            .result_payload_bytes
            .checked_add(payload_bytes)
            .ok_or(EvaluationAccountingError::CounterOverflow(
                EvaluationResource::ResultCustody,
            ))?;
        let limits = self.account.sponsor.limits();
        if aggregate_cells > limits.maximum_result_storage_cells()
            || aggregate_bytes > limits.maximum_result_payload_bytes()
        {
            return Err(EvaluationAccountingError::BudgetExhausted(
                EvaluationResource::ResultCustody,
            ));
        }
        shared.result_storage_cells = aggregate_cells;
        shared.result_payload_bytes = aggregate_bytes;
        local.result_storage_cells = invocation_cells;
        local.result_payload_bytes = invocation_bytes;
        Ok(())
    }

    pub fn snapshot(&self) -> Result<EvaluationInvocationUsage, EvaluationAccountingError> {
        let (shared, local) = self.account.consumption()?;
        let limits = self.account.sponsor.limits();
        let EvaluationMeasurementIdentity::TerminalPsi { work_schedule, .. } =
            self.account.measurement;
        let (usage_schema_version, step_schedule_marker) = (8, work_schedule.marker());
        Ok(EvaluationInvocationUsage {
            account: Arc::clone(&self.account),
            record: BuildEvaluationUsage {
                measurement: self.account.measurement,
                usage_schema_version,
                step_schedule_marker,
                invocation_work_budget: self.account.work_budget,
                sponsor_schema_version: Some(limits.schema_version()),
                session_work_budget: Some(limits.work_budget()),
                session_build_log_byte_ceiling: Some(limits.maximum_build_log_bytes()),
                session_filesystem_attempt_ceiling: Some(
                    limits.maximum_filesystem_operation_attempts(),
                ),
                session_live_filesystem_handle_ceiling: Some(
                    limits.maximum_live_filesystem_handles(),
                ),
                session_live_storage_cell_ceiling: Some(limits.maximum_live_storage_cells()),
                session_live_payload_byte_ceiling: Some(limits.maximum_live_payload_bytes()),
                session_result_storage_cell_ceiling: Some(limits.maximum_result_storage_cells()),
                session_result_payload_byte_ceiling: Some(limits.maximum_result_payload_bytes()),
                session_peak_live_filesystem_handles: shared.peak_live_filesystem_handles,
                session_peak_live_storage_cells: shared.peak_live_storage_cells,
                session_peak_live_payload_bytes: shared.peak_live_payload_bytes,
                work_units: local.work_units,
                build_log_bytes: local.build_log_bytes,
                filesystem_operation_attempts: local.filesystem_operation_attempts,
                peak_live_storage_cells: local.peak_live_storage_cells,
                peak_live_payload_bytes: local.peak_live_payload_bytes,
                result_storage_cells: local.result_storage_cells,
                result_payload_bytes: local.result_payload_bytes,
            },
        })
    }

    pub fn owns_usage(&self, usage: &EvaluationInvocationUsage) -> bool {
        Arc::ptr_eq(&self.account, &usage.account)
    }
}

#[derive(Debug, Clone)]
pub struct EvaluationInvocationUsage {
    account: Arc<InvocationAccount>,
    record: BuildEvaluationUsage,
}

impl EvaluationInvocationUsage {
    pub fn record(&self) -> &BuildEvaluationUsage {
        &self.record
    }
}

#[derive(Debug)]
pub struct EvaluationStorageLease {
    account: EvaluationInvocationAccount,
    cells: u64,
    payload_bytes: u64,
}

impl EvaluationStorageLease {
    pub fn absorb(&mut self, other: &mut Self) -> Result<(), EvaluationAccountingError> {
        if !self.account.same_invocation(&other.account) {
            return Err(EvaluationAccountingError::LeaseAccountMismatch);
        }
        let cells = self.cells.checked_add(other.cells).ok_or(
            EvaluationAccountingError::CounterOverflow(EvaluationResource::LiveStorage),
        )?;
        let payload_bytes = self.payload_bytes.checked_add(other.payload_bytes).ok_or(
            EvaluationAccountingError::CounterOverflow(EvaluationResource::LiveStorage),
        )?;
        self.cells = cells;
        self.payload_bytes = payload_bytes;
        other.cells = 0;
        other.payload_bytes = 0;
        Ok(())
    }

    pub fn truncate_cells(&mut self, retained: u64) {
        let retained = retained.min(self.cells);
        self.account.release_storage(self.cells - retained, 0);
        self.cells = retained;
    }

    pub fn grow(
        &mut self,
        cells: u64,
        payload_bytes: u64,
    ) -> Result<(), EvaluationAccountingError> {
        let new_cells =
            self.cells
                .checked_add(cells)
                .ok_or(EvaluationAccountingError::CounterOverflow(
                    EvaluationResource::LiveStorage,
                ))?;
        let new_bytes = self.payload_bytes.checked_add(payload_bytes).ok_or(
            EvaluationAccountingError::CounterOverflow(EvaluationResource::LiveStorage),
        )?;
        self.account.increase_site(0, cells, payload_bytes)?;
        self.cells = new_cells;
        self.payload_bytes = new_bytes;
        Ok(())
    }

    pub fn shrink(
        &mut self,
        cells: u64,
        payload_bytes: u64,
    ) -> Result<(), EvaluationAccountingError> {
        if cells > self.cells || payload_bytes > self.payload_bytes {
            return Err(EvaluationAccountingError::LeaseReleaseExceedsReservation);
        }
        self.account.release_storage(cells, payload_bytes);
        self.cells -= cells;
        self.payload_bytes -= payload_bytes;
        Ok(())
    }
}

impl Drop for EvaluationStorageLease {
    fn drop(&mut self) {
        self.account.release_storage(self.cells, self.payload_bytes);
    }
}

#[cfg(test)]
mod measurement_record {
    use super::EvaluationMeasurementIdentity;

    #[test]
    fn terminal_measurement_record_round_trips() {
        let limits = compiler_storage::RecordLimits::default();
        let identity = EvaluationMeasurementIdentity::current();
        let bytes =
            compiler_storage::encode(&identity, limits).expect("current measurement encodes");
        assert_eq!(
            compiler_storage::decode::<EvaluationMeasurementIdentity>(&bytes, limits).unwrap(),
            identity
        );
    }
}
