/*
We compute the canonical SHA-256 parent identities for hosted builtin
settlements, admitted provider normalized foreign calls, in-place provider
settlements and installed provider internal calls.
*/

use crate::NativeSelectedProviderPlanDigest;
pub(crate) use crate::OptimizedBoundaryOccurrence;
use crate::digest_encoder::DomainDigestEncoder;
use crate::physical::derivation::hashing::{
    hash_boundary_settlement_record, hash_boundary_structural_result, hash_call_site_owner,
    hash_internal_scalar_argument_source, hash_normalized_foreign_mechanism,
    hash_port_effect_record, hash_provider_execution_record, hash_structural_argument,
    hash_structural_parameter_declaration, hash_target,
};
use crate::{BoundaryInternalCallCustody, CompilerBuiltinResult, CompilerBuiltinScalarArgument};
use machine_code::PortEffectRecord;
use semantic_vocabulary::BoundaryMachineId;
use target::NativeTarget;
use target_vocabulary::ProviderExecutionBinding;

pub(crate) fn hosted_builtin_settlement_identity(
    occurrence: &OptimizedBoundaryOccurrence,
    requirement_identity: &str,
    selected_plan_digest: NativeSelectedProviderPlanDigest,
    target: NativeTarget,
    identity_tags: [u8; 2],
    scalar_argument: Option<&CompilerBuiltinScalarArgument>,
    result: &CompilerBuiltinResult,
) -> Result<[u8; 32], &'static str> {
    let mut digest = DomainDigestEncoder::new(match result {
        CompilerBuiltinResult::Unit => {
            b"omega.d41-boundary-trait-settlement.sha256.v1\0".as_slice()
        }
        CompilerBuiltinResult::Structural(_) => {
            b"omega.d41-boundary-trait-settlement.sha256.v3\0".as_slice()
        }
    });
    digest.raw(&occurrence.identity().bytes());
    digest.bytes(requirement_identity.as_bytes());
    digest.raw(selected_plan_digest.as_bytes());
    hash_target(&mut digest, target);
    digest.byte(1);
    digest.raw(&identity_tags);
    if let Some(scalar_argument) = scalar_argument {
        hash_builtin_scalar_argument(&mut digest, scalar_argument)?;
    }
    if let CompilerBuiltinResult::Structural(result) = result {
        hash_boundary_structural_result(&mut digest, result).map_err(
            |_| "hosted builtin settlement result declaration cannot be encoded canonically",
        )?;
    }
    Ok(digest.finish())
}

fn hash_builtin_scalar_argument(
    digest: &mut DomainDigestEncoder,
    scalar_argument: &CompilerBuiltinScalarArgument,
) -> Result<(), &'static str> {
    match scalar_argument {
        CompilerBuiltinScalarArgument::Immediate(scalar_argument) => {
            digest.raw(&scalar_argument.source_value.get().to_le_bytes());
            digest.byte(1);
            let semantic_vocabulary::IntegerValue::Signed(value) = scalar_argument.immediate else {
                return Err("hosted builtin settlement argument requires a signed i32 immediate");
            };
            digest.raw(
                &i32::try_from(value)
                    .map_err(
                        |_| "hosted builtin settlement argument requires a signed i32 immediate",
                    )?
                    .to_le_bytes(),
            );
        }
        CompilerBuiltinScalarArgument::RuntimeScalar(scalar_argument) => {
            digest.raw(&scalar_argument.parameter_index.to_le_bytes());
            hash_internal_scalar_argument_source(digest, &scalar_argument.source);
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn admitted_provider_boundary_trait_settlement_identity(
    occurrence: &OptimizedBoundaryOccurrence,
    requirement_identity: &str,
    selected_supply: crate::physical::NativeSelectedSupply,
    target: NativeTarget,
    execution: target_vocabulary::ExternalExecutionBinding,
    boundary_plan_identity: [u8; 32],
    mechanism: &target_vocabulary::NormalizedForeignCallMechanism,
    structural_arguments: &[terminal_psi::StructuralArgument],
    structural_parameters: &[terminal_psi::StructuralParameterDeclaration],
) -> [u8; 32] {
    let mut digest = DomainDigestEncoder::new(b"omega.d41-boundary-trait-settlement.sha256.v7\0");
    digest.raw(&occurrence.identity().bytes());
    digest.bytes(requirement_identity.as_bytes());
    match selected_supply {
        crate::physical::NativeSelectedSupply::ProviderPlan(plan) => {
            digest.byte(0);
            digest.raw(plan.as_bytes());
        }
        crate::physical::NativeSelectedSupply::NativeBinding(binding) => {
            digest.byte(1);
            digest.raw(&binding);
        }
    }
    hash_target(&mut digest, target);
    digest.byte(2);
    let mut execution_bytes = Vec::new();
    target_vocabulary::encode_external_execution(&mut execution_bytes, execution);
    digest.raw(&execution_bytes);
    if let Some(contribution) = hash_normalized_foreign_mechanism(&mut digest, mechanism) {
        digest.raw(&contribution.commitment().as_bytes());
    }
    digest.raw(&boundary_plan_identity);
    digest.len(structural_arguments.len());
    for argument in structural_arguments {
        hash_structural_argument(&mut digest, argument);
    }
    digest.len(structural_parameters.len());
    for parameter in structural_parameters {
        hash_structural_parameter_declaration(&mut digest, parameter);
    }
    digest.finish()
}

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
    let mut digest = DomainDigestEncoder::new(b"omega.d41-boundary-trait-settlement.sha256.v4\0");
    digest.raw(&occurrence.identity().bytes());
    digest.bytes(requirement_identity.as_bytes());
    digest.raw(selected_plan_digest.as_bytes());
    hash_target(&mut digest, target);
    digest.byte(3);
    hash_provider_execution_record(&mut digest, &execution.into());
    hash_boundary_settlement_record(&mut digest, settlement)?;
    match port_effect {
        None => digest.byte(0),
        Some(effect) => {
            digest.byte(1);
            hash_port_effect_record(&mut digest, effect);
        }
    }
    Ok(digest.finish())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn installed_provider_internal_call_identity(
    occurrence: &OptimizedBoundaryOccurrence,
    requirement_identity: &str,
    selected_plan_digest: NativeSelectedProviderPlanDigest,
    target: NativeTarget,
    boundary: BoundaryMachineId,
    provider_declaration: &str,
    provider_identity: &str,
    candidate_identity: &str,
    call: &BoundaryInternalCallCustody,
) -> [u8; 32] {
    let mut digest = DomainDigestEncoder::new(b"omega.d41-boundary-trait-settlement.sha256.v6\0");
    digest.raw(&occurrence.identity().bytes());
    digest.bytes(requirement_identity.as_bytes());
    digest.raw(selected_plan_digest.as_bytes());
    hash_target(&mut digest, target);
    digest.byte(4);
    digest.raw(&boundary.get().to_le_bytes());
    digest.bytes(provider_declaration.as_bytes());
    digest.bytes(provider_identity.as_bytes());
    digest.bytes(candidate_identity.as_bytes());
    hash_internal_call_custody(&mut digest, call);
    digest.finish()
}

pub(super) fn hash_internal_call_custody(
    digest: &mut DomainDigestEncoder,
    call: &BoundaryInternalCallCustody,
) {
    let (family_tag, owner, call_target, call_field_offset, frame_terms, caller_live_bytes) =
        match call {
            BoundaryInternalCallCustody::Unit {
                owner,
                target,
                call_field_offset,
                active_frame_bytes,
                transient_bytes,
                caller_live_bytes,
            } => (
                1u8,
                owner,
                *target,
                *call_field_offset,
                Some((*active_frame_bytes, *transient_bytes)),
                *caller_live_bytes,
            ),
            BoundaryInternalCallCustody::Scalar {
                owner,
                target,
                call_field_offset,
                caller_live_bytes,
            } => (
                2u8,
                owner,
                *target,
                *call_field_offset,
                None,
                *caller_live_bytes,
            ),
        };
    digest.byte(family_tag);
    hash_call_site_owner(digest, owner);
    digest.raw(&call_target.get().to_le_bytes());
    digest.len(call_field_offset);
    match frame_terms {
        None => digest.byte(0),
        Some((active_frame_bytes, transient_bytes)) => {
            digest.byte(1);
            digest.raw(&active_frame_bytes.to_le_bytes());
            digest.raw(&transient_bytes.to_le_bytes());
        }
    }
    digest.raw(&caller_live_bytes.to_le_bytes());
}
