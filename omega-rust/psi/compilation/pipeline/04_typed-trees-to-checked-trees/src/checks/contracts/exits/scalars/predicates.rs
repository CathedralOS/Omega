//! Live exit predicates and completed return values share captured-value transport.
//! A result is bound only by its authenticated outgoing occurrence. Sources keep
//! their read epochs; contract inputs observe storage after return evaluation.

use super::{
    ExitScalars, ExpressionHandle, PlaceRoot, ScalarValue,
    semantic_contexts_prove_boolean_expression,
};
use crate::checks::contracts::{captured_predicates, prover, return_values};

impl ExitScalars<'_, '_> {
    pub(super) fn current_predicate(&self, expression: ExpressionHandle) -> Option<bool> {
        // Non-scalar propositions retain their existing semantic owner. It
        // speaks directly in declaration symbols only while those binders
        // still agree with the exit namespace; rebased scalar observations
        // below use the explicit origin correspondence instead.
        if !self
            .contract_origins()
            .any(|origin| origin.entry_parameter != origin.state_parameter)
            && self
                .state()
                .and_then(|state| {
                    validation::expression_result_type_reference(
                        self.program,
                        self.machine,
                        state,
                        expression,
                    )
                })
                .and_then(|reference| self.program.primitive_type_reference(reference))
                == Some(typed_trees::types::PrimitiveType::Bool)
            && semantic_contexts_prove_boolean_expression(
                self.program,
                &self.facts.semantic,
                self.contexts,
                facts::ProgramPoint::Statement {
                    machine_symbol: self.machine.symbol,
                    state_symbol: self.exit.state_symbol,
                    statement_index: self.exit.statement_index,
                },
                expression,
            )
        {
            return Some(true);
        }
        let caller = self
            .facts
            .flow
            .control
            .states
            .iter()
            .find_map(|(_, state)| {
                (state.machine_symbol == self.exit.machine_symbol
                    && state.state_symbol == self.exit.state_symbol)
                    .then_some(state)
            });
        if let Some(frames) = self.call_frames
            && let Some(caller) = caller
            && let Some(entry) = self.program.machine_states(self.machine).first()
            && prover::call_guarantees::proves_current(
                self.program,
                self.facts,
                caller,
                self.exit.statement_index,
                self.contexts,
                frames,
                prover::call_guarantees::callable::Callable::Machine {
                    machine: self.machine,
                    state: entry,
                },
                expression,
                &|expression| {
                    let parameters = self.program.state_parameters(entry);
                    let (position, path, _) = crate::values::structural_parameter_place(
                        self.program,
                        parameters,
                        expression,
                    )?;
                    let (root, segments, _, _) = crate::values::resolve_structural_parameter_path(
                        self.program,
                        parameters,
                        position,
                        &path,
                    )?;
                    let place = validation::CanonicalPlace {
                        root: PlaceRoot::Symbol(root),
                        segments,
                    };
                    let place = self.contract_place_at_exit(place)?;
                    crate::flow::rebase_exact_local_place(
                        self.program,
                        self.exit.state_symbol,
                        self.exit.statement_index,
                        place,
                        Some(frames),
                    )
                },
            )
        {
            return Some(true);
        }
        let Some(expression) = crate::values::lower_scalar_contract_predicate(
            self.program,
            &self.facts.operators,
            self.machine,
            expression,
            true,
            &mut 4096,
        ) else {
            return None;
        };
        let entry = self.program.machine_states(self.machine).first()?;
        let completed =
            return_values::completed_return_operand(self.program, self.facts, self.exit);
        let result = completed.map(|operand| validation::CanonicalPlace {
            root: PlaceRoot::Expression(operand.expression),
            segments: Vec::new(),
        });
        let mut required = crate::facts::boolean_predicates::bind_parameters(
            self.program,
            self.machine.symbol,
            entry.symbol,
            self.program.state_parameters(entry),
            result.as_ref().map(|place| (place, entry.return_type)),
            expression,
        )?;
        let mut captures = Vec::new();
        for position in 0..required.bindings.len() {
            let binding = &mut required.bindings[position];
            if result
                .as_ref()
                .is_some_and(|result| binding.place.root == result.root)
            {
                // Result is the authenticated completed outgoing value, including
                // effectful producers. Its authored expression is never replayed.
                let operand = crate::flow::CapturedOperand::recorded(
                    &self.facts.flow.control,
                    self.exit.returned_value,
                )?;
                let state = caller?;
                let values = crate::flow::captured_operand_values(
                    self.program,
                    &self.facts.flow,
                    &self.facts.semantic,
                    state,
                    operand,
                    entry.return_type,
                    &binding.place.segments,
                )?;
                let observed = crate::flow::captured_observation_places(
                    self.program,
                    &self.facts.flow,
                    values.first()?,
                )?;
                let [observed] = observed.as_slice() else {
                    return None;
                };
                if !values.iter().all(|value| {
                    crate::flow::captured_observation_places(self.program, &self.facts.flow, value)
                        .is_some_and(|places| {
                            places.len() == 1 && places[0].projected == observed.projected
                        })
                }) {
                    return None;
                }
                binding.place = observed.projected.clone();
                for value in &values {
                    captured_predicates::retain_bindings(
                        self.program,
                        self.facts,
                        value,
                        operand.row(),
                        std::slice::from_ref(binding),
                        position,
                        &mut captures,
                    );
                }
            } else {
                binding.place = self.contract_place_at_exit(binding.place.clone())?;
                if let Some(operand) = completed {
                    // Non-result contract observations read current storage at
                    // outgoing completion, after all return operands have run.
                    captures.push(captured_predicates::CapturedBinding::observation(
                        position,
                        binding.place.clone(),
                        operand,
                    ));
                }
            }
        }
        let mut predicates = crate::facts::boolean_predicates::retained(
            self.program,
            &self.facts.operators,
            &self.facts.semantic,
            self.contexts,
            facts::ProgramPoint::Statement {
                machine_symbol: self.machine.symbol,
                state_symbol: self.exit.state_symbol,
                statement_index: self.exit.statement_index,
            },
        );
        if let Some(frames) = self.call_frames
            && let Some(state) = caller
        {
            predicates.extend(prover::call_guarantees::current_predicates(
                self.program,
                self.facts,
                state,
                prover::call_guarantees::GuaranteeUse::StatementEntry(
                    self.exit.statement_index + usize::from(completed.is_some()),
                ),
                self.contexts,
                frames,
            ));
        }
        if let Some(state) = caller {
            let transported = captured_predicates::transport(
                self.program,
                self.facts,
                state,
                &mut required,
                &captures,
                &predicates,
                self.call_frames,
            );
            if transported.proves_required {
                return Some(true);
            }
            predicates.extend(transported.predicates);
        }
        crate::facts::boolean_predicates::truth(&predicates, &required, |place| {
            prover::boolean_value_at_place(
                self.program,
                &self.facts.operators,
                &self.facts.semantic,
                self.contexts,
                facts::ProgramPoint::Statement {
                    machine_symbol: self.machine.symbol,
                    state_symbol: self.exit.state_symbol,
                    statement_index: self.exit.statement_index,
                },
                place,
            )
            .map(ScalarValue::Boolean)
            .or_else(|| {
                crate::values::scalar_value_at_place(
                    self.program,
                    &self.facts.semantic,
                    self.contexts
                        .iter()
                        .map(|context| self.facts.semantic.contexts.get(*context)),
                    place,
                )
            })
        })
    }
}
