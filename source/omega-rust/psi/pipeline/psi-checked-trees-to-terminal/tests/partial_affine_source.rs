use psi_proof_admission::AdmissionProfile;
use psi_source_files_to_tokens::Lexer;
use psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
use psi_terminal::{OperationKind, StructuralFieldType, StructuralPathSegment, Terminator};
use psi_terminal_codec::{decode_module, encode_module, encode_proof_bundle};
use psi_terminal_fuel::{FuelChargeSite, FuelExhaustion, TerminalFuelMeter, TerminalFuelSchedule};
use psi_terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalStructuralValue,
};
use psi_tokens_to_syntax_trees::parse_syntax_trees;
use psi_typed_trees_to_checked_trees::lower_typed_trees;

const SOURCE: &str = r#"
    domain [u8; 3]::Utf8
    requires
        valid_utf8(self);
    domain [u8; 8]::Utf8
    requires
        valid_utf8(self);
    data Token { value: u64; }
    data Quintet {
        before: u8;
        before_bytes: [u8; 3] in Utf8;
        before_float: f32;
        first: Token;
        between: bool;
        between_bytes: [u8; 8] in Utf8;
        between_float: f64;
        second: Token;
        third: Token;
        fourth: Token;
        fifth: Token;
        after: u64;
    }

    data Sink {}
    machine Sink::take(token: Token) {}

    data Root {}
    machine Root::enter(value: Quintet) {
        Sink::take(value.third);
        Sink::take(value.first);
    }
"#;

const MIXED_SOURCE: &str = r#"
    data Token { value: u64; }
    data Deep { low: Token; middle: Token; high: Token; }
    data Branch { head: Token; deep: Deep; tail: Token; }
    data Outer { first: Token; left: Branch; right: Branch; last: Token; }

    data Sink {}
    machine Sink::take(token: Token) {}

    data Root {}
    machine Root::enter(value: Outer) {
        Sink::take(value.left.deep.middle);
        Sink::take(value.right.tail);
        Sink::take(value.first);
    }
"#;

const AFFINE_PAIR_SOURCE: &str = r#"
    data Token { value: u64; }
    data Sink {}
    machine Sink::take(token: Token) {}
    data Root {}
    machine Root::first(values: [Token; 2]) {
        Sink::take(values[0]);
    }
    machine Root::second(values: [Token; 2]) {
        Sink::take(values[1]);
    }
    machine Root::forward(values: [Token; 2]) {
        Sink::take(values[0]);
        Sink::take(values[1]);
    }
    machine Root::reverse(values: [Token; 2]) {
        Sink::take(values[1]);
        Sink::take(values[0]);
    }
"#;

const AFFINE_TRIPLE_SOURCE: &str = r#"
    data Token { value: u64; }
    data Sink {}
    machine Sink::take(token: Token) {}
    data Root {}
    machine Root::middle(values: [Token; 3]) {
        Sink::take(values[2]);
        Sink::take(values[0]);
    }
    machine Root::last(values: [Token; 3]) {
        Sink::take(values[0]);
        Sink::take(values[1]);
    }
    machine Root::first(values: [Token; 3]) {
        Sink::take(values[1]);
        Sink::take(values[2]);
    }
    machine Root::one(values: [Token; 3]) {
        Sink::take(values[0]);
    }
    machine Root::all(values: [Token; 3]) {
        Sink::take(values[0]);
        Sink::take(values[1]);
        Sink::take(values[2]);
    }
"#;

const AFFINE_QUARTET_SOURCE: &str = r#"
    data Token { value: u64; }
    data Sink {}
    machine Sink::take(token: Token) {}
    data Root {}
    machine Root::enter(values: [Token; 4]) {
        Sink::take(values[1]);
        Sink::take(values[3]);
    }
"#;

const NESTED_AFFINE_ARRAY_SOURCE: &str = r#"
    data Token { value: u64; }
    data Sink {}
    machine Sink::take(token: Token) {}
    data Root {}
    machine Root::enter(values: [[Token; 3]; 2]) {
        Sink::take(values[1][0]);
        Sink::take(values[0][1]);
    }
"#;

const NESTED_AFFINE_QUARTET_SOURCE: &str = r#"
    data Token { value: u64; }
    data Sink {}
    machine Sink::take(token: Token) {}
    data Root {}
    machine Root::enter(values: [[Token; 4]; 2]) {
        Sink::take(values[1][3]);
        Sink::take(values[0][1]);
    }
"#;

const NESTED_AFFINE_QUINTET_SOURCE: &str = r#"
    data Token { value: u64; }
    data Sink {}
    machine Sink::take(token: Token) {}
    data Root {}
    machine Root::enter(values: [[Token; 5]; 2]) {
        Sink::take(values[1][4]);
        Sink::take(values[0][1]);
    }
"#;

const NESTED_AFFINE_SEXTET_SOURCE: &str = r#"
    data Token { value: u64; }
    data Sink {}
    machine Sink::take(token: Token) {}
    data Root {}
    machine Root::enter(values: [[Token; 6]; 2]) {
        Sink::take(values[1][5]);
        Sink::take(values[0][1]);
    }
"#;

#[test]
fn two_element_affine_array_cleanup_crosses_source_codec_verifier_and_interpreter() {
    let tokens = Lexer::new(AFFINE_PAIR_SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");

    for (machine, moved, residual) in [("Root::first", 0, 1), ("Root::second", 1, 0)] {
        let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, machine)
            .expect("one literal array move and its exact sibling cleanup lower");
        let entry = lowered
            .semantic_module
            .machines
            .iter()
            .find(|candidate| candidate.id == lowered.semantic_module.entry)
            .expect("entry machine");
        let [root] = entry.structural_parameters.as_slice() else {
            panic!("affine pair slice has one structural root")
        };
        let root_shape = lowered
            .semantic_module
            .structural_types
            .iter()
            .find(|shape| shape.id == root.structural_type)
            .expect("array root shape");
        let psi_terminal::StructuralTypeShape::FixedArray {
            element: element_type,
            length,
        } = root_shape.shape
        else {
            panic!("root remains a fixed array")
        };
        assert_eq!(length, 2);
        let [block] = entry.blocks.as_slice() else {
            panic!("affine pair slice has one block")
        };
        let [call] = block.operations.as_slice() else {
            panic!("affine pair slice has one call")
        };
        let OperationKind::CallUnit {
            structural_arguments,
            claim_transfers,
            ..
        } = &call.kind
        else {
            panic!("affine pair performs an ordinary Unit call")
        };
        assert!(claim_transfers.is_empty());
        let [argument] = structural_arguments.as_slice() else {
            panic!("ordinary call has one projected argument")
        };
        assert_eq!(argument.place, root.place);
        assert_eq!(argument.path, [StructuralPathSegment::FixedIndex(moved)]);
        let Terminator::ReturnUnitPartialAffine {
            edge,
            trivial_affine_discards,
            residual_affine_discards,
        } = &block.terminator
        else {
            panic!("affine pair returns through partial cleanup")
        };
        assert!(trivial_affine_discards.is_empty());
        let [discard] = residual_affine_discards.as_slice() else {
            panic!("affine pair has one residual")
        };
        assert_eq!(discard.place, root.place);
        assert_eq!(discard.structural_type, element_type);
        assert_eq!(discard.path, [StructuralPathSegment::FixedIndex(residual)]);

        psi_terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .expect("verifier reconstructs the opposite array element");
        let semantic = encode_module(&lowered.semantic_module).expect("module encodes");
        assert_eq!(
            decode_module(&semantic).expect("module decodes"),
            lowered.semantic_module
        );
        let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encodes");
        let argument_value = TerminalStructuralValue {
            opaque_identity: 0x4152_5241,
            structural_type: root.structural_type,
            qualifications: root.qualifications.clone(),
            path: Vec::new(),
        };
        let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[],
            &[argument_value],
        )
        .expect("verified affine pair artifact starts");
        let mut meter = TerminalFuelMeter::with_allowance(2);
        assert_eq!(
            execution.resume(&mut meter).expect("execution suspends"),
            TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
                schedule: TerminalFuelSchedule::CURRENT.identity(),
                site: FuelChargeSite::Edge(*edge),
                required_units: 1,
                remaining_units: 0,
            })
        );
        assert_eq!(execution.live_affine_frontier().count(), 1);
        meter.replenish(1).expect("replenish cleanup edge");
        assert_eq!(
            execution.resume(&mut meter).expect("execution completes"),
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
        );
        assert!(execution.live_affine_frontier().next().is_none());
        assert_eq!(meter.usage().total_units(), 3);

        let mut wrong_length = lowered.semantic_module.clone();
        let declaration = wrong_length
            .structural_types
            .iter_mut()
            .find(|shape| shape.id == root.structural_type)
            .expect("array declaration");
        let psi_terminal::StructuralTypeShape::FixedArray { length, .. } = &mut declaration.shape
        else {
            unreachable!()
        };
        *length = 3;
        assert!(
            psi_terminal_verifier::verify_module(
                &wrong_length,
                &lowered.proof_bundle,
                &AdmissionProfile::default(),
            )
            .is_err()
        );

        let mut duplicate_path = lowered.semantic_module.clone();
        let entry = duplicate_path
            .machines
            .iter_mut()
            .find(|candidate| candidate.id == duplicate_path.entry)
            .expect("entry machine");
        let [block] = entry.blocks.as_mut_slice() else {
            unreachable!()
        };
        let Terminator::ReturnUnitPartialAffine {
            residual_affine_discards,
            ..
        } = &mut block.terminator
        else {
            unreachable!()
        };
        residual_affine_discards[0].path = vec![StructuralPathSegment::FixedIndex(moved)];
        assert!(
            psi_terminal_verifier::verify_module(
                &duplicate_path,
                &lowered.proof_bundle,
                &AdmissionProfile::default(),
            )
            .is_err()
        );
    }
}

#[test]
fn fully_consumed_affine_array_uses_two_calls_and_an_ordinary_return() {
    let tokens = Lexer::new(AFFINE_PAIR_SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");

    for (machine, expected_paths) in [("Root::forward", [0, 1]), ("Root::reverse", [1, 0])] {
        let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, machine)
            .expect("both affine array elements lower in authored order");
        let entry = lowered
            .semantic_module
            .machines
            .iter()
            .find(|candidate| candidate.id == lowered.semantic_module.entry)
            .expect("entry machine");
        let [root] = entry.structural_parameters.as_slice() else {
            panic!("fully consumed affine pair has one structural root")
        };
        let [block] = entry.blocks.as_slice() else {
            panic!("fully consumed affine pair has one block")
        };
        let [first_call, second_call] = block.operations.as_slice() else {
            panic!("fully consumed affine pair has two calls")
        };
        let moved_paths = [first_call, second_call]
            .into_iter()
            .map(|call| {
                let OperationKind::CallUnit {
                    structural_arguments,
                    claim_transfers,
                    ..
                } = &call.kind
                else {
                    panic!("fully consumed affine pair performs Unit calls")
                };
                assert!(claim_transfers.is_empty());
                let [argument] = structural_arguments.as_slice() else {
                    panic!("each array call has one projected argument")
                };
                assert_eq!(argument.place, root.place);
                argument.path.clone()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            moved_paths,
            expected_paths
                .into_iter()
                .map(|index| vec![StructuralPathSegment::FixedIndex(index)])
                .collect::<Vec<_>>()
        );
        let Terminator::ReturnUnit {
            edge,
            trivial_affine_discards,
        } = &block.terminator
        else {
            panic!("full array consumption needs no partial-cleanup terminator")
        };
        assert!(trivial_affine_discards.is_empty());

        psi_terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .expect("verifier reconstructs exact complete array consumption");
        let semantic = encode_module(&lowered.semantic_module).expect("module encodes");
        assert_eq!(decode_module(&semantic).unwrap(), lowered.semantic_module);
        let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encodes");
        let argument_value = TerminalStructuralValue {
            opaque_identity: 0x4655_4c4c,
            structural_type: root.structural_type,
            qualifications: root.qualifications.clone(),
            path: Vec::new(),
        };
        let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[],
            &[argument_value],
        )
        .expect("verified fully consumed affine pair starts");
        let mut meter = TerminalFuelMeter::with_allowance(4);
        assert_eq!(
            execution.resume(&mut meter).expect("execution suspends"),
            TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
                schedule: TerminalFuelSchedule::CURRENT.identity(),
                site: FuelChargeSite::Edge(*edge),
                required_units: 1,
                remaining_units: 0,
            })
        );
        assert!(execution.live_affine_frontier().next().is_none());
        meter.replenish(1).expect("replenish caller return");
        assert_eq!(
            execution.resume(&mut meter).expect("execution completes"),
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
        );
        assert_eq!(meter.usage().total_units(), 5);

        let mut duplicate = lowered.semantic_module.clone();
        let duplicate_entry = duplicate
            .machines
            .iter_mut()
            .find(|candidate| candidate.id == duplicate.entry)
            .unwrap();
        let second_path = match &mut duplicate_entry.blocks[0].operations[1].kind {
            OperationKind::CallUnit {
                structural_arguments,
                ..
            } => &mut structural_arguments[0].path,
            _ => unreachable!(),
        };
        *second_path = moved_paths[0].clone();
        assert!(
            psi_terminal_verifier::verify_module(
                &duplicate,
                &lowered.proof_bundle,
                &AdmissionProfile::default(),
            )
            .is_err(),
            "a duplicated array move cannot complete the root"
        );

        let mut missing = lowered.semantic_module.clone();
        let missing_entry = missing
            .machines
            .iter_mut()
            .find(|candidate| candidate.id == missing.entry)
            .unwrap();
        missing_entry.blocks[0].operations.remove(1);
        assert!(
            psi_terminal_verifier::verify_module(
                &missing,
                &lowered.proof_bundle,
                &AdmissionProfile::default(),
            )
            .is_err(),
            "one array move cannot justify an ordinary affine return"
        );

        let mut wrong_length = lowered.semantic_module.clone();
        let declaration = wrong_length
            .structural_types
            .iter_mut()
            .find(|shape| shape.id == root.structural_type)
            .unwrap();
        let psi_terminal::StructuralTypeShape::FixedArray { length, .. } = &mut declaration.shape
        else {
            unreachable!()
        };
        *length = 3;
        assert!(
            psi_terminal_verifier::verify_module(
                &wrong_length,
                &lowered.proof_bundle,
                &AdmissionProfile::default(),
            )
            .is_err(),
            "two moves cannot complete a wider array"
        );
    }
}

#[test]
fn affine_triple_residuals_follow_the_exact_decreasing_live_index_order() {
    let tokens = Lexer::new(AFFINE_TRIPLE_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");

    assert!(psi_checked_trees_to_terminal::lower_machine(&checked, "Root::all").is_err());

    let one = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::one")
        .expect("one triple move and two ordered residuals lower");
    let entry = one
        .semantic_module
        .machines
        .iter()
        .find(|candidate| candidate.id == one.semantic_module.entry)
        .expect("entry machine");
    let [root] = entry.structural_parameters.as_slice() else {
        panic!("affine triple has one structural root")
    };
    let [block] = entry.blocks.as_slice() else {
        panic!("affine triple has one block")
    };
    assert_eq!(block.operations.len(), 1);
    let Terminator::ReturnUnitPartialAffine {
        edge,
        trivial_affine_discards,
        residual_affine_discards,
    } = &block.terminator
    else {
        panic!("one triple move uses one partial-cleanup return")
    };
    assert!(trivial_affine_discards.is_empty());
    assert_eq!(
        residual_affine_discards
            .iter()
            .map(|discard| {
                assert_eq!(discard.place, root.place);
                match discard.path.as_slice() {
                    [StructuralPathSegment::FixedIndex(index)] => *index,
                    _ => panic!("array residual is one literal index"),
                }
            })
            .collect::<Vec<_>>(),
        vec![2, 1],
    );
    psi_terminal_verifier::verify_module(
        &one.semantic_module,
        &one.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier reconstructs two decreasing triple residuals");

    let mut wrong_order = one.semantic_module.clone();
    let entry = wrong_order
        .machines
        .iter_mut()
        .find(|candidate| candidate.id == wrong_order.entry)
        .unwrap();
    let Terminator::ReturnUnitPartialAffine {
        residual_affine_discards,
        ..
    } = &mut entry.blocks[0].terminator
    else {
        unreachable!()
    };
    residual_affine_discards.reverse();
    assert!(
        psi_terminal_verifier::verify_module(
            &wrong_order,
            &one.proof_bundle,
            &AdmissionProfile::default(),
        )
        .is_err(),
        "producer-authored increasing cleanup order carries no authority",
    );

    let semantic = encode_module(&one.semantic_module).expect("module encodes");
    assert_eq!(decode_module(&semantic).unwrap(), one.semantic_module);
    let proof = encode_proof_bundle(&one.proof_bundle).expect("proof encodes");
    let argument_value = TerminalStructuralValue {
        opaque_identity: 0x5452_4950,
        structural_type: root.structural_type,
        qualifications: root.qualifications.clone(),
        path: Vec::new(),
    };
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument_value],
    )
    .expect("verified one-move affine triple starts");
    let mut meter = TerminalFuelMeter::with_allowance(2);
    assert_eq!(
        execution.resume(&mut meter).expect("execution suspends"),
        TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
            schedule: TerminalFuelSchedule::CURRENT.identity(),
            site: FuelChargeSite::Edge(*edge),
            required_units: 1,
            remaining_units: 0,
        })
    );
    meter.replenish(1).expect("replenish residual return edge");
    assert_eq!(
        execution.resume(&mut meter).expect("execution completes"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(meter.usage().total_units(), 3);

    for (machine, expected_paths, residual) in [
        ("Root::middle", [2, 0], 1),
        ("Root::last", [0, 1], 2),
        ("Root::first", [1, 2], 0),
    ] {
        let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, machine)
            .expect("two triple elements and the sole residual lower");
        let entry = lowered
            .semantic_module
            .machines
            .iter()
            .find(|candidate| candidate.id == lowered.semantic_module.entry)
            .expect("entry machine");
        let [root] = entry.structural_parameters.as_slice() else {
            panic!("affine triple has one structural root")
        };
        let [block] = entry.blocks.as_slice() else {
            panic!("affine triple has one block")
        };
        assert_eq!(
            block
                .operations
                .iter()
                .map(|operation| {
                    let OperationKind::CallUnit {
                        structural_arguments,
                        claim_transfers,
                        ..
                    } = &operation.kind
                    else {
                        panic!("affine triple body retains only Unit calls")
                    };
                    assert!(claim_transfers.is_empty());
                    let [StructuralPathSegment::FixedIndex(index)] =
                        structural_arguments[0].path.as_slice()
                    else {
                        panic!("each triple call retains one literal index")
                    };
                    *index
                })
                .collect::<Vec<_>>(),
            expected_paths
        );
        let Terminator::ReturnUnitPartialAffine {
            edge,
            trivial_affine_discards,
            residual_affine_discards,
        } = &block.terminator
        else {
            panic!("affine triple uses one partial-cleanup return")
        };
        assert!(trivial_affine_discards.is_empty());
        let [discard] = residual_affine_discards.as_slice() else {
            panic!("affine triple has one residual")
        };
        assert_eq!(discard.place, root.place);
        assert_eq!(discard.path, [StructuralPathSegment::FixedIndex(residual)]);

        psi_terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .expect("verifier reconstructs the sole triple residual");
        let semantic = encode_module(&lowered.semantic_module).expect("module encodes");
        assert_eq!(decode_module(&semantic).unwrap(), lowered.semantic_module);
        let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encodes");
        let argument_value = TerminalStructuralValue {
            opaque_identity: 0x5452_4950,
            structural_type: root.structural_type,
            qualifications: root.qualifications.clone(),
            path: Vec::new(),
        };
        let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[],
            &[argument_value],
        )
        .expect("verified affine triple starts");
        let mut meter = TerminalFuelMeter::with_allowance(4);
        assert_eq!(
            execution.resume(&mut meter).expect("execution suspends"),
            TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
                schedule: TerminalFuelSchedule::CURRENT.identity(),
                site: FuelChargeSite::Edge(*edge),
                required_units: 1,
                remaining_units: 0,
            })
        );
        assert_eq!(execution.live_affine_frontier().count(), 1);
        meter.replenish(1).expect("replenish residual return edge");
        assert_eq!(
            execution.resume(&mut meter).expect("execution completes"),
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
        );
        assert!(execution.live_affine_frontier().next().is_none());
        assert_eq!(meter.usage().total_units(), 5);

        let mut duplicate = lowered.semantic_module.clone();
        let entry = duplicate
            .machines
            .iter_mut()
            .find(|candidate| candidate.id == duplicate.entry)
            .unwrap();
        let first_path = match &entry.blocks[0].operations[0].kind {
            OperationKind::CallUnit {
                structural_arguments,
                ..
            } => structural_arguments[0].path.clone(),
            _ => unreachable!(),
        };
        let OperationKind::CallUnit {
            structural_arguments,
            ..
        } = &mut entry.blocks[0].operations[1].kind
        else {
            unreachable!()
        };
        structural_arguments[0].path = first_path;
        assert!(
            psi_terminal_verifier::verify_module(
                &duplicate,
                &lowered.proof_bundle,
                &AdmissionProfile::default(),
            )
            .is_err()
        );

        let mut wrong_length = lowered.semantic_module.clone();
        let declaration = wrong_length
            .structural_types
            .iter_mut()
            .find(|shape| shape.id == root.structural_type)
            .unwrap();
        let psi_terminal::StructuralTypeShape::FixedArray { length, .. } = &mut declaration.shape
        else {
            unreachable!()
        };
        *length = 2;
        assert!(
            psi_terminal_verifier::verify_module(
                &wrong_length,
                &lowered.proof_bundle,
                &AdmissionProfile::default(),
            )
            .is_err()
        );

        let mut wrong_residual = lowered.semantic_module.clone();
        let entry = wrong_residual
            .machines
            .iter_mut()
            .find(|candidate| candidate.id == wrong_residual.entry)
            .unwrap();
        let Terminator::ReturnUnitPartialAffine {
            residual_affine_discards,
            ..
        } = &mut entry.blocks[0].terminator
        else {
            unreachable!()
        };
        residual_affine_discards[0].path =
            vec![StructuralPathSegment::FixedIndex(expected_paths[0])];
        assert!(
            psi_terminal_verifier::verify_module(
                &wrong_residual,
                &lowered.proof_bundle,
                &AdmissionProfile::default(),
            )
            .is_err()
        );

        let mut scalar_parameter = lowered.semantic_module.clone();
        let entry = scalar_parameter
            .machines
            .iter_mut()
            .find(|candidate| candidate.id == scalar_parameter.entry)
            .unwrap();
        entry.parameters.push(psi_terminal::ValueDeclaration {
            id: psi_core::ValueId::new(1).unwrap(),
            scalar_type: psi_core::ScalarType::Boolean,
        });
        assert!(
            psi_terminal_verifier::verify_module(
                &scalar_parameter,
                &lowered.proof_bundle,
                &AdmissionProfile::default(),
            )
            .is_err()
        );

        let mut missing_call = lowered.semantic_module.clone();
        let entry = missing_call
            .machines
            .iter_mut()
            .find(|candidate| candidate.id == missing_call.entry)
            .unwrap();
        entry.blocks[0].operations.remove(1);
        assert!(
            psi_terminal_verifier::verify_module(
                &missing_call,
                &lowered.proof_bundle,
                &AdmissionProfile::default(),
            )
            .is_err()
        );

        let mut block_parameter = lowered.semantic_module.clone();
        let entry = block_parameter
            .machines
            .iter_mut()
            .find(|candidate| candidate.id == block_parameter.entry)
            .unwrap();
        entry.blocks[0]
            .parameters
            .push(psi_terminal::ValueDeclaration {
                id: psi_core::ValueId::new(1).unwrap(),
                scalar_type: psi_core::ScalarType::Boolean,
            });
        assert!(
            psi_terminal_verifier::verify_module(
                &block_parameter,
                &lowered.proof_bundle,
                &AdmissionProfile::default(),
            )
            .is_err()
        );

        let mut extra_block = lowered.semantic_module.clone();
        let entry = extra_block
            .machines
            .iter_mut()
            .find(|candidate| candidate.id == extra_block.entry)
            .unwrap();
        entry.blocks.push(entry.blocks[0].clone());
        assert!(
            psi_terminal_verifier::verify_module(
                &extra_block,
                &lowered.proof_bundle,
                &AdmissionProfile::default(),
            )
            .is_err()
        );

        let mut callee_contract = lowered.semantic_module.clone();
        let entry = callee_contract
            .machines
            .iter()
            .find(|candidate| candidate.id == callee_contract.entry)
            .unwrap();
        let OperationKind::CallUnit { callee, .. } = &entry.blocks[0].operations[0].kind else {
            unreachable!()
        };
        let callee = *callee;
        callee_contract
            .machines
            .iter_mut()
            .find(|candidate| candidate.id == callee)
            .unwrap()
            .contract
            .requires
            .push(psi_core::Proposition::Truth);
        assert!(
            psi_terminal_verifier::verify_module(
                &callee_contract,
                &lowered.proof_bundle,
                &AdmissionProfile::default(),
            )
            .is_err()
        );
    }
}

#[test]
fn affine_quartet_two_moves_retain_authored_calls_and_decreasing_residuals() {
    let tokens = Lexer::new(AFFINE_QUARTET_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("two quartet moves and their decreasing complement lower");
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|candidate| candidate.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let [root] = entry.structural_parameters.as_slice() else {
        panic!("affine quartet has one structural root")
    };
    let [block] = entry.blocks.as_slice() else {
        panic!("affine quartet has one block")
    };
    assert_eq!(
        block
            .operations
            .iter()
            .map(|operation| match &operation.kind {
                OperationKind::CallUnit {
                    structural_arguments,
                    ..
                } => match structural_arguments[0].path.as_slice() {
                    [StructuralPathSegment::FixedIndex(index)] => *index,
                    _ => panic!("quartet call is one literal index"),
                },
                _ => panic!("quartet body retains only Unit calls"),
            })
            .collect::<Vec<_>>(),
        vec![1, 3],
    );
    let Terminator::ReturnUnitPartialAffine {
        edge,
        residual_affine_discards,
        ..
    } = &block.terminator
    else {
        panic!("quartet uses a partial-affine return")
    };
    assert_eq!(
        residual_affine_discards
            .iter()
            .map(|discard| {
                assert_eq!(discard.place, root.place);
                match discard.path.as_slice() {
                    [StructuralPathSegment::FixedIndex(index)] => *index,
                    _ => panic!("quartet residual is one literal index"),
                }
            })
            .collect::<Vec<_>>(),
        vec![2, 0],
    );
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier reconstructs the quartet complement");

    let mut wrong_order = lowered.semantic_module.clone();
    let entry = wrong_order
        .machines
        .iter_mut()
        .find(|candidate| candidate.id == wrong_order.entry)
        .unwrap();
    let Terminator::ReturnUnitPartialAffine {
        residual_affine_discards,
        ..
    } = &mut entry.blocks[0].terminator
    else {
        unreachable!()
    };
    residual_affine_discards.reverse();
    assert!(
        psi_terminal_verifier::verify_module(
            &wrong_order,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .is_err(),
        "increasing producer order rejects",
    );

    let semantic = encode_module(&lowered.semantic_module).expect("module encodes");
    assert_eq!(decode_module(&semantic).unwrap(), lowered.semantic_module);
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encodes");
    let argument_value = TerminalStructuralValue {
        opaque_identity: 0x5155_4152,
        structural_type: root.structural_type,
        qualifications: root.qualifications.clone(),
        path: Vec::new(),
    };
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument_value],
    )
    .expect("verified affine quartet starts");
    let mut meter = TerminalFuelMeter::with_allowance(4);
    assert_eq!(
        execution.resume(&mut meter).expect("execution suspends"),
        TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
            schedule: TerminalFuelSchedule::CURRENT.identity(),
            site: FuelChargeSite::Edge(*edge),
            required_units: 1,
            remaining_units: 0,
        })
    );
    meter.replenish(1).expect("replenish quartet return edge");
    assert_eq!(
        execution.resume(&mut meter).expect("execution completes"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(meter.usage().total_units(), 5);
}

fn assert_nested_affine_array_cleanup_crosses_source_codec_verifier_and_interpreter(
    source: &str,
    expected_moves: &[(u64, u64)],
    expected_residuals: &[(u64, u64)],
) {
    let tokens = Lexer::new(source).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("nested leaf moves and their decreasing complements lower");
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|candidate| candidate.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let [root] = entry.structural_parameters.as_slice() else {
        panic!("nested affine array has one structural root")
    };
    let [block] = entry.blocks.as_slice() else {
        panic!("nested affine array has one block")
    };
    let path = |path: &[StructuralPathSegment]| match path {
        [
            StructuralPathSegment::FixedIndex(outer),
            StructuralPathSegment::FixedIndex(inner),
        ] => (*outer, *inner),
        _ => panic!("nested array leaf has exactly two literal indices"),
    };
    assert_eq!(
        block
            .operations
            .iter()
            .map(|operation| match &operation.kind {
                OperationKind::CallUnit {
                    structural_arguments,
                    ..
                } => path(&structural_arguments[0].path),
                _ => panic!("nested body retains only Unit calls"),
            })
            .collect::<Vec<_>>(),
        expected_moves,
    );
    let Terminator::ReturnUnitPartialAffine {
        edge,
        residual_affine_discards,
        ..
    } = &block.terminator
    else {
        panic!("nested array uses a partial-affine return")
    };
    assert_eq!(
        residual_affine_discards
            .iter()
            .map(|discard| {
                assert_eq!(discard.place, root.place);
                path(&discard.path)
            })
            .collect::<Vec<_>>(),
        expected_residuals,
    );
    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier reconstructs both decreasing nested complements");

    let mut wrong_order = lowered.semantic_module.clone();
    let entry = wrong_order
        .machines
        .iter_mut()
        .find(|candidate| candidate.id == wrong_order.entry)
        .unwrap();
    let Terminator::ReturnUnitPartialAffine {
        residual_affine_discards,
        ..
    } = &mut entry.blocks[0].terminator
    else {
        unreachable!()
    };
    residual_affine_discards.reverse();
    assert!(
        psi_terminal_verifier::verify_module(
            &wrong_order,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .is_err(),
        "reversed nested residual order rejects",
    );

    let mut wrong_inner_length = lowered.semantic_module.clone();
    let outer = wrong_inner_length
        .structural_types
        .iter()
        .find(|shape| shape.id == root.structural_type)
        .expect("outer array declaration");
    let psi_terminal::StructuralTypeShape::FixedArray { element: inner, .. } = outer.shape else {
        unreachable!()
    };
    let inner = wrong_inner_length
        .structural_types
        .iter_mut()
        .find(|shape| shape.id == inner)
        .expect("inner array declaration");
    let psi_terminal::StructuralTypeShape::FixedArray { length, .. } = &mut inner.shape else {
        unreachable!()
    };
    *length = match *length {
        3 => 4,
        6 => 7,
        _ => 3,
    };
    assert!(
        psi_terminal_verifier::verify_module(
            &wrong_inner_length,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .is_err(),
        "cross-width inner declaration mutation rejects",
    );

    let mut out_of_bounds_path = lowered.semantic_module.clone();
    let entry = out_of_bounds_path
        .machines
        .iter_mut()
        .find(|candidate| candidate.id == out_of_bounds_path.entry)
        .unwrap();
    let OperationKind::CallUnit {
        structural_arguments,
        ..
    } = &mut entry.blocks[0].operations[0].kind
    else {
        unreachable!()
    };
    let [_, StructuralPathSegment::FixedIndex(inner)] = structural_arguments[0].path.as_mut_slice()
    else {
        unreachable!()
    };
    *inner = u64::try_from(expected_residuals.len() / 2 + 1).expect("bounded inner width");
    assert!(
        psi_terminal_verifier::verify_module(
            &out_of_bounds_path,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .is_err(),
        "one-past-inner-width path mutation rejects",
    );

    let semantic = encode_module(&lowered.semantic_module).expect("module encodes");
    assert_eq!(decode_module(&semantic).unwrap(), lowered.semantic_module);
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof encodes");
    let argument_value = TerminalStructuralValue {
        opaque_identity: 0x4e45_5354,
        structural_type: root.structural_type,
        qualifications: root.qualifications.clone(),
        path: Vec::new(),
    };
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument_value],
    )
    .expect("verified nested affine artifact starts");
    let mut meter = TerminalFuelMeter::with_allowance(4);
    assert_eq!(
        execution.resume(&mut meter).expect("execution suspends"),
        TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
            schedule: TerminalFuelSchedule::CURRENT.identity(),
            site: FuelChargeSite::Edge(*edge),
            required_units: 1,
            remaining_units: 0,
        })
    );
    meter.replenish(1).expect("replenish nested return edge");
    assert_eq!(
        execution.resume(&mut meter).expect("execution completes"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(meter.usage().total_units(), 5);
}

#[test]
fn nested_affine_array_cleanup_crosses_source_codec_verifier_and_interpreter() {
    assert_nested_affine_array_cleanup_crosses_source_codec_verifier_and_interpreter(
        NESTED_AFFINE_ARRAY_SOURCE,
        &[(1, 0), (0, 1)],
        &[(1, 2), (1, 1), (0, 2), (0, 0)],
    );
    assert_nested_affine_array_cleanup_crosses_source_codec_verifier_and_interpreter(
        NESTED_AFFINE_QUARTET_SOURCE,
        &[(1, 3), (0, 1)],
        &[(1, 2), (1, 1), (1, 0), (0, 3), (0, 2), (0, 0)],
    );
    assert_nested_affine_array_cleanup_crosses_source_codec_verifier_and_interpreter(
        NESTED_AFFINE_QUINTET_SOURCE,
        &[(1, 4), (0, 1)],
        &[
            (1, 3),
            (1, 2),
            (1, 1),
            (1, 0),
            (0, 4),
            (0, 3),
            (0, 2),
            (0, 0),
        ],
    );
    assert_nested_affine_array_cleanup_crosses_source_codec_verifier_and_interpreter(
        NESTED_AFFINE_SEXTET_SOURCE,
        &[(1, 5), (0, 1)],
        &[
            (1, 4),
            (1, 3),
            (1, 2),
            (1, 1),
            (1, 0),
            (0, 5),
            (0, 4),
            (0, 3),
            (0, 2),
            (0, 0),
        ],
    );
}

#[test]
fn direct_field_partial_affine_cleanup_crosses_source_codec_verifier_and_interpreter() {
    let tokens = Lexer::new(SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("direct field transfer plus residual affine cleanup lowers");

    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let [root] = entry.structural_parameters.as_slice() else {
        panic!("partial affine source slice has one structural root")
    };
    let root_shape = lowered
        .semantic_module
        .structural_types
        .iter()
        .find(|shape| shape.id == root.structural_type)
        .expect("mixed scalar/affine root shape");
    let psi_terminal::StructuralTypeShape::Record { fields } = &root_shape.shape else {
        panic!("mixed scalar/affine root remains a record")
    };
    assert_eq!(
        fields
            .iter()
            .map(|field| field.identity.as_str())
            .collect::<Vec<_>>(),
        vec![
            "before",
            "before_bytes",
            "before_float",
            "first",
            "between",
            "between_bytes",
            "between_float",
            "second",
            "third",
            "fourth",
            "fifth",
            "after",
        ],
        "scalar, float, and exact-capacity byte fields remain ordered structural identity"
    );
    assert_eq!(
        fields
            .iter()
            .find(|field| field.identity == "before_float")
            .expect("f32 field")
            .field_type,
        StructuralFieldType::IeeeFloat(psi_core::IeeeFloatFormat::Binary32)
    );
    assert_eq!(
        fields
            .iter()
            .find(|field| field.identity == "between_float")
            .expect("f64 field")
            .field_type,
        StructuralFieldType::IeeeFloat(psi_core::IeeeFloatFormat::Binary64)
    );
    assert_eq!(
        fields
            .iter()
            .filter_map(|field| match field.field_type {
                StructuralFieldType::ByteSequence(carrier) => {
                    Some((field.identity.as_str(), carrier))
                }
                _ => None,
            })
            .collect::<Vec<_>>(),
        vec![
            (
                "before_bytes",
                psi_terminal::ByteSequenceCarrier::BoundedOwned { capacity: 3 },
            ),
            (
                "between_bytes",
                psi_terminal::ByteSequenceCarrier::BoundedOwned { capacity: 8 },
            ),
        ]
    );
    let [block] = entry.blocks.as_slice() else {
        panic!("partial affine source slice has one block")
    };
    let [first_call, second_call] = block.operations.as_slice() else {
        panic!("partial affine source slice has two projected calls")
    };
    let moved_paths = [first_call, second_call]
        .into_iter()
        .map(|call| {
            let OperationKind::CallUnit {
                structural_arguments,
                claim_transfers,
                ..
            } = &call.kind
            else {
                panic!("expected direct Unit call")
            };
            assert!(claim_transfers.is_empty());
            let [argument] = structural_arguments.as_slice() else {
                panic!("each projected call has one argument")
            };
            assert_eq!(argument.place, root.place);
            argument.path.clone()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        moved_paths,
        vec![
            vec![StructuralPathSegment::Field("third".into())],
            vec![StructuralPathSegment::Field("first".into())],
        ]
    );

    let Terminator::ReturnUnitPartialAffine {
        edge,
        trivial_affine_discards,
        residual_affine_discards,
    } = &block.terminator
    else {
        panic!("expected path-sensitive Unit return")
    };
    assert!(trivial_affine_discards.is_empty());
    assert_eq!(
        residual_affine_discards
            .iter()
            .map(|residual| {
                assert_eq!(residual.place, root.place);
                residual.path.clone()
            })
            .collect::<Vec<_>>(),
        vec![
            vec![StructuralPathSegment::Field("fifth".into())],
            vec![StructuralPathSegment::Field("fourth".into())],
            vec![StructuralPathSegment::Field("second".into())],
        ]
    );

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier reconstructs moved field plus residual root exhaustion");

    let mut scalar_as_structural = lowered.semantic_module.clone();
    let root_shape = scalar_as_structural
        .structural_types
        .iter_mut()
        .find(|shape| shape.id == root.structural_type)
        .expect("mixed scalar/affine root shape");
    let psi_terminal::StructuralTypeShape::Record { fields } = &mut root_shape.shape else {
        unreachable!()
    };
    let token_type = fields
        .iter()
        .find_map(|field| match field.field_type {
            StructuralFieldType::Structural(structural_type) => Some(structural_type),
            _ => None,
        })
        .expect("token type");
    fields
        .iter_mut()
        .find(|field| field.identity == "before_float")
        .expect("interleaved float")
        .field_type = StructuralFieldType::Structural(token_type);
    assert!(
        psi_terminal_verifier::verify_module(
            &scalar_as_structural,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .is_err(),
        "reclassifying a cleanup-free float as affine structural requires a new residual"
    );

    let mut bytes_as_structural = lowered.semantic_module.clone();
    let root_shape = bytes_as_structural
        .structural_types
        .iter_mut()
        .find(|shape| shape.id == root.structural_type)
        .expect("mixed byte/affine root shape");
    let psi_terminal::StructuralTypeShape::Record { fields } = &mut root_shape.shape else {
        unreachable!()
    };
    fields
        .iter_mut()
        .find(|field| field.identity == "before_bytes")
        .expect("interleaved bounded byte field")
        .field_type = StructuralFieldType::Structural(token_type);
    assert!(
        psi_terminal_verifier::verify_module(
            &bytes_as_structural,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .is_err(),
        "reclassifying a cleanup-free bounded byte field as structural requires a residual"
    );

    let mut bytes_as_borrowed = lowered.semantic_module.clone();
    let root_shape = bytes_as_borrowed
        .structural_types
        .iter_mut()
        .find(|shape| shape.id == root.structural_type)
        .expect("mixed byte/affine root shape");
    let psi_terminal::StructuralTypeShape::Record { fields } = &mut root_shape.shape else {
        unreachable!()
    };
    fields
        .iter_mut()
        .find(|field| field.identity == "before_bytes")
        .expect("interleaved bounded byte field")
        .field_type =
        StructuralFieldType::ByteSequence(psi_terminal::ByteSequenceCarrier::BorrowedView);
    assert!(
        psi_terminal_verifier::verify_module(
            &bytes_as_borrowed,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .is_err(),
        "a borrowed byte view cannot masquerade as no-code bounded storage"
    );

    let mut moved_as_float = lowered.semantic_module.clone();
    let root_shape = moved_as_float
        .structural_types
        .iter_mut()
        .find(|shape| shape.id == root.structural_type)
        .expect("mixed scalar/affine root shape");
    let psi_terminal::StructuralTypeShape::Record { fields } = &mut root_shape.shape else {
        unreachable!()
    };
    fields
        .iter_mut()
        .find(|field| field.identity == "third")
        .expect("moved structural field")
        .field_type = StructuralFieldType::IeeeFloat(psi_core::IeeeFloatFormat::Binary32);
    assert!(
        psi_terminal_verifier::verify_module(
            &moved_as_float,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .is_err(),
        "a projected move cannot target a float field"
    );
    let mut moved_as_bytes = lowered.semantic_module.clone();
    let root_shape = moved_as_bytes
        .structural_types
        .iter_mut()
        .find(|shape| shape.id == root.structural_type)
        .expect("mixed byte/affine root shape");
    let psi_terminal::StructuralTypeShape::Record { fields } = &mut root_shape.shape else {
        unreachable!()
    };
    fields
        .iter_mut()
        .find(|field| field.identity == "third")
        .expect("moved structural field")
        .field_type =
        StructuralFieldType::ByteSequence(psi_terminal::ByteSequenceCarrier::BoundedOwned {
            capacity: 3,
        });
    assert!(
        psi_terminal_verifier::verify_module(
            &moved_as_bytes,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .is_err(),
        "a projected move cannot target a bounded byte leaf"
    );
    let semantic = encode_module(&lowered.semantic_module).expect("semantic module encodes");
    assert_eq!(
        decode_module(&semantic).expect("semantic module decodes"),
        lowered.semantic_module,
        "the partial frontier is canonical artifact identity"
    );
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof bundle encodes");

    let argument = TerminalStructuralValue {
        opaque_identity: 0x5041_4952,
        structural_type: root.structural_type,
        qualifications: root.qualifications.clone(),
        path: Vec::new(),
    };
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument],
    )
    .expect("verified source artifact starts");

    // The projected call and callee return commit first. With no remaining
    // allowance, the caller suspends at its return edge while every residual
    // remains live; cleanup commits only after replenishment.
    let mut meter = TerminalFuelMeter::with_allowance(4);
    assert_eq!(
        execution
            .resume(&mut meter)
            .expect("execution suspends cleanly"),
        TerminalExecutionStatus::SponsorExhausted(FuelExhaustion {
            schedule: TerminalFuelSchedule::CURRENT.identity(),
            site: FuelChargeSite::Edge(*edge),
            required_units: 1,
            remaining_units: 0,
        })
    );
    let live_residuals = execution
        .live_affine_frontier()
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(live_residuals.len(), residual_affine_discards.len());
    assert!(
        residual_affine_discards
            .iter()
            .all(|residual| live_residuals.contains(residual))
    );

    meter.replenish(1).expect("replenish return-edge fuel");
    assert_eq!(
        execution.resume(&mut meter).expect("execution completes"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert!(execution.live_affine_frontier().next().is_none());
    assert_eq!(meter.usage().total_units(), 5);
}

#[test]
fn mixed_field_partial_affine_cleanup_crosses_source_codec_verifier_and_interpreter() {
    let tokens = Lexer::new(MIXED_SOURCE).tokenize().expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = lower_syntax_trees(&syntax).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed).expect("check");
    let lowered = psi_checked_trees_to_terminal::lower_machine(&checked, "Root::enter")
        .expect("mixed field transfers plus recursive residual cleanup lower");
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .expect("entry machine");
    let [root] = entry.structural_parameters.as_slice() else {
        panic!("nested partial slice has one structural root")
    };
    let [block] = entry.blocks.as_slice() else {
        panic!("nested partial slice has one block")
    };
    assert_eq!(
        block
            .operations
            .iter()
            .map(|operation| match &operation.kind {
                OperationKind::CallUnit {
                    structural_arguments,
                    ..
                } => structural_arguments[0].path.clone(),
                _ => panic!("mixed partial slice calls Unit"),
            })
            .collect::<Vec<_>>(),
        vec![
            vec![
                StructuralPathSegment::Field("left".into()),
                StructuralPathSegment::Field("deep".into()),
                StructuralPathSegment::Field("middle".into()),
            ],
            vec![
                StructuralPathSegment::Field("right".into()),
                StructuralPathSegment::Field("tail".into()),
            ],
            vec![StructuralPathSegment::Field("first".into())],
        ]
    );
    let Terminator::ReturnUnitPartialAffine {
        residual_affine_discards,
        ..
    } = &block.terminator
    else {
        panic!("nested partial slice has path-sensitive return")
    };
    assert_eq!(
        residual_affine_discards
            .iter()
            .map(|residual| residual.path.clone())
            .collect::<Vec<_>>(),
        vec![
            vec![StructuralPathSegment::Field("last".into())],
            vec![
                StructuralPathSegment::Field("right".into()),
                StructuralPathSegment::Field("deep".into()),
            ],
            vec![
                StructuralPathSegment::Field("right".into()),
                StructuralPathSegment::Field("head".into()),
            ],
            vec![
                StructuralPathSegment::Field("left".into()),
                StructuralPathSegment::Field("tail".into()),
            ],
            vec![
                StructuralPathSegment::Field("left".into()),
                StructuralPathSegment::Field("deep".into()),
                StructuralPathSegment::Field("high".into()),
            ],
            vec![
                StructuralPathSegment::Field("left".into()),
                StructuralPathSegment::Field("deep".into()),
                StructuralPathSegment::Field("low".into()),
            ],
            vec![
                StructuralPathSegment::Field("left".into()),
                StructuralPathSegment::Field("head".into()),
            ],
        ]
    );

    psi_terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier reconstructs nested moved leaf plus maximal residual subtrees");
    let semantic = encode_module(&lowered.semantic_module).expect("semantic module encodes");
    assert_eq!(
        decode_module(&semantic).expect("semantic module decodes"),
        lowered.semantic_module
    );
    let proof = encode_proof_bundle(&lowered.proof_bundle).expect("proof bundle encodes");
    let argument = TerminalStructuralValue {
        opaque_identity: 0x4e45_5354,
        structural_type: root.structural_type,
        qualifications: root.qualifications.clone(),
        path: Vec::new(),
    };
    let mut execution = TerminalExecution::start_artifact_with_structural_arguments(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        &[argument],
    )
    .expect("verified nested source artifact starts");
    let mut meter = TerminalFuelMeter::with_allowance(7);
    assert_eq!(
        execution.resume(&mut meter).expect("execution completes"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert!(execution.live_affine_frontier().next().is_none());
    assert_eq!(meter.usage().total_units(), 7);
}
