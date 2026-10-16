//! The settlement identities of builtin and admitted-provider boundary
//! traits.

use crate::NativeSelectedProviderPlanDigest;
pub(crate) use crate::OptimizedBoundaryOccurrence;
use crate::physical::derivation::hashing::{
    canonical_usize, hash_boundary_settlement_record, hash_boundary_structural_result, hash_bytes,
    hash_claim_source, hash_internal_scalar_argument_source, hash_port_effect_record,
    hash_structural_argument, hash_structural_parameter_declaration, hash_structural_path,
    hash_target,
};
use crate::{BoundaryInternalCallCustody, CompilerBuiltinResult, CompilerBuiltinScalarArgument};
use machine_code::PortEffectRecord;
use semantic_vocabulary::BoundaryMachineId;
use target::NativeTarget;
use target_operations::{CallSiteOwner, ProviderExecutionBinding};

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
    // Structural-result custody has its own domain; scalar-only and unit-result
    // custody share the original builtin domain.
    let mut digest = crate::digest_encoder::DomainDigestEncoder::new(match result {
        CompilerBuiltinResult::Unit => {
            b"omega.d41-boundary-trait-settlement.sha256.v1\0".as_slice()
        }
        CompilerBuiltinResult::Structural(_) => {
            b"omega.d41-boundary-trait-settlement.sha256.v3\0".as_slice()
        }
    });
    digest.raw(&occurrence.identity().bytes());
    hash_bytes(&mut digest, requirement_identity.as_bytes());
    digest.raw(selected_plan_digest.as_bytes());
    hash_target(&mut digest, target);
    digest.byte(1); // HostedV1 compiler-builtin catalog.
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
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
    scalar_argument: &CompilerBuiltinScalarArgument,
) -> Result<(), &'static str> {
    match scalar_argument {
        CompilerBuiltinScalarArgument::Immediate(scalar_argument) => {
            digest.raw(&scalar_argument.source_value.get().to_le_bytes());
            digest.byte(1); // exact signed i32 scalar schema
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
    mechanism: &target_operations::NormalizedForeignCallMechanism,
    structural_arguments: &[terminal_psi::StructuralArgument],
    structural_parameters: &[terminal_psi::StructuralParameterDeclaration],
) -> [u8; 32] {
    let mut digest = crate::digest_encoder::DomainDigestEncoder::new(
        b"omega.d41-boundary-trait-settlement.sha256.v7\0",
    );
    digest.raw(&occurrence.identity().bytes());
    hash_bytes(&mut digest, requirement_identity.as_bytes());
    digest.raw(selected_plan_digest.as_bytes());
    hash_target(&mut digest, target);
    digest.byte(2); // AdmittedProvider::NormalizedForeignCall.
    digest.raw(
        &execution
            .provider_plan_report_identity()
            .get()
            .to_le_bytes(),
    );
    digest.raw(&execution.provider_execution_report_identity().to_le_bytes());
    digest.raw(
        &execution
            .provider_execution_report_fingerprint()
            .to_le_bytes(),
    );
    digest.raw(&execution.normalized_root_report_identity().to_le_bytes());
    digest.raw(
        &execution
            .boundary_contract_report_fingerprint()
            .to_le_bytes(),
    );
    match mechanism {
        target_operations::NormalizedForeignCallMechanism::Table {
            target,
            byte_offset,
            dispatch_only,
            same_stack_contribution,
        } => {
            digest.byte(3);
            hash_bytes(&mut digest, target.target_name().as_bytes());
            digest.raw(&byte_offset.to_le_bytes());
            digest.byte(u8::from(*dispatch_only));
            digest.raw(&same_stack_contribution.commitment().as_bytes());
        }
        target_operations::NormalizedForeignCallMechanism::Import {
            locator,
            same_stack_contribution,
        } => {
            digest.byte(1);
            digest.raw(&locator.identity_digest().as_bytes());
            digest.raw(&same_stack_contribution.commitment().as_bytes());
        }
        target_operations::NormalizedForeignCallMechanism::SystemCall(kernel) => {
            digest.byte(2);
            hash_bytes(&mut digest, kernel.target.target_name().as_bytes());
            digest.raw(&kernel.number.to_le_bytes());
            digest.raw(&kernel.provider_plan_report_identity.to_le_bytes());
            digest.raw(&kernel.provider_plan_commitment);
            hash_bytes(&mut digest, kernel.requirement_identity.as_bytes());
        }
    }
    digest.raw(&boundary_plan_identity);
    digest.raw(&canonical_usize(structural_arguments.len()));
    for argument in structural_arguments {
        hash_structural_argument(&mut digest, argument);
    }
    digest.raw(&canonical_usize(structural_parameters.len()));
    for parameter in structural_parameters {
        hash_structural_parameter_declaration(&mut digest, parameter);
    }
    digest.finish()
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
    let mut digest = crate::digest_encoder::DomainDigestEncoder::new(
        b"omega.d41-boundary-trait-settlement.sha256.v4\0",
    );
    digest.raw(&occurrence.identity().bytes());
    hash_bytes(&mut digest, requirement_identity.as_bytes());
    digest.raw(selected_plan_digest.as_bytes());
    hash_target(&mut digest, target);
    digest.byte(3); // AdmittedProviderSettlement role.
    digest.raw(
        &execution
            .provider_plan_report_identity()
            .get()
            .to_le_bytes(),
    );
    digest.raw(&execution.provider_execution_report_identity().to_le_bytes());
    digest.raw(
        &execution
            .provider_execution_report_fingerprint()
            .to_le_bytes(),
    );
    digest.raw(&execution.normalized_root_report_identity().to_le_bytes());
    digest.raw(
        &execution
            .boundary_contract_report_fingerprint()
            .to_le_bytes(),
    );
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

/// Strong identity for one installed provider boundary call realized as a
/// resolved internal call into its selected checked-machine candidate. The
/// hash binds the boundary coordinate, the exact choosing declaration, the
/// provider label and candidate identity, and complete call-stack custody —
/// owner, callee machine, call-field offset, and the frame or caller-live
/// bytes its record family carries — so no coordinate can be substituted
/// underneath this parent identity.
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
    let mut digest = crate::digest_encoder::DomainDigestEncoder::new(
        b"omega.d41-boundary-trait-settlement.sha256.v6\0",
    );
    digest.raw(&occurrence.identity().bytes());
    hash_bytes(&mut digest, requirement_identity.as_bytes());
    digest.raw(selected_plan_digest.as_bytes());
    hash_target(&mut digest, target);
    digest.byte(4); // InstalledProviderInternalCall role.
    digest.raw(&boundary.get().to_le_bytes());
    hash_bytes(&mut digest, provider_declaration.as_bytes());
    hash_bytes(&mut digest, provider_identity.as_bytes());
    hash_bytes(&mut digest, candidate_identity.as_bytes());
    hash_internal_call_custody(&mut digest, call);
    digest.finish()
}

/// Encode physical call-stack custody independently from its selection authority.
pub(super) fn hash_internal_call_custody(
    digest: &mut crate::digest_encoder::DomainDigestEncoder,
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
    match owner {
        CallSiteOwner::Operation(operation) => {
            digest.byte(1);
            digest.raw(&operation.get().to_le_bytes());
            digest.raw(&0_u32.to_le_bytes());
        }
        CallSiteOwner::ExternalCompletion(edge) => {
            digest.byte(3);
            digest.raw(&edge.get().to_le_bytes());
            digest.raw(&0_u32.to_le_bytes());
        }
        CallSiteOwner::CleanupAction {
            edge,
            action_ordinal,
        } => {
            digest.byte(2);
            digest.raw(&edge.get().to_le_bytes());
            digest.raw(&action_ordinal.to_le_bytes());
        }
    }
    digest.raw(&call_target.get().to_le_bytes());
    digest.raw(&canonical_usize(call_field_offset));
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

/// Bind family selection to the authentic portable head and the same emitted
/// internal-call custody used by provider calls, without a provider plan.
#[allow(clippy::too_many_arguments)]
pub(super) fn target_family_internal_call_identity(
    terminal: terminal_psi::TerminalPsiIdentity,
    occurrence: &OptimizedBoundaryOccurrence,
    head_identity: &str,
    head_contract: &[u8; 32],
    member: &terminal_psi::TargetMachineFamilyMember,
    target: NativeTarget,
    completion_claim_sources: &[target_operations::CompletionClaimSource],
    completion_receipts: &[terminal_psi::CompletionReceipt],
    call: &BoundaryInternalCallCustody,
) -> Result<[u8; 32], &'static str> {
    let terminal_psi::TargetMachineCallable::Machine(machine) = member.callable else {
        return Err("target-family internal-call identity requires a checked machine");
    };
    let supply_tag = match member.supply {
        terminal_psi::TargetMachineSupply::CheckedBody => 1,
        terminal_psi::TargetMachineSupply::Boundary => 2,
        _ => return Err("target-family internal-call identity requires executable supply"),
    };
    let mut digest = crate::digest_encoder::DomainDigestEncoder::new(
        b"omega.d41-target-family-internal-call.sha256.v1\0",
    );
    digest.raw(&crate::physical::identity_bytes::terminal_identity_bytes(
        terminal,
    ));
    digest.raw(&occurrence.identity().bytes());
    hash_bytes(&mut digest, head_identity.as_bytes());
    digest.raw(head_contract);
    hash_target(&mut digest, target);
    digest.raw(&occurrence.boundary().get().to_le_bytes());
    hash_bytes(&mut digest, member.target.as_bytes());
    digest.raw(&machine.get().to_le_bytes());
    digest.byte(supply_tag);
    digest.u64(completion_claim_sources.len() as u64);
    for source in completion_claim_sources {
        hash_claim_source(&mut digest, source);
    }
    digest.u64(completion_receipts.len() as u64);
    for receipt in completion_receipts {
        digest.raw(&receipt.claim.get().to_le_bytes());
        digest.raw(&receipt.argument_index.to_le_bytes());
        hash_structural_path(&mut digest, &receipt.path);
    }
    hash_internal_call_custody(&mut digest, call);
    Ok(digest.finish())
}
