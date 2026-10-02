//! A `let`-bound closed scalar array inside a multi-state machine. Checking
//! gives the array its own structural binding ordinal and its own type, so the
//! state graph must retain that type in the unit's catalog and count that
//! ordinal before any later structural binding of the same state.

use checked_trees_to_lowered_psi::TerminalMachineSelection;
use terminal_psi::OperationKind;
use terminal_verifier::validate_module;

const SOURCE: &str = r#"
    data Pair { a: i32; b: i32; }
    data Root { out: i32; }
    machine pick(items: &[i32; 2]) -> i32 {
        items[1]
    }
    machine Root::enter(context: &mut Root) {
        let values: [i32; 2] = [30, 40];
        let pair: Pair = Pair { a: 1, b: 2 };
        context.out = pick(&values);
        transition { _ -> done(context) }
        state done(context: &mut Root) {}
    }
"#;

#[test]
fn state_graph_scalar_array_local_keeps_its_type_and_binding_ordinal() {
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &crate::front_end::checked_program(SOURCE),
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("a state's scalar-array local lowers ahead of a later record local");
    validate_module(&lowered.semantic_module).expect("the lowered state graph validates");
    assert!(
        lowered
            .semantic_module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(operation.kind, OperationKind::EstablishScalarArray { .. })),
        "the array local establishes its elements"
    );
}

const LOCAL_STORES: &str = r#"
    data Root { out: i32; }
    machine Root::enter(context: &mut Root) {
        let mut values: [i32; 2];
        let mut other: [i32; 2];
        values[0] = 30;
        values[1] = 40;
        transition values[0] == 30 && values[1] == 40 {
            true -> finish(context, 70)
            false -> finish(context, 71)
        }
        state finish(context: &mut Root, result: i32) {
            context.out = result;
        }
    }
"#;

#[test]
fn state_graph_local_array_stores_rejoin_the_authored_destination() {
    let checked = crate::front_end::checked_program(LOCAL_STORES);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("local array stores compose with a conditional state transfer");
    validate_module(&lowered.semantic_module).expect("local array storage validates");

    let other = checked
        .machines()
        .iter()
        .flat_map(|machine| checked.machine_states(machine))
        .flat_map(|state| checked.statement_table.statements(state.statement_nodes))
        .find_map(|statement| match statement {
            checked_trees::statement::StatementNode::LocalData(local)
                if local.name.as_str() == "other" =>
            {
                Some(local.symbol)
            }
            _ => None,
        })
        .expect("other local array");
    for forged_destination in [symbols::SymbolHandle::invalid(), other] {
        let mut changed = checked.clone();
        let destination = changed
            .facts
            .flow
            .terminal_unit_effects
            .composed_machines
            .iter_mut()
            .flat_map(|machine| &mut machine.states)
            .flat_map(|state| &mut state.operations)
            .find_map(|operation| match operation {
                checked_trees::CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
                    destination: checked_trees::CheckedStoreRoot::Local { symbol },
                    ..
                } => Some(symbol),
                _ => None,
            })
            .expect("projected store to a local array");
        *destination = forged_destination;
        assert!(
            checked_trees_to_lowered_psi::lower_machine(
                &changed,
                TerminalMachineSelection::Name("Root::enter"),
            )
            .is_err(),
            "a missing or substituted local destination must reject"
        );
    }
}

const INITIALIZED_LOAN: &str = r#"
    data Root { out: u32; }
    machine Root::enter(context: &mut Root) {
        let mut values: [u32; 2] = [30, 40];
        let mut other: [u32; 2] = [50, 60];
        transition { _ -> write(context, &mut values) }
        state write(context: &mut Root, values: &mut [u32; 2]) {
            values[0] = 7;
            transition values[0] == 7 && values[1] == 40 {
                true -> done(context, 70)
                false -> done(context, 71)
            }
        }
        state done(context: &mut Root, result: u32) {
            context.out = result;
        }
    }
"#;

#[test]
fn initialized_array_state_loan_rejoins_its_local_and_constructor() {
    use checked_trees::{
        CheckedComposedUnitControlTerminatorPlan, CheckedStructuralControlTransferSourcePlan,
        CheckedUnitEffectOperationPlan, CheckedUnitStructuralArgumentSourcePlan,
    };

    let checked = crate::front_end::checked_program(INITIALIZED_LOAN);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("initialized storage loans through the ordinary state edge");
    validate_module(&lowered.semantic_module).expect("initialized array loan validates");

    for mutation in ["loan source", "constructor elements", "constructor owner"] {
        let mut changed = checked.clone();
        let entry = &mut changed.facts.flow.terminal_unit_effects.composed_machines[0].states[0];
        let arrays = entry
            .operations
            .iter()
            .filter_map(|operation| {
                if let CheckedUnitEffectOperationPlan::EstablishScalarArray {
                    result,
                    elements,
                    ..
                } = operation
                {
                    Some((result.clone(), elements.clone()))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(arrays.len(), 2);
        if mutation == "loan source" {
            let CheckedComposedUnitControlTerminatorPlan::Jump { successor } =
                &mut entry.terminator
            else {
                panic!("authored state jump");
            };
            let subject = successor
                .transfers
                .iter_mut()
                .find_map(|transfer| {
                    if let CheckedStructuralControlTransferSourcePlan::ProjectedLoan {
                        subject,
                        ..
                    } = &mut transfer.source
                    {
                        Some(subject)
                    } else {
                        None
                    }
                })
                .expect("local array loan");
            subject.source = CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                binding_ordinal: arrays[1].0.binding_ordinal,
            };
        } else {
            let (result, elements) = entry
                .operations
                .iter_mut()
                .find_map(|operation| {
                    if let CheckedUnitEffectOperationPlan::EstablishScalarArray {
                        result,
                        elements,
                        ..
                    } = operation
                    {
                        Some((result, elements))
                    } else {
                        None
                    }
                })
                .expect("first array constructor");
            if mutation == "constructor elements" {
                *elements = arrays[1].1.clone();
            } else {
                result.statement_index = arrays[1].0.statement_index;
            }
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(
                &changed,
                TerminalMachineSelection::Name("Root::enter"),
            )
            .is_err(),
            "replay rejects substituted {mutation}"
        );
    }
}

#[test]
fn mutable_array_initializer_also_composes_with_an_ordinary_call() {
    let checked =
        crate::front_end::checked_program(&SOURCE.replace("let values:", "let mut values:"));
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("mutable array storage uses the existing ordinary call loan");
    validate_module(&lowered.semantic_module).expect("mutable array call validates");
}

fn owned_array_source(local: bool) -> String {
    let (prefix, argument) = if local {
        ("let values: [i32; 2] = [30, 40];", "values")
    } else {
        ("", "[30, 40]")
    };
    format!(
        "boundary trait Output {{ machine write(marker: i32) reaches Output; }}
         data Root {{}}
         machine Root::enter() reaches Output {{
             {prefix}
             transition {{ _ -> take(7, {argument}, 9) }}
             state take(before: i32, values: [i32; 2], after: i32) {{
                 Output::write(before);
                 Output::write(values[0]);
                 Output::write(values[1]);
                 Output::write(after);
             }}
         }}"
    )
}

fn owned_array_effects(checked: &checked_trees::CheckedTrees) -> Vec<i128> {
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("owned arrays use the ordinary structural state transfer");
    let execution = super::interpret_terminal_artifact_measured(
        &super::encode_module(&lowered.semantic_module).unwrap(),
        &super::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap(),
        &super::AdmissionProfile::default(),
        &[],
        terminal_interpreter::TerminalStructuralInputs::default(),
        &mut terminal_interpreter::AcceptTerminalEffects,
    )
    .expect("owned array state transfer survives independent artifact replay");
    assert_eq!(execution.value(), super::TerminalExecutionResult::Unit);
    execution
        .effects()
        .iter()
        .map(|effect| {
            let super::TerminalEffect::BoundaryCall { arguments, .. } = effect else {
                panic!("expected scalar output");
            };
            let [
                terminal_interpreter::TerminalScalarValue::Integer {
                    value: semantic_vocabulary::IntegerValue::Signed(value),
                    ..
                },
            ] = arguments.as_slice()
            else {
                panic!("one signed output value");
            };
            *value
        })
        .collect()
}

#[test]
fn owned_array_state_arguments_preserve_literals_locals_and_scalar_positions() {
    for local in [false, true] {
        let checked = crate::front_end::checked_program(&owned_array_source(local));
        assert_eq!(owned_array_effects(&checked), [7, 30, 40, 9]);
    }
}

#[test]
fn owned_array_state_argument_replay_rejects_substituted_producers_and_custody() {
    use checked_trees::{
        CheckedComposedUnitControlTerminatorPlan, CheckedStructuralControlTransferSourcePlan,
        CheckedStructuralValueKind, CheckedUnitEffectOperationPlan,
    };
    let checked = crate::front_end::checked_program(&owned_array_source(false));
    assert_eq!(owned_array_effects(&checked), [7, 30, 40, 9]);
    let array = checked
        .facts
        .values
        .structural_values
        .nodes
        .iter()
        .find_map(|(handle, node)| {
            matches!(node.kind, CheckedStructuralValueKind::ScalarArray { .. }).then_some(handle)
        })
        .expect("authored primitive array");
    for mutation in ["leaf order", "zeroed producer", "owned custody"] {
        let mut changed = checked.clone();
        if mutation == "owned custody" {
            let edge = changed
                .facts
                .flow
                .terminal_unit_effects
                .composed_machines
                .iter_mut()
                .flat_map(|machine| &mut machine.states)
                .find_map(|state| match &mut state.terminator {
                    CheckedComposedUnitControlTerminatorPlan::Jump { successor } => Some(successor),
                    _ => None,
                })
                .expect("array state edge");
            let establishment = edge
                .transfers
                .iter_mut()
                .find_map(|transfer| match &mut transfer.source {
                    CheckedStructuralControlTransferSourcePlan::EstablishedValue {
                        establishment,
                        ..
                    } => Some(establishment),
                    _ => None,
                })
                .expect("edge array establishment");
            let CheckedUnitEffectOperationPlan::EstablishStructuralValue { result, .. } =
                establishment.as_mut()
            else {
                panic!("structural array establishment");
            };
            result.multiplicity = language_semantics::Multiplicity::Affine;
        } else {
            let node = changed.facts.values.structural_values.nodes.get_mut(array);
            if mutation == "zeroed producer" {
                node.kind = CheckedStructuralValueKind::ZeroedStructural {};
            } else {
                let CheckedStructuralValueKind::ScalarArray { elements } = &mut node.kind else {
                    panic!("primitive array leaf roster");
                };
                elements.swap(0, 1);
            }
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(
                &changed,
                TerminalMachineSelection::Name("Root::enter"),
            )
            .is_err(),
            "independent source replay rejects {mutation}"
        );
    }
}

#[test]
fn owned_array_state_operands_execute_once_only_on_the_selected_edge() {
    let source = r#"
        boundary trait Output { machine write(marker: i32) reaches Output; }
        machine trace(value: i32) -> i32 reaches Output {
            Output::write(value);
            value
        }
        machine relay(selected: bool) reaches Output {
            transition selected {
                true -> take(trace(7), [trace(30), trace(40)], trace(9))
                false -> take(trace(5), [trace(50), trace(60)], trace(8))
            }
            state take(before: i32, values: [i32; 2], after: i32) {
                Output::write(before);
                Output::write(values[0]);
                Output::write(values[1]);
                Output::write(after);
            }
        }
        data Root {}
        machine Root::enter() reaches Output { relay(true); relay(false); }
    "#;
    assert_eq!(
        owned_array_effects(&crate::front_end::checked_program(source)),
        [7, 30, 40, 9, 7, 30, 40, 9, 5, 50, 60, 8, 5, 50, 60, 8],
    );
}

#[test]
fn owned_array_state_arguments_preserve_nested_array_values() {
    let source = owned_array_source(false)
        .replace("[i32; 2]", "[[i32; 2]; 2]")
        .replace("[30, 40]", "[[30, 40], [50, 60]]")
        .replace("Output::write(values[0]);", "Output::write(values[0][1]);")
        .replace("Output::write(values[1]);", "Output::write(values[1][0]);");
    assert_eq!(
        owned_array_effects(&crate::front_end::checked_program(&source)),
        [7, 40, 50, 9],
    );
}

#[test]
fn owned_array_state_terminal_replay_rejects_custody_and_late_producers() {
    use terminal_psi::{StructuralAccess, StructuralMultiplicity, Terminator};
    let checked = crate::front_end::checked_program(&owned_array_source(false));
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("owned array arrival lowers");
    validate_module(&lowered.semantic_module).expect("unaltered arrival validates");
    for mutation in ["borrowed access", "affine custody", "late producer"] {
        let mut changed = lowered.semantic_module.clone();
        let machine = changed
            .machines
            .iter_mut()
            .find(|machine| {
                machine.blocks.iter().any(|block| {
                    block.operations.iter().any(|operation| {
                        matches!(operation.kind, OperationKind::EstablishScalarArray { .. })
                    })
                })
            })
            .expect("array-producing machine");
        let source_index = machine
            .blocks
            .iter()
            .position(|block| {
                block.operations.iter().any(|operation| {
                    matches!(operation.kind, OperationKind::EstablishScalarArray { .. })
                })
            })
            .unwrap();
        let Terminator::Jump { target, .. } = machine.blocks[source_index].terminator else {
            panic!("array construction precedes its state edge");
        };
        let target_index = machine
            .blocks
            .iter()
            .position(|block| block.id == target)
            .unwrap();
        match mutation {
            "borrowed access" => {
                machine.blocks[target_index].structural_parameters[0].access =
                    StructuralAccess::SharedBorrow;
            }
            "affine custody" => {
                machine.blocks[target_index].structural_parameters[0].multiplicity =
                    StructuralMultiplicity::Affine;
            }
            "late producer" => {
                let operation_index = machine.blocks[source_index]
                    .operations
                    .iter()
                    .position(|operation| {
                        matches!(operation.kind, OperationKind::EstablishScalarArray { .. })
                    })
                    .unwrap();
                let operation = machine.blocks[source_index]
                    .operations
                    .remove(operation_index);
                machine.blocks[target_index].operations.insert(0, operation);
            }
            _ => unreachable!(),
        }
        assert!(validate_module(&changed).is_err(), "reject {mutation}");
    }
}

#[test]
fn owned_array_state_arguments_preserve_record_erased_declarations() {
    let source = r#"
        boundary trait Output { machine write(marker: i32) reaches Output; }
        data Payload { proof [erased]: i32; values: [i32; 2]; }
        data Root {}
        machine Root::enter() reaches Output {
            transition { _ -> take(Payload { proof: 0, values: [30, 40] }) }
            state take(payload: Payload) {
                Output::write(payload.values[0]);
                Output::write(payload.values[1]);
            }
        }
    "#;
    assert_eq!(
        owned_array_effects(&crate::front_end::checked_program(source)),
        [30, 40],
    );
}

#[test]
fn owned_array_state_contract_keeps_copied_view_record_custody() {
    let source = r#"
        boundary trait Output { machine write(marker: u64) reaches Output; }
        data Room [copy] { label: &[u8]; }
        data Root { room: Room; }
        machine Root::enter(context: &mut Root) reaches Output {
            context.room.label = "Gate";
            transition { _ -> take(context.room) }
            state take(room: Room) { Output::write(room.label.len); }
        }
    "#;
    let checked = crate::front_end::checked_program(source);
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .expect("a copied descriptor record retains owned carrier custody");
    validate_module(&lowered.semantic_module).expect("copied view record transfer validates");
}
