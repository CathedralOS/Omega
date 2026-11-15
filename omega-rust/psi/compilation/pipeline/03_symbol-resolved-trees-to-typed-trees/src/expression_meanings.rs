//! Prepare declaration-level expression meanings from operands outward. A cast
//! may supply an operator's operand type, and an applied operator's result may
//! supply a cast's operand type; neither gets a second, shape-specific path.

use typed_trees::TypedTrees;

pub(crate) fn prepare(
    source: &symbol_resolved_trees::SymbolResolvedTrees,
    program: &mut TypedTrees,
    frontier: usize,
    type_frontier: usize,
    cast_destinations: &[typed_trees::types::TypeReferenceHandle],
) -> Result<(), diagnostics::Diagnostic> {
    enum Meaning {
        Cast(usize),
        Operator(typed_trees::expression::TableBinaryExpression),
    }
    let casts = crate::domain_cast_accounting::PreparedCasts::new(program, frontier)?;
    let mut sites = casts
        .sites()
        .map(|(handle, position)| (handle, Meaning::Cast(position)))
        .collect::<Vec<_>>();
    sites.extend(
        crate::operator_calls::sites(program, frontier)
            .into_iter()
            .map(|(handle, binary)| (handle, Meaning::Operator(binary))),
    );
    let mut type_values = program
        .type_reference_table
        .type_reference_rows_from(type_frontier)
        .filter_map(|(_, node)| match node {
            typed_trees::types::TypeReferenceNode::ConstExpression(expression)
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
    sites.sort_unstable_by_key(|(handle, _)| handle.arena_index());
    let owners = typed_trees::references::collect_program_operand_owners(program);
    let mut types = crate::typing_queries::DeclaredExpressionTypes::default();
    for (handle, meaning) in sites {
        match meaning {
            Meaning::Cast(position) => {
                casts.account(position, source, program, &owners, cast_destinations)?
            }
            Meaning::Operator(binary) => {
                crate::operator_calls::prepare_site(program, &owners, &mut types, handle, binary)?
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
    crate::contracts::proof_facts::normalize_membership_selections(program, &owners)?;
    crate::contracts::proof_facts::intern_proof_membership_instances(program, &owners)?;
    Ok(())
}
