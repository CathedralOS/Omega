/*
We provide deterministic resource accounting for compile-time build evaluations,
combining aggregate session sponsors (`BuildEvaluationSponsor`,
`BuildEvaluationSponsorLimits`), per-invocation accounts and movable RAII
storage leases (`EvaluationInvocationAccount`, `EvaluationStorageLease` in
`invocation`), and versioned usage telemetry records (`BuildEvaluationUsage`,
`BuildEvaluationUsageError`).
*/

/*
Compile-time evaluation on the Terminal Psi measurement requires
strict, deterministic resource bounds over logical work units, BuildLog bytes,
filesystem operation attempts, live filesystem handles, live storage cells, live
payload bytes, and retained result custody. Separating pure counter transactions
and RAII leases from interpreter state lets us enforce fail-closed pre-site
reservations where any refused charge leaves both aggregate and invocation
accounts unchanged.
*/

/*
Every sponsor and usage record binds an explicit `EvaluationMeasurementIdentity`
so a different schedule cannot spend or substitute for this account's budget.
`BuildEvaluationSponsor::charge_pure_usage`
atomically replays validated pure evaluation costs against a current sponsor
without importing the producer's account authority.
*/

#![forbid(unsafe_code)]

mod invocation;

use evaluation_contracts::{
    BuildEvaluationSponsorLimits, BuildEvaluationUsage, EvaluationAccountingError,
    EvaluationMeasurementIdentity,
};
pub use invocation::*;

use std::sync::{Arc, Mutex, MutexGuard};

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
        Self::new_with_measurement(limits, EvaluationMeasurementIdentity::current())
            .expect("the current Terminal Psi measurement is supported")
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
