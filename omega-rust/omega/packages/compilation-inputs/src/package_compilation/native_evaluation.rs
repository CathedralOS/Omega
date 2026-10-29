//! Completed Build custody, independent of source symbol selection.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeBuildEvaluation {
    pub terminal_code: terminal_psi::TerminalPsiIdentity,
    pub usage: crate::BuildEvaluationUsage,
    pub observation_digest: [u8; 32],
}

impl NativeBuildEvaluation {
    pub fn from_execution(
        terminal_code: terminal_psi::TerminalPsiIdentity,
        usage: crate::BuildEvaluationUsage,
        observations: &crate::BuildObservationSummary,
    ) -> Result<Self, String> {
        usage
            .validate()
            .map_err(|reason| format!("native Build evaluation usage: {reason:?}"))?;
        // Never relabel a tree interpreter measurement as Terminal units.
        if !matches!(
            usage.measurement,
            evaluation_accounting::EvaluationMeasurementIdentity::TerminalPsi { .. }
        ) {
            return Err("native Build bindings require their actual Terminal evaluation".into());
        }
        // Use the existing path-independent observation commitment. The
        // report codec also contains physical host roots and cannot name a
        // portable native provider.
        Ok(Self {
            terminal_code,
            usage,
            observation_digest: observations.identity().digest(),
        })
    }

    /// Code and invocation observations retained by the candidate. Aggregate
    /// sponsorship is still reported in `usage`; another package's concurrent
    /// billing must not change this provider's semantic identity.
    pub fn identity_digest(&self) -> [u8; 32] {
        let mut digest = optimization_core::digest_encoder::DomainDigestEncoder::new(
            b"omega.native-build-evaluation.v1\0",
        );
        digest.raw(&self.terminal_code.vocabulary_marker.get().to_le_bytes());
        digest.raw(self.terminal_code.program_fingerprint.as_bytes());
        let usage = &self.usage;
        digest.raw(&usage.usage_schema_version.to_le_bytes());
        digest.raw(&usage.step_schedule_marker.to_le_bytes());
        if let evaluation_accounting::EvaluationMeasurementIdentity::TerminalPsi {
            fuel_schedule,
            storage_schedule,
        } = usage.measurement
        {
            digest.raw(&fuel_schedule.marker().to_le_bytes());
            digest.raw(&storage_schedule.marker().to_le_bytes());
        }
        for counter in [
            usage.invocation_fuel_ceiling,
            usage.fuel_units,
            usage.build_log_bytes,
            usage.filesystem_operation_attempts,
            usage.peak_live_storage_cells,
            usage.peak_live_payload_bytes,
            usage.result_storage_cells,
            usage.result_payload_bytes,
        ] {
            digest.u64(counter);
        }
        digest.raw(&self.observation_digest);
        digest.finish()
    }
}
