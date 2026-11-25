//! Prepare declaration-level expression meanings from operands outward. A cast
//! may supply an operator's operand type; an initialized local may supply a
//! member's receiver type. Each consumer reads its operands' retained types.

use crate::source_state::TypedTrees;

pub(crate) fn prepare(
    source: &crate::source_resolved::SymbolResolvedTrees,
    program: &mut TypedTrees,
    frontier: usize,
    type_frontier: usize,
    cast_destinations: &[crate::source_state::types::TypeReferenceHandle],
) -> Result<(), diagnostics::Diagnostic> {
    enum Meaning {
        Cast(usize),
        Operator,
        Member(crate::source_state::expression::TableMemberExpression),
        Range,
        Local(
            crate::source_state::statement::StatementHandle,
            symbols::SymbolHandle,
        ),
    }
    let casts = crate::domain_cast_accounting::PreparedCasts::new(program, frontier)?;
    let mut sites = casts
        .sites()
        .map(|(handle, position)| (handle, Meaning::Cast(position)))
        .collect::<Vec<_>>();
    sites.extend(
        crate::operator_calls::sites(program, frontier)
            .into_iter()
            .map(|handle| (handle, Meaning::Operator)),
    );
    sites.extend(
        program
            .expression_table
            .iter_backing_expressions()
            .filter_map(|(handle, node)| match node {
                crate::source_state::expression::ExpressionNode::Indexed(indexed)
                    if handle.arena_index() as usize > frontier
                        && program.expression_table.range_node(indexed.index).is_some() =>
                {
                    Some((handle, Meaning::Range))
                }
                _ => None,
            }),
    );
    sites.extend(
        program
            .expression_table
            .iter_backing_expressions()
            .filter_map(|(handle, node)| match node {
                crate::source_state::expression::ExpressionNode::Member(member)
                    if handle.arena_index() as usize > frontier
                        && member.selection
                            == crate::source_state::expression::MemberSelection::Unresolved =>
                {
                    Some((handle, Meaning::Member(member.clone())))
                }
                _ => None,
            }),
    );
    for machine in program.machines() {
        for state in program.machine_states(machine) {
            sites.extend(
                program
                    .statement_table
                    .iter_statements(state.statement_nodes)
                    .filter_map(|(statement, node)| {
                        let crate::source_state::statement::StatementNode::LocalData(local) = node
                        else {
                            return None;
                        };
                        (local.type_is_inferred
                            && local.role.has_ordinary_initializer()
                            && !local.type_reference.is_valid()
                            && program
                                .expression_table
                                .expression_is_valid(local.initial_value)
                            && local.initial_value.arena_index() as usize > frontier)
                            .then_some((
                                local.initial_value,
                                Meaning::Local(statement, machine.symbol),
                            ))
                    }),
            );
        }
    }
    let mut type_values = program
        .type_reference_table
        .type_reference_rows_from(type_frontier)
        .filter_map(|(_, node)| match node {
            crate::source_state::types::TypeReferenceNode::ConstExpression(expression)
                if program.expression_table.expression_is_valid(*expression)
                    && expression.arena_index() as usize > frontier =>
            {
                Some(*expression)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    type_values.sort_unstable_by_key(|expression| expression.arena_index());
    type_values.dedup();
    let has_memberships = crate::contracts::proof_facts::has_pending_memberships(program);
    if sites.is_empty() && type_values.is_empty() && !has_memberships {
        return Ok(());
    }
    // Typing allocates operands before their consumers. A simultaneous cast
    // run is one operation, positioned at its first entry, just as before.
    // Complete a local after its initializer and before names that read it;
    // no earlier query can depend on that newly published binding.
    sites.sort_unstable_by_key(|(handle, meaning)| {
        (handle.arena_index(), matches!(meaning, Meaning::Local(..)))
    });
    let owners = crate::source_state::references::collect_program_operand_owners(program);
    let mut types = crate::typing_queries::DeclaredExpressionTypes::default();
    for (handle, meaning) in sites {
        match meaning {
            Meaning::Cast(position) => {
                casts.account(position, source, program, &owners, cast_destinations)?
            }
            Meaning::Member(member) => {
                prepare_member(program, &owners, &mut types, handle, &member)?;
            }
            Meaning::Operator => {
                crate::operator_calls::prepare_site(program, &owners, &mut types, handle)?
            }
            // A range a declared `[..]` selected is already its call.
            Meaning::Range
                if !matches!(
                    program.expression_table.expression(handle),
                    crate::source_state::expression::ExpressionNode::Indexed(_)
                ) => {}
            Meaning::Range => {
                crate::constructor_applications::retain_indexed_operand_type(
                    program, &owners, &mut types, handle,
                )?;
            }
            Meaning::Local(statement, owner) => {
                // A cast or selected operator can acquire its result only here.
                // Publish that result on an inferred local before later operands
                // read it. A real declared receiving type is never replaced.
                if let Some(reference) = types.get(program, owner, handle) {
                    let crate::source_state::statement::StatementNode::LocalData(local) =
                        program.statement_table.statement_mut(statement)
                    else {
                        unreachable!("prepared local statement retains its kind")
                    };
                    local.type_reference = reference;
                }
            }
        }
    }
    // Evaluation runs these expressions later. Give it the type selection
    // already known at this source occurrence.
    for expression in type_values {
        let mut carriers = owners
            .expression_owners(expression)
            .map(|owner| types.get(program, owner, expression));
        if let Some(Some(carrier)) = carriers.next()
            && carriers.all(|other| other == Some(carrier))
        {
            program
                .expression_table
                .set_constructed_value_type_reference(expression, carrier);
        }
    }
    crate::contracts::proof_facts::normalize_membership_selections(
        source, program, &owners,
    )?;
    crate::contracts::proof_facts::intern_proof_membership_instances(program, &owners)?;
    Ok(())
}

/// Complete the same member selection once the receiver's actual type exists.
/// Access permission and live-value checks remain with the receiving judgment.
fn prepare_member(
    program: &mut TypedTrees,
    owners: &crate::source_state::references::ProgramOperandOwners,
    types: &mut crate::typing_queries::DeclaredExpressionTypes,
    handle: crate::source_state::expression::ExpressionHandle,
    member: &crate::source_state::expression::TableMemberExpression,
) -> Result<(), diagnostics::Diagnostic> {
    use crate::source_state::expression::{CollectionMeasure, ExpressionNode, MemberSelection};
    use crate::source_state::types::TypeReferenceNode;

    if member.case_variant.is_some() {
        return Ok(());
    }
    let mut receivers = owners
        .expression_owners(handle)
        .map(|owner| types.get(program, owner, member.receiver));
    let Some(Some(receiver)) = receivers.next() else {
        return Ok(());
    };
    if !receivers.all(|other| other == Some(receiver)) {
        return Ok(());
    }
    let selected = crate::typing_queries::declared_projection_field(
        program,
        receiver,
        member.member.as_str(),
        Default::default(),
    )
    .ok()
    .map(|field| MemberSelection::Field(field.symbol));
    let selected = selected.or_else(|| {
        let receiver = program.type_reference_table.peeled_type_reference(receiver);
        (matches!(
            program.type_reference_table.type_reference(receiver),
            TypeReferenceNode::Slice { .. } | TypeReferenceNode::FixedArray { .. }
        ) && CollectionMeasure::from_authored_spelling(member.member.as_str())
            == Some(CollectionMeasure::Length))
        .then_some(MemberSelection::CollectionMeasure(
            CollectionMeasure::Length,
        ))
    });
    let Some(selected) = selected else {
        return Ok(());
    };
    if let ExpressionNode::Member(member) = program.expression_table.expression_mut(handle) {
        member.selection = selected;
    }
    crate::expressions::expression::table::member_binding::finalize_authored_member_selection(
        program, handle,
    )
}
