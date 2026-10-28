//! Formation consumes the actual ordered inputs before an operation executes.
//! Closed private arrivals use the same observation and all-path rule as
//! operand landing. The operation's value and guarantees are never premises.

use super::structural_judgment::normal_returns::OperationOperandDemand;
use super::structural_prefix::{PrefixObservation, StructuralPrefixSite, StructuralPrefixVisitor};
use typed_trees::expression::ExpressionHandle;
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::TypeReferenceHandle;

/// Exclude a simultaneous assignment to completed operation operands. Operand
/// positions keep their evaluation snapshots, including mutations by siblings.
pub(crate) fn operation_inputs_exclude_integer_values(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    excluded: &[(usize, i64)],
) -> bool {
    let Some(binary) = program.expression_table.binary_node(expression) else {
        return false;
    };
    let references = [binary.left, binary.right]
        .map(|operand| crate::expression_result_type_reference(program, machine, state, operand));
    let [Some(left), Some(right)] = references else {
        return false;
    };
    let Some(statement_index) =
        super::completed_values::expression_statement_index(program, state, expression)
    else {
        return false;
    };
    if excluded.is_empty() || excluded.iter().any(|(position, _)| *position >= 2) {
        return false;
    }
    let mut visitor = InputExclusion {
        site: StructuralPrefixSite {
            state: state.symbol,
            statement_index,
            in_transition_target: super::structural_prefix::expression_is_transition_target(
                program,
                state,
                statement_index,
                expression,
            ),
        },
        expression,
        references: [left, right],
        excluded,
        seen: false,
        proven: true,
    };
    if super::structural_prefix::observe_runtime_state_prefix(program, machine, state, &mut visitor)
        .is_some()
        && visitor.has_positive_proof()
    {
        return true;
    }
    visitor.seen = false;
    visitor.proven = true;
    super::structural_prefix::observe_private_arrivals(program, machine, &mut visitor, false)
        == Some(true)
        && visitor.has_positive_proof()
}

#[derive(Clone)]
struct InputExclusion<'a> {
    site: StructuralPrefixSite,
    expression: ExpressionHandle,
    references: [TypeReferenceHandle; 2],
    excluded: &'a [(usize, i64)],
    seen: bool,
    proven: bool,
}

impl StructuralPrefixVisitor for InputExclusion<'_> {
    fn has_positive_proof(&self) -> bool {
        self.seen && self.proven
    }

    fn site(&self) -> StructuralPrefixSite {
        self.site
    }

    fn entered(&mut self) {
        self.seen = true;
    }

    fn unresolved(&mut self, _machine: &Machine) {
        self.proven = false;
    }

    fn observe<'program>(&mut self, observation: PrefixObservation<'_, 'program>) {
        let (_, machine, state, judge, environment) = observation;
        let Some(entries) = judge.normal_operation_entries(
            self.site.statement_index,
            self.expression,
            OperationOperandDemand::All,
            environment,
            (machine, state, self.references[0]),
        ) else {
            self.proven = false;
            return;
        };
        if entries.is_empty() {
            self.proven = false;
        }
        for entry in entries {
            if entry.expression != self.expression || entry.arguments.len() != 2 {
                self.proven = false;
                continue;
            }
            let values = self
                .excluded
                .iter()
                .map(|&(position, excluded)| {
                    entry.arguments[position]
                        .clone()
                        .map(|value| (self.references[position], value, excluded))
                })
                .collect::<Option<Vec<_>>>();
            self.proven &= values
                .as_deref()
                .is_some_and(|values| entry.judge.observed_integer_values_exclude(values));
        }
    }
}
