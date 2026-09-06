//! Mixed Unit signatures keep authored crash parameters distinct from ABI positions.

use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecution,
    TerminalExecutionResult, TerminalExecutionStatus, TerminalScalarValue,
    TerminalStructuralBooleanFieldValue, TerminalStructuralValue,
};
use tokens_to_syntax_trees::parse_syntax_trees;

fn checked(source: &str) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    typed_trees_to_checked_trees::lower_typed_trees(typed)
        .unwrap_or_else(|errors| panic!("{source}: {errors:#?}"))
}

const SOURCE: &str = r#"
    data Flag { enabled: bool; }
    boundary trait Sink { machine record(before: bool, after: bool); }
    data Helper {}
    machine Helper::consume(before: bool, flag: Flag, after: bool)
    crashes Abort flag.enabled && after
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
        let lowered = roundtrip(&checked(&source));
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
        roundtrip(&checked(&source));
    }
}

#[test]
fn ordinary_unit_crash_guard_combines_interleaved_scalar_and_structural_parameters() {
    let checked = checked(SOURCE);
    roundtrip(&checked);
}

fn roundtrip(checked: &checked_trees::CheckedTrees) -> lowered_psi::LoweredPsi {
    let lowered = checked_trees_to_lowered_psi::lower_machine(checked, "Main::main")
        .expect("mixed Unit crash predicate lowers through its authored signature");
    let semantic = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    let evidence = terminal_codec::encode_proof_bundle(&lowered.proof_bundle).unwrap();
    let module = terminal_codec::decode_module(&semantic).unwrap();
    let proof = terminal_codec::decode_proof_bundle(&evidence).unwrap();
    terminal_verifier::verify_module(
        &module,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("mixed Unit crash predicate verifies independently");
    let artifact = terminal_production::produce_terminal_artifact(checked, "Main::main")
        .expect("mixed Unit crash predicate publishes");
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
        machine {callee}({signature})
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
fn mixed_unit_calls_preserve_authored_positions_and_reordered_actuals() {
    for layout in 0..4 {
        for reverse in [false, true] {
            for free in [false, true] {
                let source = reordered_source(layout, reverse, free);
                let lowered = roundtrip(&checked(&source));
                let module = &lowered.semantic_module;
                let root = module
                    .machines
                    .iter()
                    .find(|machine| machine.id == module.entry)
                    .unwrap();
                assert_eq!(root.parameters.len(), 2);
                assert_eq!(root.structural_parameters.len(), 2);
                let (callee, scalar_arguments, structural_arguments) = root
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .find_map(|operation| match &operation.kind {
                        terminal_psi::OperationKind::CallUnit {
                            callee,
                            arguments,
                            structural_arguments,
                            ..
                        } => Some((*callee, arguments, structural_arguments)),
                        _ => None,
                    })
                    .expect("ordinary mixed call");
                assert_eq!(
                    scalar_arguments.as_slice(),
                    if reverse {
                        [root.parameters[1].id, root.parameters[0].id]
                    } else {
                        [root.parameters[0].id, root.parameters[1].id]
                    }
                );
                let callee = module
                    .machines
                    .iter()
                    .find(|machine| machine.id == callee)
                    .unwrap();
                assert_eq!(callee.parameters.len(), 2);
                assert_eq!(callee.structural_parameters.len(), 2);
                assert_eq!(callee.attachment.is_none(), free);
                let expected_places = if reverse {
                    [
                        root.structural_parameters[1].place,
                        root.structural_parameters[0].place,
                    ]
                } else {
                    [
                        root.structural_parameters[0].place,
                        root.structural_parameters[1].place,
                    ]
                };
                assert_eq!(
                    structural_arguments
                        .iter()
                        .map(|argument| argument.place)
                        .collect::<Vec<_>>(),
                    expected_places
                );
                for first in [false, true] {
                    for second in [false, true] {
                        for flags in [[false, true], [true, false]] {
                            execute(&lowered, [first, second], flags, reverse);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn mixed_unit_routes_reject_wrong_scalar_place_and_foreign_formal_bindings() {
    assert_mixed_unit_route_bindings_reject_tampering(&reordered_source(0, true, false));
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
    let lowered = roundtrip(&checked(&source));
    for first in [false, true] {
        for second in [false, true] {
            for fields in [[false, true], [true, false]] {
                execute(&lowered, [first, second], fields, true);
            }
        }
    }
    assert_mixed_unit_route_bindings_reject_tampering(&source);
}

#[test]
fn compound_member_route_field_identity_survives_artifact_encoding() {
    let source = compound_member_source("(flag.left && before) == (flag.right || after)");
    let lowered = roundtrip(&checked(&source));
    let mut module = lowered.semantic_module.clone();
    let root = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let structural_type = root.structural_parameters[0].structural_type;
    let declaration = module
        .structural_types
        .iter_mut()
        .find(|declaration| declaration.id == structural_type)
        .unwrap();
    let terminal_psi::StructuralTypeShape::Record { fields } = &mut declaration.shape else {
        panic!("exact Flag record");
    };
    fields.last_mut().expect("Flag fields").id =
        semantic_vocabulary::StructuralFieldId::new(u64::MAX).unwrap();
    assert!(
        matches!(
            terminal_codec::encode_module(&module),
            Err(terminal_codec::CodecError::InvalidModule(
                terminal_verifier::ModuleError::InvalidBooleanFieldTerm { .. }
            ))
        ),
        "independent codec validation rejects the stale compound field identity before artifact publication"
    );
}

fn assert_mixed_unit_route_bindings_reject_tampering(source: &str) {
    let lowered = roundtrip(&checked(source));
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
        let expected = if mutation == 4 {
            terminal_verifier::ModuleError::CallCrashContinuationUncovered {
                operation,
                cause: terminal_psi::CrashCause::Abort,
            }
        } else {
            terminal_verifier::ModuleError::CallCrashContinuationsMismatch {
                operation,
                callee: target,
            }
        };
        assert_eq!(
            terminal_verifier::validate_module(&changed).unwrap_err(),
            expected,
            "mutation={mutation}"
        );
    }
}

#[test]
fn mixed_integer_comparisons_rebase_fields_and_reversed_scalar_parameters() {
    for free in [false, true] {
        let target = if free { "consume" } else { "Helper::consume" };
        let source = format!(
            r#"
            data Meter {{ value: u16; }}
            data Helper {{}}
            boundary trait Sink {{ machine record(first: u16, last: u16); }}
            machine {target}(first: u16, meter: Meter, last: u16)
            crashes Abort meter.value == last && first < last
            {{ Sink::record(first, last); }}
            data Main {{}}
            machine Main::main(first: u16, meter: Meter, last: u16)
            crashes Abort meter.value == first && last < first
            {{ {target}(last, meter, first); }}
        "#
        );
        roundtrip(&checked(&source));
    }
}

#[test]
fn mixed_structural_scalar_crash_divisors_still_require_totality() {
    for predicate in [
        "metrics.current / divisor <= limit",
        "numerator / metrics.divisor <= limit",
    ] {
        let source = format!(
            r#"
            data Metrics {{ current: u64; divisor: u64; }}
            data Main {{}}
            machine Main::main(numerator: u64, metrics: Metrics, divisor: u64, limit: u64)
            crashes Abort {predicate}
            {{}}
        "#
        );
        let tokens = Lexer::new(&source).tokenize().unwrap();
        let syntax = parse_syntax_trees(&tokens).unwrap();
        let resolved = lower_syntax_trees(&syntax).unwrap();
        let typed = lower_symbol_resolved_trees(&resolved).unwrap();
        let diagnostics = typed_trees_to_checked_trees::lower_typed_trees(typed)
            .expect_err("a mixed signature does not make an unproven divisor total");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("divisor must be proven nonzero")),
            "{source}: {diagnostics:?}"
        );
    }
}

#[test]
fn mixed_member_integer_boolean_cleanup_remains_outside_the_supported_source_shape() {
    let checked = checked(
        r#"
        data Token { observed: bool; other: bool; }
        machine Token::drop(&mut self) {}
        data Main {}
        machine Main::main(token: Token, input: u64, enabled: bool) -> bool {
            let staged: bool = token.observed && ((input < 1u64) || enabled);
            staged
        }
    "#,
    );
    let root = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .unwrap();
    assert!(
        !checked
            .facts
            .flow
            .terminal_structural_scalar_returns
            .machines
            .iter()
            .any(|plan| plan.machine == root.symbol),
        "the nominal-cleanup consumer does not admit mixed member/comparison bodies"
    );
    assert!(
        matches!(
            checked_trees_to_lowered_psi::lower_machine(&checked, "Main::main"),
            Err(checked_trees_to_lowered_psi::LoweringError::Unsupported(
                "attached Unit closure is missing a checked transitive machine plan"
            ))
        ),
        "unsupported source shape rejects before Terminal construction"
    );
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
    let evidence = terminal_codec::encode_proof_bundle(&lowered.proof_bundle).unwrap();
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
    let mut execution =
        TerminalExecution::start_artifact_with_structural_arguments_and_boolean_fields(
            &semantic,
            &evidence,
            &proof_admission::AdmissionProfile::default(),
            &scalars.map(TerminalScalarValue::Boolean),
            &arguments,
            &fields,
        )
        .unwrap();
    assert_eq!(execution.live_affine_frontier().count(), 2);
    let mut observer = Observe::default();
    assert_eq!(
        execution
            .resume_with_effect_handler(
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
