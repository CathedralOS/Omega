//! Checked direct-syscall argument-contract identity.

use effects::{
    CheckedSyscallArgumentContractIdentity, SyscallTerminalMechanismIdentity,
    TerminalMechanismIdentity,
};
use semantic_vocabulary::{IeeeFloatFormat, IntegerCarrier, IntegerSign, ScalarType};
use sha2::{Digest, Sha256};
use terminal_psi::{StructuralAccess, StructuralMultiplicity};

const CONSERVATIVE_ARGUMENT_CONTRACT_DOMAIN: &[u8] =
    b"omega.checked-syscall-argument-contract.conservative-unqualified.v1\0";

/// Derive the first checked syscall argument contract from one verified
/// target-neutral boundary signature.
///
/// This rung is deliberately conservative: it commits the complete admitted
/// parameter carrier/access contract but treats every runtime value within
/// that contract as reachable. Structural-domain commitments, constants,
/// ranges, descriptor provenance, and other narrowing proofs require later
/// distinct identities.
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
    if declaration.identity.is_empty() {
        return Err("direct syscall boundary has an empty checked identity".to_owned());
    }
    if !declaration.requires.is_empty() {
        return Err(
            "direct syscall argument-contract checking does not yet support boundary structural-domain requirements"
                .to_owned(),
        );
    }

    let calls = plan
        .functions
        .iter()
        .flat_map(|function| &function.operations)
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

    let mut digest = Sha256::new();
    digest.update(CONSERVATIVE_ARGUMENT_CONTRACT_DOMAIN);
    push_count(&mut digest, declaration.scalar_parameters.len())?;
    for scalar in &declaration.scalar_parameters {
        encode_scalar(&mut digest, *scalar);
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
        if carrier.identity.is_empty() {
            return Err("direct syscall structural carrier has an empty identity".to_owned());
        }
        digest.update(parameter.position.to_be_bytes());
        digest.update([u8::from(parameter.is_self)]);
        digest.update([multiplicity_tag(parameter.multiplicity)]);
        digest.update([access_tag(parameter.access)]);
        push_bytes(&mut digest, carrier.identity.as_bytes())?;
    }
    let checked_argument_contract =
        CheckedSyscallArgumentContractIdentity::from_digest(digest.finalize().into());
    Ok(SyscallTerminalMechanismIdentity::new(target, number, checked_argument_contract).into())
}

fn push_count(digest: &mut Sha256, count: usize) -> Result<(), String> {
    digest.update(
        u32::try_from(count)
            .map_err(|_| "checked syscall argument-contract count exceeds u32".to_owned())?
            .to_be_bytes(),
    );
    Ok(())
}

fn push_bytes(digest: &mut Sha256, bytes: &[u8]) -> Result<(), String> {
    push_count(digest, bytes.len())?;
    digest.update(bytes);
    Ok(())
}

fn encode_scalar(digest: &mut Sha256, scalar: ScalarType) {
    match scalar {
        ScalarType::Boolean => digest.update([0]),
        ScalarType::Integer(integer) => {
            digest.update([1]);
            digest.update([match integer.carrier() {
                IntegerCarrier::Fixed => 0,
                IntegerCarrier::Address => 1,
            }]);
            digest.update([match integer.sign() {
                IntegerSign::Signed => 0,
                IntegerSign::Unsigned => 1,
            }]);
            digest.update(integer.bits().to_be_bytes());
        }
        ScalarType::IeeeFloat(format) => {
            digest.update([2]);
            digest.update([match format {
                IeeeFloatFormat::Binary32 => 0,
                IeeeFloatFormat::Binary64 => 1,
            }]);
        }
    }
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
