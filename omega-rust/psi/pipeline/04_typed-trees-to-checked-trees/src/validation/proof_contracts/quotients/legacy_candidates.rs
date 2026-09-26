//! Legacy quotient call candidates.

use crate::validation::proof_contracts::quotients::equivalence_selection::base_data_symbol;
use symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees;
use symbol_resolved_trees_to_typed_trees::typed_trees::machine::Machine;
use symbol_resolved_trees_to_typed_trees::typed_trees::state::State;
use symbol_resolved_trees_to_typed_trees::typed_trees::types::TypeReferenceHandle;

/// A bare representative call whose operands happen to be quotient values.
/// This shape is retained only to replace generic nominal mismatch cascades
/// with the settled explicit-wrapper diagnostic. It carries no admission.
pub(crate) struct LegacyQuotientCallCandidate<'program> {
    pub(crate) quotient:
        &'program symbol_resolved_trees_to_typed_trees::typed_trees::data::DataDefinition,
    pub(crate) operation: &'program Machine,
}

pub(crate) fn legacy_quotient_call_candidate<'program>(
    program: &'program TypedTrees,
    receiver_type: Option<TypeReferenceHandle>,
    argument_types: &[Option<TypeReferenceHandle>],
    state: &'program State,
) -> Option<LegacyQuotientCallCandidate<'program>> {
    let parameters = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    if parameters.len() != argument_types.len() {
        return None;
    }

    let operation = program.machine_holding_state(state.symbol)?;
    let is_attached = operation.attached_data.is_some();
    if is_attached != receiver_type.is_some() {
        return None;
    }

    let first_operand = receiver_type.or_else(|| argument_types.first().copied().flatten())?;
    let quotient = quotient_for_type(program, first_operand)?;
    let quotient_metadata = quotient.quotient.as_ref()?;
    let carrier = base_data_symbol(program, quotient_metadata.carrier)?;
    if base_data_symbol(program, state.return_type) != Some(carrier) {
        return None;
    }
    if let Some(receiver_type) = receiver_type {
        if quotient_for_type(program, receiver_type)?.symbol != quotient.symbol {
            return None;
        }
        let attached_carrier = program.attached_data_definition(operation)?;
        if attached_carrier.symbol != carrier {
            return None;
        }
    }
    for (parameter, argument_type) in parameters.iter().zip(argument_types) {
        if base_data_symbol(program, parameter.type_reference) != Some(carrier) {
            return None;
        }
        let argument_quotient = quotient_for_type(program, (*argument_type)?)?;
        if argument_quotient.symbol != quotient.symbol {
            return None;
        }
    }

    Some(LegacyQuotientCallCandidate {
        quotient,
        operation,
    })
}

/// Identify a bare attached representative call solely for a precise
/// migration diagnostic. This does not resolve the call, validate arguments,
/// inspect proof machines, or grant any lift authority.
pub(crate) fn legacy_attached_quotient_call_candidate<'program>(
    program: &'program TypedTrees,
    receiver_type: TypeReferenceHandle,
    target: &str,
) -> Option<LegacyQuotientCallCandidate<'program>> {
    let quotient = quotient_for_type(program, receiver_type)?;
    let carrier_symbol = base_data_symbol(program, quotient.quotient.as_ref()?.carrier)?;
    let carrier =
        crate::validation::machine_calls::effect_inference::plan_scope::data_definition_by_symbol(
            program,
            carrier_symbol,
        )?;
    let operation = program.machines().iter().find(|machine| {
        machine
            .attached_data
            .as_ref()
            .is_some_and(|attached| attached.as_str() == carrier.name.as_str())
            && program
                .machine_states(machine)
                .iter()
                .any(|state| state.name.as_str() == target)
    })?;
    Some(LegacyQuotientCallCandidate {
        quotient,
        operation,
    })
}

pub(crate) fn quotient_for_type(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<&symbol_resolved_trees_to_typed_trees::typed_trees::data::DataDefinition> {
    let symbol = base_data_symbol(program, type_reference)?;
    program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == symbol && definition.quotient.is_some())
}
