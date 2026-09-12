//! Membership observes a discriminator without consuming its owner. Declaration
//! lookup supplies nominal identity, not liveness: control-flow availability and
//! the ownership frontier independently check the occurrence before it executes.

use super::*;

pub(super) fn validate(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    source: PlaceId,
    case: semantic_vocabulary::StructuralCaseId,
) -> Result<(), ModuleError> {
    let invalid = || ModuleError::InvalidStructuralCaseObservation {
        operation: operation.id,
        source,
        case,
    };
    if operation.result.scalar().is_none_or(|result| {
        result.scalar_type != ScalarType::Boolean || !result.qualifications.is_empty()
    }) {
        return Err(invalid());
    }
    let parameter = machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == source)
        .or_else(|| super::block_views::parameter(machine, source));
    if parameter.is_some_and(|parameter| parameter.access == StructuralAccess::WriteOnlyBorrow) {
        return Err(ModuleError::StructuralObservationRequiresReadableAccess {
            operation: operation.id,
            source,
        });
    }
    let signature = super::structural_result_contracts::source_signature(machine, source)
        .ok_or_else(invalid)?;
    let declaration = module
        .structural_types
        .iter()
        .find(|declaration| declaration.id == signature.structural_type)
        .ok_or_else(invalid)?;
    let cases = match &declaration.shape {
        StructuralTypeShape::Sum { cases } | StructuralTypeShape::Mixed { cases, .. } => cases,
        _ => return Err(invalid()),
    };
    if !cases.iter().any(|candidate| candidate.id == case) {
        return Err(invalid());
    }
    Ok(())
}

pub(super) fn validate_available(
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    available: &BTreeSet<PlaceId>,
) -> Result<(), ModuleError> {
    let OperationKind::StructuralCaseMembership { source, case } = operation.kind else {
        return Ok(());
    };
    // Entry parameters already exist. Block parameters, including exclusive
    // views forwarded across edges, and operation results require a current
    // dominating establishment. A same-typed sibling result supplies nothing.
    if !machine
        .structural_parameters
        .iter()
        .any(|parameter| parameter.place == source)
        && !available.contains(&source)
    {
        return Err(ModuleError::InvalidStructuralCaseObservation {
            operation: operation.id,
            source,
            case,
        });
    }
    Ok(())
}
