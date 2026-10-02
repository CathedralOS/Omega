//! Publication independently rejoins computed primitive borrows to source.

use arena::{Handle, HandleSpan};
use checked_trees::expression::ExpressionNode;
use checked_trees::{
    BorrowAccessKind, BorrowCallFact, CheckedScalarComputationHandle, CheckedScalarComputationKind,
    CheckedScalarComputationStructuralArgument, CheckedStructuralAccess, CheckedTrees,
    CheckedUnitStructuralArgumentSourcePlan,
};
use terminal_production::{
    TerminalMachineSelection, TerminalProductionCustody, TerminalProductionTimings,
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

const INDEXED_BORROW_SOURCE: &str = r#"
    data Cell [copy] { value: u64; }
    machine inspect(cell: &Cell) -> u64 { cell.value }
    machine relay(cell: &mut Cell) -> u64 { inspect(cell) }
    machine enter(cells: &mut [Cell; 2], index: u64) -> u64
    requires index <= 1 {
        relay(&mut cells[index])
    }
"#;

const MUTABLE_RANGE_INITIALIZER_SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../../../tests/omega/pass/slices/mutable_field_range_scalar_initializer_exit/main.omg"
));

#[test]
fn mutable_range_computation_executes_once_before_observing_the_backing() {
    let checked = crate::front_end::checked_program(MUTABLE_RANGE_INITIALIZER_SOURCE);
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name("main"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("the authored mutable range scalar initializer publishes")
    .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let calls = module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .filter(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::CallStructuralScalar { .. }
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(calls.len(), 1, "exactly one retained stamp invocation");
    let mut execution = terminal_interpreter::TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        &[],
        terminal_interpreter::TerminalStructuralInputs::default(),
    )
    .unwrap();
    let mut meter = terminal_fuel::TerminalFuelMeter::with_allowance(100_000);
    assert_eq!(
        execution
            .resume(&mut meter, &mut terminal_interpreter::AcceptTerminalEffects)
            .unwrap(),
        terminal_interpreter::TerminalExecutionStatus::Complete(
            terminal_interpreter::TerminalExecutionResult::Unit
        ),
        "the authored guard observes [0, 7, 9, 0] and the untouched marker"
    );
    assert_eq!(
        meter
            .usage()
            .at(terminal_fuel::FuelChargeSite::Operation(calls[0].id))
            .expect("the selected stamp call executes")
            .executions(),
        1,
        "the comparison must not replay its effectful operand"
    );
}

#[test]
fn mutable_range_computation_rejects_access_source_and_occurrence_drift() {
    let original = crate::front_end::checked_program(MUTABLE_RANGE_INITIALIZER_SOURCE);
    let lower = |checked: &CheckedTrees| {
        checked_trees_to_lowered_psi::lower_machine(checked, TerminalMachineSelection::Name("main"))
    };
    lower(&original).expect("the original range custody lowers before corruption");
    for mutation in ["shared", "owned", "source", "field", "occurrence"] {
        let mut changed = original.clone();
        for call in calls_to(&original, "stamp") {
            let span = structural_span(&original, call);
            let call_expression = original
                .facts
                .values
                .scalar_computations
                .nodes
                .get(call)
                .authored_root;
            let argument = changed
                .facts
                .values
                .scalar_computations
                .structural_arguments
                .get_mut(span.start())
                .as_place_mut()
                .expect("the computed call retains its range operand");
            assert_eq!(argument.access, CheckedStructuralAccess::MutableBorrow);
            match mutation {
                "shared" => argument.access = CheckedStructuralAccess::SharedBorrow,
                "owned" => argument.access = CheckedStructuralAccess::Owned,
                "field" => {
                    assert!(!argument.path.is_empty());
                    argument.path.clear();
                }
                "source" | "occurrence" => {
                    let CheckedUnitStructuralArgumentSourcePlan::ElementViewSubslice {
                        root,
                        expression,
                        ..
                    } = &mut argument.source
                    else {
                        panic!("the original operand retains an element range");
                    };
                    if mutation == "source" {
                        *root = checked_trees::CheckedStorageRoot::Parameter { index: 0 };
                    } else {
                        assert_ne!(*expression, call_expression);
                        *expression = call_expression;
                    }
                }
                _ => unreachable!(),
            }
        }
        assert!(
            lower(&changed).is_err(),
            "computed mutable range accepted changed {mutation} custody"
        );
    }
    let (establishment, value) = original
        .facts
        .values
        .structural_values
        .roots
        .iter()
        .find_map(|(handle, root)| {
            matches!(
                original
                    .facts
                    .values
                    .structural_values
                    .nodes
                    .get(root.root)
                    .kind,
                checked_trees::CheckedStructuralValueKind::ZeroedStructural {}
            )
            .then_some((handle, root.root))
        })
        .expect("the default WindowBuffer has an explicit storage establishment");
    for mutation in ["missing establishment", "substituted establishment"] {
        let mut changed = original.clone();
        let plans = &mut changed.facts.values.structural_values;
        if mutation == "missing establishment" {
            plans.roots.get_mut(establishment).statement_ordinal = u32::MAX;
        } else {
            plans.nodes.get_mut(value).kind = checked_trees::CheckedStructuralValueKind::Record {
                data_symbol: symbols::SymbolHandle::invalid(),
                fields: HandleSpan::default(),
            };
        }
        assert!(
            lower(&changed).is_err(),
            "computed range borrowed storage with {mutation}"
        );
    }
}

#[test]
fn indexed_borrow_evaluates_selector_before_its_retained_loan() {
    let original = publish_original(INDEXED_BORROW_SOURCE);
    let computation = calls_to(&original, "relay")[0];
    let call = borrow_call(&original, computation);
    let accesses = original.facts.borrow.calls.get(call).accesses;
    let rows = original
        .facts
        .borrow
        .argument_accesses
        .span(accesses)
        .unwrap();
    assert_eq!(
        rows.len(),
        2,
        "selector read precedes the selected cell loan"
    );
    assert_eq!(rows[0].kind, BorrowAccessKind::Read);
    assert_eq!(rows[0].end_before_access, Some(1));
    assert_eq!(rows[1].kind, BorrowAccessKind::Mutable);
    assert_eq!(rows[1].end_before_access, None);
    assert_ne!(rows[0].root_symbol, rows[1].root_symbol);
    for mutation in [
        "swap rows",
        "omit selector",
        "weaken loan",
        "extend selector",
        "shorten loan",
    ] {
        let mut changed = original.clone();
        let rows = changed
            .facts
            .borrow
            .argument_accesses
            .span_mut_or_empty(accesses);
        match mutation {
            "swap rows" => rows.swap(0, 1),
            "omit selector" => rows[0] = rows[1].clone(),
            "weaken loan" => rows[1].kind = BorrowAccessKind::Read,
            "extend selector" => rows[0].end_before_access = None,
            "shorten loan" => rows[1].end_before_access = Some(2),
            _ => unreachable!(),
        }
        reject(&changed, mutation);
    }
}

#[test]
fn indexed_shared_borrow_and_local_array_use_the_same_observation_order() {
    publish_original(&INDEXED_BORROW_SOURCE.replace("&mut", "&"));
    let local = INDEXED_BORROW_SOURCE.replace(
        "machine enter(cells: &mut [Cell; 2], index: u64)",
        "machine enter(index: u64)",
    ).replace("relay(&mut cells[index])", "let mut cells: [Cell; 2] = [Cell { value: 11 }, Cell { value: 29 }]; relay(&mut cells[index])");
    publish_original(&local);
}

#[test]
fn nested_forwarded_reference_uses_the_selected_formal_access() {
    let source = PARAMETER_SOURCE.replace("&mut other", "other");
    let original = publish_original(&source);
    let computation = calls_to(&original, "second")[0];
    let call = borrow_call(&original, computation);
    let accesses = original.facts.borrow.calls.get(call).accesses;
    let rows = original
        .facts
        .borrow
        .argument_accesses
        .span(accesses)
        .unwrap();
    assert_eq!(rows[0].kind, BorrowAccessKind::Mutable);
    assert_eq!(rows[0].end_before_access, Some(1));
    for mutation in [
        "weaken nested forwarded loan",
        "retain nested forwarded loan",
    ] {
        let mut changed = original.clone();
        let rows = changed
            .facts
            .borrow
            .argument_accesses
            .span_mut_or_empty(accesses);
        if mutation == "weaken nested forwarded loan" {
            rows[0].kind = BorrowAccessKind::Read;
        } else {
            rows[0].end_before_access = None;
        }
        reject(&changed, mutation);
    }
}

fn publish_original(source: &str) -> CheckedTrees {
    let checked = crate::front_end::checked_program(source);
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name("enter"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("unmodified source must publish before custody mutations are meaningful")
    .into_artifact();
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
        terminal_production::TerminalProductionRequest::new(
            checked,
            TerminalMachineSelection::Name("enter")
        )
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default()
        ))
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
