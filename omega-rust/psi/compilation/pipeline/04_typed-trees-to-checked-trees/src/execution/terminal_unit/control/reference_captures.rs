//! Local reference establishments consume the common occurrence source capture.
//! Local binding eligibility is separate from the captured source, storage
//! interpretation and selectors used by every reference-value position.

pub(in crate::execution::terminal_unit) use crate::values::reference_sources::ReferenceCapture;

pub(in crate::execution::terminal_unit) fn plan(
    program: &dyn validation::CheckingInput,
    facts: &checked_trees::CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: usize,
    local: &typed_trees::statement::TableLocalData,
) -> Option<ReferenceCapture> {
    if local.is_mutable || local.relevance.is_erased() {
        return None;
    }
    // A retained value graph owns executable selector evaluations, including
    // children whose proven coordinates are fixed. The direct local capture
    // cannot steal that schedule or replay it while reconstructing a loan.
    if let Some(root) = facts.values.structural_values.root_for_expression(
        state.symbol,
        u32::try_from(statement_index).ok()?,
        local.initial_value,
    ) && let checked_trees::CheckedStructuralValueKind::Reference { selectors, .. } =
        facts.values.structural_values.nodes.get(root.root).kind
        && !selectors.is_empty()
    {
        return None;
    }
    let selector = |expression| {
        let reference = validation::expression_type_reference_in_state(
            program,
            state.symbol,
            statement_index,
            expression,
        )?;
        let primitive = program.primitive_type_reference(reference)?;
        if !primitive.is_fixed_integer() {
            return None;
        }
        let checked_trees::CheckedScalarExpression::Local { position, .. } =
            crate::values::lower_unit_scalar_argument(
                program,
                &facts.operators,
                state,
                statement_index,
                expression,
                primitive,
            )?
        else {
            return None;
        };
        Some(
            checked_trees::CheckedUnitStructuralPathSegment::RuntimeIndex(
                checked_trees::CheckedRuntimeIndex::Local {
                    position: u32::try_from(position).ok()?,
                },
            ),
        )
    };
    let capture = crate::values::reference_sources::capture(
        program,
        machine,
        state,
        statement_index,
        local.initial_value,
        local.type_reference,
        &selector,
    )?;
    // This direct establishment can only borrow an existing home. Computed
    // backing belongs to the structural value graph, which retains and
    // establishes its producer before the reference and retires it afterward.
    if matches!(
        capture.source.source,
        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralExpression { .. }
    ) {
        return None;
    }
    // Primitive homes have their own existing local producer. A structural
    // capture must not steal those bindings merely because it can name a leaf.
    super::super::types::base_type_identity(program, capture.backing_type, &[], &[])?;
    Some(capture)
}
