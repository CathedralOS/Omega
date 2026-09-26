//! A case actual retains its constructor and each authored payload occurrence.
use super::{LoweringError, lower_machine};
use crate::TerminalMachineSelection;
use checked_trees::{CheckedScalarComputationKind, CheckedScalarComputationStructuralArgument};
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use terminal_interpreter::{AcceptTerminalEffects, TerminalStructuralInputs};
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};
use terminal_psi::{OperationKind, ScalarCaseField, StructuralAccess, StructuralTypeShape};

const ORDERED_CONSTRUCTOR: &str = r#"
data Unrelated [copy] { case Other; }
machine Unrelated::new() -> Unrelated { Unrelated::Other }
data MemoryAlignment [copy] {
    case Alignment1;
    case Alignment2;
    case Alignment4;
    case Alignment8;
}
machine MemoryAlignment::default() -> MemoryAlignment { MemoryAlignment::Alignment4 }
machine MemoryAlignment::from(size: i32) -> MemoryAlignment {
    transition size {
        1 -> (MemoryAlignment::Alignment1)
        2 -> (MemoryAlignment::Alignment2)
        4 -> (MemoryAlignment::Alignment4)
        8 -> (MemoryAlignment::Alignment8)
        _ -> (MemoryAlignment::Alignment1)
    }
}
machine MemoryAlignment::get_size_in_bytes(&self) -> u64 [1..=8] {
    transition self {
        MemoryAlignment::Alignment1 -> (1)
        MemoryAlignment::Alignment2 -> (2)
        MemoryAlignment::Alignment4 -> (4)
        MemoryAlignment::Alignment8 -> (8)
    }
}
"#;

const ORDERED_PAYLOAD_EFFECTS: &str = r#"
data Choice [copy] {
    case First(before: u64, after: u64);
    case Second(before: u64, after: u64);
    case Last(before: u64, after: u64);
}
machine replace(value: &mut u64, next: u64) -> u64 {
    let before: u64 = value;
    value = next;
    before
}
machine choose(selector: i32, value: &mut u64) -> Choice {
    transition {
        selector < 2 -> (Choice::First {
            after: replace(&mut value, 11), before: replace(&mut value, 12)
        })
        selector < 4 -> (Choice::Second {
            after: replace(&mut value, 21), before: replace(&mut value, 22)
        })
        _ -> (Choice::Last {
            after: replace(&mut value, 31), before: replace(&mut value, 32)
        })
    }
}
"#;

fn ordered_plan_index(checked: &checked_trees::CheckedTrees, name: &str) -> usize {
    let machine = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == name)
        .unwrap();
    checked
        .facts
        .flow
        .terminal_unit_effects
        .composed_machines
        .iter()
        .position(|plan| plan.machine == machine.symbol)
        .expect("complete ordered source plan")
}

#[test]
fn ordered_structural_constructor_replays_guards_fallback_and_exact_values() {
    use checked_trees::{CheckedComposedUnitControlTerminatorPlan, CheckedUnitEffectOperationPlan};
    let checked = crate::front_end::checked_program(ORDERED_CONSTRUCTOR);
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("MemoryAlignment::from"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("source ordered constructor reaches canonical Terminal")
    .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    for (input, expected) in [
        (i32::MIN, "Alignment1"),
        (-1, "Alignment1"),
        (0, "Alignment1"),
        (1, "Alignment1"),
        (2, "Alignment2"),
        (3, "Alignment1"),
        (4, "Alignment4"),
        (8, "Alignment8"),
        (i32::MAX, "Alignment1"),
    ] {
        let mut execution = terminal_interpreter::TerminalExecution::start_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[terminal_interpreter::TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                value: IntegerValue::Signed(i128::from(input)),
            }],
            TerminalStructuralInputs::default(),
        )
        .unwrap();
        let terminal_interpreter::TerminalExecutionStatus::Complete(
            terminal_interpreter::TerminalExecutionResult::ScalarCase(result),
        ) = execution
            .resume(
                &mut terminal_fuel::TerminalFuelMeter::with_allowance(1000),
                &mut AcceptTerminalEffects,
            )
            .unwrap()
        else {
            panic!("constructor did not return a real case for {input}");
        };
        assert!(
            module
                .structural_types
                .iter()
                .any(|declaration| match &declaration.shape {
                    StructuralTypeShape::Sum { cases } => cases.iter().any(|case| case.id
                        == result.value.result_case
                        && case.identity == expected),
                    _ => false,
                }),
            "wrong selected constructor for {input}"
        );
    }
    let plan_index = ordered_plan_index(&checked, "MemoryAlignment::from");
    let unrelated = ordered_plan_index(&checked, "Unrelated::new");
    let unrelated_identity = checked.facts.flow.terminal_unit_effects.composed_machines[unrelated]
        .attachment_type_identity
        .clone()
        .expect("unrelated nominal owner");
    assert_ne!(
        checked.facts.flow.terminal_unit_effects.composed_machines[plan_index]
            .attachment_type_identity
            .as_ref(),
        Some(&unrelated_identity)
    );
    let mut changed = checked.clone();
    changed.facts.flow.terminal_unit_effects.composed_machines[plan_index]
        .attachment_type_identity = Some(unrelated_identity);
    assert!(
        matches!(
            lower_machine(
                &changed,
                TerminalMachineSelection::Name("MemoryAlignment::from")
            ),
            Err(LoweringError::Unsupported(
                "composed Unit attachment disagrees with its authored owner"
            ))
        ),
        "an existing unrelated nominal type cannot replace the static constructor's owner"
    );
    for corruption in [
        "reordered guards",
        "missing first guard",
        "missing fallback",
        "swapped value roots",
    ] {
        let mut changed = checked.clone();
        let CheckedComposedUnitControlTerminatorPlan::Guarded {
            arms,
            fallback,
            return_values,
        } = &mut changed.facts.flow.terminal_unit_effects.composed_machines[plan_index].states[0]
            .terminator
        else {
            panic!("ordered value destinations");
        };
        match corruption {
            "reordered guards" => changed
                .facts
                .flow
                .terminal_scalar_graphs
                .guarded_exits
                .span_mut(*arms)
                .unwrap()
                .swap(0, 1),
            "missing first guard" => {
                let mut guards = changed
                    .facts
                    .flow
                    .terminal_scalar_graphs
                    .guarded_exits
                    .span(*arms)
                    .unwrap()
                    .to_vec();
                guards.remove(0);
                let mut replacement = arena::HandleSpan::empty();
                for guard in guards {
                    changed
                        .facts
                        .flow
                        .terminal_scalar_graphs
                        .guarded_exits
                        .append_to_span(&mut replacement, guard);
                }
                // Corrupt both retained consumers; source replay must still
                // reject omission rather than merely comparing the plans.
                changed
                    .facts
                    .flow
                    .terminal_scalar_graphs
                    .guarded_tails
                    .iter_mut()
                    .find(|tail| tail.arms == *arms)
                    .unwrap()
                    .arms = replacement;
                *arms = replacement;
                return_values.remove(0);
            }
            "missing fallback" => {
                assert!(fallback.take().is_some());
                changed
                    .facts
                    .flow
                    .terminal_scalar_graphs
                    .guarded_tails
                    .iter_mut()
                    .find(|tail| tail.arms == *arms)
                    .unwrap()
                    .fallback = None;
                return_values.pop();
            }
            "swapped value roots" => {
                let (first, rest) = return_values.split_at_mut(1);
                let CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                    value: first, ..
                } = &mut first[0]
                else {
                    panic!("first return value")
                };
                let CheckedUnitEffectOperationPlan::EstablishStructuralValue {
                    value: second, ..
                } = &mut rest[0]
                else {
                    panic!("second return value")
                };
                std::mem::swap(first, second);
            }
            _ => unreachable!(),
        }
        assert!(
            lower_machine(
                &changed,
                TerminalMachineSelection::Name("MemoryAlignment::from")
            )
            .is_err(),
            "accepted {corruption}"
        );
    }
}

#[test]
fn ordered_constructor_local_sum_lends_original_result_to_scalar_getter() {
    let source = format!(
        "{ORDERED_CONSTRUCTOR}\n
        machine evaluate(size: i32) -> u64 {{
            let alignment: MemoryAlignment = MemoryAlignment::from(size);
            alignment.get_size_in_bytes()
        }}"
    );
    let checked = crate::front_end::checked_program(&source);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("evaluate"))
        .expect("ordinary scalar completion borrows the original copy sum result");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("independent completed-home custody");
    let mut changed = lowered.semantic_module.clone();
    let entry = changed
        .machines
        .iter_mut()
        .find(|machine| machine.id == changed.entry)
        .unwrap();
    let (producer_block, producer) = entry
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .operations
                .iter()
                .position(|operation| {
                    matches!(
                        operation.kind,
                        OperationKind::CallStructural { .. }
                            | OperationKind::CallStructuralWithScalarArguments { .. }
                    )
                })
                .map(|operation_index| (block_index, operation_index))
        })
        .expect("original constructor call");
    let (getter_block, getter) = entry
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .operations
                .iter()
                .position(|operation| {
                    matches!(operation.kind, OperationKind::CallStructuralScalar { .. })
                })
                .map(|operation_index| (block_index, operation_index))
        })
        .expect("borrowed getter call");
    let home = entry.blocks[producer_block].operations[producer]
        .result
        .structural()
        .unwrap()
        .place;
    let OperationKind::CallStructuralScalar {
        structural_arguments,
        ..
    } = &entry.blocks[getter_block].operations[getter].kind
    else {
        panic!("borrowed getter call");
    };
    assert_eq!(structural_arguments.len(), 1);
    assert_eq!(structural_arguments[0].place, home);
    assert_eq!(
        structural_arguments[0].access,
        StructuralAccess::SharedBorrow
    );
    assert!(structural_arguments[0].path.is_empty());
    let producer_operation = entry.blocks[producer_block].operations.remove(producer);
    let after_getter =
        getter + 1 - usize::from(producer_block == getter_block && producer < getter);
    entry.blocks[getter_block]
        .operations
        .insert(after_getter, producer_operation);
    assert!(
        terminal_verifier::validate_module(&changed).is_err(),
        "an unchanged declared copy-sum home is unavailable before its producer"
    );
}

#[test]
fn ordered_structural_payload_effects_preserve_selected_mutation_and_reject_substitution() {
    let checked = crate::front_end::checked_program(ORDERED_PAYLOAD_EFFECTS);
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        terminal_production::TerminalMachineSelection::Name("choose"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("selected payload effects reach canonical Terminal")
    .into_artifact();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let unsigned = |value| terminal_interpreter::TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        value: IntegerValue::Unsigned(value),
    };
    for (input, case_name, first_write, final_write) in [
        (-1, "First", 11, 12),
        (1, "First", 11, 12),
        (2, "Second", 21, 22),
        (3, "Second", 21, 22),
        (4, "Last", 31, 32),
    ] {
        let mut execution = terminal_interpreter::TerminalExecution::start_artifact(
            artifact.semantic_bytes(),
            artifact.proof_bytes(),
            &proof_admission::AdmissionProfile::default(),
            &[terminal_interpreter::TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                value: IntegerValue::Signed(input),
            }],
            TerminalStructuralInputs {
                arguments: &[terminal_interpreter::TerminalStructuralValue {
                    opaque_identity: 91,
                    structural_type: entry.structural_parameters[0].structural_type,
                    qualifications: Vec::new(),
                    path: Vec::new(),
                }],
                primitive_values: &[terminal_interpreter::TerminalStructuralPrimitiveValue {
                    argument_index: 0,
                    value: unsigned(7),
                }],
                ..Default::default()
            },
        )
        .unwrap();
        let terminal_interpreter::TerminalExecutionStatus::Complete(
            terminal_interpreter::TerminalExecutionResult::ScalarCase(result),
        ) = execution
            .resume(
                &mut terminal_fuel::TerminalFuelMeter::with_allowance(1000),
                &mut AcceptTerminalEffects,
            )
            .unwrap()
        else {
            panic!("selected payload did not return");
        };
        let case = module
            .structural_types
            .iter()
            .find_map(|declaration| match &declaration.shape {
                StructuralTypeShape::Sum { cases } => cases
                    .iter()
                    .find(|case| case.id == result.value.result_case),
                _ => None,
            })
            .unwrap();
        assert_eq!(case.identity, case_name);
        for (name, expected) in [("after", 7), ("before", first_write)] {
            let field = case
                .fields
                .iter()
                .find(|field| field.identity == name)
                .unwrap();
            assert!(
                result
                    .value
                    .fields
                    .contains(&(field.id, unsigned(expected))),
                "authored field order or prior snapshot changed"
            );
        }
        assert_eq!(
            execution.structural_primitive_values(),
            [terminal_interpreter::TerminalStructuralPrimitiveValue {
                argument_index: 0,
                value: unsigned(final_write),
            }]
        );
    }
    let fields = &checked.facts.values.scalar_computations.case_fields;
    let (first_handle, first) = fields.iter().next().expect("effectful payload field");
    let replacement = fields
        .iter()
        .find(|(_, field)| field.value != first.value)
        .unwrap()
        .1
        .value;
    let mut changed = checked.clone();
    changed
        .facts
        .values
        .scalar_computations
        .case_fields
        .get_mut(first_handle)
        .value = replacement;
    assert!(
        lower_machine(&changed, TerminalMachineSelection::Name("choose")).is_err(),
        "another authored mutation cannot substitute the payload call"
    );
}

#[test]
fn case_call_operands_reject_substituted_payload_and_constructor_occurrences() {
    let checked = crate::front_end::checked_program(
        r#"
        data Payload [copy] { case Empty; case Item(value: u64); }
        machine replace(value: &mut u64, replacement: u64) -> u64 {
            let previous: u64 = value;
            value = replacement;
            previous
        }
        machine consume(first: u64, payload: Payload, last: u64) -> u64 { first ^ last }
        machine evaluate(selected: bool, value: &mut u64) -> u64 {
            match selected {
                true -> consume(replace(&mut value, 11),
                    Payload::Item { value: replace(&mut value, 22) },
                    replace(&mut value, 33)),
                false -> 0
            }
        }
        "#,
    );
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("evaluate"))
        .expect("authored case actual lowers");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("independent case call verification");

    let computations = &checked.facts.values.scalar_computations;
    let mut fields = computations.case_fields.iter();
    let (field_handle, field) = fields.next().expect("one authored payload");
    assert!(fields.next().is_none());
    let (replacement, replacement_source) = computations
        .nodes
        .iter()
        .find(|(handle, node)| {
            *handle != field.value && matches!(node.kind, CheckedScalarComputationKind::Call { .. })
        })
        .map(|(handle, node)| (handle, node.authored_root))
        .expect("another effectful call with the same scalar carrier");
    let mut changed = checked.clone();
    changed
        .facts
        .values
        .scalar_computations
        .case_fields
        .get_mut(field_handle)
        .value = replacement;
    assert!(
        lower_machine(&changed, TerminalMachineSelection::Name("evaluate")).is_err(),
        "another argument's effectful call cannot replace the payload occurrence"
    );

    let mut constructors =
        computations
            .structural_arguments
            .iter()
            .filter_map(|(handle, argument)| {
                matches!(
                    argument,
                    CheckedScalarComputationStructuralArgument::Case(_)
                )
                .then_some(handle)
            });
    let constructor = constructors.next().expect("one fresh case actual");
    assert!(constructors.next().is_none());
    let mut changed = checked.clone();
    let CheckedScalarComputationStructuralArgument::Case(constructor) = changed
        .facts
        .values
        .scalar_computations
        .structural_arguments
        .get_mut(constructor)
    else {
        panic!("retained case actual");
    };
    assert_ne!(constructor.expression, replacement_source);
    constructor.expression = replacement_source;
    assert!(
        lower_machine(&changed, TerminalMachineSelection::Name("evaluate")).is_err(),
        "the fresh constructor cannot borrow another call's authored identity"
    );
}

const BOUNDED_PAYLOAD_ACTUAL: &str = r#"
data Message [copy] {
    case Move(dx: i32 [0..=50], dy: i32 [0..=50]);
    case Stop(code: i32);
}
machine consume(message: Message) -> i32 { 1 }
machine evaluate(offset: i32 [0..=40]) -> i32 {
    consume(Message::Move { dx: offset + 10, dy: 40 })
}
"#;

fn scalar_case_fields(module: &mut terminal_psi::TerminalModule) -> &mut Vec<ScalarCaseField> {
    let mut establishments = module
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .filter_map(|operation| match &mut operation.kind {
            OperationKind::EstablishScalarCase { fields, .. } => Some(fields),
            _ => None,
        });
    let fields = establishments.next().expect("one computed case actual");
    assert!(establishments.next().is_none());
    fields
}

fn replace_integer_constant(module: &mut terminal_psi::TerminalModule, from: i128, to: i128) {
    let mut constants = module
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.blocks)
        .flat_map(|block| &mut block.operations)
        .filter_map(|operation| match &mut operation.kind {
            OperationKind::IntegerConstant { value } if *value == IntegerValue::Signed(from) => {
                Some(value)
            }
            _ => None,
        });
    let value = constants.next().expect("the authored integer literal");
    assert!(constants.next().is_none());
    *value = IntegerValue::Signed(to);
}

#[test]
fn bounded_case_payload_actual_requests_each_declared_range() {
    let checked = crate::front_end::checked_program(BOUNDED_PAYLOAD_ACTUAL);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("evaluate"))
        .expect("computed payloads establish into their declared ranges");
    let verify = |module: &terminal_psi::TerminalModule| {
        terminal_verifier::verify_module(
            module,
            &lowered.proof_bundle,
            &proof_admission::AdmissionProfile::default(),
        )
        .map(drop)
        .map_err(|error| format!("{error:?}"))
    };
    verify(&lowered.semantic_module).expect("the verifier discharges each payload range");
    let mut module = lowered.semantic_module.clone();
    let fields = scalar_case_fields(&mut module);
    let requests = fields
        .iter()
        .map(|field| {
            field
                .range_obligation
                .expect("each bounded payload requests its range")
        })
        .collect::<Vec<_>>();
    assert_eq!(requests.len(), 2);
    assert_ne!(
        requests[0], requests[1],
        "each payload proves its own range"
    );

    let mut omitted = lowered.semantic_module.clone();
    scalar_case_fields(&mut omitted)[0].range_obligation = None;
    assert!(
        verify(&omitted).is_err(),
        "a bounded payload cannot be established without its range request"
    );

    let mut shared = lowered.semantic_module.clone();
    let fields = scalar_case_fields(&mut shared);
    fields[1].range_obligation = fields[0].range_obligation;
    assert!(
        verify(&shared).is_err(),
        "one range proof cannot stand in for another payload"
    );

    let mut escaped = lowered.semantic_module.clone();
    replace_integer_constant(&mut escaped, 10, 11);
    assert!(
        verify(&escaped).is_err(),
        "the verifier reconstructs the range against the actual computed payload"
    );

    let mut literal = lowered.semantic_module.clone();
    replace_integer_constant(&mut literal, 40, 51);
    assert!(
        verify(&literal).is_err(),
        "a literal payload outside its declared range is rejected"
    );

    let mut narrowed = lowered.semantic_module.clone();
    let bounds = narrowed
        .structural_types
        .iter_mut()
        .flat_map(|declaration| match &mut declaration.shape {
            StructuralTypeShape::Sum { cases } => cases.as_mut_slice(),
            _ => &mut [],
        })
        .flat_map(|case| &mut case.fields)
        .find_map(|field| match &mut field.field_type {
            terminal_psi::StructuralFieldType::BoundedInteger(bounds) => Some(bounds),
            _ => None,
        })
        .expect("a declared payload range");
    *bounds = semantic_vocabulary::BoundedIntegerType::new(
        bounds.integer_type(),
        bounds.minimum(),
        IntegerValue::Signed(49),
    )
    .unwrap();
    assert!(
        verify(&narrowed).is_err(),
        "the proof answers the declaration's range, not a producer's claim"
    );
}
