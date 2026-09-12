//! Publication independently rejoins computed primitive borrows to source.

use arena::{Handle, HandleSpan};
use checked_trees::expression::ExpressionNode;
use checked_trees::{
    BorrowAccessKind, BorrowCallFact, CheckedScalarComputationHandle, CheckedScalarComputationKind,
    CheckedScalarComputationStructuralArgument, CheckedStructuralAccess, CheckedTrees,
    CheckedUnitStructuralArgumentSourcePlan,
};

const LOCAL_SOURCE: &str = r#"
    machine stamp(value: &mut u64, number: u64) -> u64 { value = number; number }
    machine second(first: u64, second: u64) -> u64 { second }
    machine consume(output: &mut u64, value: u64) { output = value; }
    machine enter(output: &mut u64) {
        let mut first: u64 = 41;
        let mut second: u64 = 53;
        consume(&mut output, second(stamp(&mut first, 7), stamp(&mut second, 9)));
        output = first;
    }
"#;

const PARAMETER_SOURCE: &str = r#"
    machine stamp(value: &mut u64, number: u64) -> u64 { value = number; number }
    machine second(first: u64, second: u64) -> u64 { second }
    machine consume(output: &mut u64, value: u64) { output = value; }
    machine enter(marker: u64, output: &mut u64, other: &mut u64) {
        consume(&mut output, second(stamp(&mut other, 7), marker));
    }
"#;

fn checked(source: &str) -> CheckedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    typed_trees_to_checked_trees::lower_typed_trees(typed).unwrap()
}

fn publish_original(source: &str) -> CheckedTrees {
    let checked = checked(source);
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "enter")
        .produce_artifact()
        .expect("unmodified source must publish before custody mutations are meaningful");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    assert!(
        module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(
                operation.kind,
                terminal_psi::OperationKind::CallStructuralScalar { .. }
            ))
    );
    checked
}

fn reject(checked: &CheckedTrees, mutation: &str) {
    assert!(
        terminal_production::TerminalProductionRequest::new(checked, "enter")
            .produce_artifact()
            .is_err(),
        "publication accepted computed borrow mutation: {mutation}"
    );
}

fn calls(checked: &CheckedTrees) -> Vec<CheckedScalarComputationHandle> {
    calls_to(checked, "stamp")
}

fn calls_to(checked: &CheckedTrees, name: &str) -> Vec<CheckedScalarComputationHandle> {
    let target = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == name)
        .expect("stamp machine")
        .symbol;
    let calls = checked
        .facts
        .values
        .scalar_computations
        .nodes
        .iter()
        .filter_map(|(handle, node)| {
            matches!(node.kind, CheckedScalarComputationKind::Call { target_machine, .. }
            if target_machine == target)
            .then_some(handle)
        })
        .collect::<Vec<_>>();
    assert!(
        !calls.is_empty(),
        "fixture retains borrowed operand computations"
    );
    calls
}

fn shared_occurrence_source(arguments: &str) -> String {
    format!(
        r#"
        machine hold(first: &u64, number: u64, second: &u64) -> u64 {{ number }}
        machine identity(value: u64) -> u64 {{ value }}
        machine consume(output: &mut u64, value: u64) {{ output = value; }}
        machine enter(output: &mut u64, other: &u64) {{
            let mut scratch: u64 = 91;
            consume(&mut output, hold({arguments}));
        }}
    "#
    )
}

#[test]
fn shared_computation_occurrences_publish_repeated_roots_with_intervening_scalar_reads() {
    for arguments in [
        "&scratch, 11, &scratch",
        "&scratch, scratch, &scratch",
        "&scratch, identity(scratch), &scratch",
        "&scratch, scratch, other",
    ] {
        let checked = publish_original(&shared_occurrence_source(arguments));
        for call in calls_to(&checked, "hold") {
            assert_eq!(structural_span(&checked, call).count(), 2);
        }
    }
}

#[test]
fn shared_computation_occurrences_reject_synchronized_access_roster_mutations() {
    for arguments in ["&scratch, scratch, &scratch", "&scratch, scratch, other"] {
        let original = publish_original(&shared_occurrence_source(arguments));
        for mutation in [
            "extra",
            "missing borrow",
            "missing scalar",
            "exclusive",
            "substituted",
            "reordered",
        ] {
            if mutation == "reordered" && arguments.ends_with("&scratch") {
                continue;
            }
            let mut changed = original.clone();
            for computation in calls_to(&original, "hold") {
                let call = borrow_call(&original, computation);
                let CheckedScalarComputationKind::Call { source_call, .. } = original
                    .facts
                    .values
                    .scalar_computations
                    .nodes
                    .get(computation)
                    .kind
                else {
                    panic!("computed hold");
                };
                let mut rows = original
                    .facts
                    .borrow
                    .argument_accesses
                    .span(original.facts.borrow.calls.get(call).accesses)
                    .unwrap()
                    .to_vec();
                assert_eq!(rows.len(), 3);
                match mutation {
                    "extra" => rows.push(rows[0].clone()),
                    "missing borrow" => {
                        rows.remove(2);
                    }
                    "missing scalar" => {
                        rows.remove(1);
                    }
                    "exclusive" => rows[2].kind = BorrowAccessKind::Mutable,
                    "substituted" => rows[1].root_symbol = symbols::SymbolHandle::invalid(),
                    "reordered" => rows.swap(0, 2),
                    _ => unreachable!(),
                }
                let span = changed.facts.borrow.argument_accesses.insert_many(rows);
                changed.facts.borrow.calls.get_mut(call).accesses = span;
                changed
                    .facts
                    .flow
                    .control
                    .calls
                    .get_mut(source_call)
                    .accesses = span;
            }
            reject(
                &changed,
                &format!("arguments={arguments}, synchronized roster={mutation}"),
            );
        }
    }
}

#[test]
fn shared_computation_occurrences_reject_swapped_plan_and_source_positions() {
    let original = publish_original(&shared_occurrence_source("&scratch, scratch, other"));
    for swap_source in [false, true] {
        let mut changed = original.clone();
        for computation in calls_to(&original, "hold") {
            if swap_source {
                let CheckedScalarComputationKind::Call { source_call, .. } = original
                    .facts
                    .values
                    .scalar_computations
                    .nodes
                    .get(computation)
                    .kind
                else {
                    panic!("hold call");
                };
                let expression = original
                    .facts
                    .flow
                    .control
                    .calls
                    .get(source_call)
                    .authored_expression;
                let ExpressionNode::Call(call) = original.expression_table.expression(expression)
                else {
                    panic!("authored hold");
                };
                let mut arguments = original
                    .expression_table
                    .expression_handles(call.arguments)
                    .to_vec();
                arguments.swap(0, 2);
                let span = changed
                    .typed
                    .expression_table
                    .insert_expression_handles(arguments);
                let ExpressionNode::Call(call) =
                    changed.typed.expression_table.expression_mut(expression)
                else {
                    panic!("authored hold");
                };
                call.arguments = span;
            } else {
                let span = structural_span(&original, computation);
                changed
                    .facts
                    .values
                    .scalar_computations
                    .structural_arguments
                    .span_mut(span)
                    .unwrap()
                    .swap(0, 1);
            }
        }
        reject(
            &changed,
            if swap_source {
                "swapped authored shared places"
            } else {
                "swapped shared plan positions"
            },
        );
    }
}

fn structural_span(
    checked: &CheckedTrees,
    call: CheckedScalarComputationHandle,
) -> HandleSpan<CheckedScalarComputationStructuralArgument> {
    let CheckedScalarComputationKind::Call {
        structural_arguments,
        ..
    } = checked
        .facts
        .values
        .scalar_computations
        .nodes
        .get(call)
        .kind
    else {
        panic!("retained call");
    };
    structural_arguments
}

fn borrow_call(
    checked: &CheckedTrees,
    computation: CheckedScalarComputationHandle,
) -> Handle<BorrowCallFact> {
    let CheckedScalarComputationKind::Call { source_call, .. } = checked
        .facts
        .values
        .scalar_computations
        .nodes
        .get(computation)
        .kind
    else {
        panic!("retained call");
    };
    let source = checked.facts.flow.control.calls.get(source_call);
    let mut matching = checked.facts.borrow.calls.iter().filter(|(_, call)| {
        call.target_symbol == source.target_symbol
            && call.statement_index == source.statement_index
            && call.call_ordinal == source.call_ordinal
    });
    let (handle, _) = matching.next().expect("exact borrow call");
    assert!(matching.next().is_none());
    handle
}

fn borrowed_expression(
    checked: &CheckedTrees,
    computation: CheckedScalarComputationHandle,
) -> checked_trees::expression::ExpressionHandle {
    let CheckedScalarComputationKind::Call { source_call, .. } = checked
        .facts
        .values
        .scalar_computations
        .nodes
        .get(computation)
        .kind
    else {
        panic!("retained call");
    };
    let source = checked.facts.flow.control.calls.get(source_call);
    let ExpressionNode::Call(call) = checked
        .expression_table
        .expression(source.authored_expression)
    else {
        panic!("authored call");
    };
    checked.expression_table.expression_handles(call.arguments)[0]
}

#[test]
fn structural_argument_spans_cannot_be_missing_stale_or_duplicated() {
    let original = publish_original(LOCAL_SOURCE);
    for mutation in ["missing", "stale", "duplicate"] {
        let mut changed = original.clone();
        for call in calls(&original) {
            let span = structural_span(&original, call);
            assert_eq!(span.count(), 1);
            let replacement = match mutation {
                "missing" => HandleSpan::empty(),
                "stale" => HandleSpan::from_parts(
                    Handle::from_parts(span.start().arena_index(), span.start().generation() + 1),
                    1,
                ),
                "duplicate" => {
                    let argument = original
                        .facts
                        .values
                        .scalar_computations
                        .structural_arguments
                        .get(span.start())
                        .clone();
                    changed
                        .facts
                        .values
                        .scalar_computations
                        .structural_arguments
                        .insert_many([argument.clone(), argument])
                }
                _ => unreachable!(),
            };
            let CheckedScalarComputationKind::Call {
                structural_arguments,
                ..
            } = &mut changed
                .facts
                .values
                .scalar_computations
                .nodes
                .get_mut(call)
                .kind
            else {
                panic!("retained call");
            };
            *structural_arguments = replacement;
        }
        reject(&changed, mutation);
    }
}

#[test]
fn same_typed_structural_arguments_cannot_move_between_call_occurrences() {
    let original = publish_original(LOCAL_SOURCE);
    let handles = calls(&original);
    let mut changed = original.clone();
    for call in &handles {
        let span = structural_span(&original, *call);
        let argument = original
            .facts
            .values
            .scalar_computations
            .structural_arguments
            .get(span.start())
            .as_place()
            .expect("retained local place");
        let replacement = handles
            .iter()
            .map(|other| structural_span(&original, *other))
            .find(|other| {
                original
                    .facts
                    .values
                    .scalar_computations
                    .structural_arguments
                    .get(other.start())
                    .as_place()
                    .expect("retained other local place")
                    .source
                    != argument.source
            })
            .expect("different same-typed local");
        let CheckedScalarComputationKind::Call {
            structural_arguments,
            ..
        } = &mut changed
            .facts
            .values
            .scalar_computations
            .nodes
            .get_mut(*call)
            .kind
        else {
            panic!("retained call");
        };
        *structural_arguments = replacement;
    }
    reject(&changed, "swapped structural rows between exact calls");
}

#[test]
fn primitive_arguments_reject_type_access_path_and_source_drift() {
    let original = publish_original(LOCAL_SOURCE);
    for mutation in [
        "type",
        "shared",
        "writeonly",
        "owned",
        "path",
        "stale source",
        "parameter",
    ] {
        let mut changed = original.clone();
        for call in calls(&original) {
            let span = structural_span(&original, call);
            let argument = changed
                .facts
                .values
                .scalar_computations
                .structural_arguments
                .get_mut(span.start())
                .as_place_mut()
                .expect("retained primitive place");
            match mutation {
                "type" => argument.type_identity = "u8".to_owned(),
                "shared" => argument.access = CheckedStructuralAccess::SharedBorrow,
                "writeonly" => argument.access = CheckedStructuralAccess::WriteOnlyBorrow,
                "owned" => argument.access = CheckedStructuralAccess::Owned,
                "path" => {
                    argument
                        .path
                        .push(checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(
                            0,
                        ))
                }
                "stale source" => {
                    let CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { symbol } =
                        &mut argument.source
                    else {
                        panic!("primitive local");
                    };
                    *symbol = symbols::SymbolHandle::from_parts(
                        symbol.arena_index(),
                        symbol.generation() + 1,
                    );
                }
                "parameter" => {
                    argument.source =
                        CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 0 }
                }
                _ => unreachable!(),
            }
        }
        reject(&changed, mutation);
    }
}

#[test]
fn primitive_borrow_rows_reject_omission_duplication_access_path_and_call_drift() {
    let original = publish_original(LOCAL_SOURCE);
    for mutation in [
        "missing access",
        "stale access",
        "copied access",
        "duplicate access",
        "access",
        "path",
        "call target",
        "call ordinal",
        "duplicate call",
    ] {
        let mut changed = original.clone();
        // Every retained computation of an occurrence must see the same changed
        // borrow row, including separately retained argument roots.
        let mut mutated = Vec::new();
        for computation in calls(&original) {
            let handle = borrow_call(&original, computation);
            if mutated.contains(&handle) {
                continue;
            }
            mutated.push(handle);
            let call = original.facts.borrow.calls.get(handle);
            assert_eq!(
                call.accesses.count(),
                1,
                "only the borrowed local is observed; scalar is literal"
            );
            match mutation {
                "missing access" => {
                    changed.facts.borrow.calls.get_mut(handle).accesses = HandleSpan::empty()
                }
                "stale access" => {
                    changed.facts.borrow.calls.get_mut(handle).accesses = HandleSpan::from_parts(
                        Handle::from_parts(
                            call.accesses.start().arena_index(),
                            call.accesses.start().generation() + 1,
                        ),
                        1,
                    )
                }
                "duplicate access" => {
                    let argument = original
                        .facts
                        .borrow
                        .argument_accesses
                        .get(call.accesses.start())
                        .clone();
                    let span = changed
                        .facts
                        .borrow
                        .argument_accesses
                        .insert_many([argument.clone(), argument]);
                    changed.facts.borrow.calls.get_mut(handle).accesses = span;
                }
                "copied access" => {
                    let rows = original
                        .facts
                        .borrow
                        .argument_accesses
                        .span(call.accesses)
                        .unwrap()
                        .to_vec();
                    let span = changed.facts.borrow.argument_accesses.insert_many(rows);
                    changed.facts.borrow.calls.get_mut(handle).accesses = span;
                }
                "access" => {
                    changed
                        .facts
                        .borrow
                        .argument_accesses
                        .get_mut(call.accesses.start())
                        .kind = BorrowAccessKind::Read
                }
                "path" => {
                    let span = changed.facts.borrow.access_segments.insert_many([
                        facts::PlaceSegment::Field {
                            symbol: original
                                .facts
                                .borrow
                                .argument_accesses
                                .get(call.accesses.start())
                                .root_symbol,
                        },
                    ]);
                    changed
                        .facts
                        .borrow
                        .argument_accesses
                        .get_mut(call.accesses.start())
                        .segments = span;
                }
                "call target" => {
                    changed.facts.borrow.calls.get_mut(handle).target_symbol =
                        symbols::SymbolHandle::invalid()
                }
                "call ordinal" => changed.facts.borrow.calls.get_mut(handle).call_ordinal += 100,
                "duplicate call" => {
                    let state = original
                        .facts
                        .borrow
                        .states
                        .iter()
                        .find_map(|(state_handle, state)| {
                            original
                                .facts
                                .borrow
                                .calls
                                .span(state.calls)
                                .unwrap()
                                .iter()
                                .any(|candidate| std::ptr::eq(candidate, call))
                                .then_some(state_handle)
                        })
                        .unwrap();
                    let mut rows = changed
                        .facts
                        .borrow
                        .calls
                        .span(changed.facts.borrow.states.get(state).calls)
                        .unwrap()
                        .to_vec();
                    rows.push(call.clone());
                    let span = changed.facts.borrow.calls.insert_many(rows);
                    changed.facts.borrow.states.get_mut(state).calls = span;
                }
                _ => unreachable!(),
            }
        }
        reject(&changed, mutation);
    }
}

#[test]
fn synchronized_plan_or_source_and_borrow_rows_cannot_substitute_another_local() {
    let original = publish_original(LOCAL_SOURCE);
    let computations = calls(&original);
    for synchronize_source in [false, true] {
        let mut changed = original.clone();
        for computation in &computations {
            let span = structural_span(&original, *computation);
            let argument = original
                .facts
                .values
                .scalar_computations
                .structural_arguments
                .get(span.start())
                .as_place()
                .expect("retained local place");
            let other = computations
                .iter()
                .copied()
                .find(|other| {
                    original
                        .facts
                        .values
                        .scalar_computations
                        .structural_arguments
                        .get(structural_span(&original, *other).start())
                        .as_place()
                        .expect("retained other local place")
                        .source
                        != argument.source
                })
                .unwrap();
            let replacement = original
                .facts
                .values
                .scalar_computations
                .structural_arguments
                .get(structural_span(&original, other).start())
                .as_place()
                .expect("retained replacement local place")
                .source
                .clone();
            let CheckedUnitStructuralArgumentSourcePlan::PrimitiveLocal { symbol } = replacement
            else {
                panic!("other local");
            };
            let call = borrow_call(&original, *computation);
            let access = original.facts.borrow.calls.get(call).accesses.start();
            changed
                .facts
                .borrow
                .argument_accesses
                .get_mut(access)
                .root_symbol = symbol;
            if synchronize_source {
                let expression = borrowed_expression(&original, *computation);
                let other_expression = borrowed_expression(&original, other);
                let ExpressionNode::Borrow(other_borrow) =
                    original.expression_table.expression(other_expression)
                else {
                    panic!("other borrow");
                };
                let ExpressionNode::Borrow(borrow) =
                    changed.typed.expression_table.expression_mut(expression)
                else {
                    panic!("authored borrow");
                };
                borrow.target = other_borrow.target;
            } else {
                changed
                    .facts
                    .values
                    .scalar_computations
                    .structural_arguments
                    .get_mut(span.start())
                    .as_place_mut()
                    .expect("retained primitive local place")
                    .source = replacement;
            }
        }
        reject(
            &changed,
            if synchronize_source {
                "source and borrow agree, retained plan differs"
            } else {
                "plan and borrow agree, authored source differs"
            },
        );
    }
}

#[test]
fn dense_structural_parameter_position_cannot_be_an_authored_or_other_position() {
    let original = publish_original(PARAMETER_SOURCE);
    for parameter_index in [0, 2, u32::MAX] {
        let mut changed = original.clone();
        for call in calls(&original) {
            let span = structural_span(&original, call);
            let argument = changed
                .facts
                .values
                .scalar_computations
                .structural_arguments
                .get_mut(span.start())
                .as_place_mut()
                .expect("retained parameter place");
            assert_eq!(
                argument.source,
                CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index: 1 }
            );
            argument.source =
                CheckedUnitStructuralArgumentSourcePlan::Parameter { parameter_index };
        }
        reject(
            &changed,
            "structural parameter index uses exact dense source position",
        );
    }
}

#[test]
fn authored_borrow_and_checked_access_cannot_widen_writeonly_parameter() {
    let original = publish_original(PARAMETER_SOURCE);
    let mut changed = original.clone();
    let machine = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "enter")
        .unwrap();
    let state = &original.machine_states(machine)[0];
    let source_reference = original.state_parameters(state)[2].type_reference;
    let checked_trees::types::TypeReferenceNode::Reference {
        referee, lifetime, ..
    } = original
        .type_reference_table
        .type_reference(source_reference)
    else {
        panic!("source reference");
    };
    changed.typed.type_reference_table.substitute_node(
        source_reference,
        checked_trees::types::TypeReferenceNode::Reference {
            referee: *referee,
            access: language_core::ReferenceAccess::WriteOnly,
            lifetime: lifetime.clone(),
        },
    );
    // Keeping both the explicit readable/mutable borrow and its checked access
    // row intact still cannot supply authority absent from the declaration.
    reject(
        &changed,
        "declared write-only source cannot supply a readable mutable borrow",
    );
}

#[test]
fn synchronized_authored_and_checked_access_cannot_change_the_formal_borrow() {
    let original = publish_original(LOCAL_SOURCE);
    let mut changed = original.clone();
    for computation in calls(&original) {
        let expression = borrowed_expression(&original, computation);
        let ExpressionNode::Borrow(borrow) =
            changed.typed.expression_table.expression_mut(expression)
        else {
            panic!("authored borrow");
        };
        borrow.access = language_core::ReferenceAccess::Shared;
        let call = borrow_call(&original, computation);
        let access = original.facts.borrow.calls.get(call).accesses.start();
        changed.facts.borrow.argument_accesses.get_mut(access).kind = BorrowAccessKind::Read;
        let span = structural_span(&original, computation);
        changed
            .facts
            .values
            .scalar_computations
            .structural_arguments
            .get_mut(span.start())
            .as_place_mut()
            .expect("retained borrowed place")
            .access = CheckedStructuralAccess::SharedBorrow;
    }
    reject(
        &changed,
        "authored and checked shared borrow cannot replace the mutable formal",
    );
}

#[test]
fn borrowed_source_expression_cannot_be_stale_or_projected() {
    let original = publish_original(LOCAL_SOURCE);
    for mutation in ["stale borrowed expression", "nested borrow"] {
        let mut changed = original.clone();
        for computation in calls(&original) {
            let expression = borrowed_expression(&original, computation);
            let ExpressionNode::Borrow(original_borrow) =
                original.expression_table.expression(expression)
            else {
                panic!("original borrow");
            };
            let replacement = match mutation {
                "stale borrowed expression" => Handle::from_parts(
                    original_borrow.target.arena_index(),
                    original_borrow.target.generation() + 1,
                ),
                "nested borrow" => changed
                    .typed
                    .expression_table
                    .insert(ExpressionNode::Borrow(*original_borrow)),
                _ => unreachable!(),
            };
            let ExpressionNode::Borrow(borrow) =
                changed.typed.expression_table.expression_mut(expression)
            else {
                panic!("authored borrow");
            };
            borrow.target = replacement;
        }
        reject(&changed, mutation);
    }
}

#[test]
fn computed_storage_read_before_or_after_borrow_keeps_its_authored_local() {
    const SOURCE: &str = r#"
        machine stamp(value: &mut u64, number: u64) -> u64 { value = number; number }
        machine second(first: u64, second: u64) -> u64 { second }
        machine consume(output: &mut u64, value: u64) { output = value; }
        machine enter(output: &mut u64) {
            let mut first: u64 = 41;
            let mut other: u64 = 53;
            consume(&mut output, second(OPERANDS));
            output = other;
        }
    "#;
    for operands in ["first, stamp(&mut first, 7)", "stamp(&mut first, 7), first"] {
        let original = publish_original(&SOURCE.replace("OPERANDS", operands));
        let caller = original
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "enter")
            .unwrap();
        let state = &original.machine_states(caller)[0];
        let locals = original
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .filter_map(|statement| match statement {
                checked_trees::statement::StatementNode::LocalData(local) => Some(local.symbol),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(locals.len(), 2);
        let literal = original
            .facts
            .values
            .scalar_computations
            .nodes
            .iter()
            .find_map(|(_, node)| {
                let CheckedScalarComputationKind::Value(
                    value @ checked_trees::CheckedScalarExpression::IntegerLiteral { .. },
                ) = &node.kind
                else {
                    return None;
                };
                Some(value.clone())
            })
            .expect("stamp retains its literal scalar operand");
        for substitute_literal in [false, true] {
            let mut changed = original.clone();
            let mut mutations = 0;
            for (handle, node) in original.facts.values.scalar_computations.nodes.iter() {
                let CheckedScalarComputationKind::Value(
                    checked_trees::CheckedScalarExpression::StorageRead {
                        symbol,
                        primitive_type,
                    },
                ) = &node.kind
                else {
                    continue;
                };
                if *symbol != locals[0] {
                    continue;
                }
                let replacement = if substitute_literal {
                    literal.clone()
                } else {
                    checked_trees::CheckedScalarExpression::StorageRead {
                        symbol: locals[1],
                        primitive_type: *primitive_type,
                    }
                };
                changed
                    .facts
                    .values
                    .scalar_computations
                    .nodes
                    .get_mut(handle)
                    .kind = CheckedScalarComputationKind::Value(replacement);
                mutations += 1;
            }
            assert!(
                mutations > 0,
                "fixture must retain the mutable-read operand"
            );
            reject(
                &changed,
                &format!("operands={operands}, literal={substitute_literal}"),
            );
        }
    }
}
