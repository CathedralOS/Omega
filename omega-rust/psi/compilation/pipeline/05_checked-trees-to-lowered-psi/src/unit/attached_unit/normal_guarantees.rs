//! Normal-return guarantees shared by ordinary and composed callable bodies.
//! Unit has no result value, but its post-state can establish caller facts.
//! Publication creates proof obligations; it does not assume the clauses.
//! Exit reconstruction joins the distinct contract result to the returned
//! value and proves guarantees from facts shared by every normal exit.
//! Crashes and incomplete calls establish neither a result nor these facts.

use crate::lowering_error::LoweringError;
use crate::scalar_graph::scalar_contracts::{self, ContractViewNamespace};
use crate::unit::ContractClause;
use checked_trees::CheckedTrees;
use symbols::SymbolHandle;
use terminal_psi::ValueDeclaration;

pub(crate) fn lower(
    checked: &CheckedTrees,
    state: SymbolHandle,
    parameters: &[ValueDeclaration],
    result: Option<ValueDeclaration>,
    erased: &[ValueDeclaration],
    views: &ContractViewNamespace<'_>,
    next_obligation: &mut u64,
) -> Result<Vec<ContractClause>, LoweringError> {
    let (source, source_state) = crate::declarations::state_declaration(checked, state)?;
    let contract = checked
        .facts
        .contract_plans
        .for_machine(source.symbol)
        .ok_or(LoweringError::Unsupported(
            "normal return has no checked contract",
        ))?;
    // A scalar result publishes its whole refined contract; a Unit or
    // structural result publishes only the rows its post-state validation
    // accepted, leaving memberships and evidence lanes to their owners.
    let refined;
    let post_state;
    let ensures = if result.is_some() {
        scalar_contracts::validate_guarantees(checked, source, source_state)?;
        refined = scalar_contracts::with_result_range(
            checked,
            state,
            parameters.len(),
            &contract.closed_scalar_values,
        )?;
        refined.ensures()
    } else {
        post_state =
            scalar_contracts::validate_post_state_guarantees(checked, source, source_state)?;
        &post_state[..]
    };
    // Returning execution restores the declared domain of every exclusive
    // loan: the declaration predicate roster's restored rows follow the
    // authored guarantees as the same kind of obligation, rederived from the
    // entry parameters' declared storage rather than read from a package.
    let mut guarantees = ensures.to_vec();
    guarantees.extend(
        validation::machine_declaration_predicates(
            &checked.typed,
            &checked.facts.operators,
            source,
        )
        .into_iter()
        .filter(|row| row.restored_on_return)
        .map(|row| {
            Some(checked_trees::ClosedScalarContractValue::Predicate(
                row.predicate,
            ))
        }),
    );
    let mut namespace = parameters.to_vec();
    namespace.extend(result);
    scalar_contracts::clauses(&guarantees, &namespace, erased, views)?
        .into_iter()
        .map(|proposition| {
            Ok(ContractClause {
                obligation: crate::terminal_identities::obligation_id(
                    crate::terminal_identities::allocate_dense(next_obligation)?,
                ),
                proposition,
            })
        })
        .collect()
}
