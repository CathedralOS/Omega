//! The settlement identities of builtin and admitted-provider boundary
//! traits.

use crate::NativeSelectedProviderPlanDigest;
pub(crate) use crate::OptimizedBoundaryOccurrence;
use crate::physical::derivation::hashing::{
    canonical_usize, hash_boundary_settlement_record, hash_bytes, hash_port_effect_record,
    hash_structural_argument, hash_structural_parameter_declaration, hash_structural_path,
    hash_sum_layout, hash_target,
};
use crate::{CompilerBuiltinResult, CompilerBuiltinScalarArgument};
use machine_code::PortEffectRecord;
use sha2::Digest;
use sha2::Sha256;
use target::NativeTarget;
use target_operations::ProviderExecutionBinding;

/// Strong identity for one hosted builtin settlement. The derivation binds
/// the builtin's catalog row — `identity_tags` name its closed execution and
/// realization pair — plus the custody shapes the settlement record actually
/// retained: the scalar argument when present, and the structural result when
/// the builtin declares one. A further hosted builtin needs only a fresh tag
/// pair and its bound custody, not a new identity function.
pub(crate) fn hosted_builtin_settlement_identity(
    occurrence: &OptimizedBoundaryOccurrence,
    requirement_identity: &str,
    selected_plan_digest: NativeSelectedProviderPlanDigest,
    target: NativeTarget,
    identity_tags: [u8; 2],
    scalar_argument: Option<&CompilerBuiltinScalarArgument>,
    result: &CompilerBuiltinResult,
) -> Result<[u8; 32], &'static str> {
    let mut digest = Sha256::new();
    // Structural-result custody has its own domain; scalar-only and unit-result
    // custody share the original builtin domain.
    digest.update(match result {
        CompilerBuiltinResult::Unit => {
            b"omega.d41-boundary-trait-settlement.sha256.v1\0".as_slice()
        }
        CompilerBuiltinResult::Structural(_) => {
            b"omega.d41-boundary-trait-settlement.sha256.v3\0".as_slice()
        }
    });
    digest.update(occurrence.identity().bytes());
    hash_bytes(&mut digest, requirement_identity.as_bytes());
    digest.update(selected_plan_digest.as_bytes());
    hash_target(&mut digest, target);
    digest.update([1]); // HostedV1 compiler-builtin catalog.
    digest.update(identity_tags);
    if let Some(scalar_argument) = scalar_argument {
        hash_builtin_scalar_argument(&mut digest, scalar_argument)?;
    }
    if let CompilerBuiltinResult::Structural(result) = result {
        hash_builtin_structural_result(&mut digest, result)?;
    }
    Ok(digest.finalize().into())
}

fn hash_builtin_scalar_argument(
    digest: &mut Sha256,
    scalar_argument: &CompilerBuiltinScalarArgument,
) -> Result<(), &'static str> {
    match scalar_argument {
        CompilerBuiltinScalarArgument::Immediate(scalar_argument) => {
            digest.update(scalar_argument.source_value.get().to_le_bytes());
            digest.update([1]); // exact signed i32 scalar schema
            let semantic_vocabulary::IntegerValue::Signed(value) = scalar_argument.immediate else {
                return Err("hosted builtin settlement argument requires a signed i32 immediate");
            };
            digest.update(
                i32::try_from(value)
                    .map_err(
                        |_| "hosted builtin settlement argument requires a signed i32 immediate",
                    )?
                    .to_le_bytes(),
            );
        }
        CompilerBuiltinScalarArgument::RuntimeScalar(scalar_argument) => {
            digest.update(scalar_argument.parameter_index.to_le_bytes());
            hash_runtime_scalar_source(digest, &scalar_argument.source);
        }
    }
    Ok(())
}

fn hash_runtime_scalar_source(
    digest: &mut Sha256,
    source: &machine_code::InternalUnitScalarArgumentSourceRecord,
) {
    match source {
        machine_code::InternalUnitScalarArgumentSourceRecord::SelectedProcessExit {
            source_value,
            instruction,
            ..
        } => {
            digest.update([6]);
            digest.update(source_value.get().to_le_bytes());
            digest.update(instruction.0.to_le_bytes());
        }
        machine_code::InternalUnitScalarArgumentSourceRecord::SelectedCall {
            source_value,
            instruction,
            ..
        } => {
            digest.update([5]);
            digest.update(source_value.get().to_le_bytes());
            digest.update(instruction.0.to_le_bytes());
        }
        machine_code::InternalUnitScalarArgumentSourceRecord::SelectedBoundary {
            source_value,
            instruction,
            scratch_byte_offset,
            ..
        } => {
            digest.update([4]);
            digest.update(source_value.get().to_le_bytes());
            digest.update(instruction.0.to_le_bytes());
            digest.update(scratch_byte_offset.to_le_bytes());
        }
        machine_code::InternalUnitScalarArgumentSourceRecord::Parameter {
            parameter_index,
            source_value,
            ..
        } => {
            digest.update([0]);
            digest.update(parameter_index.to_le_bytes());
            digest.update(source_value.get().to_le_bytes());
        }
        machine_code::InternalUnitScalarArgumentSourceRecord::IntegerImmediate {
            defining_operation,
            source_value,
            value,
            ..
        } => {
            digest.update([1]);
            digest.update(defining_operation.get().to_le_bytes());
            digest.update(source_value.get().to_le_bytes());
            match value {
                semantic_vocabulary::IntegerValue::Signed(value) => {
                    digest.update(value.to_le_bytes())
                }
                semantic_vocabulary::IntegerValue::Unsigned(value) => {
                    digest.update(value.to_le_bytes())
                }
            }
        }
        machine_code::InternalUnitScalarArgumentSourceRecord::BooleanImmediate {
            defining_operation,
            source_value,
            value,
            definition_ordinal,
        } => {
            digest.update([3]);
            digest.update(defining_operation.get().to_le_bytes());
            digest.update(source_value.get().to_le_bytes());
            digest.update([u8::from(*value)]);
            digest.update(
                u64::try_from(*definition_ordinal)
                    .expect("validated definition ordinal is u64-representable")
                    .to_le_bytes(),
            );
        }
        machine_code::InternalUnitScalarArgumentSourceRecord::Home(home) => {
            digest.update([2]);
            digest.update(home.defining_operation.get().to_le_bytes());
            digest.update(home.source_value.get().to_le_bytes());
            digest.update(home.byte_offset.to_le_bytes());
        }
    }
}

fn hash_builtin_structural_result(
    digest: &mut Sha256,
    result: &machine_code::BoundaryStructuralResultRecord,
) -> Result<(), &'static str> {
    digest.update(result.defining_operation.get().to_le_bytes());
    digest.update(result.result.place.get().to_le_bytes());
    digest.update(result.result.structural_type.get().to_le_bytes());
    digest.update([match result.result.multiplicity {
        terminal_psi::StructuralMultiplicity::Unrestricted => 1,
        terminal_psi::StructuralMultiplicity::Affine => 2,
        terminal_psi::StructuralMultiplicity::Linear => 3,
    }]);
    digest.update((result.result.qualifications.len() as u64).to_le_bytes());
    for domain in &result.result.qualifications {
        digest.update(domain.get().to_le_bytes());
    }
    digest.update((result.result.projected_qualifications.len() as u64).to_le_bytes());
    for qualification in &result.result.projected_qualifications {
        hash_structural_path(digest, &qualification.path);
        digest.update(qualification.domain.get().to_le_bytes());
    }
    digest.update((result.result.claims.len() as u64).to_le_bytes());
    for claim in &result.result.claims {
        digest.update(claim.claim.get().to_le_bytes());
        hash_structural_path(digest, &claim.path);
    }
    let declaration = terminal_codec::encode_structural_type_declaration(&result.declaration)
        .map_err(
            |_| "hosted builtin settlement result declaration cannot be encoded canonically",
        )?;
    hash_bytes(digest, &declaration);
    hash_sum_layout(digest, &result.layout);
    digest.update(result.home_byte_offset.to_le_bytes());
    Ok(())
}

/// Strong identity for one exact normalized foreign call. Besides the
/// observed execution, locator, evaluated boundary-plan commitment, and
/// same-stack custody, the identity binds the call's expected structural
/// custody: every authored structural argument row (place, access, path) and
/// every declared structural formal row (position, place, self-ness, type,
/// multiplicity, access, qualifications). The observed plan commitment alone
/// cannot carry those semantic coordinates — no field can be substituted
/// underneath this parent identity.
#[allow(clippy::too_many_arguments)]
pub(crate) fn admitted_provider_boundary_trait_settlement_identity(
    occurrence: &OptimizedBoundaryOccurrence,
    requirement_identity: &str,
    selected_plan_digest: NativeSelectedProviderPlanDigest,
    target: NativeTarget,
    execution: ProviderExecutionBinding,
    boundary_plan_identity: [u8; 32],
    locator: &target::NormalizedForeignLocator,
    same_stack_identity: [u8; 32],
    structural_arguments: &[terminal_psi::StructuralArgument],
    structural_parameters: &[terminal_psi::StructuralParameterDeclaration],
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"omega.d41-boundary-trait-settlement.sha256.v2\0");
    digest.update(occurrence.identity().bytes());
    hash_bytes(&mut digest, requirement_identity.as_bytes());
    digest.update(selected_plan_digest.as_bytes());
    hash_target(&mut digest, target);
    digest.update([2]); // AdmittedProvider::NormalizedForeignCall.
    digest.update(
        execution
            .provider_plan_report_identity()
            .get()
            .to_le_bytes(),
    );
    digest.update(execution.provider_execution_report_identity().to_le_bytes());
    digest.update(
        execution
            .provider_execution_report_fingerprint()
            .to_le_bytes(),
    );
    digest.update(execution.normalized_root_report_identity().to_le_bytes());
    digest.update(
        execution
            .boundary_contract_report_fingerprint()
            .to_le_bytes(),
    );
    digest.update(locator.identity_digest().as_bytes());
    digest.update(boundary_plan_identity);
    digest.update(same_stack_identity);
    digest.update(canonical_usize(structural_arguments.len()));
    for argument in structural_arguments {
        hash_structural_argument(&mut digest, argument);
    }
    digest.update(canonical_usize(structural_parameters.len()));
    for parameter in structural_parameters {
        hash_structural_parameter_declaration(&mut digest, parameter);
    }
    digest.finalize().into()
}

/// Strong identity for one exact installed provider settlement realized in
/// place. The hash binds the complete retained settlement row — execution,
/// realization, scalar/structural/byte-sequence argument custody, completion
/// claim sources, receipts, provider custody, result placement, and source
/// coordinates — plus the joined privileged port effect when one exists.
#[allow(clippy::too_many_arguments)]
pub(crate) fn admitted_provider_settlement_identity(
    occurrence: &OptimizedBoundaryOccurrence,
    requirement_identity: &str,
    selected_plan_digest: NativeSelectedProviderPlanDigest,
    target: NativeTarget,
    execution: ProviderExecutionBinding,
    settlement: &machine_code::BoundarySettlementRecord,
    port_effect: Option<&PortEffectRecord>,
) -> Result<[u8; 32], &'static str> {
    let mut digest = Sha256::new();
    digest.update(b"omega.d41-boundary-trait-settlement.sha256.v4\0");
    digest.update(occurrence.identity().bytes());
    hash_bytes(&mut digest, requirement_identity.as_bytes());
    digest.update(selected_plan_digest.as_bytes());
    hash_target(&mut digest, target);
    digest.update([3]); // AdmittedProviderSettlement role.
    digest.update(
        execution
            .provider_plan_report_identity()
            .get()
            .to_le_bytes(),
    );
    digest.update(execution.provider_execution_report_identity().to_le_bytes());
    digest.update(
        execution
            .provider_execution_report_fingerprint()
            .to_le_bytes(),
    );
    digest.update(execution.normalized_root_report_identity().to_le_bytes());
    digest.update(
        execution
            .boundary_contract_report_fingerprint()
            .to_le_bytes(),
    );
    hash_boundary_settlement_record(&mut digest, settlement)?;
    match port_effect {
        None => digest.update([0]),
        Some(effect) => {
            digest.update([1]);
            hash_port_effect_record(&mut digest, effect);
        }
    }
    Ok(digest.finalize().into())
}
