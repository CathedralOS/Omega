// Normalize a completed Build supply into its locator receipt. This does not
// select a provider or admit a service schema; those require canonical authority.
use super::NativeBuildSupply;
use effects::provider_plan::{
    BuildNativeBindingOrigin, EvaluatedBindingEvaluationDigest,
    EvaluatedBindingMaterializationDigest, EvaluatedBindingProducerClosureDigest,
    EvaluatedBindingReceipt, EvaluatedBindingUsage, EvaluatedForeignImport,
    EvaluatedForeignSyscall,
};
use semantic_vocabulary::NativeBindingLocator;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeBuildRowOrigin {
    pub origin: package_compilation::PackageNativeBindingOrigin,
}

/// Retains the actual Build evaluation's target-specific locator and receipt.
/// The requirement identity must come from its canonical declaration; the result
/// alone grants no provider, signature, package, or execution admission.
pub fn normalize_native_binding(
    supply: &NativeBuildSupply,
    requirement: &str,
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
) -> Result<crate::evaluated_via_bindings::EvaluatedNativeBinding, String> {
    let target =
        target::TargetProfile::from_canonical_target_name(supply.selection.target.canonical_name())
            .map_err(|error| error.to_string())?;
    evaluate(supply, requirement, target, identities)
}

fn evaluate(
    supply: &NativeBuildSupply,
    requirement: &str,
    target: target::TargetProfile,
    identities: &mut terminal_psi::TerminalIdentityTableBuilder,
) -> Result<crate::evaluated_via_bindings::EvaluatedNativeBinding, String> {
    use crate::evaluated_via_bindings::EvaluatedNativeBinding;
    let selection = &supply.selection;
    let author_identity = &selection.producer.callable_identity;
    let source_occurrence_digest = selection.source_occurrence_digest;
    let (locator, binding_digest) = match &selection.locator {
        NativeBindingLocator::Import(raw) => {
            let locator = target::normalize_foreign_locator(raw.clone(), target)
                .map_err(|reason| format!("native Build locator: {reason}"))?;
            let digest = locator.identity_digest();
            (Some(locator), digest)
        }
        NativeBindingLocator::Syscall { number } => {
            if !matches!(
                target,
                target::TargetProfile::LinuxArm64 | target::TargetProfile::LinuxX64
            ) {
                return Err("native Build syscall is not applicable to its target".into());
            }
            let number = u32::try_from(*number)
                .map_err(|_| "native Build syscall number does not fit u32")?;
            (
                None,
                effects::provider_plan::evaluated_syscall_identity_digest(target, number),
            )
        }
    };
    let mut closure = crate::digest_encoder::DomainDigestEncoder::new(
        b"omega.native-build-producer-closure.v1\0",
    );
    closure.raw(
        supply
            .evaluation
            .terminal_code
            .program_fingerprint
            .as_bytes(),
    );
    closure.raw(&source_occurrence_digest);
    let closure = EvaluatedBindingProducerClosureDigest::from_bytes(closure.finish())?;
    let usage = supply.evaluation.usage;
    let (work_schedule, storage_schedule) = match usage.measurement {
        evaluation_accounting::EvaluationMeasurementIdentity::TerminalPsi {
            work_schedule,
            storage_schedule,
        } => (work_schedule.marker(), storage_schedule.marker()),
        _ => return Err("native Build evidence has a different evaluator measurement".into()),
    };
    let retained_usage = EvaluatedBindingUsage::from_evaluator(
        usage.usage_schema_version,
        usage.step_schedule_marker,
        usage.work_units,
        usage.invocation_work_budget,
        usage.build_log_bytes,
        usage.filesystem_operation_attempts,
        usage.peak_live_storage_cells,
        usage.peak_live_payload_bytes,
        usage.result_storage_cells,
        usage.result_payload_bytes,
    )?;
    let mut evaluation = crate::digest_encoder::DomainDigestEncoder::new(
        b"omega.native-build-binding-evaluation.v1\0",
    );
    evaluation.raw(&supply.evaluation.identity_digest());
    evaluation.raw(&source_occurrence_digest);
    evaluation.raw(&binding_digest.as_bytes());
    let evaluation = EvaluatedBindingEvaluationDigest::from_bytes(evaluation.finish())?;
    let mut materialization = crate::digest_encoder::DomainDigestEncoder::new(
        b"omega.native-build-binding-materialization.v1\0",
    );
    materialization.bytes(target.target_name().as_bytes());
    materialization.raw(&binding_digest.as_bytes());
    let materialization =
        EvaluatedBindingMaterializationDigest::from_bytes(materialization.finish())?;
    let requirement_identity = identities.intern(requirement);
    let receipt = EvaluatedBindingReceipt::from_evaluation(
        Some(selection.supplier_package),
        author_identity,
        identities,
        closure,
        work_schedule,
        retained_usage,
        evaluation,
        2,
        materialization,
        binding_digest,
    )?
    .with_build_origin(
        BuildNativeBindingOrigin {
            requirement_identity,
            source_occurrence_digest,
            terminal_code: supply.evaluation.terminal_code,
            work_schedule,
            storage_schedule,
            observation_digest: supply.evaluation.observation_digest,
        },
        identities.as_table(),
    )?;
    Ok(match (locator, &selection.locator) {
        (Some(locator), _) => EvaluatedNativeBinding::Import(
            EvaluatedForeignImport::from_retained_evidence(locator, receipt)?,
        ),
        (None, NativeBindingLocator::Syscall { number }) => EvaluatedNativeBinding::Syscall(
            EvaluatedForeignSyscall::from_retained_evidence(target, *number, receipt)?,
        ),
        _ => return Err("native Build materialization changed locator mechanism".into()),
    })
}
