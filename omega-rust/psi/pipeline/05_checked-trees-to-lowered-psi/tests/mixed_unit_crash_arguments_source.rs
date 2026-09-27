//! Mixed Unit signatures keep authored crash parameters distinct from ABI positions.

use checked_trees_to_lowered_psi::TerminalMachineSelection;
use terminal_interpreter::TerminalStructuralInputs;
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecution,
    TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    TerminalStructuralBooleanFieldValue, TerminalStructuralValue,
};
use terminal_production::{TerminalProductionCustody, TerminalProductionTimings};

const SOURCE: &str = r#"
    data Flag { enabled: bool; }
    boundary trait Sink { machine record(before: bool, after: bool); }
    data Helper {}
    machine Helper::consume(before: bool, flag: Flag, after: bool)
    reaches Sink crashes Abort flag.enabled && after
    { Sink::record(before, after); }
    data Main {}
    machine Main::main(before: bool, flag: Flag, after: bool)
    crashes Abort flag.enabled && after
    { Helper::consume(before, flag, after); }
"#;

#[test]
fn compound_member_boolean_equality_keeps_mixed_unit_crash_namespaces() {
    for predicate in [
        "(flag.left && flag.right) == after",
        "after == (flag.left && flag.right)",
        "(flag.left && flag.right) != after",
        "after != (flag.left && flag.right)",
        "!(flag.left && flag.right) == after",
        "!(flag.left || flag.right) == after",
        "((flag.left && flag.right) == after) == before",
        "(flag.left && before) == (flag.right || after)",
        "(flag.left && flag.right) == (flag.left || flag.right)",
        "!((flag.left && before) == (flag.right || after))",
        "((flag.left && before) == (flag.right || after)) == true",
        "false == ((flag.left && before) != (flag.right || after))",
    ] {
        let source = compound_member_source(predicate);
        let lowered = roundtrip(&crate::front_end::checked_program(&source));
        let module = &lowered.semantic_module;
        let root = module
            .machines
            .iter()
            .find(|machine| machine.id == module.entry)
            .expect("exact entry");
        assert_eq!(root.parameters.len(), 2);
        assert_eq!(root.structural_parameters.len(), 1);
        let (arguments, structural_arguments, crash_continuations) = root
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find_map(|operation| match &operation.kind {
                terminal_psi::OperationKind::CallUnit {
                    arguments,
                    structural_arguments,
                    crash_continuations,
                    ..
                } => Some((arguments, structural_arguments, crash_continuations)),
                _ => None,
            })
            .expect("exact mixed call");
        assert_eq!(
            arguments.as_slice(),
            &[root.parameters[0].id, root.parameters[1].id]
        );
        assert_eq!(structural_arguments.len(), 1);
        assert_eq!(
            structural_arguments[0].place,
            root.structural_parameters[0].place
        );
        assert_eq!(
            crash_continuations, &root.contract.crash_routes,
            "{predicate}"
        );
    }
}

fn compound_member_source(predicate: &str) -> String {
    SOURCE
        .replace(
            "data Flag { enabled: bool; }",
            "data Flag { left: bool; right: bool; }",
        )
        .replace("flag.enabled && after", predicate)
}

#[test]
fn proposition_only_float_comparisons_compose_as_boolean_crash_operands() {
    for predicate in [
        "(flag.left == flag.right) == after",
        "before == (flag.left != flag.right)",
        "(flag.left != flag.right) != after",
    ] {
        let source = SOURCE
            .replace(
                "data Flag { enabled: bool; }",
                "data Flag { left: f64; right: f64; }",
            )
            .replace("flag.enabled && after", predicate);
        roundtrip(&crate::front_end::checked_program(&source));
    }
}

fn roundtrip(checked: &checked_trees::CheckedTrees) -> lowered_psi::LoweredPsi {
    let lowered = checked_trees_to_lowered_psi::lower_machine(
        checked,
        TerminalMachineSelection::Name("Main::main"),
    )
    .expect("mixed Unit crash predicate lowers through its authored signature");
    let semantic = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    let evidence =
        terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .unwrap();
    let module = terminal_codec::decode_module(&semantic).unwrap();
    let proof = terminal_codec::decode_proof_bundle(&evidence).unwrap();
    terminal_verifier::verify_module(
        &module,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("mixed Unit crash predicate verifies independently");
    let artifact = terminal_production::TerminalProductionRequest::new(
        checked,
        TerminalMachineSelection::Name("Main::main"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("mixed Unit crash predicate publishes")
    .into_artifact();
    assert_eq!(
        terminal_codec::decode_module(artifact.semantic_bytes()).unwrap(),
        module
    );
    assert_eq!(module, lowered.semantic_module);
    assert_eq!(proof, lowered.proof_bundle);
    lowered
}

fn reordered_source(layout: usize, reverse: bool, free: bool) -> String {
    let names = match layout {
        0 => ["left", "first", "right", "second"],
        1 => ["first", "left", "second", "right"],
        2 => ["first", "second", "left", "right"],
        3 => ["left", "right", "first", "second"],
        _ => unreachable!(),
    };
    let signature = names
        .iter()
        .map(|name| {
            let carrier = if matches!(*name, "left" | "right") {
                "Flag"
            } else {
                "bool"
            };
            format!("{name}: {carrier}")
        })
        .collect::<Vec<_>>()
        .join(", ");
    let arguments = names
        .iter()
        .map(|name| {
            if !reverse {
                return *name;
            }
            match *name {
                "left" => "right",
                "right" => "left",
                "first" => "second",
                "second" => "first",
                _ => unreachable!(),
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    let callee = if free { "consume" } else { "Helper::consume" };
    let root_predicate = if reverse {
        "right.enabled && first && (left.enabled == second)"
    } else {
        "left.enabled && second && (right.enabled == first)"
    };
    format!(
        r#"
        data Flag {{ enabled: bool; }}
        data Helper {{}}
        boundary trait Sink {{ machine record(first: bool, second: bool); }}
        machine {callee}({signature}) reaches Sink
        crashes Abort left.enabled && second && (right.enabled == first)
        {{ Sink::record(first, second); }}
        data Main {{}}
        machine Main::main(first: bool, left: Flag, second: bool, right: Flag)
        crashes Abort {root_predicate}
        {{ {callee}({arguments}); }}
    "#
    )
}

#[test]
fn compound_member_routes_reject_wrong_roots_and_foreign_formal_bindings() {
    let source = reordered_source(0, true, false)
        .replace(
            "left.enabled && second && (right.enabled == first)",
            "(left.enabled && second) == (right.enabled || first)",
        )
        .replace(
            "right.enabled && first && (left.enabled == second)",
            "(right.enabled && first) == (left.enabled || second)",
        );
    let lowered = roundtrip(&crate::front_end::checked_program(&source));
    for first in [false, true] {
        for second in [false, true] {
            for fields in [[false, true], [true, false]] {
                execute(&lowered, [first, second], fields, true);
            }
        }
    }
    assert_mixed_unit_route_bindings_reject_tampering(&source);
}

fn assert_mixed_unit_route_bindings_reject_tampering(source: &str) {
    let lowered = roundtrip(&crate::front_end::checked_program(source));
    let module = &lowered.semantic_module;
    let owner = module
        .machines
        .iter()
        .position(|machine| machine.id == module.entry)
        .unwrap();
    let (block_index, operation_index, target) = module.machines[owner]
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .operations
                .iter()
                .enumerate()
                .find_map(|(operation_index, operation)| match operation.kind {
                    terminal_psi::OperationKind::CallUnit { callee, .. } => {
                        Some((block_index, operation_index, callee))
                    }
                    _ => None,
                })
        })
        .unwrap();
    let operation = module.machines[owner].blocks[block_index].operations[operation_index].id;
    let foreign = module
        .machines
        .iter()
        .find(|machine| machine.id == target)
        .unwrap()
        .contract
        .crash_routes
        .clone();
    for mutation in 0..5 {
        let mut changed = module.clone();
        let terminal_psi::OperationKind::CallUnit {
            arguments,
            structural_arguments,
            crash_continuations,
            ..
        } = &mut changed.machines[owner].blocks[block_index].operations[operation_index].kind
        else {
            unreachable!();
        };
        match mutation {
            0 => crash_continuations.clear(),
            1 => *crash_continuations = foreign.clone(),
            2 => arguments.swap(0, 1),
            3 => structural_arguments.swap(0, 1),
            4 => changed.machines[owner].contract.crash_routes.clear(),
            _ => unreachable!(),
        }
        if mutation == 4 {
            // The uncovered continuation stays structurally valid; the
            // producer's own replay finds no certificate that discharges the
            // reconstructed question, so no roster is emitted.
            terminal_verifier::validate_module(&changed)
                .expect("an uncovered continuation is still a module");
            assert!(
                matches!(
                    checked_trees_to_lowered_psi::produce_crash_obligation_evidence(&changed),
                    Err(checked_trees_to_lowered_psi::CrashRosterError::Undischarged(owners))
                        if owners.iter().any(|owner| matches!(
                            owner,
                            terminal_psi::CrashObligationOwner::Continuation {
                                operation: row_operation,
                                cause: terminal_psi::CrashCause::Abort,
                                ..
                            } if *row_operation == operation
                        ))
                ),
                "mutation={mutation}"
            );
        } else {
            let expected = terminal_verifier::ModuleError::CallCrashContinuationsMismatch {
                operation,
                callee: target,
            };
            assert_eq!(
                terminal_verifier::validate_module(&changed).unwrap_err(),
                expected,
                "mutation={mutation}"
            );
        }
    }
}

#[derive(Default)]
struct Observe(Vec<Vec<TerminalScalarValue>>);

impl TerminalEffectHandler for Observe {
    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        let TerminalEffect::BoundaryCall { arguments, .. } = effect else {
            panic!("observable Sink effect");
        };
        self.0.push(arguments.clone());
        Ok(())
    }
}

fn execute(lowered: &lowered_psi::LoweredPsi, scalars: [bool; 2], flags: [bool; 2], reverse: bool) {
    let module = &lowered.semantic_module;
    let root = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let semantic = terminal_codec::encode_module(module).unwrap();
    let evidence =
        terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .unwrap();
    let arguments = root
        .structural_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| TerminalStructuralValue {
            opaque_identity: 100 + index as u64,
            structural_type: parameter.structural_type,
            qualifications: parameter.qualifications.clone(),
            path: Vec::new(),
        })
        .collect::<Vec<_>>();
    let fields = root
        .structural_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            let declaration = module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == parameter.structural_type)
                .unwrap();
            let terminal_psi::StructuralTypeShape::Record { fields } = &declaration.shape else {
                panic!("Flag record");
            };
            assert_eq!(fields.len(), 1);
            TerminalStructuralBooleanFieldValue {
                argument_index: index as u32,
                path: Vec::new(),
                field: fields[0].id,
                value: flags[index],
            }
        })
        .collect::<Vec<_>>();
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &evidence,
        &proof_admission::AdmissionProfile::default(),
        &scalars.map(TerminalScalarValue::Boolean),
        TerminalStructuralInputs {
            arguments: &arguments,
            boolean_fields: &fields,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(execution.live_affine_frontier().count(), 2);
    let mut observer = Observe::default();
    assert_eq!(
        execution
            .resume(
                &mut terminal_fuel::TerminalFuelMeter::unbounded(),
                &mut observer
            )
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(execution.live_affine_frontier().count(), 0);
    let expected = if reverse {
        [scalars[1], scalars[0]]
    } else {
        scalars
    };
    assert_eq!(
        observer.0,
        vec![expected.map(TerminalScalarValue::Boolean).to_vec()]
    );
}
