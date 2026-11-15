//! Prepare declaration-level expression meanings from operands outward. A cast
//! may supply an operator's operand type, and an applied operator's result may
//! supply a cast's operand type; neither gets a second, shape-specific path.

use typed_trees::TypedTrees;

pub(crate) fn prepare(
    source: &symbol_resolved_trees::SymbolResolvedTrees,
    program: &mut TypedTrees,
    frontier: usize,
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
    let has_memberships = crate::contracts::proof_facts::has_pending_memberships(program);
    if sites.is_empty() && !has_memberships {
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
    crate::contracts::proof_facts::normalize_membership_selections(program, &owners)?;
    crate::contracts::proof_facts::intern_proof_membership_instances(program, &owners)?;
    Ok(())
}
