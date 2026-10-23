//! Typed builtin element observations, shared by authored contracts and
//! completed runtime operands. This is value identity, not a reference-origin
//! recovery route. Prefix callers separately preserve the observed storage.

use super::StructuralTerm;
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;
use typed_trees::signature::StateParameter;
use typed_trees::state::State;
use typed_trees::types::TypeReferenceHandle;

pub(in crate::proof_contracts::contract_entailment) fn indexed_term_in_scope(
    program: &dyn crate::CheckingInput,
    scope: Option<(&Machine, &State, TypeReferenceHandle)>,
    frame: Option<(SymbolHandle, &[StateParameter])>,
    expression: ExpressionHandle,
    collection: StructuralTerm,
    index: StructuralTerm,
) -> Option<StructuralTerm> {
    let indexed = program.expression_table.indexed_node(expression)?;
    if program.expression_table.indexed_on_range(indexed) {
        return None;
    }
    let state_scope = scope
        .map(|(machine, state, _)| (machine, state))
        .or_else(|| {
            super::binary::expression_scope(program, expression, 0)
                .and_then(|symbol| super::binary::scoped_machine_state(program, symbol))
        });
    let owner = frame
        .map(|(owner, _)| owner)
        .or_else(|| state_scope.map(|(machine, _)| machine.symbol))?;
    let expression_type = |expression| {
        if let Some((owner, parameters)) = frame {
            crate::parameter_expression_result_type_reference(
                program, owner, parameters, expression,
            )
        } else {
            let (machine, state) = state_scope?;
            crate::expression_result_type_reference(program, machine, state, expression)
        }
    };
    let references = [indexed.collection, indexed.index].map(expression_type);
    let spelling = language_core::OperatorSpelling::Index;
    if references[0].is_none()
        || !typed_trees::operator::has_builtin_spelled_expression_meaning(
            program,
            owner,
            expression,
            spelling,
            &references,
        )
        || !typed_trees::operator::resolve_indexed_spelling_for_operands(
            program,
            spelling,
            &references,
            program.expression_table.source_span(expression),
        )
        .is_empty()
    {
        return None;
    }
    let element_type = expression_type(expression)?;
    // The collection's storage must be observed before selecting its element.
    // A stored reference contributes descriptor identity only: selecting it
    // supplies no live-origin receipt for the contents behind that permission.
    if !super::scalar_operand_is_observed(program, references[0]?, &collection)
        || references[1]
            .is_some_and(|reference| !super::scalar_operand_is_observed(program, reference, &index))
    {
        return None;
    }
    let owned = super::owned_observation_type(program, &collection).is_some();
    let selected = StructuralTerm::IndexedValue {
        element_type,
        element_identity: program.normalized_type_identity(element_type),
        collection: Box::new(if owned {
            collection.into_semantic_value()
        } else {
            collection
        }),
        index: Box::new(index),
    };
    Some(if crate::reference_shell(program, element_type).is_some() {
        super::descriptor_only_observation(element_type, selected)
    } else if owned {
        super::owned_projection_observation(program, element_type, selected)
    } else {
        selected
    })
}
