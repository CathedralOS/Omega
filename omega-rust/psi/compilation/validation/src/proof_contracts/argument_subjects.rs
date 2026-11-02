//! Typed roots of declaration-owned nominal proposition arguments. This is
//! projection data, not a premise, read permission or executable observation.

/// The root a guaranteeing contract's occurrence re-anchors to at the caller:
/// the callee's reserved `result`, a bare occurrence of one of the target
/// state's parameters (by authored position), or anything else.
pub fn contract_argument_root(
    program: &dyn crate::CheckingInput,
    target_state: &typed_trees::state::State,
    occurrence: typed_trees::expression::ExpressionHandle,
) -> checked_trees::OutcomeOccurrenceRoot {
    if !program.expression_table.expression_is_valid(occurrence) {
        return checked_trees::OutcomeOccurrenceRoot::Other;
    }
    if program.expression_table.names_reserved_result(occurrence) {
        return checked_trees::OutcomeOccurrenceRoot::CalleeResult;
    }
    if let typed_trees::expression::ExpressionNode::Name(path) =
        program.expression_table.expression(occurrence)
        && path.symbol.is_valid()
        && let Some(position) = program
            .state_parameters(target_state)
            .iter()
            .position(|parameter| parameter.symbol == path.symbol)
        && let Ok(source_position) = u32::try_from(position)
    {
        return checked_trees::OutcomeOccurrenceRoot::CalleeParameter { source_position };
    }
    if let typed_trees::expression::ExpressionNode::Member(member) =
        program.expression_table.expression(occurrence)
        && member.member_symbol.is_valid()
        && member.case_variant.is_none()
    {
        let (source_position, mut fields) =
            match contract_argument_root(program, target_state, member.receiver) {
                checked_trees::OutcomeOccurrenceRoot::CalleeParameter { source_position } => {
                    (source_position, Vec::new())
                }
                checked_trees::OutcomeOccurrenceRoot::CalleeParameterProjection {
                    source_position,
                    fields,
                } => (source_position, fields),
                _ => return checked_trees::OutcomeOccurrenceRoot::Other,
            };
        fields.push(member.member_symbol);
        return checked_trees::OutcomeOccurrenceRoot::CalleeParameterProjection {
            source_position,
            fields,
        };
    }
    if program
        .closed_integer_value_in(occurrence, target_state.symbol)
        .is_some()
        || matches!(
            program.expression_table.expression(occurrence),
            typed_trees::expression::ExpressionNode::Boolean(_)
        )
    {
        return checked_trees::OutcomeOccurrenceRoot::ClosedValue;
    }
    checked_trees::OutcomeOccurrenceRoot::Other
}
