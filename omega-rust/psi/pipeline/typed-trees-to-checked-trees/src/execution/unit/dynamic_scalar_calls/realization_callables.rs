//! Complete checked conformance rosters, with per-member bodies and finite-family instances.

use super::realization_bodies::checked_realization_scalar_body;
use crate::execution::terminal_unit::{
    CheckFacts, MachineSupplyMode, PrimitiveType, TypedTrees, is_unit,
    structural_access_for_type_reference,
};
use checked_trees::CheckedDynamicRealizationBodyPlan;

pub(super) fn checked_dynamic_realization_callables(
    program: &TypedTrees,
    facts: &CheckFacts,
    conformance: &typed_trees::trait_definition::Conformance,
    selection: &checked_trees::DynamicConformanceBindingFact,
    source_access: checked_trees::CheckedStructuralAccess,
) -> Option<Vec<checked_trees::CheckedDynamicRealizationCallablePlan>> {
    let closed_rows = program.closed_conformance_rows(conformance)?;
    if closed_rows.len() != selection.rows.len() {
        return None;
    }
    let mut callables = Vec::new();
    for (closed, retained) in closed_rows.iter().zip(&selection.rows) {
        if closed.declaring_trait != retained.declaring_trait
            || closed.requirement != retained.requirement
            || closed.realization_machine != retained.realization_machine
            || closed.realization_state != retained.realization_state
        {
            return None;
        }
        let (requirement_identity, row_realization_identity) =
            crate::facts::normalized_dynamic_row_identities(program, closed).ok()?;
        if requirement_identity != retained.requirement_identity
            || row_realization_identity != retained.realization_identity
        {
            return None;
        }
        let declaring_trait = program
            .traits()
            .iter()
            .find(|definition| definition.symbol == closed.declaring_trait)?;
        let requirement = program
            .trait_machine_signatures(declaring_trait)
            .iter()
            .find(|candidate| candidate.symbol == closed.requirement)?;
        let row_realization_machine = program
            .machines()
            .iter()
            .find(|candidate| candidate.symbol == closed.realization_machine)?;
        let row_realization_state = program
            .machine_states(row_realization_machine)
            .iter()
            .find(|candidate| candidate.symbol == closed.realization_state)?;
        let [requirement_self] = program.state_signature_parameters(requirement) else {
            return None;
        };
        let result_type = if is_unit(program, requirement.return_type) {
            None
        } else {
            let primitive = program.primitive_type_reference(requirement.return_type)?;
            if !matches!(primitive, PrimitiveType::Bool | PrimitiveType::I32) {
                return None;
            }
            Some(primitive)
        };
        if !requirement_self.is_self
            || structural_access_for_type_reference(program, requirement_self.type_reference)
                != Some(source_access)
            || row_realization_machine.supply_mode != MachineSupplyMode::CheckedBody
            || row_realization_machine.attached_data_symbol != selection.source_data
        {
            return None;
        }
        // A generic requirement contributes one callable per roster tuple,
        // each naming that tuple's exact specialization instance.
        let family_tuples = family_tuple_roster(program, requirement)?;
        for family_tuple in family_tuples {
            let (realization_machine, realization_state, realization_identity) =
                dynamic_family_realization(
                    program,
                    row_realization_machine,
                    row_realization_state,
                    row_realization_identity.clone(),
                    &family_tuple,
                )?;
            let [realization_self] = program.state_parameters(realization_state) else {
                return None;
            };
            if !realization_self.is_self
                || structural_access_for_type_reference(program, realization_self.type_reference)
                    != Some(source_access)
                || realization_machine.supply_mode != MachineSupplyMode::CheckedBody
                || realization_machine.attached_data_symbol != selection.source_data
            {
                return None;
            }
            let body = match result_type {
                None => {
                    if !is_unit(program, realization_state.return_type)
                        || !program
                            .statement_table
                            .statements(realization_state.statement_nodes)
                            .is_empty()
                        || !program.state_contracts(realization_state).is_empty()
                    {
                        return None;
                    }
                    CheckedDynamicRealizationBodyPlan::Unit
                }
                Some(result_type) => {
                    if program.primitive_type_reference(realization_state.return_type)
                        != Some(result_type)
                    {
                        return None;
                    }
                    let body = checked_realization_scalar_body(
                        program,
                        facts,
                        realization_machine,
                        realization_state,
                        result_type,
                    )?;
                    CheckedDynamicRealizationBodyPlan::Scalar {
                        result_type,
                        structural_scalar_field_stores: body.structural_scalar_field_stores,
                        return_expression: body.return_expression,
                    }
                }
            };
            let contract = facts
                .contract_plans
                .for_machine(realization_machine.symbol)?;
            if contract.report_fingerprint == 0 || contract.commitment.is_zero() {
                return None;
            }
            callables.push(checked_trees::CheckedDynamicRealizationCallablePlan {
                declaring_trait: closed.declaring_trait,
                requirement: closed.requirement,
                requirement_identity: requirement_identity.clone(),
                realization_machine: realization_machine.symbol,
                realization_state: realization_state.symbol,
                realization_identity,
                family_tuple,
                body,
                contract_report_fingerprint: contract.report_fingerprint,
                contract_commitment: contract.commitment,
            });
        }
    }
    Some(callables)
}

/// The tuples one requirement contributes to a callable roster: the complete
/// declared roster for a finite family, or one empty tuple for a nongeneric
/// requirement. Any other generic shape stays out of the lane.
pub(crate) fn family_tuple_roster(
    program: &TypedTrees,
    requirement: &typed_trees::signature::StateSignature,
) -> Option<Vec<Box<[String]>>> {
    match program.finite_signature_family(requirement) {
        typed_trees::finite_family::FamilyProbe::Finite { tuples, .. } => {
            Some(tuples.into_iter().map(|tuple| tuple.identities).collect())
        }
        typed_trees::finite_family::FamilyProbe::NotFinite(_) => {
            if program
                .state_signature_type_parameters(requirement)
                .is_empty()
            {
                Some(vec![Box::default()])
            } else {
                None
            }
        }
    }
}

/// Resolve the finite-family tuple one dynamic call selects from its static
/// machine arguments: exactly one complete roster tuple, or the empty tuple
/// for a nongeneric requirement. Any other shape — non-member values, arity
/// mismatches, non-const arguments, machine arguments on a requirement that
/// declares no finite family, or a generic requirement with no roster — stays
/// out of the lane.
pub(crate) fn dynamic_family_tuple(
    program: &TypedTrees,
    requirement: &typed_trees::signature::StateSignature,
    machine_arguments: &[typed_trees::expression::StaticMachineArgument],
) -> Option<Box<[String]>> {
    match program.finite_signature_family(requirement) {
        typed_trees::finite_family::FamilyProbe::Finite { arity, tuples } => {
            let identities: Option<Vec<String>> = machine_arguments
                .iter()
                .map(|argument| program.static_const_argument_identity(argument))
                .collect();
            let identities = identities?;
            if identities.len() != arity
                || !tuples
                    .iter()
                    .any(|tuple| tuple.identities.as_ref() == identities.as_slice())
            {
                return None;
            }
            Some(identities.into_boxed_slice())
        }
        typed_trees::finite_family::FamilyProbe::NotFinite(_) => {
            if machine_arguments.is_empty()
                && program
                    .state_signature_type_parameters(requirement)
                    .is_empty()
            {
                Some(Box::default())
            } else {
                None
            }
        }
    }
}

/// Resolve the realization one family tuple selects from the row's provider:
/// the row's own machine/state for the empty tuple, otherwise the unique bare
/// value-tuple specialization instance (every other specialization coordinate
/// empty) whose `const_argument_identities` equal the tuple. The instance's
/// state is the specialization's state at the row state's declaration
/// position.
pub(crate) fn dynamic_family_realization<'program>(
    program: &'program TypedTrees,
    row_realization_machine: &'program typed_trees::machine::Machine,
    row_realization_state: &'program typed_trees::state::State,
    row_realization_identity: String,
    family_tuple: &[String],
) -> Option<(
    &'program typed_trees::machine::Machine,
    &'program typed_trees::state::State,
    String,
)> {
    if family_tuple.is_empty() {
        return Some((
            row_realization_machine,
            row_realization_state,
            row_realization_identity,
        ));
    }
    let mut candidates = program
        .machine_specializations
        .iter()
        .filter(|specialization| {
            specialization.template == row_realization_machine.symbol
                && specialization.const_argument_identities.as_slice() == family_tuple
                && specialization.type_argument_identities.is_empty()
                && specialization.machine_arguments.is_empty()
                && specialization.conformance_arguments.is_empty()
                && specialization.inferred_conformance_arguments.is_empty()
                && specialization.conformance_applications.is_empty()
        });
    let specialization = candidates.next()?;
    if candidates.next().is_some() {
        return None;
    }
    let realization_machine = program
        .machines()
        .iter()
        .find(|candidate| candidate.symbol == specialization.instance)?;
    let state_position = program
        .machine_states(row_realization_machine)
        .iter()
        .position(|candidate| candidate.symbol == row_realization_state.symbol)?;
    let realization_state = program
        .machine_states(realization_machine)
        .get(state_position)?;
    let realization_identity = program
        .normalized_machine_overload_identity(realization_machine)?
        .identity();
    Some((realization_machine, realization_state, realization_identity))
}
