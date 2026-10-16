//! Scalar postconditions consume exact return and live place values. Entry
//! parameter names are substituted only through the retained exit origin map.

use validation::canonical_place_from_expression_in_state;

use checked_trees::{CheckFacts, CheckedScalarExpressionRole, FlowExitFact};
use facts::{FactContextHandle, FactPayload, PlaceRoot, PlaceSegment};
use typed_trees::{expression::ExpressionHandle, machine::Machine};

use super::super::{
    prover::{
        ScalarValue, evaluate_checked_scalar, evaluate_for_atoms, evaluate_scalar,
        has_builtin_operators, semantic_contexts_prove_boolean_expression,
    },
    return_values::{exit_return_expression, is_result_reference},
};
use crate::flow::canonical_place_from_symbol;

mod boolean_results;
pub(crate) mod calls;
mod result_bounds;
mod result_fields;
mod static_calls;
mod write_transport;

pub(super) fn proves<'program>(
    program: &'program dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    exit: &FlowExitFact,
    contexts: &[FactContextHandle],
    requirement: &facts::Fact,
    call_frames: Option<&validation::CallFrameResolver<'program>>,
) -> bool {
    let FactPayload::ContractBooleanExpression {
        fact: contract,
        expression,
        ..
    } = requirement.payload
    else {
        return false;
    };
    let Some(machine) = program.machine(exit.machine_symbol) else {
        return false;
    };
    let evaluator = ExitScalars {
        program,
        facts,
        machine,
        exit,
        contexts,
        contract,
        call_frames,
    };
    // IEEE predicates consume landed enclosures, not the integer-only scalar
    // evaluator. The shared judgment checks the selected comparison meaning
    // and binds only this exit's exact reserved result occurrence.
    if evaluator.state().is_some_and(|state| {
        validation::float_return_postcondition(
            program,
            machine,
            state,
            exit_return_expression(program, exit),
            expression,
        ) == Some(true)
    }) {
        return true;
    }
    // A missing checked operator row is not builtin authority. Rejoin the
    // declaration meaning before either symbolic or concrete comparison.
    if !has_builtin_operators(program, &facts.operators, expression)
        || !validation::has_builtin_bound_expression_meaning(
            program,
            machine,
            program.machine_states(machine).first(),
            expression,
        )
    {
        return false;
    }
    let cases = super::cases::CaseObservation::for_requirement(
        program,
        facts,
        exit,
        contexts,
        requirement,
        call_frames,
    );
    evaluate_for_atoms(
        program,
        expression,
        &mut |leaf| evaluator.contract_value(leaf),
        &|_, _| true,
        &mut |atom| {
            cases
                .as_ref()
                .and_then(|cases| cases.observe(atom))
                .or_else(|| {
                    // A current predicate can be known without fixing its scalar
                    // operands to single values. Use the same exit premises as the
                    // whole-contract prover; failed search never supplies false.
                    // Each atom uses the same return and current-value
                    // judgments. Proving only whole expressions with these
                    // owners loses facts when a contract combines a returned
                    // value equation with an independently observed guard.
                    (evaluator.proves_boolean_result(atom) == Some(true)
                        || evaluator.proves_immutable_result_comparison(atom)
                        || evaluator.proves_return_arithmetic(atom)
                        || evaluator.proves_result_field_equality(atom)
                        || evaluator.write_transported_equality(atom))
                    .then_some(true)
                    .or_else(|| evaluator.current_predicate(atom))
                })
        },
    ) == Some(ScalarValue::Boolean(true))
}

struct ExitScalars<'program, 'facts> {
    program: &'program dyn ::validation::CheckingInput,
    facts: &'facts CheckFacts,
    machine: &'program Machine,
    exit: &'facts FlowExitFact,
    contexts: &'facts [FactContextHandle],
    contract: arena::Handle<typed_trees::domain::ProofFact>,
    call_frames: Option<&'facts validation::CallFrameResolver<'program>>,
}

impl<'program, 'facts> ExitScalars<'program, 'facts> {
    /// The state this exit sits on, resolved through the exit's machine.
    fn state(&self) -> Option<&'program typed_trees::state::State> {
        validation::find_state_for_exit(self.program, self.exit)
    }
}

/// Parameters typed primitive — the leading slots of a scalar binding
/// roster at any occurrence in `state`.
pub(crate) fn primitive_parameters<'a>(
    program: &'a dyn ::validation::CheckingInput,
    state: &'a typed_trees::state::State,
) -> impl Iterator<Item = symbols::SymbolHandle> + 'a {
    program
        .state_parameters(state)
        .iter()
        .filter(|parameter| {
            program
                .primitive_type_reference(parameter.type_reference)
                .is_some()
        })
        .map(|parameter| parameter.symbol)
}

/// An immutable local carrying an initializer whose declared type is
/// primitive — the only locals a scalar binding roster names.
pub(crate) fn is_scalar_roster_local(
    program: &dyn ::validation::CheckingInput,
    local: &typed_trees::statement::TableLocalData,
) -> bool {
    !local.is_mutable
        && local.initial_value.is_valid()
        && program
            .primitive_type_reference(local.type_reference)
            .is_some()
}

impl ExitScalars<'_, '_> {
    /// Current pointees retain their entry binders; later states must retain
    /// the exact parameter correspondence for this contract. A write changes
    /// storage facts, not the declaration namespace used to name that storage.
    fn contract_place_at_exit(
        &self,
        mut place: validation::CanonicalPlace,
    ) -> Option<validation::CanonicalPlace> {
        let entry = self.program.machine_states(self.machine).first()?;
        if entry.symbol != self.exit.state_symbol {
            let root = place.root.as_symbol()?;
            let origin = self
                .facts
                .flow
                .control
                .exit_parameter_origins
                .span_or_empty(self.exit.parameter_origins)
                .iter()
                .find(|origin| {
                    origin.contract == self.contract
                        && origin.entry_parameter == root
                        && origin.state_parameter.is_valid()
                })?;
            place.root = PlaceRoot::Symbol(origin.state_parameter);
        }
        Some(place)
    }

    fn current_predicate(&self, expression: ExpressionHandle) -> Option<bool> {
        // Non-scalar propositions retain their existing semantic owner. It
        // speaks directly in declaration symbols only while those binders
        // still agree with the exit namespace; rebased scalar observations
        // below use the explicit origin correspondence instead.
        if !self
            .facts
            .flow
            .control
            .exit_parameter_origins
            .span_or_empty(self.exit.parameter_origins)
            .iter()
            .any(|origin| {
                origin.contract == self.contract && origin.entry_parameter != origin.state_parameter
            })
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
            && super::super::prover::call_guarantees::proves_current(
                self.program,
                self.facts,
                caller,
                self.exit.statement_index,
                self.contexts,
                frames,
                super::super::prover::call_guarantees::callable::Callable::Machine {
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
        let Some(mut required) = crate::facts::boolean_predicates::bind_entry(
            self.program,
            self.machine.symbol,
            expression,
        ) else {
            return None;
        };
        for binding in &mut required.bindings {
            let Some(place) = self.contract_place_at_exit(binding.place.clone()) else {
                return None;
            };
            binding.place = place;
        }
        let mut predicates = crate::facts::boolean_predicates::retained(
            self.program,
            &self.facts.operators,
            &self.facts.semantic,
            self.contexts,
        );
        if let Some(frames) = self.call_frames
            && let Some(state) = caller
        {
            predicates.extend(super::super::prover::call_guarantees::current_predicates(
                self.program,
                self.facts,
                state,
                self.exit.statement_index,
                self.contexts,
                frames,
            ));
        }
        crate::facts::boolean_predicates::truth(&predicates, &required, |place| {
            super::super::prover::boolean_value_at_place(
                self.program,
                &self.facts.operators,
                &self.facts.semantic,
                self.contexts,
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

    fn proves_immutable_result_comparison(&self, expression: ExpressionHandle) -> bool {
        use typed_trees::expression::{BinaryOperator, ExpressionNode};
        if self.proves_bounds(expression) {
            return true;
        }
        let Some(binary) = self.program.expression_table.binary_node(expression) else {
            return false;
        };
        let spelling = match binary.operator {
            BinaryOperator::And => {
                return self.proves_immutable_result_comparison(binary.left)
                    && self.proves_immutable_result_comparison(binary.right);
            }
            BinaryOperator::Or => {
                return self.proves_immutable_result_comparison(binary.left)
                    || self.proves_immutable_result_comparison(binary.right);
            }
            BinaryOperator::Equal => language_core::OperatorSpelling::Equal,
            BinaryOperator::LessOrEqual => language_core::OperatorSpelling::LessEqual,
            BinaryOperator::GreaterOrEqual => language_core::OperatorSpelling::GreaterEqual,
            _ => return false,
        };
        let argument = if is_result_reference(self.program, self.machine, binary.left) {
            binary.right
        } else if is_result_reference(self.program, self.machine, binary.right) {
            binary.left
        } else {
            return false;
        };
        let Some(path) = self.program.expression_table.name_path_node(argument) else {
            return false;
        };
        if !path.symbol.is_valid() || path.head_symbol != path.symbol {
            return false;
        }
        let Some(entry) = self.program.machine_states(self.machine).first() else {
            return false;
        };
        if let Some(parameter) = self
            .program
            .machine_type_parameters(self.machine)
            .iter()
            .find(|parameter| parameter.symbol == path.symbol)
            && let typed_trees::data::TypeParameterKind::Const { type_reference }
            | typed_trees::data::TypeParameterKind::Value { type_reference } = parameter.kind
        {
            let primitive = self.program.primitive_type_reference(type_reference);
            if !primitive.is_some_and(|primitive| {
                primitive.accepts_integer_literal()
                    || (primitive == typed_trees::types::PrimitiveType::Bool
                        && binary.operator == BinaryOperator::Equal)
            }) || primitive != self.program.primitive_type_reference(entry.return_type)
            {
                return false;
            }
            let types = if argument == binary.right {
                [Some(entry.return_type), Some(type_reference)]
            } else {
                [Some(type_reference), Some(entry.return_type)]
            };
            // A static binder keeps one immutable identity across every state;
            // unlike runtime parameters it needs no edge-origin substitution.
            return typed_trees::operator::has_builtin_spelled_expression_meaning(
                self.program,
                self.machine.symbol,
                expression,
                spelling,
                &types,
            ) && (matches!(self.program.expression_table.expression(exit_return_expression(self.program, self.exit)),
                ExpressionNode::Name(returned) if returned.symbol == parameter.symbol
                    && returned.head_symbol == parameter.symbol
                    && self.program.expression_table.name_path_members(returned.members).len() == 1)
                || self.static_call_returns_symbol(
                    exit_return_expression(self.program, self.exit),
                    parameter.symbol,
                ));
        }
        let Some(parameter) = self.program.state_parameter_by_symbol(entry, path.symbol) else {
            return false;
        };
        // Immutable identity proves Boolean equality without choosing true or
        // false. Ordering remains integral; floating equality is not reflexive.
        if (parameter.is_mutable)
            || parameter.is_const
            || !self
                .program
                .primitive_type_reference(parameter.type_reference)
                .is_some_and(|primitive| {
                    primitive.accepts_integer_literal()
                        || (primitive == typed_trees::types::PrimitiveType::Bool
                            && binary.operator == BinaryOperator::Equal)
                })
            || self
                .program
                .primitive_type_reference(parameter.type_reference)
                != self.program.primitive_type_reference(entry.return_type)
        {
            return false;
        }
        let types = if argument == binary.right {
            [Some(entry.return_type), Some(parameter.type_reference)]
        } else {
            [Some(parameter.type_reference), Some(entry.return_type)]
        };
        if !typed_trees::operator::has_builtin_spelled_expression_meaning(
            self.program,
            self.machine.symbol,
            expression,
            spelling,
            &types,
        ) {
            return false;
        }
        let Some(origin) = self
            .facts
            .flow
            .control
            .exit_parameter_origins
            .span_or_empty(self.exit.parameter_origins)
            .iter()
            .find(|origin| {
                origin.contract == self.contract && origin.entry_parameter == parameter.symbol
            })
        else {
            return false;
        };
        let Some(state) = self.state() else {
            return false;
        };
        if !self
            .program
            .state_parameters(state)
            .iter()
            .any(|parameter| {
                (parameter.symbol == origin.state_parameter && !parameter.is_mutable)
                    && !parameter.is_const
            })
        {
            return false;
        }
        // The return is this immutable entry value, identified through the
        // retained state-edge origin. No initializer or storage read is replayed.
        matches!(self.program.expression_table.expression(exit_return_expression(self.program, self.exit)), ExpressionNode::Name(returned)
            if origin.state_parameter.is_valid() && returned.symbol == origin.state_parameter && returned.head_symbol == origin.state_parameter)
            || self.call_returns_immutable_symbol(
                exit_return_expression(self.program, self.exit),
                origin.state_parameter,
            )
    }

    fn contract_value(&self, expression: ExpressionHandle) -> Option<ScalarValue> {
        if let Some(value) = self.result_field_value(expression) {
            return Some(value);
        }
        if is_result_reference(self.program, self.machine, expression) {
            return self.return_value();
        }
        let entry = self.program.machine_states(self.machine).first()?;
        // A fixed array's length is owned by its type, not by a live storage
        // snapshot. Reuse scalar lowering's exact projection checks; only a
        // literal result is usable here, never a dynamic view/carrier length.
        if matches!(
            self.program.expression_table.expression(expression),
            typed_trees::expression::ExpressionNode::Member(_)
        ) && let Some(checked_trees::CheckedScalarExpression::IntegerLiteral { literal }) =
            crate::values::lower_unit_scalar_argument(
                self.program,
                &self.facts.operators,
                entry,
                0,
                expression,
                typed_trees::types::PrimitiveType::U64,
            )
        {
            return literal.value_bignum().map(ScalarValue::Integer);
        }
        let mut place =
            canonical_place_from_expression_in_state(self.program, entry.symbol, 0, expression)?;
        let root = place.root.as_symbol()?;
        if self
            .program
            .state_parameter_by_symbol(entry, root)
            .is_some()
        {
            let origin = self
                .facts
                .flow
                .control
                .exit_parameter_origins
                .span_or_empty(self.exit.parameter_origins)
                .iter()
                .find(|origin| {
                    origin.contract == self.contract && origin.entry_parameter == root
                })?;
            if !origin.state_parameter.is_valid() {
                return None;
            }
            place.root = PlaceRoot::Symbol(origin.state_parameter);
        }
        // Dynamic selectors need their own evaluated occurrence, not a replay
        // of an entry expression under a new state or storage revision.
        if !stable_segments(&place.segments) {
            return None;
        }
        self.value_at_place(&place)
    }

    fn return_value(&self) -> Option<ScalarValue> {
        let expression = exit_return_expression(self.program, self.exit);
        if !self.return_expression_is_stable(expression) {
            return None;
        }
        self.selected_return_value(expression)
            .or_else(|| self.closed_call_value(expression))
            .or_else(|| self.closed_static_call_value(expression))
            .or_else(|| {
                evaluate_scalar(
                    self.program,
                    expression,
                    &mut |leaf| {
                        let place = canonical_place_from_expression_in_state(
                            self.program,
                            self.exit.state_symbol,
                            self.exit.statement_index,
                            leaf,
                        )?;
                        if !stable_segments(&place.segments) {
                            return None;
                        }
                        self.value_at_place(&place)
                    },
                    &|_, _| true,
                )
            })
    }

    fn return_expression_is_stable(&self, expression: ExpressionHandle) -> bool {
        // Exit contexts describe storage after return-expression effects.
        // Even short-circuit operands cannot be reread after a later write.
        let mut nodes = Vec::new();
        validation::expression_traversal::collect_expression_tree(
            self.program,
            expression,
            &mut nodes,
        );
        let state =
            validation::find_state_on_machine(self.program, self.machine, self.exit.state_symbol);
        self.program
            .expression_table
            .expression_is_valid(expression)
            && nodes.iter().all(|node| {
                !matches!(
                    self.program.expression_table.expression(*node),
                    typed_trees::expression::ExpressionNode::Atomic(_)
                ) && has_builtin_operators(self.program, &self.facts.operators, *node)
                    && validation::has_builtin_bound_expression_meaning(
                        self.program,
                        self.machine,
                        state,
                        *node,
                    )
            })
            && self.call_frames.is_some_and(|frames| {
                frames
                    .expression_write_frame(self.machine, expression)
                    .into_complete_paths()
                    .is_some_and(|paths| paths.is_empty())
            })
    }

    fn selected_return_value(&self, expression: ExpressionHandle) -> Option<ScalarValue> {
        let (expression, symbols) = self.selected_return_expression(expression)?;
        evaluate_checked_scalar(
            expression,
            &mut crate::values::BoundScalarValues {
                symbols,
                value_at_symbol: |symbol| {
                    let place = canonical_place_from_symbol(symbol)?;
                    self.value_at_place(&place)
                },
            },
        )
    }

    fn selected_return_expression(
        &self,
        expression: ExpressionHandle,
    ) -> Option<(
        &checked_trees::CheckedScalarExpression,
        &[symbols::SymbolHandle],
    )> {
        self.selected_scalar_expression(
            u32::try_from(self.exit.statement_index).ok()?,
            self.return_expression_role()?,
            expression,
        )
    }

    fn return_expression_role(&self) -> Option<CheckedScalarExpressionRole> {
        let state = self.state()?;
        Some(
            match self
                .program
                .statement_table
                .statement_at(state, self.exit.statement_index)?
            {
                typed_trees::statement::StatementNode::Transition(transition)
                    if self.exit.transition_target.is_valid()
                        && self.exit.transition_target == transition.continuation =>
                {
                    CheckedScalarExpressionRole::ContinuationReturn
                }
                _ => CheckedScalarExpressionRole::Return,
            },
        )
    }

    fn selected_scalar_expression(
        &self,
        statement_ordinal: u32,
        role: CheckedScalarExpressionRole,
        expression: ExpressionHandle,
    ) -> Option<(
        &checked_trees::CheckedScalarExpression,
        &[symbols::SymbolHandle],
    )> {
        let plans = &self.facts.values.scalar_expressions;
        let mut bindings = plans.source_bindings.iter().filter(|(_, binding)| {
            binding.state == self.exit.state_symbol
                && binding.statement_ordinal == statement_ordinal
                && binding.role == role
        });
        let (_, binding) = bindings.next()?;
        if bindings.next().is_some() || binding.expression != expression {
            return None;
        }
        if matches!(role, CheckedScalarExpressionRole::CallArgument { .. })
            && binding.destination.is_valid()
        {
            return None;
        }
        let mut selected = plans.expressions.iter().filter(|plan| {
            plan.state == binding.state
                && plan.statement_ordinal == binding.statement_ordinal
                && plan.role == binding.role
        });
        let plan = selected.next()?;
        if selected.next().is_some() {
            return None;
        }
        let symbols = plans.binding_symbols.span_or_empty(binding.symbols);
        let state = self.state()?;
        // Dense positions describe the declaration roster at this occurrence,
        // not a caller-chosen substitution. Check it without reading values.
        let parameters = primitive_parameters(self.program, state);
        let locals = self
            .program
            .statement_table
            .statements(state.statement_nodes)
            .get(..statement_ordinal as usize)?
            .iter()
            .filter_map(|statement| {
                let local = statement.as_local_data()?;
                is_scalar_roster_local(self.program, local).then_some(local.symbol)
            });
        if !parameters.chain(locals).eq(symbols.iter().copied()) {
            return None;
        }
        Some((&plan.expression, symbols))
    }
}

fn stable_segments(segments: &[PlaceSegment]) -> bool {
    segments.iter().all(|segment| match segment {
        PlaceSegment::Field { symbol } => symbol.is_valid(),
        PlaceSegment::Case { variant } => variant.is_valid(),
        PlaceSegment::FixedIndex { .. } => true,
        _ => false,
    })
}
