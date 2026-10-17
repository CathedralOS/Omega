//! Batch arithmetic observations from the existing normal-value path owner.
//!
//! Field syntax is not a value identity. The completed operand owns its read
//! epoch, including reads before a later sibling call writes the same storage.
//! This owner's semantic keys preserve completed values across query prefixes;
//! disagreeing normal paths publish no observation. Descriptor measures remain
//! with the collection observation owner and never enter this field roster.

use super::ScopedArithmeticValue;
use super::citations::CitationFacts;
use super::structural_case_arms::structural_arm_judge;
use super::structural_judgment::{StructuralJudge, StructuralTerm};
use super::structural_prefix::{
    StructuralPrefixSite, StructuralPrefixVisitor, observe_runtime_state_prefix,
};
use symbols::SymbolHandle;
use typed_trees::expression::ExpressionHandle;
use typed_trees::{machine::Machine, state::State};

/// One actual declared-field read, not a current-place or initializer lookup.
#[derive(Clone)]
pub struct CapturedScalarObservation {
    pub expression: ExpressionHandle,
    pub value: ScopedArithmeticValue,
}

/// Capture the finite field-read roster once at this executed prefix. The
/// normal-value owner supplies source ordering, branch joins and call effects;
/// this adapter neither evaluates a second operand language nor unfolds callees.
pub fn captured_scalar_observations(
    program: &dyn crate::CheckingInput,
    machine: &Machine,
    state: &State,
    statement_index: usize,
    in_transition_target: bool,
) -> Vec<CapturedScalarObservation> {
    let mut candidates = Vec::new();
    for statement in program.statement_table.statements(state.statement_nodes) {
        for (_, root) in program.tables.expression_roots(statement) {
            let mut expressions = Vec::new();
            crate::expression_traversal::collect_expression_tree(program, root, &mut expressions);
            for expression in expressions {
                if super::observed_projections::declared_integer_field_read(
                    program, machine, state, expression,
                )
                .is_some()
                    && !candidates.contains(&expression)
                {
                    candidates.push(expression);
                }
            }
        }
    }
    if candidates.is_empty() {
        return Vec::new();
    }
    let mut visitor = Observations {
        state: state.symbol,
        statement_index,
        in_transition_target,
        candidates,
        values: Vec::new(),
    };
    if observe_runtime_state_prefix(program, machine, state, &mut visitor).is_none() {
        return Vec::new();
    }
    visitor
        .values
        .into_iter()
        .filter_map(|(expression, value)| {
            Some(CapturedScalarObservation {
                expression,
                value: value?,
            })
        })
        .collect()
}

struct Observations {
    state: SymbolHandle,
    statement_index: usize,
    in_transition_target: bool,
    candidates: Vec<ExpressionHandle>,
    values: Vec<(ExpressionHandle, Option<ScopedArithmeticValue>)>,
}

impl Observations {
    fn invalidate_root(&mut self, program: &dyn crate::CheckingInput, root: ExpressionHandle) {
        for expression in &self.candidates {
            if !crate::expression_owners::expression_contains(program, root, *expression) {
                continue;
            }
            if let Some((_, value)) = self
                .values
                .iter_mut()
                .find(|(source, _)| source == expression)
            {
                *value = None;
            } else {
                self.values.push((*expression, None));
            }
        }
    }

    fn collect(
        &mut self,
        program: &dyn crate::CheckingInput,
        machine: &Machine,
        state: &State,
        judge: &StructuralJudge<'_>,
    ) {
        for (expression, value) in judge.completed_expression_values() {
            if !self.candidates.contains(expression) {
                continue;
            }
            let Some(primitive) = super::observed_projections::declared_integer_field_read(
                program,
                machine,
                state,
                *expression,
            ) else {
                continue;
            };
            let value = judge.completed_value_identity(value.clone());

            let value = match value.semantic_value() {
                StructuralTerm::Integer(integer) => ScopedArithmeticValue::Integer(integer.clone()),
                _ => {
                    let Some(key) = value.observation_identity_key() else {
                        self.invalidate_root(program, *expression);
                        continue;
                    };
                    ScopedArithmeticValue::Atom {
                        identity: format!("state:{:?}:completed-scalar:{key}", self.state),
                        unsigned: !primitive.is_signed_integer(),
                    }
                }
            };
            if let Some((_, prior)) = self
                .values
                .iter_mut()
                .find(|(source, _)| source == expression)
            {
                if prior.as_ref() != Some(&value) {
                    *prior = None;
                }
            } else {
                self.values.push((*expression, Some(value)));
            }
        }
    }
}

impl StructuralPrefixVisitor for Observations {
    fn site(&self) -> StructuralPrefixSite {
        StructuralPrefixSite {
            state: self.state,
            statement_index: self.statement_index,
            in_transition_target: self.in_transition_target,
        }
    }

    fn visit<'program>(
        &mut self,
        program: &'program dyn crate::CheckingInput,
        machine: &Machine,
        judge: &StructuralJudge<'program>,
        environment: &[(String, StructuralTerm)],
        hypotheses: &[(String, StructuralTerm)],
        equations: &[(StructuralTerm, StructuralTerm)],
        guarantees: &CitationFacts,
    ) {
        let mut judge = structural_arm_judge(program, judge, hypotheses, equations);
        if judge.hypotheses_contradictory {
            return;
        }
        guarantees.intake(&mut judge);
        let Some(state) = program.state_in_machine(machine, self.state) else {
            return;
        };
        self.collect(program, machine, state, &judge);
        let Some(statement) = program
            .statement_table
            .statement_at(state, self.statement_index)
        else {
            return;
        };
        // The normal-value owner installs each root's completed leading roots
        // itself, including ordered call arguments and guard/target context.
        for (_, expression) in program.tables.expression_roots(statement) {
            let reference =
                crate::expression_result_type_reference(program, machine, state, expression)
                    .unwrap_or(state.return_type);
            let Some(outcomes) = judge.completed_observation_paths(
                self.statement_index,
                expression,
                environment,
                (machine, state, reference),
                &self.candidates,
            ) else {
                // Unknown completion of a potentially evaluated root cannot
                // leave a convenient sibling-path capture behind. Earlier
                // snapshots outside this root retain their completed custody.
                self.invalidate_root(program, expression);
                continue;
            };
            for outcome in outcomes {
                self.collect(program, machine, state, &outcome);
            }
        }
    }
}
