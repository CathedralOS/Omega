use super::StatementNode;
use crate::CheckingRequest;
use crate::lower_typed_trees;
use crate::tests::front_end::{checked_program, typed_program};

mod bitwise_integer_bounds;
mod byte_subslice_arguments;
mod byte_write_bounds;
mod call_result_relations;
mod case_predicate_meaning;
mod exact_cancellation;
mod guard_boolean_custody;
mod index_guard_custody;
mod initializer_call_computations;
mod loop_arithmetic_policy;
mod requires_operator_custody;
mod source_bindings;
mod typed_integer_remainder;
mod unit_initializers;

#[test]
fn mutable_scalar_reads_require_consistent_exact_resolved_name_handles() {
    use checked_trees::{
        CheckedBooleanExpression, CheckedOperatorFacts, CheckedScalarExpression,
        CheckedScalarExpressionRole,
    };
    use symbols::SymbolHandle;
    use typed_trees::{expression::ExpressionNode, types::PrimitiveType};

    for (scalar_type, initial_value, primitive_type) in [
        ("u8", "7", PrimitiveType::U8),
        ("bool", "true", PrimitiveType::Bool),
    ] {
        let source = format!(
            "machine value(input: {scalar_type}) -> {scalar_type} {{ let mut current: {scalar_type} = {initial_value}; current }}"
        );
        let program = typed_program(&source);
        let state = &program.machine_states(&program.machines()[0])[0];
        let state_symbol = state.symbol;
        let parameter = program.state_parameters(state)[0].symbol;
        let [
            StatementNode::LocalData(local),
            StatementNode::Expression(returned),
        ] = program.statement_table.statements(state.statement_nodes)
        else {
            panic!("one mutable declaration and its returned read")
        };
        let symbol = local.symbol;
        let returned = *returned;
        let expected = if primitive_type == PrimitiveType::Bool {
            CheckedScalarExpression::Boolean(Box::new(CheckedBooleanExpression::StorageRead {
                symbol,
            }))
        } else {
            CheckedScalarExpression::StorageRead {
                symbol,
                primitive_type,
            }
        };
        let plans = crate::values::build_checked_scalar_expression_plans(
            &program,
            &CheckedOperatorFacts::default(),
            &[],
            &mut Vec::new(),
        );
        assert_eq!(
            plans.expression_at(state_symbol, 1, CheckedScalarExpressionRole::Return),
            Some(&expected)
        );
        let stale = SymbolHandle::from_parts(symbol.arena_index(), symbol.generation() + 1);
        let missing = SymbolHandle::invalid();
        for (target, head) in [
            (stale, stale),
            (stale, symbol),
            (symbol, stale),
            (parameter, symbol),
            (symbol, parameter),
            (missing, missing),
            (missing, symbol),
            (symbol, missing),
        ] {
            let mut changed = program.clone();
            let ExpressionNode::Name(path) = changed.expression_table.expression_mut(returned)
            else {
                panic!("returned scalar has a resolved name")
            };
            path.symbol = target;
            path.head_symbol = head;
            let plans = crate::values::build_checked_scalar_expression_plans(
                &changed,
                &CheckedOperatorFacts::default(),
                &[],
                &mut Vec::new(),
            );
            assert_eq!(
                plans.expression_at(state_symbol, 1, CheckedScalarExpressionRole::Return),
                None,
                "{scalar_type}: target {target:?}, head {head:?} cannot recover storage from spelling"
            );
        }
    }
}

#[test]
fn scalar_return_custody_keeps_same_spelling_state_bindings_and_source_occurrences_distinct() {
    use checked_trees::{CheckedScalarExpression, CheckedScalarExpressionRole};
    use typed_trees::{statement::TransitionTargetNode, types::PrimitiveType};

    let checked = lower_typed_trees(
        typed_program(
            r#"
        data Packet [copy] { value: u64; }
        machine choose(packet: Packet, input: u8, flag: bool) -> u8 {
            transition flag {
                true -> left(packet, input)
                false -> right(packet, input)
            }
            state left(packet: Packet, current: u8) -> u8 {
                let owned: Packet = packet;
                let saved: u8 = current;
                transition { _ -> saved }
            }
            state right(packet: Packet, current: u8) -> u8 {
                let owned: Packet = packet;
                let saved: u8 = current;
                transition { _ -> saved }
            }
        }
    "#,
        ),
        &CheckingRequest::settled(),
    )
    .expect("explicit state argument source checks");
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "choose")
        .unwrap();
    let states = checked.machine_states(machine);
    let plans = &checked.facts.values.scalar_expressions;
    assert_eq!(
        plans
            .source_bindings
            .iter()
            .filter(|(_, binding)| binding.role == CheckedScalarExpressionRole::Return)
            .count(),
        2
    );
    assert!(
        !plans
            .source_bindings
            .iter()
            .any(|(_, binding)| binding.state == states[0].symbol
                && binding.role == CheckedScalarExpressionRole::Return),
        "named transitions are not value-return source bindings"
    );
    let mut prior = None;
    for name in ["left", "right"] {
        let state = states
            .iter()
            .find(|state| state.name.as_str() == name)
            .unwrap();
        let statements = checked.statement_table.statements(state.statement_nodes);
        let StatementNode::LocalData(local) = &statements[1] else {
            panic!("scalar local")
        };
        let StatementNode::Transition(transition) = &statements[2] else {
            panic!("explicit value transition")
        };
        let TransitionTargetNode::Value(expression) =
            checked.statement_table.transition_target(transition.target)
        else {
            panic!("value target")
        };
        let rows: Vec<_> = plans
            .source_bindings
            .iter()
            .map(|(_, binding)| binding)
            .filter(|binding| {
                binding.state == state.symbol && binding.role == CheckedScalarExpressionRole::Return
            })
            .collect();
        let [binding] = rows.as_slice() else {
            panic!("one exact row per return state")
        };
        let symbols = plans.binding_symbols.span_or_empty(binding.symbols);
        assert_eq!(binding.expression, *expression);
        assert_eq!(binding.statement_ordinal, 2);
        assert_eq!(binding.role, CheckedScalarExpressionRole::Return);
        assert_eq!(
            symbols,
            &[checked.state_parameters(state)[1].symbol, local.symbol]
        );
        assert_eq!(
            plans.expression_at(state.symbol, 2, CheckedScalarExpressionRole::Return),
            Some(&CheckedScalarExpression::Local {
                position: 1,
                primitive_type: PrimitiveType::U8
            })
        );
        if let Some((previous_state, previous_expression, previous_local)) = prior {
            assert_ne!(binding.state, previous_state);
            assert_ne!(binding.expression, previous_expression);
            assert_ne!(symbols[1], previous_local);
        }
        prior = Some((binding.state, binding.expression, symbols[1]));
    }
}

#[test]
fn transition_scalar_facts_skip_implicit_self_but_retain_target_position() {
    let source = r#"
        data Main {}

        machine Main::main(&mut self, first: bool, second: bool) {
            transition first { true -> dispatch(second) _ -> done() }

            state dispatch(&mut self, flag: bool) {
                transition flag { true -> done() _ -> done() }
            }

            state done(&mut self) {}
        }
    "#;

    let checked = checked_program(source);
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("main machine");
    let entry = &checked.machine_states(machine)[0];
    assert_eq!(
        checked.facts.values.scalar_expressions.expression_at(
            entry.symbol,
            0,
            checked_trees::CheckedScalarExpressionRole::TransitionArgument {
                argument_ordinal: 1,
            },
        ),
        Some(&checked_trees::CheckedScalarExpression::Boolean(Box::new(
            checked_trees::CheckedBooleanExpression::Parameter { position: 1 }
        ))),
        "the dense scalar expression must retain the target parameter's raw position",
    );
    assert_eq!(
        checked.facts.values.scalar_expressions.expression_at(
            entry.symbol,
            0,
            checked_trees::CheckedScalarExpressionRole::TransitionArgument {
                argument_ordinal: 0,
            },
        ),
        None,
        "implicit self must not masquerade as an authored scalar argument",
    );
}

/// The value captured on `observed` at `done`'s entry, read straight from the
/// semantic contexts appended for that state point.
#[cfg(test)]
fn entry_scalar_values(
    checked: &checked_trees::CheckedTrees,
    state_name: &str,
    parameter_name: &str,
) -> Vec<facts::ScalarValue> {
    let machine = &checked.machines()[0];
    let state = checked
        .machine_states(machine)
        .iter()
        .find(|state| checked.symbols.name(state.symbol) == state_name)
        .unwrap_or_else(|| panic!("state {state_name}"));
    let parameter = checked
        .state_parameters(state)
        .iter()
        .find(|parameter| !parameter.is_self && parameter.name.as_str() == parameter_name)
        .unwrap_or_else(|| panic!("parameter {parameter_name}"));
    let semantic = &checked.facts.semantic;
    semantic
        .contexts
        .iter()
        .filter(|(_, context)| {
            context.point
                == facts::ProgramPoint::State {
                    machine_symbol: machine.symbol,
                    state_symbol: state.symbol,
                }
        })
        .flat_map(|(_, context)| {
            semantic.context_view(context).facts().filter_map(|fact| {
                let facts::FactPayload::AssignedScalarValue { value } = fact.payload else {
                    return None;
                };
                let facts::FactPlace::Place(place) = fact.place else {
                    return None;
                };
                (semantic.places.get(place).root == facts::PlaceRoot::Symbol(parameter.symbol))
                    .then(|| semantic.scalar_values.get(value).clone())
            })
        })
        .collect()
}

#[test]
fn byte_carrier_live_length_reaches_the_saved_state_argument() {
    // `self.out.len` is the live length of a bounded byte carrier: an exact
    // u64 observation of the caller's "XXX" entry snapshot, not a carrier-wide
    // u64 guess.
    let checked = lower_typed_trees(
        typed_program(
            r#"
        domain [u8;3]::Utf8 requires valid_utf8(self);
        data Record { out: [u8;3] in Utf8; }
        machine Record::measure(&mut self) {
            self.out = "XXX";
            let n: u64 = self.out.len;
            transition { _ -> done(n) }
            state done(&mut self, count: u64) {}
        }
    "#,
        ),
        &CheckingRequest::settled(),
    )
    .expect("live length checks");
    assert!(
        entry_scalar_values(&checked, "done", "count").contains(&facts::ScalarValue::Integer(
            numerics::bignum::BigInt::from_u64(3)
        )),
        "the literal byte carrier's live length must reach the saved argument"
    );
}
