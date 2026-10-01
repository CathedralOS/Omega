//! Bounded byte locals own a snapshot of their initializer, not a borrow of
//! its operands. Literals, readable places and built-in concatenation share
//! the byte-value plan used by stores. Construction proves a capacity bound
//! and the declared predicates; it neither truncates nor pads live contents.
//! Raw fixed arrays retain their exact-width construction rule.
//!
//! The declaration's mutability affects subsequent writes, not construction.
//! Each write must preserve its predicates; initializer contents are not
//! immutable facts about the resulting storage.
use super::{CheckedStructuralAccess, Multiplicity, TypedTrees};
use crate::execution::terminal_unit::types::{
    byte_sequence_carrier, structural_access_for_type_reference,
};
use checked_trees::CheckedByteSequenceCarrier;

/// Plan the initializer under the declaration's capacity and byte predicates.
/// The caller owns the result binding and publishes ordinary establishment.
pub(in crate::execution::terminal_unit) fn value_plan(
    program: &TypedTrees,
    operators: &checked_trees::CheckedOperatorFacts,
    state: &typed_trees::state::State,
    statement_index: u32,
    local: &typed_trees::statement::TableLocalData,
) -> Option<checked_trees::CheckedByteSequenceValuePlan> {
    if !program
        .expression_table
        .expression_is_valid(local.initial_value)
        || program.type_multiplicity(local.type_reference) != Multiplicity::Unrestricted
        || structural_access_for_type_reference(program, local.type_reference)
            != Some(CheckedStructuralAccess::Owned)
    {
        return None;
    }
    let CheckedByteSequenceCarrier::BoundedOwned { capacity } =
        byte_sequence_carrier(program, local.type_reference, &[])?
    else {
        return None;
    };
    let domains =
        crate::facts::field_domain::domain_constraint_symbols(program, local.type_reference);
    let plain_byte_domain = |symbol| {
        program
            .domain_definitions()
            .iter()
            .find(|domain| domain.symbol == symbol)
            .is_some_and(|domain| {
                domain.establishment_routes.is_empty()
                    && domain.semantic_roles == Default::default()
            })
            && typed_trees::byte_predicates::domain_byte_predicate(program, symbol).is_some()
    };
    if !domains.iter().copied().all(plain_byte_domain) {
        return None;
    }
    let mut value = checked_trees::CheckedByteSequenceValuePlan::default();
    let (root, maximum_length) =
        crate::execution::terminal_unit::structural_scalar_store::byte_value_plan(
            program,
            operators,
            state,
            statement_index,
            local.initial_value,
            &domains,
            &mut value,
            0,
        )?;
    if maximum_length > capacity {
        return None;
    }
    value.root = root;
    Some(value)
}

/// Literal construction is shared by local establishment and a selected
/// normal result. Neither site gains a view or infers an encoding domain.
pub(in crate::execution::terminal_unit) fn expression_octets(
    program: &TypedTrees,
    reference: typed_trees::types::TypeReferenceHandle,
    expression: typed_trees::expression::ExpressionHandle,
) -> Option<Vec<u8>> {
    if !program.expression_table.expression_is_valid(expression)
        || program.type_multiplicity(reference) != Multiplicity::Unrestricted
        || structural_access_for_type_reference(program, reference)
            != Some(CheckedStructuralAccess::Owned)
    {
        return None;
    }
    let CheckedByteSequenceCarrier::BoundedOwned { capacity } =
        byte_sequence_carrier(program, reference, &[])?
    else {
        return None;
    };
    let typed_trees::expression::ExpressionNode::String(bytes) = program.expression_table.expression(expression) else {
        return None;
    };
    if u64::try_from(bytes.len()).ok()? > capacity {
        return None;
    }
    let domains = crate::facts::field_domain::domain_constraint_symbols(program, reference);
    let plain_literal_domain = |symbol| {
        program
            .domain_definitions()
            .iter()
            .find(|domain| domain.symbol == symbol)
            .is_some_and(|domain| {
                domain.establishment_routes.is_empty()
                    && domain.semantic_roles == Default::default()
            })
            && crate::facts::field_domain::string_literal_expression_grants_domain(
                program, expression, symbol,
            )
    };
    domains
        .into_iter()
        .all(plain_literal_domain)
        .then(|| bytes.to_vec())
}

#[cfg(test)]
mod tests;
