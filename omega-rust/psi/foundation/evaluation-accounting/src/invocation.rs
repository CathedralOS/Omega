//! Atomic invocation reservations within one aggregate sponsor account.
//! All two-account operations lock aggregate state before invocation state.
//! Leases are move-only; backing aliases share the owning lease through Arc.

use crate::{BuildEvaluationSponsor, BuildEvaluationUsage};
use semantic_vocabulary::FuelScheduleIdentity;
use std::sync::{Arc, Mutex, MutexGuard};

/// Resource category retained by a refused charge, not a parsed diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvaluationResource {
    Fuel,
    LiveStorage,
    ResultCustody,
    BuildLog,
    FilesystemAttempts,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvaluationAccountingError {
    UnsupportedMeasurementIdentity,
    MeasurementSponsorMismatch,
    ZeroInvocationFuelCeiling,
    AccountUnavailable,
    CounterOverflow(EvaluationResource),
    AllowanceExhausted(EvaluationResource),
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
            Self::ZeroInvocationFuelCeiling => {
                formatter.write_str("evaluation invocation fuel ceiling is zero")
            }
            Self::AccountUnavailable => formatter.write_str("evaluation account is unavailable"),
            Self::CounterOverflow(resource) => {
                write!(formatter, "evaluation {resource:?} accounting overflowed")
            }
            Self::AllowanceExhausted(resource) => {
                write!(formatter, "evaluation {resource:?} allowance exhausted")
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

/// Logical Terminal storage schedule, separate from its semantic fuel schedule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

/// Equal numeric schedule markers from different engines are different units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvaluationMeasurementIdentity {
    CheckedTree,
    TerminalPsi {
        fuel_schedule: FuelScheduleIdentity,
        storage_schedule: TerminalStorageScheduleIdentity,
    },
}

impl EvaluationMeasurementIdentity {
    pub const fn is_current(self) -> bool {
        match self {
            Self::CheckedTree => true,
            Self::TerminalPsi {
                fuel_schedule,
                storage_schedule,
            } => fuel_schedule.marker() == 1 && storage_schedule.marker() == 1,
        }
    }

    pub const fn is_supported(self, schema: u32, schedule: u32) -> bool {
        match self {
            Self::CheckedTree => schema == 7 && schedule == 1,
            Self::TerminalPsi {
                fuel_schedule,
                storage_schedule,
            } => {
                schema == 8
                    && schedule == fuel_schedule.marker()
                    && fuel_schedule.marker() == 1
                    && storage_schedule.marker() == 1
            }
        }
    }
}

#[derive(Debug, Default)]
struct InvocationConsumption {
    fuel_units: u64,
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
    fuel_ceiling: u64,
    consumed: Mutex<InvocationConsumption>,
}

/// Clone retains the same invocation, never a second allowance.
#[derive(Debug, Clone)]
pub struct EvaluationInvocationAccount {
    account: Arc<InvocationAccount>,
}

impl BuildEvaluationSponsor {
    pub fn owns_usage(&self, usage: &EvaluationInvocationUsage) -> bool {
        Arc::ptr_eq(&self.account, &usage.account.sponsor.account)
    }

    /// Establish one metered invocation. Storage ceilings remain shared across
    /// the entire closure; the invocation cannot exceed those same ceilings.
    pub fn begin_invocation(
        &self,
        measurement: EvaluationMeasurementIdentity,
        fuel_ceiling: u64,
    ) -> Result<EvaluationInvocationAccount, EvaluationAccountingError> {
        if fuel_ceiling == 0 {
            return Err(EvaluationAccountingError::ZeroInvocationFuelCeiling);
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
                fuel_ceiling,
                consumed: Mutex::new(InvocationConsumption::default()),
            }),
        })
    }
}

impl InvocationAccount {
    /// Locks this invocation's own and its sponsor's consumption counters; a
    /// poisoned account fails the charge closed.
    fn consumption(
        &self,
    ) -> Result<
        (
            MutexGuard<'_, crate::sponsor::BuildEvaluationSponsorConsumption>,
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
    /// Identity of the exact invocation, not equality of limits or sponsor.
    pub fn same_invocation(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.account, &other.account)
    }

    pub fn measurement(&self) -> EvaluationMeasurementIdentity {
        self.account.measurement
    }

    /// Call only at the engine's admitted pre-site charge boundary. Failed
    /// payment changes neither account; semantic execution follows payment.
    pub fn charge_fuel(&self, units: u64) -> Result<(), EvaluationAccountingError> {
        let (mut shared, mut local) = self.account.consumption()?;
        let aggregate = shared.fuel_units.checked_add(units).ok_or(
            EvaluationAccountingError::CounterOverflow(EvaluationResource::Fuel),
        )?;
        let invocation = local.fuel_units.checked_add(units).ok_or(
            EvaluationAccountingError::CounterOverflow(EvaluationResource::Fuel),
        )?;
        if aggregate > self.account.sponsor.limits().maximum_fuel_units()
            || invocation > self.account.fuel_ceiling
        {
            return Err(EvaluationAccountingError::AllowanceExhausted(
                EvaluationResource::Fuel,
            ));
        }
        shared.fuel_units = aggregate;
        local.fuel_units = invocation;
        Ok(())
    }

    /// Charge compiler-owned host output before publication. It has a shared
    /// byte unit independent of the engine's storage schedule.
    pub fn charge_build_log_bytes(&self, bytes: u64) -> Result<(), EvaluationAccountingError> {
        let (mut shared, mut local) = self.account.consumption()?;
        let aggregate = shared.build_log_bytes.checked_add(bytes).ok_or(
            EvaluationAccountingError::CounterOverflow(EvaluationResource::BuildLog),
        )?;
        let invocation = local.build_log_bytes.checked_add(bytes).ok_or(
            EvaluationAccountingError::CounterOverflow(EvaluationResource::BuildLog),
        )?;
        if aggregate > self.account.sponsor.limits().maximum_build_log_bytes() {
            return Err(EvaluationAccountingError::AllowanceExhausted(
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
            return Err(EvaluationAccountingError::AllowanceExhausted(
                EvaluationResource::FilesystemAttempts,
            ));
        }
        shared.filesystem_operation_attempts = aggregate;
        local.filesystem_operation_attempts = invocation;
        Ok(())
    }

    /// Reserve real owned storage before allocation or mutation. Row counts and
    /// payload bytes use the exact selected storage schedule, not heap capacity.
    pub fn reserve_storage(
        &self,
        cells: u64,
        payload_bytes: u64,
    ) -> Result<EvaluationStorageLease, EvaluationAccountingError> {
        self.reserve_site(0, cells, payload_bytes)
    }

    /// Stage a semantic site's fuel and storage together before execution.
    /// Resource refusal pays neither fuel nor storage; dropping the lease
    /// releases storage only, never refunds an executed site's fuel.
    pub fn reserve_site(
        &self,
        fuel_units: u64,
        cells: u64,
        payload_bytes: u64,
    ) -> Result<EvaluationStorageLease, EvaluationAccountingError> {
        self.increase_site(fuel_units, cells, payload_bytes)?;
        Ok(EvaluationStorageLease {
            account: self.clone(),
            cells,
            payload_bytes,
        })
    }

    fn increase_site(
        &self,
        fuel_units: u64,
        cells: u64,
        payload_bytes: u64,
    ) -> Result<(), EvaluationAccountingError> {
        let (mut shared, mut local) = self.account.consumption()?;
        let limits = self.account.sponsor.limits();
        let aggregate_fuel = shared.fuel_units.checked_add(fuel_units).ok_or(
            EvaluationAccountingError::CounterOverflow(EvaluationResource::Fuel),
        )?;
        let invocation_fuel = local.fuel_units.checked_add(fuel_units).ok_or(
            EvaluationAccountingError::CounterOverflow(EvaluationResource::Fuel),
        )?;
        if aggregate_fuel > limits.maximum_fuel_units()
            || invocation_fuel > self.account.fuel_ceiling
        {
            return Err(EvaluationAccountingError::AllowanceExhausted(
                EvaluationResource::Fuel,
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
            return Err(EvaluationAccountingError::AllowanceExhausted(
                EvaluationResource::LiveStorage,
            ));
        }
        shared.fuel_units = aggregate_fuel;
        local.fuel_units = invocation_fuel;
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

    /// Result publication is an aggregate consumed allowance. Dropping a result
    /// does not refund work already issued; a retry must pay again.
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
            return Err(EvaluationAccountingError::AllowanceExhausted(
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
        let (usage_schema_version, step_schedule_marker) = match self.account.measurement {
            EvaluationMeasurementIdentity::CheckedTree => (7, 1),
            EvaluationMeasurementIdentity::TerminalPsi { fuel_schedule, .. } => {
                (8, fuel_schedule.marker())
            }
        };
        Ok(EvaluationInvocationUsage {
            account: Arc::clone(&self.account),
            record: BuildEvaluationUsage {
                measurement: self.account.measurement,
                usage_schema_version,
                step_schedule_marker,
                invocation_fuel_ceiling: self.account.fuel_ceiling,
                sponsor_schema_version: Some(limits.schema_version()),
                session_fuel_ceiling: Some(limits.maximum_fuel_units()),
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
                fuel_units: local.fuel_units,
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

/// Live accounting observation coupled to the exact account that measured it.
/// Its durable record alone carries telemetry, never a transferable authority.
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
    /// Transfer a reservation without charging it again. On refusal both
    /// leases remain unchanged; even sibling invocations cannot merge custody.
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

    /// Retain at most this many cells, like truncating an owned collection.
    /// This can only retire this lease's cells; it never reserves growth or
    /// changes payload sponsorship. The owner calls after retiring its rows.
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
