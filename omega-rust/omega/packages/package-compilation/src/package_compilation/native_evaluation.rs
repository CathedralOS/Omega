// Completed Terminal Psi evaluation custody (`NativeBuildEvaluation`) for
// native Build bindings. The measurement is the Terminal Psi identity, and
// the path-independent `BuildObservationSummary` digest is committed alongside
// the invocation's own usage counters (excluding aggregate sponsorship so
// concurrent package billing cannot perturb the provider's semantic identity).

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
        Ok(Self {
            terminal_code,
            usage,
            observation_digest: observations.identity().digest(),
        })
    }

    pub fn identity_digest(&self) -> [u8; 32] {
        let mut digest = optimization_core::digest_encoder::DomainDigestEncoder::new(
            b"omega.native-build-evaluation.v1\0",
        );
        digest.raw(&self.terminal_code.vocabulary_marker.get().to_le_bytes());
        digest.raw(self.terminal_code.program_fingerprint.as_bytes());
        let usage = &self.usage;
        digest.raw(&usage.usage_schema_version.to_le_bytes());
        digest.raw(&usage.step_schedule_marker.to_le_bytes());
        let evaluation_accounting::EvaluationMeasurementIdentity::TerminalPsi {
            work_schedule,
            storage_schedule,
        } = usage.measurement;
        digest.raw(&work_schedule.marker().to_le_bytes());
        digest.raw(&storage_schedule.marker().to_le_bytes());
        for counter in [
            usage.invocation_work_budget,
            usage.work_units,
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
