//! Completed evaluation records that source settlement and native planning read.
//!
//! A record identifies the accepted code, observations and actual invocation
//! charges. It carries no package resolver, source tree or execution capability.
#![forbid(unsafe_code)]

mod dependencies;
#[cfg(test)]
mod tests;
pub use dependencies::DependencyPreparations;
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
    pub dependency_preparations: Arc<DependencyPreparations>,
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
        dependency_preparations: Arc<DependencyPreparations>,
        usage: evaluation_contracts::BuildEvaluationUsage,
        observation_digest: [u8; 32],
    ) -> Result<Self, String> {
        let receipt = Self {
            dependency_preparations,
            terminal_code,
            usage,
            observation_digest,
        };
        receipt.validate()?;
        Ok(receipt)
    }

    pub fn validate(&self) -> Result<(), String> {
        self.usage
            .validate()
            .map_err(|reason| format!("native Build evaluation usage: {reason:?}"))
    }

    pub fn identity_digest(&self) -> [u8; 32] {
        let mut digest =
            wire::digest_encoder::DomainDigestEncoder::new(b"omega.native-build-evaluation.v3\0");
        self.dependency_preparations.write_identity(&mut digest);
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

// The fixed tag distinguishes this layout before any variable-length fields.
const EVALUATION_RECORD_TAG: u32 = u32::from_le_bytes(*b"BER3");

impl Record for NativeBuildEvaluation {
    fn write(
        &self,
        out: &mut record_storage::RecordWriter,
    ) -> Result<(), record_storage::RecordError> {
        out.record(|out| {
            self.terminal_code.write(out)?;
            EVALUATION_RECORD_TAG.write(out)?;
            self.usage.write(out)?;
            self.observation_digest.write(out)?;
            self.dependency_preparations.write(out)
        })
    }

    fn read<C>(
        input: &mut record_storage::RecordReader<'_, C>,
    ) -> Result<Self, record_storage::RecordError> {
        input.record(|input| {
            let code = psi_program_identity::TerminalPsiIdentity::read(input)?;
            if u32::read(input)? != EVALUATION_RECORD_TAG {
                return Err(input.error("unrecognized Build evaluation receipt"));
            }
            let usage = evaluation_contracts::BuildEvaluationUsage::read(input)?;
            let observations = <[u8; 32]>::read(input)?;
            let preparations = Arc::<DependencyPreparations>::read(input)?;
            Self::from_execution(code, preparations, usage, observations)
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
