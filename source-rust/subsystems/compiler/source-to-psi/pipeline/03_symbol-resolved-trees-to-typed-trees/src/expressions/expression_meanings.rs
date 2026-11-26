//! Prepare declaration-level expression meanings from operands outward. A cast
//! may supply an operator's operand type; an initialized local may supply a
//! member's receiver type. Each consumer reads its operands' retained types.

use crate::source_state::TypedTrees;

#[cfg(test)]
pub(crate) fn prepare(
    source: &crate::source_resolved::SymbolResolvedTrees,
    program: &mut TypedTrees,
    frontier: usize,
    type_frontier: usize,
    cast_destinations: &[crate::source_state::types::TypeReferenceHandle],
) -> Result<(), diagnostics::Diagnostic> {
    let cohort = crate::lowerer::TypingCohort::for_expression_meanings(frontier, type_frontier);
    let owners = crate::expressions::operand_owners::CohortOperandOwners::full(program);
    prepare_in_cohort(source, program, &cohort, &owners, cast_destinations)
}

pub(crate) fn prepare_in_cohort(
    source: &crate::source_resolved::SymbolResolvedTrees,
    program: &mut TypedTrees,
    cohort: &crate::lowerer::TypingCohort<'_>,
    owners: &crate::expressions::operand_owners::CohortOperandOwners,
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
    let frontier = cohort.typed_expressions;
    let type_frontier = cohort.typed_type_references;
    let mut cast_sites = Vec::new();
    let mut operator_sites = Vec::new();
    let mut range_sites = Vec::new();
    let mut member_sites = Vec::new();
    for (handle, node) in program.expression_table.iter_expressions_from(frontier + 1) {
        if crate::expressions::operator_calls::forms(program, handle)[0].is_some() {
            operator_sites.push(handle);
        }
        match node {
            crate::source_state::expression::ExpressionNode::Cast(cast)
                if cast.form.entry().is_some() =>
            {
                cast_sites.push((handle, *cast));
            }
            crate::source_state::expression::ExpressionNode::Indexed(indexed)
                if program.expression_table.range_node(indexed.index).is_some() =>
            {
                range_sites.push(handle);
            }
            crate::source_state::expression::ExpressionNode::Member(member)
                if member.selection
                    == crate::source_state::expression::MemberSelection::Unresolved =>
            {
                member_sites.push((handle, member.clone()));
            }
            _ => {}
        }
    }
    let casts =
        crate::types::domain_cast_accounting::PreparedCasts::from_sites(program, cast_sites)?;
    let mut sites = casts
        .sites()
        .map(|(handle, position)| (handle, Meaning::Cast(position)))
        .collect::<Vec<_>>();
    sites.extend(
        operator_sites
            .into_iter()
            .map(|handle| (handle, Meaning::Operator)),
    );
    sites.extend(
        range_sites
            .into_iter()
            .map(|handle| (handle, Meaning::Range)),
    );
    sites.extend(
        member_sites
            .into_iter()
            .map(|(handle, member)| (handle, Meaning::Member(member))),
    );
    for (ordinal, machine) in program.machines().iter().enumerate() {
        if !cohort.owns_machine(ordinal, machine.symbol) {
            continue;
        }
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
    let proof_fact_frontier = cohort.typed_proof_facts;
    let has_memberships =
        crate::contracts::proof_facts::has_pending_memberships_from(program, proof_fact_frontier);
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
    // A refinement's self membership selects its parent on the declaration's
    // known carrier. A cast may already need that selection while typing its
    // operand. Memberships on expressions must wait for their operand types.
    let mut subjects = std::collections::HashSet::new();
    if has_memberships {
        for domain in program
            .domain_definitions()
            .iter()
            .skip(cohort.domain_definitions)
        {
            for handle in domain.facts.handles() {
                if handle.arena_index() as usize > proof_fact_frontier
                    && program
                        .proof_facts
                        .get(handle)
                        .as_membership()
                        .is_some_and(|fact| {
                            crate::source_state::domain::domain_subject_reference(
                                program, domain, fact.value,
                            )
                        })
                {
                    subjects.insert(handle);
                }
            }
        }
    }
    if !subjects.is_empty() {
        crate::contracts::proof_facts::normalize_membership_selections(
            source,
            program,
            owners,
            proof_fact_frontier,
            |fact| subjects.contains(&fact),
        )?;
        crate::contracts::proof_facts::intern_proof_membership_instances(
            program,
            owners,
            proof_fact_frontier,
            |fact| subjects.contains(&fact),
        )?;
    }
    let mut types = crate::types::typing_queries::DeclaredExpressionTypes::default();
    for (handle, meaning) in sites {
        match meaning {
            Meaning::Cast(position) => {
                casts.account(position, source, program, owners, cast_destinations)?
            }
            Meaning::Member(member) => {
                prepare_member(program, owners, &mut types, handle, &member)?;
            }
            Meaning::Operator => crate::expressions::operator_calls::prepare_site(
                program, owners, &mut types, handle,
            )?,
            // A range a declared `[..]` selected is already its call.
            Meaning::Range
                if !matches!(
                    program.expression_table.expression(handle),
                    crate::source_state::expression::ExpressionNode::Indexed(_)
                ) => {}
            Meaning::Range => {
                crate::declarations::constructor_applications::retain_indexed_operand_type(
                    program, owners, &mut types, handle,
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
    if has_memberships {
        crate::contracts::proof_facts::normalize_membership_selections(
            source,
            program,
            owners,
            proof_fact_frontier,
            |fact| !subjects.contains(&fact),
        )?;
        crate::contracts::proof_facts::intern_proof_membership_instances(
            program,
            owners,
            proof_fact_frontier,
            |fact| !subjects.contains(&fact),
        )?;
    }
    Ok(())
}

/// Complete the same member selection once the receiver's actual type exists.
/// Access permission and live-value checks remain with the receiving judgment.
fn prepare_member(
    program: &mut TypedTrees,
    owners: &crate::expressions::operand_owners::CohortOperandOwners,
    types: &mut crate::types::typing_queries::DeclaredExpressionTypes,
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
    let selected = crate::types::typing_queries::declared_projection_field(
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
