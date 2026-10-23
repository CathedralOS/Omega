//! Replay logical cost without replaying execution or importing its account.
//! A validated record describes work, not authority. The caller must establish
//! exact result identity and current admission before spending this sponsor.

use crate::{BuildEvaluationSponsor, BuildEvaluationUsage};

impl BuildEvaluationSponsor {
    /// Charge an output-free cached evaluation's recorded cost atomically.
    /// Engine/schedule identities must match. Filesystem-observing evaluations
    /// require their own custody protocol and cannot use this operation.
    ///
    /// Cumulative fuel, log and result custody are spent again. Recorded live
    /// demand is admitted beside current reservations and immediately retired;
    /// no execution storage is allocated. Returned telemetry retains invocation
    /// counts with this sponsor's policy and peaks, never the producer's account.
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
        // Calculate every axis before changing any counters. A result-custody
        // refusal cannot spend fuel, and concurrent hits cannot both spend the
        // same remaining allowance. Historical aggregate peaks are not costs.
        let fuel = charged_total(
            consumed.fuel_units,
            usage.fuel_units,
            limits.maximum_fuel_units(),
            "aggregate fuel",
            "fuel units",
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
        usage.session_fuel_ceiling = Some(limits.maximum_fuel_units());
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
        consumed.fuel_units = fuel;
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
