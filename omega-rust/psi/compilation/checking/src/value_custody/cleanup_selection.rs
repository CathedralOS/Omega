//! Cleanup belongs to an owned qualification, rather than a machine spelling.
//! Resolve the default and authored domain graph before peeling the carrier:
//! different domains on one subject may repeat one application, never chain
//! competing hooks. Fields are deliberately absent from this walk; each field
//! is a separate owned subject with an independently planned death.

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::expression::StaticMachineArgument;
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode};

pub(super) fn selected_application<'program>(
    program: &'program dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
) -> Result<Option<&'program StaticMachineArgument>, Diagnostic> {
    let mut selected = None;
    visit_type(program, reference, &mut Vec::new(), &mut selected)?;
    Ok(selected)
}

fn merge<'program>(
    program: &dyn crate::CheckingInput,
    selection: &'program StaticMachineArgument,
    selected: &mut Option<&'program StaticMachineArgument>,
) -> Result<(), Diagnostic> {
    if !selection.symbol.is_valid() || program.machine(selection.symbol).is_none_or(|machine| machine.attached_data.is_some()) {
        return Err(Diagnostic::error("dropped by must resolve one independent static machine"));
    }
    if let Some(previous) = *selected {
        if !same_application(program, previous, selection) {
            return Err(Diagnostic::error("conflicting dropped by machine applications on one owned subject"));
        }
    } else {
        *selected = Some(selection);
    }
    Ok(())
}

fn same_application(
    program: &dyn crate::CheckingInput,
    left: &StaticMachineArgument,
    right: &StaticMachineArgument,
) -> bool {
    if left.type_reference.is_valid() || right.type_reference.is_valid() {
        return left.type_reference.is_valid() && right.type_reference.is_valid()
            && program.normalized_type_identity(left.type_reference)
                == program.normalized_type_identity(right.type_reference);
    }
    if left.symbol != right.symbol || left.const_literal != right.const_literal
        || left.evidence_projection != right.evidence_projection {
        return false;
    }
    match (&left.application, &right.application) {
        (None, None) => true,
        (Some(left), Some(right)) => left.lifetime_arguments == right.lifetime_arguments
            && left.arguments.len() == right.arguments.len()
            && left.arguments.iter().zip(right.arguments.iter())
                .all(|(left, right)| same_application(program, left, right)),
        _ => false,
    }
}

fn visit_type<'program>(
    program: &'program dyn crate::CheckingInput,
    reference: TypeReferenceHandle,
    domains: &mut Vec<SymbolHandle>,
    selected: &mut Option<&'program StaticMachineArgument>,
) -> Result<(), Diagnostic> {
    match program.type_reference_table.type_reference(reference) {
        TypeReferenceNode::Constrained { base_type, constraints, .. } => {
            visit_type(program, *base_type, domains, selected)?;
            for constraint in program.type_reference_table.constraints(*constraints) {
                if let TypeConstraintNode::Domain(domain) = constraint {
                    visit_domain(program, domain.symbol, domains, selected)?;
                }
            }
        }
        TypeReferenceNode::Named { symbol, .. }
        | TypeReferenceNode::Generic { base_symbol: symbol, .. } => {
            if let Some(data) = program.unique_data_definition_by_symbol(*symbol)
                && let Some(selection) = &data.cleanup_selection {
                merge(program, selection, selected)?;
            }
        }
        // A loan does not own the referent's disposal, and arrays dispose
        // their elements separately rather than inheriting their selection.
        _ => {}
    }
    Ok(())
}

fn visit_domain<'program>(
    program: &'program dyn crate::CheckingInput,
    symbol: SymbolHandle,
    visited: &mut Vec<SymbolHandle>,
    selected: &mut Option<&'program StaticMachineArgument>,
) -> Result<(), Diagnostic> {
    if visited.contains(&symbol) {
        return Ok(());
    }
    visited.push(symbol);
    let Some(domain) = program.domain_definitions().iter().find(|domain| domain.symbol == symbol) else {
        return Ok(());
    };
    if let Some(selection) = &domain.cleanup_selection {
        merge(program, selection, selected)?;
    }
    if let Some(alias) = &domain.alias {
        for constituent in &alias.constituents {
            visit_domain(program, constituent.domain_symbol, visited, selected)?;
        }
    }
    for fact in program.proof_facts(domain) {
        if let typed_trees::domain::ProofFact::Membership(membership) = fact {
            visit_domain(program, membership.domain_symbol, visited, selected)?;
        }
    }
    Ok(())
}

pub(super) fn owner_for_machine(
    program: &dyn crate::CheckingInput,
    machine: SymbolHandle,
) -> Option<SymbolHandle> {
    if let Some(data) = program.data_definitions().iter().find(|data|
        data.cleanup_selection.as_ref().is_some_and(|selection| selection.symbol == machine)) {
        return Some(data.symbol);
    }
    program.domain_definitions().iter().find_map(|domain| {
        if !domain.cleanup_selection.as_ref().is_some_and(|selection| selection.symbol == machine) {
            return None;
        }
        match program.type_reference_table.type_reference(crate::peel_declared_constraints(program, domain.target_type)) {
            TypeReferenceNode::Named { symbol, .. } => Some(*symbol),
            TypeReferenceNode::Generic { base_symbol, .. } => Some(*base_symbol),
            _ => None,
        }
    })
}
