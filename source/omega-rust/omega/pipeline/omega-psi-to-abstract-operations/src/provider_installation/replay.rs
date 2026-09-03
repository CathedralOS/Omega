use super::error::ProviderInstallationError;
use super::model::AdmittedInstalledProviderUnitCall;
use crate::shared::*;

pub(super) fn replay_installed_provider_unit_calls(
    plan: &AbstractOperationPlan,
    module: &psi_terminal::TerminalModule,
    installed: &[ProviderCandidateConformance],
) -> Result<Vec<AdmittedInstalledProviderUnitCall>, ProviderInstallationError> {
    let mut calls = Vec::new();
    for caller in &plan.functions {
        for operation in &caller.operations {
            let AbstractOperation::BoundaryCall {
                psi_operation,
                result,
                boundary,
                arguments,
                structural_arguments,
                completion_claim_sources,
                completion_receipts,
            } = operation
            else {
                continue;
            };
            let Some(provider) = installed.iter().find(|row| row.boundary == *boundary) else {
                continue;
            };
            let malformed = || ProviderInstallationError::InstalledUnitCallReplayMismatch {
                caller: caller.machine,
                operation: *psi_operation,
                boundary: *boundary,
            };
            let boundary_declaration = plan
                .boundary_machines
                .iter()
                .find(|row| row.id == *boundary)
                .ok_or_else(malformed)?;
            let candidate = plan
                .functions
                .iter()
                .find(|function| function.machine == provider.candidate)
                .ok_or_else(malformed)?;
            let terminal_candidate = module
                .machines
                .iter()
                .find(|machine| machine.id == provider.candidate)
                .ok_or_else(malformed)?;
            let terminal_caller = module
                .machines
                .iter()
                .find(|machine| machine.id == caller.machine)
                .ok_or_else(malformed)?;
            if !result.is_unit()
                || boundary_declaration.identity != provider.requirement_identity
                || !boundary_declaration.result.is_unit()
                || !matches!(&candidate.result, AbstractFunctionResult::Unit)
                || !matches!(
                    &terminal_candidate.result,
                    psi_terminal::TerminalMachineResult::Unit
                )
                || structural_arguments.len() != provider.signature.parameters.len()
                || boundary_declaration.structural_parameters.len() != structural_arguments.len()
                || candidate.structural_parameters.len() != structural_arguments.len()
            {
                return Err(malformed());
            }
            if !replays_supported_scalar_call(
                caller,
                terminal_caller,
                arguments,
                boundary_declaration,
                candidate,
                terminal_candidate,
            ) {
                return Err(malformed());
            }
            for (index, (((argument, signature), boundary_parameter), candidate_parameter)) in
                structural_arguments
                    .iter()
                    .zip(&provider.signature.parameters)
                    .zip(&boundary_declaration.structural_parameters)
                    .zip(&candidate.structural_parameters)
                    .enumerate()
            {
                let Some(caller_parameter) = caller
                    .structural_parameters
                    .iter()
                    .find(|parameter| parameter.place == argument.place)
                else {
                    return Err(malformed());
                };
                let Some(argument_type) = resolve_structural_argument_type(
                    module,
                    caller_parameter.structural_type,
                    &argument.path,
                ) else {
                    return Err(malformed());
                };
                let caller_matches = if argument.path.is_empty() {
                    caller_parameter.structural_type == signature.structural_type
                        && caller_parameter.multiplicity == signature.multiplicity
                        && caller_parameter.access == signature.access
                        && caller_parameter.qualifications == signature.qualifications
                } else {
                    argument_type == signature.structural_type
                        && signature.multiplicity == StructuralMultiplicity::Linear
                        && structural_access_can_supply(caller_parameter.access, signature.access)
                        && signature.qualifications.is_empty()
                };
                if signature.position as usize != index
                    || argument.access != signature.access
                    || boundary_parameter.position != signature.position
                    || boundary_parameter.is_self != signature.is_self
                    || boundary_parameter.structural_type != signature.structural_type
                    || boundary_parameter.multiplicity != signature.multiplicity
                    || boundary_parameter.access != signature.access
                    || boundary_parameter.qualifications != signature.qualifications
                    || candidate_parameter.position != signature.position
                    || candidate_parameter.is_self != signature.is_self
                    || candidate_parameter.structural_type != signature.structural_type
                    || candidate_parameter.multiplicity != signature.multiplicity
                    || candidate_parameter.access != signature.access
                    || candidate_parameter.qualifications != signature.qualifications
                    || !caller_matches
                {
                    return Err(malformed());
                }
            }

            let mut expected_claims = Vec::new();
            for claim in &terminal_candidate.entry_claims {
                if !claim.path.is_empty() {
                    return Err(malformed());
                }
                let argument_index = terminal_candidate
                    .structural_parameters
                    .iter()
                    .position(|parameter| parameter.place == claim.input)
                    .ok_or_else(malformed)? as u32;
                expected_claims.push((argument_index, claim.claim));
            }
            if completion_receipts.len() != expected_claims.len() {
                return Err(malformed());
            }
            if structural_arguments
                .iter()
                .enumerate()
                .any(|(index, argument)| {
                    !argument.path.is_empty()
                        && !expected_claims
                            .iter()
                            .any(|(argument_index, _)| *argument_index as usize == index)
                })
            {
                return Err(malformed());
            }
            for (receipt, (argument_index, candidate_claim)) in
                completion_receipts.iter().zip(&expected_claims)
            {
                let argument = structural_arguments
                    .get(*argument_index as usize)
                    .ok_or_else(malformed)?;
                let source = completion_claim_sources
                    .iter()
                    .find(|source| source.claim == receipt.claim)
                    .ok_or_else(malformed)?;
                let entry = source.entry.as_ref().ok_or_else(malformed)?;
                if receipt.argument_index != *argument_index
                    || entry.input != argument.place
                    || entry.path != argument.path
                {
                    return Err(malformed());
                }
                if let Some(candidate_content) = terminal_candidate
                    .content_entry_claims
                    .iter()
                    .find(|content| content.claim == *candidate_claim)
                {
                    if !argument.path.is_empty() {
                        return Err(malformed());
                    }
                    let caller_content = source.content.as_ref().ok_or_else(malformed)?;
                    if caller_content.input.root != argument.place
                        || caller_content.input.segments != candidate_content.input.segments
                        || caller_content.projections != candidate_content.projections
                    {
                        return Err(malformed());
                    }
                } else if source.content.is_some() {
                    return Err(malformed());
                }
            }
            if terminal_candidate
                .content_entry_claims
                .iter()
                .any(|content| {
                    !terminal_candidate
                        .entry_claims
                        .iter()
                        .any(|entry| entry.claim == content.claim)
                })
            {
                return Err(malformed());
            }
            calls.push(AdmittedInstalledProviderUnitCall {
                caller: caller.machine,
                psi_operation: *psi_operation,
                boundary: *boundary,
                provider: provider.clone(),
                scalar_arguments: arguments.clone(),
                structural_arguments: structural_arguments.clone(),
                completion_claim_sources: completion_claim_sources.clone(),
                completion_receipts: completion_receipts.clone(),
            });
        }
    }
    Ok(calls)
}

fn replays_supported_scalar_call(
    caller: &AbstractFunction,
    terminal_caller: &TerminalMachine,
    arguments: &[psi_core::ValueId],
    boundary: &psi_terminal::BoundaryMachineDeclaration,
    candidate: &AbstractFunction,
    terminal_candidate: &TerminalMachine,
) -> bool {
    match arguments {
        [] => {
            boundary.scalar_parameters.is_empty()
                && candidate.parameters.is_empty()
                && terminal_candidate.parameters.is_empty()
        }
        [argument] => {
            let [caller_parameter] = caller.parameters.as_slice() else {
                return false;
            };
            let [terminal_caller_parameter] = terminal_caller.parameters.as_slice() else {
                return false;
            };
            let [boundary_parameter] = boundary.scalar_parameters.as_slice() else {
                return false;
            };
            let [candidate_parameter] = candidate.parameters.as_slice() else {
                return false;
            };
            let [terminal_candidate_parameter] = terminal_candidate.parameters.as_slice() else {
                return false;
            };
            let signed_i32 = ScalarType::Integer(
                psi_core::IntegerType::new(psi_core::IntegerSign::Signed, 32)
                    .expect("fixed signed i32 is a valid scalar type"),
            );
            *argument == caller_parameter.value
                && caller_parameter.value == terminal_caller_parameter.id
                && caller_parameter.scalar_type == terminal_caller_parameter.scalar_type
                && caller_parameter.scalar_type == signed_i32
                && *boundary_parameter == signed_i32
                && candidate_parameter.scalar_type == signed_i32
                && terminal_candidate_parameter.scalar_type == signed_i32
                && candidate_parameter.value == terminal_candidate_parameter.id
        }
        _ => false,
    }
}

fn resolve_structural_argument_type(
    module: &psi_terminal::TerminalModule,
    mut structural_type: psi_core::StructuralTypeId,
    path: &[psi_terminal::StructuralPathSegment],
) -> Option<psi_core::StructuralTypeId> {
    for segment in path {
        let declaration = module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == structural_type)?;
        structural_type = match (segment, &declaration.shape) {
            (
                psi_terminal::StructuralPathSegment::Field(identity),
                psi_terminal::StructuralTypeShape::Record { fields },
            ) => {
                let field = fields
                    .iter()
                    .find(|field| field.identity == *identity && !field.relevance.is_erased())?;
                let psi_terminal::StructuralFieldType::Structural(next) = field.field_type else {
                    return None;
                };
                next
            }
            (
                psi_terminal::StructuralPathSegment::FixedIndex(index),
                psi_terminal::StructuralTypeShape::FixedArray { element, length },
            ) if index < length => *element,
            _ => return None,
        };
    }
    Some(structural_type)
}

fn structural_access_can_supply(
    source: psi_terminal::StructuralAccess,
    presented: psi_terminal::StructuralAccess,
) -> bool {
    use psi_terminal::StructuralAccess;

    match source {
        StructuralAccess::Owned => true,
        StructuralAccess::SharedBorrow => presented == StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow => matches!(
            presented,
            StructuralAccess::SharedBorrow
                | StructuralAccess::MutableBorrow
                | StructuralAccess::WriteOnlyBorrow
        ),
        StructuralAccess::WriteOnlyBorrow => presented == StructuralAccess::WriteOnlyBorrow,
    }
}
