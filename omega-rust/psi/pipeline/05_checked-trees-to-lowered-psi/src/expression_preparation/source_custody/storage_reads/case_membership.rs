//! A valid case in the emitted sum does not prove it was the authored case.
//! Rejoin the selected nominal meaning and parameter occurrence before comparing
//! the checked observation. This also detects erasure or operand substitution.

use super::{
    CheckedTrees, ExpressionHandle, ExpressionNode, LoweringError, authored_state, unsupported,
};
pub(crate) fn authored(
    checked: &CheckedTrees,
    state: &checked_trees::state::State,
    expression: ExpressionHandle,
) -> Result<
    Option<(
        symbols::SymbolHandle,
        Vec<checked_trees::CheckedStructuralPredicatePathSegment>,
        String,
    )>,
    LoweringError,
> {
    let ExpressionNode::Binary(binary) = checked.expression_table.expression(expression) else {
        return Ok(None);
    };
    if !matches!(
        binary.operator,
        checked_trees::expression::BinaryOperator::Equal
            | checked_trees::expression::BinaryOperator::CaseMembership
    ) {
        return Ok(None);
    }
    let (machine, _) = authored_state(checked, state.symbol)?;
    if !validation::has_exact_case_membership_meaning(
        &checked.typed,
        machine,
        Some(state),
        expression,
        binary,
    ) {
        return Ok(None);
    }
    let mut subject = match checked.expression_table.expression(binary.left) {
        ExpressionNode::Borrow(borrow) => borrow.target,
        _ => binary.left,
    };
    let mut path = Vec::new();
    let mut visited = Vec::new();
    loop {
        if visited.contains(&subject) {
            return unsupported("case observation has a cyclic authored projection");
        }
        visited.push(subject);
        match checked.expression_table.expression(subject) {
            ExpressionNode::Member(member) => {
                if member.case_variant.is_some() || !member.member_symbol.is_valid() {
                    return Ok(None);
                }
                let field = validation::exact_self_field(&checked.typed, machine, subject)
                    .or_else(|| {
                        if matches!(checked.expression_table.expression(member.receiver),
                            ExpressionNode::Name(name) if name.symbol == machine.symbol
                                || matches!(checked.expression_table.name_path_members(name.members),
                                    [spelling] if spelling.is_self_receiver()))
                        {
                            return None;
                        }
                        let mut reference = validation::declared_place_type_raw(
                            &checked.typed,
                            machine,
                            Some(state),
                            member.receiver,
                        )?;
                        let owner = loop {
                            use checked_trees::types::TypeReferenceNode;
                            match checked.type_reference_table.type_reference(reference) {
                                TypeReferenceNode::Reference { referee, .. }
                                | TypeReferenceNode::Constrained {
                                    base_type: referee, ..
                                } => reference = *referee,
                                TypeReferenceNode::Named { symbol, .. }
                                | TypeReferenceNode::Generic {
                                    base_symbol: symbol,
                                    ..
                                } => break *symbol,
                                _ => return None,
                            }
                        };
                        let declaration = checked
                            .data_definitions()
                            .iter()
                            .find(|data| owner.is_valid() && data.symbol == owner)?;
                        // Ordinary members can name generated accessors. Their
                        // field identity comes from this exact receiver type.
                        validation::exact_data_member_field(
                            &checked.typed,
                            declaration,
                            if checked.symbols.get(member.member_symbol).kind
                                == symbols::SymbolKind::Field
                            {
                                member.member_symbol
                            } else {
                                symbols::SymbolHandle::invalid()
                            },
                            member.member.as_str(),
                            None,
                        )
                    })
                    .ok_or(LoweringError::Unsupported(
                        "case observation lost its authored field",
                    ))?;
                if field.relevance.is_erased() {
                    return Ok(None);
                }
                path.push(checked_trees::CheckedStructuralPredicatePathSegment::Field(
                    field
                        .identity
                        .map(|identity| format!("#{identity}"))
                        .unwrap_or_else(|| field.name.as_str().to_owned()),
                ));
                subject = member.receiver;
            }
            ExpressionNode::Indexed(indexed) => {
                if !validation::place_has_builtin_coordinates(
                    &checked.typed, machine, Some(state), subject,
                )
                    || checked.facts.operators.uses.iter().any(|(_, selected)| {
                        selected.expression == subject
                            && (selected.spelling != language_core::OperatorSpelling::Index
                                || selected.selected_operator_symbol.is_valid()
                                || selected.candidate_count != 0
                                || !matches!(selected.status,
                                    checked_trees::CheckedOperatorResolutionStatus::Missing
                                    | checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback))
                    })
                {
                    return Ok(None);
                }
                let Some(element_index) = checked
                    .expression_table
                    .constant_integer_value(indexed.index)
                    .and_then(|value| u64::try_from(value).ok())
                else {
                    return Ok(None);
                };
                path.push(
                    checked_trees::CheckedStructuralPredicatePathSegment::FixedIndex(element_index),
                );
                subject = indexed.collection;
            }
            _ => break,
        }
    }
    path.reverse();
    let ExpressionNode::Name(subject) = checked.expression_table.expression(subject) else {
        return Ok(None);
    };
    let subject = if checked
        .state_parameters(state)
        .iter()
        .any(|parameter| parameter.symbol == subject.symbol)
    {
        subject.symbol
    } else {
        // Authored `self` resolves to its declaring machine, whereas the
        // observation namespace names the receiver formal. Rejoin that exact
        // declaration; spelling alone or another machine cannot select it.
        if subject.symbol != machine.symbol
            || subject.head_symbol != machine.symbol
            || checked.symbols.get(machine.symbol).kind != symbols::SymbolKind::Machine
            || !matches!(checked.expression_table.name_path_members(subject.members),
                [name] if name.is_self_receiver())
            || !checked
                .machine_states(machine)
                .iter()
                .any(|candidate| candidate.symbol == state.symbol)
            || checked.symbols.get(state.symbol).parent != machine.symbol
        {
            return Ok(None);
        }
        let mut receivers = checked
            .state_parameters(state)
            .iter()
            .filter(|parameter| parameter.is_self);
        let Some(receiver) = receivers.next() else {
            return Ok(None);
        };
        if receivers.next().is_some()
            || receiver.is_const
            || !receiver.name.is_self_receiver()
            || checked.symbols.get(receiver.symbol).kind != symbols::SymbolKind::Parameter
            || checked.symbols.get(receiver.symbol).parent != state.symbol
            || checked
                .state_parameters(state)
                .iter()
                .filter(|parameter| parameter.symbol == receiver.symbol)
                .count()
                != 1
        {
            return Ok(None);
        }
        let mut reference = receiver.type_reference;
        let mut exact_receiver = false;
        for _ in 0..64 {
            use checked_trees::types::TypeReferenceNode;
            match checked.type_reference_table.type_reference(reference) {
                TypeReferenceNode::Reference { referee, .. } => reference = *referee,
                TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
                TypeReferenceNode::Named { symbol, .. } => {
                    exact_receiver =
                        *symbol == machine.symbol || *symbol == machine.attached_data_symbol;
                    break;
                }
                _ => break,
            }
        }
        if !exact_receiver
            || !checked
                .data_definitions()
                .iter()
                .any(|data| data.symbol.is_valid() && data.symbol == machine.attached_data_symbol)
        {
            return Ok(None);
        }
        receiver.symbol
    };
    let ExpressionNode::Name(selected) = checked.expression_table.expression(binary.right) else {
        return Ok(None);
    };
    let case = checked
        .data_definitions()
        .iter()
        .flat_map(|data| checked.data_members(data))
        .find_map(|member| match member {
            checked_trees::data::DataMember::Variant(case) if case.symbol == selected.symbol => {
                Some(case)
            }
            _ => None,
        })
        .ok_or(LoweringError::Unsupported(
            "case observation has no selected case declaration",
        ))?;
    let identity = case
        .identity
        .map(|identity| format!("#{identity}"))
        .unwrap_or_else(|| case.name.as_str().to_owned());
    Ok(Some((subject, path, identity)))
}
