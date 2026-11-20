//! Completed evaluation records that source settlement and native planning read.
//!
//! A record identifies the accepted code, observations and actual invocation
//! charges. It carries no package resolver, source tree or execution capability.
#![forbid(unsafe_code)]

mod history;
pub use history::{BuildCallSite, BuildExecutionHistory, BuildExecutionStep};
use record_storage::Record;
use std::sync::Arc;

// Completed Terminal Psi evaluation custody (`NativeBuildEvaluation`) for
// native Build bindings. The measurement is the Terminal Psi identity, and
// the path-independent `BuildObservationSummary` digest is committed alongside
// the invocation's own usage counters (excluding aggregate sponsorship so
// concurrent package billing cannot perturb the provider's semantic identity).

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeBuildEvaluation {
    pub terminal_code: psi_program_identity::TerminalPsiIdentity,
    pub usage: evaluation_contracts::BuildEvaluationUsage,
    pub observation_digest: [u8; 32],
    pub execution_history: Arc<BuildExecutionHistory>,
}

impl NativeBuildEvaluation {
    pub fn from_observation_digest(
        terminal_code: psi_program_identity::TerminalPsiIdentity,
        usage: evaluation_contracts::BuildEvaluationUsage,
        observation_digest: [u8; 32],
    ) -> Result<Self, String> {
        Self::from_execution(terminal_code, Arc::default(), usage, observation_digest)
    }

    pub fn from_execution(
        terminal_code: psi_program_identity::TerminalPsiIdentity,
        execution_history: Arc<BuildExecutionHistory>,
        usage: evaluation_contracts::BuildEvaluationUsage,
        observation_digest: [u8; 32],
    ) -> Result<Self, String> {
        let receipt = Self {
            execution_history,
            terminal_code,
            usage,
            observation_digest,
        };
        receipt.validate()?;
        Ok(receipt)
    }

    pub fn validate(&self) -> Result<(), String> {
        if !self.execution_history.ends_with(self.terminal_code) {
            return Err("Build history ends in a different checked program".into());
        }
        self.usage
            .validate()
            .map_err(|reason| format!("native Build evaluation usage: {reason:?}"))
    }

    pub fn identity_digest(&self) -> [u8; 32] {
        let mut digest = if self.execution_history.steps().is_empty() {
            wire::digest_encoder::DomainDigestEncoder::new(b"omega.native-build-evaluation.v1\0")
        } else {
            let mut digest = wire::digest_encoder::DomainDigestEncoder::new(
                b"omega.native-build-evaluation.v2\0",
            );
            self.execution_history
                .write_bytes(|bytes| digest.raw(bytes));
            digest
        };
        digest.raw(&self.terminal_code.vocabulary_marker.get().to_le_bytes());
        digest.raw(self.terminal_code.program_fingerprint.as_bytes());
        let usage = &self.usage;
        digest.raw(&usage.usage_schema_version.to_le_bytes());
        digest.raw(&usage.step_schedule_marker.to_le_bytes());
        let evaluation_contracts::EvaluationMeasurementIdentity::TerminalPsi {
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

// Existing package locks wrote the program, usage and observation consecutively.
// A zero usage schema cannot describe a valid execution and announces the extension.
impl Record for NativeBuildEvaluation {
    fn write(
        &self,
        out: &mut record_storage::RecordWriter,
    ) -> Result<(), record_storage::RecordError> {
        out.record(|out| {
            self.terminal_code.write(out)?;
            if self.execution_history.steps().is_empty() {
                self.usage.write(out)?;
                return self.observation_digest.write(out);
            }
            let mut usage = self.usage;
            usage.usage_schema_version = 0;
            usage.write(out)?;
            self.usage.usage_schema_version.write(out)?;
            self.observation_digest.write(out)?;
            self.execution_history.write(out)
        })
    }

    fn read<C>(
        input: &mut record_storage::RecordReader<'_, C>,
    ) -> Result<Self, record_storage::RecordError> {
        input.record(|input| {
            let code = psi_program_identity::TerminalPsiIdentity::read(input)?;
            let mut usage = evaluation_contracts::BuildEvaluationUsage::read(input)?;
            let staged = usage.usage_schema_version == 0;
            if staged {
                usage.usage_schema_version = u32::read(input)?;
            }
            let observations = <[u8; 32]>::read(input)?;
            let history = if staged {
                let history = Arc::<BuildExecutionHistory>::read(input)?;
                if history.steps().is_empty() {
                    return Err(input.error("extended Build receipt contains no execution history"));
                }
                history
            } else {
                let bytes = record_storage::shared_allocation_bytes(std::alloc::Layout::new::<
                    BuildExecutionHistory,
                >())
                .ok_or_else(|| input.error("Build history allocation overflow"))?;
                input.allocate(bytes)?;
                Arc::default()
            };
            Self::from_execution(code, history, usage, observations)
                .map_err(|_| input.error("invalid native Build execution receipt"))
        })
    }
}

/// The value and receipt of a completed native-binding invocation. Source
/// settlement joins its requirement first; native consumers receive no symbol
/// table handle, source span, or declaration lookup capability.
#[derive(Debug, Clone, PartialEq, Eq, record_storage::Record)]
pub struct NativeBindingMaterialization {
    pub requirement_identity: String,
    pub service_declaration: [u8; 32],
    pub target: String,
    pub locator: semantic_vocabulary::NativeBindingLocator,
    pub producer_identity: String,
    pub supplier_package: semantic_vocabulary::PackageKeyIdentity,
    pub source_occurrence_digest: [u8; 32],
    pub evaluation: std::sync::Arc<NativeBuildEvaluation>,
}
