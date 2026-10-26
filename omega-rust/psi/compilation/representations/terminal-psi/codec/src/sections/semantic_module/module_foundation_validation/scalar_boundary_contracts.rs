//! Exact scalar membership joins at external boundary-declaration ingress.
//! Predicate truth and original introduction belong to source checking. These
//! joins prevent transport from substituting a bare value, unrelated domain or
//! provider with stronger nominal requirements for that checked contract.

use super::{CodecError, TerminalMachine, TerminalModule, malformed};
use semantic_vocabulary::{ScalarDomainId, ScalarQualificationSetId, ValueId};
use terminal_psi::{BoundaryMachineDeclaration, ValueDeclaration};

fn domains(
    module: &TerminalModule,
    set: ScalarQualificationSetId,
) -> Result<&[ScalarDomainId], CodecError> {
    super::qualification_domains(module, set).ok_or(CodecError::MalformedStructuralFoundation(
        "boundary scalar value names an absent qualification set",
    ))
}

pub(super) fn validate_declarations(module: &TerminalModule) -> Result<(), CodecError> {
    // Results are promises of the public declaration, not just ABI carriers.
    // Check the same namespace for boundary calls and dynamic interface slots.
    for result in module
        .boundary_machines
        .iter()
        .map(|boundary| &boundary.result)
        .chain(
            module
                .dynamic_dispatch
                .interfaces
                .iter()
                .flat_map(|interface| interface.slots.iter().map(|slot| &slot.result)),
        )
    {
        if let terminal_psi::BoundaryMachineResult::Scalar(result) = result {
            for membership in domains(module, result.qualifications)? {
                if !module
                    .scalar_qualifications
                    .domains
                    .iter()
                    .any(|domain| domain.id == *membership && domain.carrier == result.scalar_type)
                {
                    return malformed("boundary scalar result qualification changes its carrier");
                }
            }
        }
    }
    for boundary in &module.boundary_machines {
        for requirement in &boundary.scalar_domain_requirements {
            let carrier = boundary
                .scalar_parameters
                .get(requirement.argument_index as usize)
                .ok_or(CodecError::MalformedStructuralFoundation(
                    "boundary scalar domain requirement has no parameter",
                ))?;
            let domain = super::scalar_domain(module, requirement.domain).ok_or(
                CodecError::MalformedStructuralFoundation(
                    "boundary scalar requirement names an absent domain",
                ),
            )?;
            if domain.carrier != *carrier {
                return malformed("boundary scalar domain requirement changes its carrier");
            }
        }
    }
    Ok(())
}

pub(super) fn validate_arguments(
    module: &TerminalModule,
    machine: &TerminalMachine,
    boundary: &BoundaryMachineDeclaration,
    arguments: &[ValueId],
) -> Result<(), CodecError> {
    for requirement in &boundary.scalar_domain_requirements {
        let argument = arguments.get(requirement.argument_index as usize).ok_or(
            CodecError::MalformedStructuralFoundation(
                "boundary scalar domain requirement has no actual argument",
            ),
        )?;
        let actual = super::values(machine)
            .find(|value| value.id == *argument)
            .ok_or(CodecError::MalformedStructuralFoundation(
                "boundary scalar membership has no exact value declaration",
            ))?;
        if !domains(module, actual.qualifications)?.contains(&requirement.domain) {
            return malformed("boundary scalar argument lacks its required domain identity");
        }
    }
    Ok(())
}

pub(super) fn validate_provider(
    module: &TerminalModule,
    boundary: &BoundaryMachineDeclaration,
    parameters: &[ValueDeclaration],
) -> Result<(), CodecError> {
    for (argument_index, parameter) in parameters.iter().enumerate() {
        for domain in domains(module, parameter.qualifications)? {
            if !boundary.scalar_domain_requirements.iter().any(|required| {
                required.argument_index as usize == argument_index && required.domain == *domain
            }) {
                return malformed(
                    "provider candidate requires scalar membership absent from its boundary",
                );
            }
        }
    }
    Ok(())
}
