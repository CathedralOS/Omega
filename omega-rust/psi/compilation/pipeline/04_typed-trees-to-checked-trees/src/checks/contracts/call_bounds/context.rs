//! Integer comparison entailment from surviving call-entry facts.
//!
//! The arithmetic engine receives current parameter atoms, landed literals,
//! and independently selected Exact addition/subtraction/multiplication trees.
//! Caller storage and earlier initializers are never replayed.
//! View metadata uses the selected arrival state's formal coordinates for
//! both machine entries and named-state transfers. Only surviving caller
//! facts are premises; an arrival's requirements remain obligations.
use super::{
    BinaryOperator, CheckFacts, ExpressionHandle, ExpressionNode, FlowCallFact, FlowStateFact,
    OperatorSpelling,
};
use symbols::SymbolHandle;
use typed_trees::signature::StateParameter;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle};
use validation::{
    StrictArithmeticBindingValue, StrictArithmeticImplicationJudgment,
    StrictArithmeticSymbolBinding,
};

mod arguments;

/// Every surviving fact across the context roster, minus the call's own
/// requires/ensures — those are the obligation, not the premise.
fn roster_premise_facts<'f>(
    semantic: &'f facts::FactPlan,
    contexts: &'f [facts::FactContextHandle],
) -> impl Iterator<Item = &'f facts::Fact> + 'f {
    contexts
        .iter()
        .flat_map(|context| {
            semantic
                .context_view(semantic.contexts.get(*context))
                .facts()
        })
        .filter(|fact| {
            !matches!(
                fact.origin,
                facts::FactOrigin::CallRequires | facts::FactOrigin::CallEnsures
            )
        })
}

pub(in crate::checks::contracts) fn proves(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    caller: &FlowStateFact,
    call: &FlowCallFact,
    contexts: &[facts::FactContextHandle],
    goal: ExpressionHandle,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> bool {
    prove(program, facts, caller, call, contexts, goal, call_frames).unwrap_or(false)
}

fn prove(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    caller: &FlowStateFact,
    call: &FlowCallFact,
    contexts: &[facts::FactContextHandle],
    goal: ExpressionHandle,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<bool> {
    let site = validation::find_call_site(
        program,
        caller.machine_symbol,
        caller.state_symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    let ordinary_call = match &site {
        validation::CallSite::Expression { call: source, .. } => {
            validation::call_target_entry_symbol(program, validation::call_target_symbol(program, source)) == call.target_symbol
                // A receiver that is a PLACE is admissible, not only a data
                // namespace. The goal is substituted through the receiver
                // before it is compared -- `Store::read`'s `self.limit <= 15`
                // becomes `self.peer.limit <= 15` at `self.peer.read()` -- so a
                // runtime receiver names the object the requirement is about
                // rather than hiding it. Reading a place cannot disturb the
                // argument facts either; a receiver that is a CALL could, and
                // is not a canonical place.
                && (!source.receiver.is_valid()
                    || validation::canonical_place_from_expression(program, source.receiver)
                        .is_some()
                    || matches!(
                        program.expression_table.expression(source.receiver),
                        ExpressionNode::Name(path)
                            if is_data_namespace(program, path.symbol)
                                && (path.head_symbol == path.symbol
                                    || !path.head_symbol.is_valid())
                    ))
                && source.static_requirement_dispatch.is_none()
                && source.quotient_operation.is_none()
                && source.private_layout_operation.is_none()
                && source.machine_arguments.is_empty()
                && source.evidence_arguments.is_empty()
        }
        validation::CallSite::Statement(source) => {
            // The shared statement-call traversal already selected this exact
            // target. A data namespace is not a runtime receiver place. A
            // receiver that resolves to a caller place is admissible for the
            // same reason the expression arm admits one: the goal is
            // substituted through the receiver before it is compared, and
            // reading a place cannot disturb the argument facts.
            let admissible_receiver = (source.receiver.is_empty()
                && !source.receiver_symbol.is_valid()
                && !source.receiver_root_symbol.is_valid())
                || (is_data_namespace(program, source.receiver_symbol)
                    && (source.receiver_root_symbol == source.receiver_symbol
                        || !source.receiver_root_symbol.is_valid()))
                || (!source.receiver.is_empty()
                    && source.receiver_root_symbol.is_valid()
                    && validation::find_state_in_machine(
                        program,
                        caller.machine_symbol,
                        caller.state_symbol,
                    )
                    .is_some_and(|caller_state| {
                        validation::projected_statement_receiver_place(
                            program,
                            caller_state,
                            call.statement_index,
                            source,
                        )
                        .is_some()
                    }));
            validation::call_target_entry_symbol(program, source.target_symbol)
                == call.target_symbol
                && admissible_receiver
                && source.static_requirement_dispatch.is_none()
                && source.machine_arguments.is_empty()
                && source.evidence_arguments.is_empty()
        }
        validation::CallSite::TransitionNamed {
            path,
            evidence_arguments,
            ..
        } => {
            validation::call_target_state(program, path.symbol)
                .map_or(path.symbol, |state| state.symbol)
                == call.target_symbol
                && (path.head_symbol == path.symbol
                    || path.head_symbol == caller.machine_symbol
                    || is_data_namespace(program, path.head_symbol))
                && evidence_arguments.is_empty()
        }
    };
    if !ordinary_call {
        return None;
    }
    let caller_machine = program.machine_by_symbol(caller.machine_symbol)?;
    let caller_state =
        validation::find_state_in_machine(program, caller.machine_symbol, caller.state_symbol)?;
    // The callee is a machine head (the machine itself or its entry state),
    // or a named state of the caller's own machine that a transition arrives
    // at. Either way the goal is the target's arrival clause over its formals.
    let head = validation::find_machine_head(program, call.target_symbol);
    let (callee, entry) = match head {
        Some(head) => head,
        None => validation::find_state(program, call.target_symbol).filter(|(machine, _)| {
            machine.symbol == caller.machine_symbol
                && matches!(site, validation::CallSite::TransitionNamed { .. })
        })?,
    };
    let parameters = program.state_parameters(entry);
    if head.is_some()
        && caller.machine_symbol == callee.symbol
        && matches!(site, validation::CallSite::TransitionNamed { .. })
        && crate::checks::termination::proves_ranked_entry_requirement(
            program,
            callee,
            goal,
            &facts.termination.rank_arrivals,
        )
    {
        return Some(true);
    }
    let arguments = validation::call_site_argument_expressions(program, &site);
    let explicit_parameters = parameters.iter();
    if explicit_parameters.clone().count() != arguments.len() {
        return None;
    }
    let arithmetic_hypotheses = roster_premise_facts(&facts.semantic, contexts)
        .filter_map(|fact| match fact.payload {
            facts::FactPayload::ContractBooleanExpression {
                kind: facts::ContractFactKind::Requires,
                expression,
                instantiated,
                ..
            } if !instantiated.is_valid() => Some((expression, true)),
            facts::FactPayload::BooleanValue { expression, value } => Some((expression, value)),
            _ => None,
        })
        .collect::<Vec<_>>();
    if validation::prove_arithmetic_call_requirement(
        program,
        caller_machine,
        caller_state,
        callee,
        entry,
        &arithmetic_hypotheses,
        goal,
        arguments,
        &facts.termination.rank_arrivals,
    ) {
        return Some(true);
    }
    if super::super::call_values::proves_captured_arithmetic_requirement(
        program, facts, caller, call, goal,
    ) {
        return Some(true);
    }
    let caller_parameters = program.state_parameters(caller_state);
    let mut owned_frames = None;
    let frames = caller_parameters
        .iter()
        .any(|parameter| parameter.is_mutable)
        .then(|| crate::flow::shared_call_frames_or(call_frames, program, &mut owned_frames))
        .flatten();
    let bindings = caller_parameters
        .iter()
        .filter_map(|parameter| {
            let primitive = fixed_parameter_type(program, parameter)?;
            Some(binding(parameter.symbol, parameter.symbol, primitive))
        })
        .collect::<Vec<_>>();
    if !comparison_is_admitted(program, facts, callee.symbol, parameters, goal) {
        return None;
    }
    // Each obligation uses only its own formal operands. An unrelated actual
    // may be a result field or effectful computation that this arithmetic
    // vocabulary cannot normalize; that supplies no reason to discard a live
    // bound on another argument. Keep the complete later-argument suffix below:
    // even an unused formal's actual can overwrite a demanded capture.
    let mut goal_occurrences = Vec::new();
    crate::facts::contract_occurrences::append_expression_occurrences(
        program,
        goal,
        &mut goal_occurrences,
    );
    let mut argument_bindings = Vec::new();
    for (position, (parameter, argument)) in explicit_parameters.zip(arguments).enumerate() {
        if !goal_occurrences.iter().any(|occurrence| {
            direct_parameter(program, parameters, *occurrence)
                .is_some_and(|demanded| demanded.symbol == parameter.symbol)
        }) {
            continue;
        }
        let Some(primitive) = program.primitive_type_reference(parameter.type_reference) else {
            continue;
        };
        if !primitive.is_fixed_integer() {
            continue;
        }
        if fixed_parameter_type(program, parameter) != Some(primitive) {
            return None;
        }
        if arguments::primitive_type(
            program,
            facts,
            caller_machine.symbol,
            caller_parameters,
            *argument,
        ) != Some(primitive)
            // A named tail-call literal may retain its anonymous source form.
            // Its exact formal is the destination; reuse checked landing rather
            // than requiring an unrelated literal-stamping pass to have run.
            && !(arguments::is_unlanded_integer(program, *argument)
                && validation::land_anonymous_integer_expression(
                    program, *argument, primitive, &[], |_| false,
                ).is_some())
        {
            return None;
        }
        if !arguments::capture_is_current(
            program,
            facts,
            caller_machine,
            caller_state,
            call.statement_index,
            *argument,
            &arguments[position + 1..],
            frames,
        ) {
            return None;
        }
        argument_bindings.push(validation::StrictArithmeticExpressionBinding {
            symbol: parameter.symbol,
            expression: *argument,
        });
    }
    // This exact call-entry roster contains simultaneously active overlays,
    // not alternative arrival paths. Their surviving facts are joint premises.
    let hypotheses = roster_premise_facts(&facts.semantic, contexts)
        .filter_map(|fact| {
            // Call substitution lives separately from the authored expression.
            // This adapter only consumes declaration-shaped caller facts.
            match fact.payload {
                facts::FactPayload::ContractBooleanExpression {
                    kind: facts::ContractFactKind::Requires,
                    expression,
                    instantiated,
                    ..
                } if !instantiated.is_valid() => Some(expression),
                facts::FactPayload::BooleanValue {
                    expression,
                    value: true,
                } => Some(expression),
                _ => None,
            }
        })
        .flat_map(|expression| true_conjuncts(program, facts, caller.machine_symbol, expression))
        .filter(|expression| {
            comparison_is_admitted(
                program,
                facts,
                caller.machine_symbol,
                caller_parameters,
                *expression,
            )
        })
        .collect::<Vec<_>>();
    Some(
        validation::strict_arithmetic_expression_implication(
            program,
            caller_machine,
            &hypotheses,
            goal,
            &bindings,
            &argument_bindings,
        ) == StrictArithmeticImplicationJudgment::Proven,
    )
}

/// Every conjunct of a true builtin `&&` is itself a true fact. A guard
/// such as `fuel > 1 && members[row].vector == at` keeps its arithmetic
/// conjunct even though the other lies outside this vocabulary.
fn true_conjuncts(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    owner: SymbolHandle,
    expression: ExpressionHandle,
) -> Vec<ExpressionHandle> {
    let mut pending = vec![expression];
    let mut conjuncts = Vec::new();
    while let Some(expression) = pending.pop() {
        let Some(expression) = positive_comparison_expression(program, facts, owner, expression)
        else {
            continue;
        };
        if let Some(binary) = program.expression_table.binary_node(expression)
            && binary.operator == BinaryOperator::And
            && !facts.operators.uses.iter().any(|(_, operator)| {
                operator.expression == expression
                    && operator.status
                        != checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback
            })
        {
            pending.extend([binary.right, binary.left]);
            continue;
        }
        conjuncts.push(expression);
    }
    conjuncts
}

// Boolean-pattern transition guards retain `comparison == true`. Peel only
// positively asserted builtin wrappers; a false arm supplies no positive fact.
fn positive_comparison_expression(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    owner: SymbolHandle,
    mut expression: ExpressionHandle,
) -> Option<ExpressionHandle> {
    for _ in 0..64 {
        let Some(binary) = program.expression_table.binary_node(expression) else {
            return Some(expression);
        };
        let spelling = match binary.operator {
            BinaryOperator::Equal => OperatorSpelling::Equal,
            BinaryOperator::NotEqual => OperatorSpelling::NotEqual,
            _ => return Some(expression),
        };
        let wrapper = [(binary.left, binary.right), (binary.right, binary.left)]
            .into_iter()
            .find_map(
                |(operand, literal)| match program.expression_table.expression(literal) {
                    ExpressionNode::Boolean(value) => Some((operand, *value)),
                    _ => None,
                },
            );
        let Some((operand, value)) = wrapper else {
            return Some(expression);
        };
        if value != (binary.operator == BinaryOperator::Equal)
            || !super::super::prover::has_builtin_operators(program, &facts.operators, expression)
            || !typed_trees::operator::has_builtin_spelled_expression_meaning(
                program,
                owner,
                expression,
                spelling,
                &[None, None],
            )
        {
            return None;
        }
        expression = operand;
    }
    None
}

fn is_data_namespace(program: &dyn ::validation::CheckingInput, symbol: SymbolHandle) -> bool {
    symbol.is_valid() && program.data_definition_by_symbol(symbol).is_some()
}

fn binding(
    formal: SymbolHandle,
    actual: SymbolHandle,
    primitive: PrimitiveType,
) -> StrictArithmeticSymbolBinding {
    StrictArithmeticSymbolBinding {
        symbol: formal,
        value: StrictArithmeticBindingValue::Atom {
            identity: format!("\0call-parameter:{actual:?}"),
            unsigned: matches!(
                primitive,
                typed_trees::unsigned_integer_primitive_pattern!()
            ),
        },
    }
}

fn fixed_parameter_type(
    program: &dyn ::validation::CheckingInput,
    parameter: &StateParameter,
) -> Option<PrimitiveType> {
    if !parameter.symbol.is_valid() || parameter.is_const {
        return None;
    }
    program
        .primitive_type_reference(parameter.type_reference)
        .filter(|primitive| primitive.is_fixed_integer())
}

fn direct_parameter<'a>(
    program: &dyn ::validation::CheckingInput,
    parameters: &'a [StateParameter],
    expression: ExpressionHandle,
) -> Option<&'a StateParameter> {
    let symbol = ::validation::single_segment_name_symbol(program, expression)?;
    parameters
        .iter()
        .find(|parameter| parameter.symbol == symbol)
}

fn comparison_is_admitted(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    owner: SymbolHandle,
    parameters: &[StateParameter],
    expression: ExpressionHandle,
) -> bool {
    let Some(binary) = program.expression_table.binary_node(expression) else {
        return false;
    };
    if binary.operator == BinaryOperator::And {
        return super::super::prover::has_builtin_operators(program, &facts.operators, expression)
            && comparison_is_admitted(program, facts, owner, parameters, binary.left)
            && comparison_is_admitted(program, facts, owner, parameters, binary.right);
    }
    let Some(spelling) = typed_trees::operator::comparison_operator_spelling(binary.operator)
    else {
        return false;
    };
    let operand = |expression| -> Option<(Option<PrimitiveType>, Option<TypeReferenceHandle>)> {
        if arguments::is_unlanded_integer(program, expression) {
            return Some((None, None));
        }
        if let Some(literal) = program.expression_table.integer_node(expression) {
            let primitive = crate::values::scalar_expression_type(
                &checked_trees::CheckedScalarExpression::IntegerLiteral {
                    literal: literal.clone(),
                },
            )?;
            return primitive
                .is_fixed_integer()
                .then_some((Some(primitive), None));
        }
        let primitive = arguments::primitive_type(program, facts, owner, parameters, expression)?;
        let type_reference = direct_parameter(program, parameters, expression)
            .map(|parameter| parameter.type_reference);
        Some((Some(primitive), type_reference))
    };
    let Some((left, left_reference)) = operand(binary.left) else {
        return false;
    };
    let Some((right, right_reference)) = operand(binary.right) else {
        return false;
    };
    (left == right || left.is_none() || right.is_none())
        && super::super::prover::has_builtin_operators(program, &facts.operators, expression)
        && typed_trees::operator::has_builtin_spelled_expression_meaning(
            program,
            owner,
            expression,
            spelling,
            &[left_reference, right_reference],
        )
}
