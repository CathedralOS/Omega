//! A function's final expression can consume values still held by its body.
//! Follow that expression and its guards before deciding which local data can
//! be released. Each returning branch keeps its own moves and cleanup boundary.

use super::{Query, Results, Transfers, query_for_storage};
use checked_trees::{
    CheckFacts, CheckedScalarBranchDestination, CheckedScalarExpressionRole,
    CheckedUnitEffectOperationPlan, CheckedUnitStructuralArgumentSourcePlan,
    CheckedUnitStructuralPathSegment, ScalarReturnPath,
};
use symbols::SymbolHandle;

#[derive(Clone, Copy)]
pub(in crate::execution::terminal_unit) struct ScalarCompletion<'a> {
    pub state: SymbolHandle,
    pub paths: &'a [ScalarReturnPath<'a>],
}

pub(in crate::execution::terminal_unit) struct CompletionTransfers {
    pub prefix: Transfers,
    pub returns: Vec<(CheckedScalarBranchDestination, Transfers)>,
}

pub(in crate::execution::terminal_unit) fn at_completion(
    program: &dyn validation::CheckingInput,
    facts: &CheckFacts,
    prepared_calls: &crate::execution::terminal_unit::structural_value_calls::PreparedCalls,
    results: &Results,
    source: CheckedUnitStructuralArgumentSourcePlan,
    formation: u32,
    selection: &[CheckedUnitStructuralPathSegment],
    operations: &[CheckedUnitEffectOperationPlan],
    completion: ScalarCompletion<'_>,
) -> Result<CompletionTransfers, &'static str> {
    let query = query_for_storage(
        program,
        facts,
        completion.state,
        prepared_calls,
        results,
        source,
        formation,
        operations,
    );
    let (prefix, still_occupies_storage) = query
        .through_prefix(selection)
        .ok_or("completion cleanup: cannot follow this value through the body")?;
    if !still_occupies_storage {
        return Err("completion cleanup: this storage contains a later value");
    }
    let mut returns = Vec::with_capacity(completion.paths.len());
    for path in completion.paths {
        let mut transfers = Transfers::default();
        for ordinal in path.guard_ordinals() {
            transfers.append(query.scalar_role(
                completion.state,
                ordinal,
                CheckedScalarExpressionRole::Guard,
            )?);
        }
        let CheckedScalarBranchDestination::Return {
            statement_ordinal,
            is_continuation,
        } = path.destination
        else {
            return Err("completion cleanup: selected branch does not return a value");
        };
        let role = if is_continuation {
            CheckedScalarExpressionRole::ContinuationReturn
        } else {
            CheckedScalarExpressionRole::Return
        };
        transfers.append(query.scalar_role(completion.state, statement_ordinal, role)?);
        returns.push((path.destination.clone(), transfers));
    }
    Ok(CompletionTransfers { prefix, returns })
}

impl Query<'_> {
    fn scalar_role(
        &self,
        state: SymbolHandle,
        statement: u32,
        role: CheckedScalarExpressionRole,
    ) -> Result<Transfers, &'static str> {
        let mut roots = self
            .facts
            .values
            .scalar_computations
            .roots_for_statement_role(state, statement, role);
        let pure = self
            .facts
            .values
            .scalar_expressions
            .bound_expression_at(state, statement, role);
        match (roots.next(), pure) {
            (Some(root), None) if roots.next().is_none() => self
                .computation(root.root)
                .ok_or("completion cleanup: cannot follow the final expression's owned arguments"),
            (None, Some(_)) => Ok(Transfers::default()),
            (None, None) => Err("completion cleanup: final expression has no retained value"),
            _ => Err("completion cleanup: final expression has conflicting retained values"),
        }
    }
}
