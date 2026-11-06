//! Each reference uses the requirements and loans recorded for its creation.
//! Prepare one immutable lookup per state so calls and reference emission can
//! select the exact occurrence without rescanning earlier references or loans.
use crate::shared::lowering_error::LoweringError;
use checked_trees::CheckedTrees;
use symbols::SymbolHandle;

/// Source checking records these duties before emission starts. One sorted
/// inventory lets calls select their actual operands without searching every
/// reference formed earlier in the body.
#[derive(Debug, Clone)]
pub(crate) struct ReferenceFormations {
    rows: Vec<SourceReferenceRequirement>,
    loans: Vec<(
        (usize, u32, u32),
        arena::Handle<checked_trees::BorrowLoanFact>,
    )>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SourceReferenceRequirement {
    statement_index: usize,
    expression: checked_trees::expression::ExpressionHandle,
    requirements: Vec<checked_trees::CheckedSourceQualification>,
}

impl ReferenceFormations {
    pub(crate) fn prepare<'a>(
        checked: &CheckedTrees,
        state: SymbolHandle,
        operations: impl Iterator<Item = &'a checked_trees::CheckedUnitEffectOperationPlan>,
        edges: impl Iterator<Item = &'a checked_trees::CheckedStructuralControlSuccessorPlan>,
    ) -> Result<Self, LoweringError> {
        let mut rows = Vec::new();
        let mut record =
            |statement_index: u32,
             expression,
             contract: &checked_trees::CheckedReferenceSourceContract| {
                if let checked_trees::CheckedReferenceSourceContract::Standing {
                    source_requirements,
                    ..
                } = contract
                {
                    rows.push(SourceReferenceRequirement {
                        statement_index: statement_index as usize,
                        expression,
                        requirements: source_requirements.clone(),
                    });
                }
            };
        for operation in operations {
            if let checked_trees::CheckedUnitEffectOperationPlan::EstablishReference {
                formation_expression,
                source_contract,
                result,
                ..
            } = operation
            {
                record(
                    result.statement_index,
                    *formation_expression,
                    source_contract,
                );
            }
        }
        for edge in edges {
            for transfer in &edge.transfers {
                if let checked_trees::CheckedStructuralControlTransferSourcePlan::ProjectedLoan {
                    formation_expression,
                    source_contract,
                    ..
                } = &transfer.source
                {
                    record(
                        edge.statement_ordinal,
                        *formation_expression,
                        source_contract,
                    );
                }
            }
        }
        let values = &checked.facts.values.structural_values;
        values.with_state_nodes(state, |handles| {
            for handle in handles {
                let value = values.nodes.get(*handle);
                if let checked_trees::CheckedStructuralValueKind::Reference {
                    source_contract,
                    ..
                } = &value.kind
                {
                    record(value.statement_index, value.expression, source_contract);
                }
            }
        });
        rows.sort_by_key(|row| {
            (
                row.statement_index,
                row.expression.arena_index(),
                row.expression.generation(),
            )
        });
        for pair in rows.windows(2) {
            if (pair[0].statement_index, pair[0].expression)
                == (pair[1].statement_index, pair[1].expression)
                && pair[0].requirements != pair[1].requirements
            {
                let (machine, _) = crate::shared::declarations::state_declaration(checked, state)?;
                return Err(LoweringError::at_statement(
                    checked,
                    machine.symbol,
                    state,
                    pair[0].statement_index as u32,
                    "reference occurrence has conflicting source requirements",
                ));
            }
        }
        rows.dedup();
        let (machine, _) = crate::shared::declarations::state_declaration(checked, state)?;
        let borrow = &checked.facts.borrow;
        let mut loans = borrow
            .unique_state_borrow(machine.symbol, state)
            .into_iter()
            .flat_map(|state| borrow.state_loans(state))
            .filter(|(_, loan)| loan.formation_expression.is_valid())
            .map(|(handle, loan)| {
                (
                    formation_key(loan.statement_index, loan.formation_expression),
                    handle,
                )
            })
            .collect::<Vec<_>>();
        loans.sort_by_key(|(key, _)| *key);
        Ok(Self { rows, loans })
    }

    pub(crate) fn find(
        &self,
        statement: usize,
        expression: checked_trees::expression::ExpressionHandle,
    ) -> Option<usize> {
        self.rows
            .binary_search_by_key(
                &(statement, expression.arena_index(), expression.generation()),
                |row| {
                    (
                        row.statement_index,
                        row.expression.arena_index(),
                        row.expression.generation(),
                    )
                },
            )
            .ok()
    }
    pub(crate) fn requirements(
        &self,
        index: usize,
    ) -> &[checked_trees::CheckedSourceQualification] {
        &self.rows[index].requirements
    }

    /// Captures retain the checked loan identity; lookup never replays the
    /// source expression or searches a different statement's descriptor rows.
    pub(crate) fn loans_for(
        &self,
        statement: usize,
        expression: checked_trees::expression::ExpressionHandle,
    ) -> impl Iterator<Item = arena::Handle<checked_trees::BorrowLoanFact>> + '_ {
        let key = formation_key(statement, expression);
        let start = self
            .loans
            .partition_point(|(candidate, _)| *candidate < key);
        let end = self
            .loans
            .partition_point(|(candidate, _)| *candidate <= key);
        self.loans[start..end].iter().map(|(_, loan)| *loan)
    }
}

fn formation_key(
    statement: usize,
    expression: checked_trees::expression::ExpressionHandle,
) -> (usize, u32, u32) {
    (statement, expression.arena_index(), expression.generation())
}
