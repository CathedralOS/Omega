//! Collection geometry consumes the ordinary ordered normal-return owner.
//!
//! A call requirement and an index must agree on the same completed values.
//! The previous arithmetic prefix walk lost nested result guarantees and Match
//! alternatives. Reuse the structural prefix, operand scheduler and integer
//! judge already used by call requirements and exact casts, rather than building
//! another call-result graph or admitting captured values as live storage.

use super::citations::CitationFacts;
use super::structural_case_arms::structural_arm_judge;
use super::structural_judgment::{StructuralJudge, StructuralJudgment, StructuralTerm};
use super::structural_prefix::{StructuralPrefixSite, StructuralPrefixVisitor};
use typed_trees::expression::{BinaryOperator, ExpressionHandle};
use typed_trees::machine::Machine;
use typed_trees::state::State;

/// Declaration-bound context for exact collection-operation observations.
/// Each query observes the operation after its actual children, before its
/// result or any later sibling can contribute a guarantee.
pub struct CollectionBoundsObservations<'program> {
    program: &'program dyn crate::CheckingInput,
    machine: &'program Machine,
    state: &'program State,
    acyclic: std::cell::Cell<Option<bool>>,
}

impl<'program> CollectionBoundsObservations<'program> {
    pub fn new(
        program: &'program dyn crate::CheckingInput,
        machine: &'program Machine,
        state: &'program State,
    ) -> Self {
        Self {
            program,
            machine,
            state,
            acyclic: std::cell::Cell::new(None),
        }
    }

    pub fn proves(&self, statement_index: usize, expression: ExpressionHandle) -> bool {
        let mut query = Bounds {
            site: StructuralPrefixSite {
                state: self.state.symbol,
                statement_index,
                in_transition_target: super::structural_prefix::expression_is_transition_target(
                    self.program,
                    self.state,
                    statement_index,
                    expression,
                ),
            },
            expression,
            seen: false,
            proven: true,
        };
        let acyclic = self.acyclic.get().unwrap_or_else(|| {
            let computed = self
                .program
                .machine_states(self.machine)
                .first()
                .is_some_and(|entry| {
                    crate::machine_calls::calls::named_state_transition_subgraph_is_acyclic(
                        self.program,
                        self.machine,
                        entry,
                    )
                });
            self.acyclic.set(Some(computed));
            computed
        });
        let recognized = if acyclic {
            super::structural_prefix::observe_acyclic_invocation_prefix(
                self.program,
                self.machine,
                &mut query,
            )
        } else {
            super::structural_prefix::observe_runtime_state_prefix(
                self.program,
                self.machine,
                self.state,
                &mut query,
            )
        };
        recognized.is_some() && query.seen && query.proven
    }
}

struct Bounds {
    site: StructuralPrefixSite,
    expression: ExpressionHandle,
    seen: bool,
    proven: bool,
}

impl StructuralPrefixVisitor for Bounds {
    fn site(&self) -> StructuralPrefixSite {
        self.site
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
        self.seen = true;
        let mut judge = structural_arm_judge(program, judge, hypotheses, equations);
        if judge.hypotheses_contradictory {
            return;
        }
        guarantees.intake(&mut judge);
        let Some(state) = program.state_in_machine(machine, self.site.state) else {
            self.proven = false;
            return;
        };
        let Some(indexed) = program.expression_table.indexed_node(self.expression) else {
            self.proven = false;
            return;
        };
        if !crate::place_has_builtin_coordinates(program, machine, Some(state), self.expression) {
            self.proven = false;
            return;
        }
        let Some(reference) =
            crate::expression_result_type_reference(program, machine, state, indexed.collection)
        else {
            self.proven = false;
            return;
        };
        let Some(entries) = judge.normal_operation_entries(
            self.site.statement_index,
            self.expression,
            super::structural_judgment::normal_returns::OperationOperandDemand::All,
            environment,
            (machine, state, state.return_type),
        ) else {
            self.proven = false;
            return;
        };
        self.proven &= !entries.is_empty();
        for entry in entries {
            let Some(collection) = entry.arguments.first().and_then(Option::as_ref) else {
                self.proven = false;
                return;
            };
            let Some(length) = super::structural_terms::observed_collection_measure(
                program,
                reference,
                language_semantics::declaration_selection::CollectionMeasure::Length,
                || Some(collection.clone()),
            ) else {
                self.proven = false;
                return;
            };
            // Selected builtin length is a u64 observation. Preserve that
            // authenticated carrier at the mathematical-integer boundary,
            // just as the ordinary expression judgment does for `.len`.
            let length = if matches!(length.semantic_value(), StructuralTerm::Integer(_)) {
                length
            } else {
                let Some(reference) =
                    crate::proof_contracts::bound_expression_meaning::builtin_type_reference(
                        program,
                        symbols::BuiltinTypeAtom::U64,
                    )
                else {
                    self.proven = false;
                    return;
                };
                StructuralTerm::IntegerEmbedding {
                    reference,
                    value: Box::new(length),
                }
            };
            let zero = StructuralTerm::Integer(numerics::bignum::BigInt::zero());
            let mut position = 1;
            let mut value = |source: ExpressionHandle, default: StructuralTerm| {
                if !source.is_valid() {
                    return Some(default);
                }
                let value = entry.arguments.get(position)?.as_ref()?.clone();
                position += 1;
                // An exact observed integer already has mathematical meaning;
                // anonymous literals need no invented finite carrier handle.
                if matches!(value.semantic_value(), StructuralTerm::Integer(_)) {
                    return Some(value);
                }
                let reference =
                    crate::expression_result_type_reference(program, machine, state, source)?;
                program
                    .primitive_type_reference(reference)
                    .filter(|primitive| primitive.is_fixed_integer())?;
                Some(StructuralTerm::IntegerEmbedding {
                    reference,
                    value: Box::new(value),
                })
            };
            let required = if let Some(range) = program.expression_table.range_node(indexed.index) {
                let (Some(start), Some(mut end)) = (
                    value(range.start, zero.clone()),
                    value(range.end, length.clone()),
                ) else {
                    self.proven = false;
                    return;
                };
                let authored_end = end.clone();
                if range.end_inclusive && range.end.is_valid() {
                    end = StructuralTerm::MathematicalBinary {
                        operator: BinaryOperator::Add,
                        left: Box::new(end),
                        right: Box::new(StructuralTerm::Integer(
                            numerics::bignum::BigInt::from_i64(1),
                        )),
                    };
                }
                vec![
                    (BinaryOperator::GreaterOrEqual, start.clone(), zero.clone()),
                    (BinaryOperator::GreaterOrEqual, authored_end, zero.clone()),
                    (BinaryOperator::LessOrEqual, start, end.clone()),
                    (BinaryOperator::LessOrEqual, end, length),
                ]
            } else {
                let Some(index) = value(indexed.index, zero.clone()) else {
                    self.proven = false;
                    return;
                };
                vec![
                    (BinaryOperator::GreaterOrEqual, index.clone(), zero),
                    (BinaryOperator::Less, index, length),
                ]
            };
            self.proven &= position == entry.arguments.len()
                && required.into_iter().all(|(operator, left, right)| {
                    let judgment = entry.judge.judge_observed_predicate(
                        StructuralTerm::MathematicalBinary {
                            operator,
                            left: Box::new(left),
                            right: Box::new(right),
                        },
                        0,
                    );
                    matches!(judgment, StructuralJudgment::Proven)
                });
        }
    }
}
