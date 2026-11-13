/*
Canonical mechanism identities for normalized-foreign imports and direct syscalls.

`normalized_foreign_terminal_mechanism` and
`normalized_foreign_terminal_mechanism_with_callback_materializations`
reconstruct the normalized-foreign role from the retained locator and canonical
admitted boundary entry plan, rejecting any plan whose revalidation rewrites the
entry plan.

`conservative_syscall_terminal_mechanism` derives the conservative checked
syscall argument contract from one verified target-neutral boundary signature on
Linux x86-64/ARM64, committing the complete parameter carrier and access
contract while rejecting boundary structural-domain requirements or
qualifications.
*/

use calling_conventions::{BoundaryEntryPlan, CallSignature};
use effects::{
    CheckedSyscallArgumentContractIdentity, SyscallTerminalMechanismIdentity,
    TerminalMechanismIdentity,
};
use semantic_vocabulary::{IeeeFloatFormat, IntegerCarrier, IntegerSign, ScalarType};
use terminal_psi::{StructuralAccess, StructuralMultiplicity};

use crate::digest_encoder::DomainDigestEncoder;

const CONSERVATIVE_ARGUMENT_CONTRACT_DOMAIN: &[u8] =
    b"omega.checked-syscall-argument-contract.conservative-unqualified.v2\0";

pub(crate) fn normalized_foreign_terminal_mechanism(
    locator: &target::NormalizedForeignLocator,
    boundary_entry_plan: &BoundaryEntryPlan,
) -> Result<TerminalMechanismIdentity, String> {
    let signature = entry_call_signature(boundary_entry_plan);
    let validated =
        calling_conventions::validate_boundary_entry_plan(boundary_entry_plan.clone(), &signature)
            .map_err(|diagnostic| diagnostic.to_string())?;
    require_canonical_plan(
        &validated,
        boundary_entry_plan,
        "normalized foreign boundary plan is not canonical",
    )?;
    Ok(
        effects::NormalizedForeignTerminalMechanismIdentity::from_normalized_locator(
            locator,
            effects::provider_plan::BoundaryCallingPlanCommitment::from_digest(
                validated.contract_commitment_digest(),
            ),
        )
        .into(),
    )
}

pub(crate) fn normalized_foreign_terminal_mechanism_with_callback_materializations(
    locator: &target::NormalizedForeignLocator,
    boundary_entry_plan: &BoundaryEntryPlan,
    context: &calling_conventions::CallbackMaterializationContext,
) -> Result<TerminalMechanismIdentity, String> {
    let signature = entry_call_signature(boundary_entry_plan);
    let validated =
        calling_conventions::validate_boundary_entry_plan_with_callback_materializations(
            boundary_entry_plan.clone(),
            &signature,
            context,
        )
        .map_err(|diagnostic| diagnostic.to_string())?;
    require_canonical_plan(
        &validated,
        boundary_entry_plan,
        "normalized foreign callback boundary plan is not canonical",
    )?;
    Ok(
        effects::NormalizedForeignTerminalMechanismIdentity::from_normalized_locator(
            locator,
            effects::provider_plan::BoundaryCallingPlanCommitment::from_digest(
                validated.contract_commitment_digest(),
            ),
        )
        .into(),
    )
}

fn entry_call_signature(boundary_entry_plan: &BoundaryEntryPlan) -> CallSignature {
    CallSignature {
        parameters: boundary_entry_plan
            .call
            .parameters
            .iter()
            .map(|placement| placement.shape)
            .collect(),
        result: boundary_entry_plan
            .call
            .result
            .as_ref()
            .map(|placement| placement.shape),
    }
}

fn require_canonical_plan(
    validated: &calling_conventions::ValidatedBoundaryEntryPlan,
    boundary_entry_plan: &BoundaryEntryPlan,
    reason: &str,
) -> Result<(), String> {
    if validated.plan() != boundary_entry_plan {
        return Err(reason.to_owned());
    }
    Ok(())
}

pub(crate) fn conservative_syscall_terminal_mechanism(
    target: target::TargetProfile,
    number: i64,
    plan: &abstract_operations::AbstractOperationPlan,
    boundary: semantic_vocabulary::BoundaryMachineId,
) -> Result<TerminalMechanismIdentity, String> {
    if !matches!(
        target,
        target::TargetProfile::LinuxX64 | target::TargetProfile::LinuxArm64
    ) {
        return Err(format!(
            "direct syscall argument-contract checking does not support target `{}`",
            target.target_name(),
        ));
    }
    let number = u32::try_from(number)
        .map_err(|_| "direct syscall number does not fit the checked u32 domain".to_owned())?;
    let declarations = plan
        .boundary_machines
        .iter()
        .filter(|candidate| candidate.id == boundary)
        .collect::<Vec<_>>();
    let [declaration] = declarations.as_slice() else {
        return Err(format!(
            "direct syscall boundary {boundary:?} resolves to {} checked declarations",
            declarations.len(),
        ));
    };
    if declaration.identity == terminal_psi::TerminalIdentityIndex::ABSENT {
        return Err("direct syscall boundary has an empty checked identity".to_owned());
    }
    if !declaration.requires.is_empty() {
        return Err(
            "direct syscall argument-contract checking does not yet support boundary structural-domain requirements"
                .to_owned(),
        );
    }

    let calls = plan
        .operations()
        .filter_map(|operation| {
            let abstract_operations::AbstractOperation::BoundaryCall {
                boundary: called,
                arguments,
                structural_arguments,
                ..
            } = operation
            else {
                return None;
            };
            (*called == boundary).then_some((arguments, structural_arguments))
        })
        .collect::<Vec<_>>();
    if calls.is_empty() {
        return Err("direct syscall boundary has no checked call occurrence".to_owned());
    }
    if calls.iter().any(|(scalar, structural)| {
        scalar.len() != declaration.scalar_parameters.len()
            || structural.len() != declaration.structural_parameters.len()
            || structural
                .iter()
                .zip(&declaration.structural_parameters)
                .any(|(argument, parameter)| argument.access != parameter.access)
    }) {
        return Err(
            "direct syscall call occurrence does not match its checked argument contract"
                .to_owned(),
        );
    }

    let mut digest = DomainDigestEncoder::new(CONSERVATIVE_ARGUMENT_CONTRACT_DOMAIN);
    if !declaration.has_valid_parameter_order() {
        return Err("direct syscall parameter order does not cover its checked lanes".to_owned());
    }
    push_count(&mut digest, declaration.parameter_order.len())?;
    for kind in &declaration.parameter_order {
        digest.byte(match kind {
            terminal_psi::BoundaryParameterKind::Scalar => 0,
            terminal_psi::BoundaryParameterKind::Structural => 1,
        });
    }
    push_count(&mut digest, declaration.scalar_parameters.len())?;
    for scalar in &declaration.scalar_parameters {
        encode_scalar(&mut digest, *scalar)?;
    }
    push_count(&mut digest, declaration.structural_parameters.len())?;
    for (ordinal, parameter) in declaration.structural_parameters.iter().enumerate() {
        if !parameter.qualifications.is_empty() {
            return Err(
                "direct syscall argument-contract checking does not yet support root structural qualifications"
                    .to_owned(),
            );
        }
        if !parameter.projected_qualifications.is_empty() {
            return Err(
                "direct syscall argument-contract checking does not yet support projected structural qualifications"
                    .to_owned(),
            );
        }
        if usize::try_from(parameter.position).ok() != Some(ordinal) {
            return Err(
                "direct syscall structural parameter positions are not canonical".to_owned(),
            );
        }
        let carriers = plan
            .structural_types
            .iter()
            .filter(|candidate| candidate.id == parameter.structural_type)
            .collect::<Vec<_>>();
        let [carrier] = carriers.as_slice() else {
            return Err(format!(
                "direct syscall structural carrier {:?} resolves to {} checked declarations",
                parameter.structural_type,
                carriers.len(),
            ));
        };
        if carrier.identity == terminal_psi::TerminalIdentityIndex::ABSENT {
            return Err("direct syscall structural carrier has an empty identity".to_owned());
        }
        digest.raw(&parameter.position.to_be_bytes());
        digest.byte(multiplicity_tag(parameter.multiplicity));
        digest.byte(access_tag(parameter.access));
        digest.raw(&carrier.identity.0.to_le_bytes());
    }
    let checked_argument_contract =
        CheckedSyscallArgumentContractIdentity::from_digest(digest.finish());
    Ok(SyscallTerminalMechanismIdentity::new(target, number, checked_argument_contract).into())
}

fn push_count(digest: &mut DomainDigestEncoder, count: usize) -> Result<(), String> {
    digest.raw(
        &u32::try_from(count)
            .map_err(|_| "checked syscall argument-contract count exceeds u32".to_owned())?
            .to_be_bytes(),
    );
    Ok(())
}

fn encode_scalar(digest: &mut DomainDigestEncoder, scalar: ScalarType) -> Result<(), String> {
    match scalar {
        ScalarType::Callable(_) => {
            return Err(
                "direct syscall arguments cannot contain local callable descriptors".to_owned(),
            );
        }
        ScalarType::Boolean => digest.byte(0),
        ScalarType::Integer(integer) => {
            digest.byte(1);
            digest.byte(match integer.carrier() {
                IntegerCarrier::Fixed => 0,
                IntegerCarrier::Address => 1,
            });
            digest.byte(match integer.sign() {
                IntegerSign::Signed => 0,
                IntegerSign::Unsigned => 1,
            });
            digest.raw(&integer.bits().to_be_bytes());
        }
        ScalarType::IeeeFloat(format) => {
            digest.byte(2);
            digest.byte(match format {
                IeeeFloatFormat::Binary32 => 0,
                IeeeFloatFormat::Binary64 => 1,
            });
        }
    }
    Ok(())
}

const fn multiplicity_tag(multiplicity: StructuralMultiplicity) -> u8 {
    match multiplicity {
        StructuralMultiplicity::Unrestricted => 0,
        StructuralMultiplicity::Affine => 1,
        StructuralMultiplicity::Linear => 2,
    }
}

const fn access_tag(access: StructuralAccess) -> u8 {
    match access {
        StructuralAccess::Owned => 0,
        StructuralAccess::SharedBorrow => 1,
        StructuralAccess::MutableBorrow => 2,
        StructuralAccess::WriteOnlyBorrow => 3,
    }
}
