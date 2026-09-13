use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use terminal_codec::{encode_module, encode_proof_bundle};
use terminal_interpreter::{
    TerminalExecutionResult, TerminalScalarValue, interpret_terminal_artifact,
};
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees::statement::{StatementNode, TransitionTargetNode};
use typed_trees_to_checked_trees::lower_typed_trees;

#[derive(Clone, Copy)]
enum BranchForm {
    Separate,
    Combined,
    ExpressionTail,
}

const ALIGNMENT_GETTER: &str = "
    data Alignment [copy] { case Byte; case Word; case Dword; case Qword; }
    machine Alignment::width(&self) -> u64 [1..=8] {
        transition self {
            Alignment::Byte -> (1)
            Alignment::Word -> (2)
            Alignment::Dword -> (4)
            Alignment::Qword -> (8)
        }
    }
";

#[test]
fn stored_returned_cases_support_borrowed_refined_getters() {
    let checked = checked_source(
        r#"
        data MemoryAlignment [copy] {
            case Alignment1; case Alignment2; case Alignment4; case Alignment8;
        }
        machine MemoryAlignment::default() -> MemoryAlignment {
            MemoryAlignment::Alignment4
        }
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
        boundary trait Sink { machine record(value: u64); }
        data Main { sink: Sink; }
        machine Main::main(&mut self) reaches Sink {
            transition { _ -> alignment_conversion() }
            state alignment_conversion(&mut self) {
            let default_alignment: MemoryAlignment = MemoryAlignment::default();
            let fallback_alignment: MemoryAlignment = MemoryAlignment::from(3);
            let default_size: u64 = default_alignment.get_size_in_bytes();
            let fallback_size: u64 = fallback_alignment.get_size_in_bytes();
            transition default_size == 4 && fallback_size == 1 {
                true -> passed()
                false -> failed()
            }
            }
            state passed(&mut self) { self.sink.record(1); }
            state failed(&mut self) { self.sink.record(0); }
        }
        "#,
        BranchForm::Separate,
    );
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "Main::main")
        .produce_artifact()
        .expect("stored ordinary case results remain exact borrowed getter receivers");
    let receivers = checked
        .facts
        .values
        .scalar_computations
        .structural_arguments
        .iter()
        .filter_map(|(handle, argument)| {
            let argument = argument.as_place()?;
            matches!(
                argument.source,
                checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { .. }
            )
            .then_some((handle, argument.source.clone()))
        })
        .collect::<Vec<_>>();
    assert_eq!(
        receivers.len(),
        2,
        "both getters use the common computation place capture"
    );
    assert_ne!(receivers[0].1, receivers[1].1);
    for mutation in 0..2 {
        let mut changed = checked.clone();
        let argument = changed
            .facts
            .values
            .scalar_computations
            .structural_arguments
            .get_mut(receivers[0].0)
            .as_place_mut()
            .unwrap();
        if mutation == 0 {
            argument.source = receivers[1].1.clone();
        } else {
            argument.access = checked_trees::CheckedStructuralAccess::MutableBorrow;
        }
        assert!(
            terminal_production::TerminalProductionRequest::new(&changed, "Main::main")
                .produce_artifact()
                .is_err(),
            "stored receiver mutation {mutation} must fail exact source replay"
        );
    }
    let artifact =
        terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes()).unwrap();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    terminal_verifier::verify_module(
        &module,
        &terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap(),
        &AdmissionProfile::default(),
    )
    .unwrap();
    let entry = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let [receiver] = entry.structural_parameters.as_slice() else {
        panic!("one persistent Main receiver")
    };
    assert!(receiver.is_self);
    assert_eq!(
        receiver.access,
        terminal_psi::StructuralAccess::MutableBorrow
    );
    let receiver_type = receiver.structural_type;
    let mut missing_provider = module.clone();
    let entry = missing_provider
        .machines
        .iter_mut()
        .find(|machine| machine.id == module.entry)
        .unwrap();
    let prior_places = entry.structural_places.len();
    entry.structural_places.retain(|place| {
        !matches!(
            place.kind,
            semantic_vocabulary::StructuralPlaceKind::ProviderAttachment { .. }
        )
    });
    assert_eq!(
        prior_places - entry.structural_places.len(),
        1,
        "only the direct Sink boundary needs a provider root"
    );
    assert!(
        terminal_verifier::verify_module(
            &missing_provider,
            &terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap(),
            &AdmissionProfile::default(),
        )
        .is_err(),
        "ordinary constructor calls cannot replace the missing provider root"
    );
    drop(checked);
    for initial_fuel in [0, 2, 1000] {
        let mut execution =
            terminal_interpreter::TerminalExecution::start_artifact_with_structural_arguments(
                artifact.semantic_bytes(),
                artifact.proof_bytes(),
                &AdmissionProfile::default(),
                &[],
                &[terminal_interpreter::TerminalStructuralValue {
                    opaque_identity: 17,
                    structural_type: receiver_type,
                    qualifications: Vec::new(),
                    path: Vec::new(),
                }],
            )
            .unwrap();
        let mut fuel = terminal_fuel::TerminalFuelMeter::with_allowance(initial_fuel);
        let mut completed = false;
        for _ in 0..256 {
            match execution.resume(&mut fuel).unwrap() {
                terminal_interpreter::TerminalExecutionStatus::Complete(result) => {
                    assert_eq!(result, TerminalExecutionResult::Unit);
                    completed = true;
                    break;
                }
                terminal_interpreter::TerminalExecutionStatus::SponsorExhausted(_) => {
                    let effects = execution.effects().to_vec();
                    assert!(matches!(
                        execution.resume(&mut fuel).unwrap(),
                        terminal_interpreter::TerminalExecutionStatus::SponsorExhausted(_)
                    ));
                    assert_eq!(execution.effects(), effects);
                    fuel.replenish(1).unwrap();
                }
                other => panic!("unexpected execution {other:?}"),
            }
        }
        assert!(completed);
        let [terminal_interpreter::TerminalEffect::BoundaryCall { arguments, .. }] =
            execution.effects()
        else {
            panic!("exactly one selected Sink outcome")
        };
        assert_eq!(arguments, &[unsigned(64, 1)]);
    }
}

#[test]
fn borrowed_case_getter_executes_every_refined_return_from_encoded_evidence() {
    for (case, expected) in [("Byte", 1), ("Word", 2), ("Dword", 4), ("Qword", 8)] {
        let source = format!(
            "{ALIGNMENT_GETTER} machine value() -> u64 {{
                 let alignment: Alignment = Alignment::{case};
                 let width: u64 [1..=8] = alignment.width();
                 16u64 / width
             }}"
        );
        let checked = checked_source(&source, BranchForm::Separate);
        let artifact = terminal_production::TerminalProductionRequest::new(&checked, "value")
            .produce_artifact()
            .expect("borrowed case getter retains its refined result into caller division");
        let artifact =
            terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes()).unwrap();
        drop(checked);
        let mut without_range = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
        let getter = without_range
            .machines
            .iter_mut()
            .find(|machine| {
                machine.id != without_range.entry && machine.structural_parameters.len() == 1
            })
            .expect("borrowed getter");
        assert!(
            !getter.contract.ensures.is_empty(),
            "the declared nonzero range must reach the executable callee contract"
        );
        getter.contract.ensures.clear();
        let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
        assert!(
            terminal_verifier::verify_module(&without_range, &proof, &AdmissionProfile::default())
                .is_err(),
            "caller division cannot borrow a source-only range after callee guarantees disappear"
        );
        assert_eq!(
            interpret_terminal_artifact(
                artifact.semantic_bytes(),
                artifact.proof_bytes(),
                &AdmissionProfile::default(),
                &[],
            )
            .unwrap_or_else(|error| panic!("{case}: {error:#?}")),
            TerminalExecutionResult::Scalar(unsigned(64, 16 / expected)),
        );
    }
}

#[test]
fn ordered_scalar_returns_execute_the_authored_fallback() {
    let checked = checked_source(
        "machine value(input: u64) -> u64 { transition input { 0 -> (1) 1 -> (2) _ -> (3) } }",
        BranchForm::Separate,
    );
    let artifact = terminal_production::TerminalProductionRequest::new(&checked, "value")
        .produce_artifact()
        .expect("ordered scalar guards with explicit fallback");
    drop(checked);
    for (input, expected) in [(0, 1), (1, 2), (2, 3), (u64::MAX, 3)] {
        assert_eq!(
            interpret_terminal_artifact(
                artifact.semantic_bytes(),
                artifact.proof_bytes(),
                &AdmissionProfile::default(),
                &[unsigned(64, u128::from(input))]
            )
            .unwrap(),
            TerminalExecutionResult::Scalar(unsigned(64, expected))
        );
    }
}

#[test]
fn guarded_case_replay_rejects_changed_order_guards_and_final_destination() {
    let checked = checked_source(ALIGNMENT_GETTER, BranchForm::Separate);
    let artifact =
        terminal_production::TerminalProductionRequest::new(&checked, "Alignment::width")
            .produce_artifact()
            .expect("complete borrowed case getter before hostile plan edits");
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    for (original, outside) in [(1, 0), (8, 9)] {
        let mut invalid = module.clone();
        let getter = invalid
            .machines
            .iter_mut()
            .find(|machine| machine.id == invalid.entry)
            .unwrap();
        let returned = getter.blocks.iter_mut().flat_map(|block| &mut block.operations)
            .find(|operation| matches!(operation.kind,
                terminal_psi::OperationKind::IntegerConstant { value: IntegerValue::Unsigned(value) }
                if value == original)).expect("declared endpoint return");
        returned.kind = terminal_psi::OperationKind::IntegerConstant {
            value: IntegerValue::Unsigned(outside),
        };
        assert!(
            terminal_verifier::verify_module(&invalid, &proof, &AdmissionProfile::default())
                .is_err(),
            "a returned value outside the declared range must not retain its proof"
        );
    }
    let getter = checked.machines()[0].symbol;
    let control = checked
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(getter)
        .unwrap()
        .scalar_control
        .as_ref()
        .unwrap();
    let checked_trees::CheckedScalarStateTerminator::Guarded { arms, fallback } =
        &control.terminator
    else {
        panic!("all authored guards remain explicit");
    };
    assert!(fallback.is_none());
    assert_eq!(arms.count(), 4);
    for mutation in 0..3 {
        let mut invalid = checked.clone();
        let rows = invalid
            .facts
            .flow
            .terminal_scalar_graphs
            .guarded_exits
            .span_mut(*arms)
            .unwrap();
        match mutation {
            0 => rows.swap(0, 1),
            1 => rows[1].guard_statement_ordinal = rows[0].guard_statement_ordinal,
            2 => rows[3].destination = rows[0].destination.clone(),
            _ => unreachable!(),
        }
        assert!(
            terminal_production::TerminalProductionRequest::new(&invalid, "Alignment::width")
                .produce_artifact()
                .is_err(),
            "changed ordered-exit custody {mutation} must reject"
        );
    }
}

fn checked_source(source: &str, form: BranchForm) -> checked_trees::CheckedTrees {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let mut typed = lower_symbol_resolved_trees(&resolved).expect("type");
    if !matches!(form, BranchForm::Separate) {
        let machine = typed.machines()[0].clone();
        let nodes = typed.machine_states(&machine)[0].statement_nodes;
        let transitions: Vec<_> = typed
            .statement_table
            .statements(nodes)
            .iter()
            .enumerate()
            .filter_map(|(index, statement)| match statement {
                StatementNode::Transition(transition) => Some((index, transition.target)),
                _ => None,
            })
            .collect();
        let [(first, _), (second, continuation)] = transitions.as_slice() else {
            panic!("two authored branch statements")
        };
        assert_eq!(*first + 1, *second);
        assert_eq!(*second + 1, nodes.count() as usize);
        if matches!(form, BranchForm::ExpressionTail) {
            let TransitionTargetNode::Value(expression) =
                typed.statement_table.transition_target(*continuation)
            else {
                panic!("tail value")
            };
            let expression = *expression;
            typed.statement_table.statements_mut(nodes)[*second] =
                StatementNode::Expression(expression);
        } else {
            let StatementNode::Transition(transition) =
                &mut typed.statement_table.statements_mut(nodes)[*first]
            else {
                unreachable!()
            };
            transition.continuation = *continuation;
            typed.machine_states_mut(&machine)[0].statement_nodes =
                arena::HandleSpan::from_parts(nodes.start(), nodes.count() - 1);
        }
    }
    lower_typed_trees(typed).unwrap_or_else(|diagnostics| panic!("{source}: {diagnostics:#?}"))
}

fn encoded(source: &str, form: BranchForm) -> (Vec<u8>, Vec<u8>) {
    let checked = checked_source(source, form);
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "value")
        .unwrap_or_else(|error| panic!("{source}: {error:#?}"));
    (
        encode_module(&lowered.semantic_module).expect("encode semantics"),
        encode_proof_bundle(&lowered.proof_bundle).expect("encode proof"),
    )
}

fn unsigned(width: u16, value: u128) -> TerminalScalarValue {
    TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, width).unwrap(),
        value: IntegerValue::Unsigned(value),
    }
}

fn assert_both_branches(source: &str, expected: TerminalScalarValue, form: BranchForm) {
    let (semantics, proof) = encoded(source, form);
    // Only artifact bytes survive from the producer; both choices execute
    // against the same decoded semantics and independently supplied inputs.
    for flag in [true, false] {
        assert_eq!(
            interpret_terminal_artifact(
                &semantics,
                &proof,
                &AdmissionProfile::default(),
                &[TerminalScalarValue::Boolean(flag)],
            )
            .unwrap_or_else(|error| panic!("{source}, flag {flag}: {error:#?}")),
            TerminalExecutionResult::Scalar(expected),
        );
    }
}

#[test]
fn guarded_anonymous_integer_returns_land_once_after_selected_evaluation() {
    for (scalar_type, expression, width, expected) in [
        ("u8", "300 - 293", 8, 7),
        ("u8", "(0 - 1) + 8", 8, 7),
        (
            "u64",
            "(18446744073709551615 + 1) - 1",
            64,
            u128::from(u64::MAX),
        ),
    ] {
        let source = format!(
            "machine value(flag: bool) -> {scalar_type}\nrequires {expected}{scalar_type} == {expected}{scalar_type}\nensures {expected}{scalar_type} == {expected}{scalar_type}\n{{ transition flag {{ true -> ({expression}) false -> {expected} }} }}"
        );
        for form in [BranchForm::Separate, BranchForm::Combined] {
            assert_both_branches(&source, unsigned(width, expected), form);
        }
    }
}

#[test]
fn guarded_returns_remap_saved_values_and_current_storage_in_both_arm_forms() {
    let source = "machine value(flag: bool) -> u8\nrequires 7u8 == 7u8\nensures 7u8 == 7u8\n{ let mut current: u8 = 7; let saved: u8 = current; current = 8; transition flag { true -> saved false -> (current - 1) } }";
    for form in [BranchForm::Separate, BranchForm::Combined] {
        assert_both_branches(source, unsigned(8, 7), form);
    }
}

#[test]
fn guarded_returns_share_control_with_named_state_successors() {
    for arms in [
        "true -> saved false -> finish(current - 1)",
        "true -> finish(current - 1) false -> saved",
    ] {
        let source = format!(
            "machine value(flag: bool) -> u8\nrequires 7u8 == 7u8\nensures 7u8 == 7u8\n{{ let mut current: u8 = 7; let saved: u8 = current; current = 8; transition flag {{ {arms} }} state finish(input: u8) -> u8 {{ input }} }}"
        );
        for form in [BranchForm::Separate, BranchForm::Combined] {
            assert_both_branches(&source, unsigned(8, 7), form);
        }
    }
}

#[test]
fn guarded_return_and_expression_tail_keep_distinct_selected_values() {
    let source = "machine value(flag: bool) -> u8\nrequires 7u8 == 7u8\nensures 7u8 == 7u8\n{ let mut current: u8 = 7; let saved: u8 = current; current = 8; transition flag { true -> saved false -> (current - 1) } }";
    assert_both_branches(source, unsigned(8, 7), BranchForm::ExpressionTail);
}

#[test]
fn guarded_boolean_returns_preserve_short_circuit_and_saved_storage() {
    let source = "machine value(flag: bool) -> bool\nrequires true == true\nensures true == true\n{ let mut current: bool = false; let saved: bool = current; current = true; transition flag { true -> (current && !saved) false -> (!saved || current) } }";
    for form in [BranchForm::Separate, BranchForm::Combined] {
        assert_both_branches(source, TerminalScalarValue::Boolean(true), form);
    }
}

#[test]
fn unselected_partial_arithmetic_does_not_execute() {
    let source = "machine value(denominator: u8) -> u8\nrequires 7u8 == 7u8\nensures 7u8 == 7u8\n{ transition (1 <= denominator) { true -> (7u8 / denominator) false -> 7 } }";
    for form in [BranchForm::Separate, BranchForm::Combined] {
        let (semantics, proof) = encoded(source, form);
        for (denominator, expected) in [(0, 7), (1, 7), (2, 3), (7, 1), (255, 0)] {
            assert_eq!(
                interpret_terminal_artifact(
                    &semantics,
                    &proof,
                    &AdmissionProfile::default(),
                    &[unsigned(8, denominator)]
                )
                .unwrap_or_else(|error| panic!("denominator {denominator}: {error:#?}")),
                TerminalExecutionResult::Scalar(unsigned(8, expected)),
            );
        }
    }
}

#[test]
fn unsigned_division_retains_guard_polarity_through_serialization() {
    for (condition, division_when_true) in [
        ("denominator == 0", false),
        ("0 == denominator", false),
        ("denominator <= 0", false),
        ("0 >= denominator", false),
        ("!(denominator != 0)", false),
        ("denominator != 0", true),
        ("!(denominator == 0)", true),
        ("0 < denominator", true),
        ("denominator >= 1", true),
    ] {
        let division = "7u8 / denominator";
        let (positive, negative) = if division_when_true {
            (division, "7")
        } else {
            ("7", division)
        };
        let source = format!(
            "machine value(denominator: u8) -> u8\nrequires 7u8 == 7u8\nensures 7u8 == 7u8\n{{ transition ({condition}) {{ true -> ({positive}) false -> ({negative}) }} }}"
        );
        for form in [BranchForm::Separate, BranchForm::Combined] {
            let (semantics, proof) = encoded(&source, form);
            for (denominator, expected) in [(0, 7), (1, 7), (2, 3), (7, 1), (255, 0)] {
                assert_eq!(
                    interpret_terminal_artifact(
                        &semantics,
                        &proof,
                        &AdmissionProfile::default(),
                        &[unsigned(8, denominator)],
                    )
                    .unwrap_or_else(|error| panic!(
                        "{source}, denominator {denominator}: {error:#?}"
                    )),
                    TerminalExecutionResult::Scalar(unsigned(8, expected)),
                );
            }
        }
    }
}

#[test]
fn signed_division_retains_both_nonzero_signs_on_the_selected_edge() {
    let signed = |value| TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Signed, 8).unwrap(),
        value: IntegerValue::Signed(value),
    };
    for (condition, division_when_true) in [
        ("denominator == 0", false),
        ("0 == denominator", false),
        ("!(denominator != 0)", false),
        ("denominator != 0", true),
        ("!(denominator == 0)", true),
    ] {
        let division = "7i8 / denominator";
        let (positive, negative) = if division_when_true {
            (division, "7")
        } else {
            ("7", division)
        };
        let source = format!(
            "machine value(denominator: i8) -> i8\nrequires 7i8 == 7i8\nensures 7i8 == 7i8\n{{ transition ({condition}) {{ true -> ({positive}) false -> ({negative}) }} }}"
        );
        for form in [BranchForm::Separate, BranchForm::Combined] {
            let (semantics, proof) = encoded(&source, form);
            for (denominator, expected) in [
                (-128, 0),
                (-7, -1),
                (-2, -3),
                (-1, -7),
                (0, 7),
                (1, 7),
                (2, 3),
                (127, 0),
            ] {
                assert_eq!(
                    interpret_terminal_artifact(
                        &semantics,
                        &proof,
                        &AdmissionProfile::default(),
                        &[signed(denominator)],
                    )
                    .unwrap_or_else(|error| panic!(
                        "{source}, denominator {denominator}: {error:#?}"
                    )),
                    TerminalExecutionResult::Scalar(signed(expected)),
                );
            }
        }
    }
}

#[test]
fn a_nonzero_guard_does_not_license_signed_division_overflow() {
    let source = "machine value(denominator: i8) -> i8\nrequires 0i8 == 0i8\nensures 0i8 == 0i8\n{ transition (denominator == 0) { true -> 0 false -> (-128i8 / denominator) } }";
    for form in [BranchForm::Separate, BranchForm::Combined] {
        let checked = checked_source(source, form);
        let result = checked_trees_to_lowered_psi::lower_machine(&checked, "value");
        assert!(
            matches!(
                result,
                Err(checked_trees_to_lowered_psi::LoweringError::OperationProofUnavailable(_))
            ),
            "the nonzero divisor may still be -1: {result:?}",
        );
    }
}

#[test]
fn branch_return_coordinates_cannot_select_a_siblings_valid_value() {
    use checked_trees::{CheckedScalarBranchDestination, CheckedScalarStateTerminator};
    let source = "machine value(flag: bool) -> u8\nrequires 7u8 == 7u8\nensures 7u8 == 7u8\n{ transition flag { true -> 7 false -> 7 } }";
    for form in [BranchForm::Separate, BranchForm::Combined] {
        let checked = checked_source(source, form);
        checked_trees_to_lowered_psi::lower_machine(&checked, "value")
            .expect("original branch lowers");
        for mutation in 0..3 {
            let mut changed = checked.clone();
            let CheckedScalarStateTerminator::Conditional {
                when_true,
                when_false,
                ..
            } = &mut changed.facts.flow.terminal_scalar_graphs.machines[0].states[0].terminator
            else {
                panic!("conditional plan")
            };
            let chosen = if mutation == 2 { when_false } else { when_true };
            let CheckedScalarBranchDestination::Return {
                statement_ordinal,
                is_continuation,
            } = chosen
            else {
                panic!("return branch")
            };
            match mutation {
                0 | 2 => *is_continuation = !*is_continuation,
                _ => *statement_ordinal += 1,
            }
            assert!(
                checked_trees_to_lowered_psi::lower_machine(&changed, "value").is_err(),
                "mutation {mutation}"
            );
        }
    }
}
