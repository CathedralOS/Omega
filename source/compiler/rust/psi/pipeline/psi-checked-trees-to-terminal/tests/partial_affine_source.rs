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
    data Token { value: u64; }
    data Quintet {
        before: u8;
        before_float: f32;
        first: Token;
        between: bool;
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
            "before_float",
            "first",
            "between",
            "between_float",
            "second",
            "third",
            "fourth",
            "fifth",
            "after",
        ],
        "integer, Boolean, and exact-format float fields remain ordered structural identity"
    );
    assert_eq!(
        fields[1].field_type,
        StructuralFieldType::IeeeFloat(psi_core::IeeeFloatFormat::Binary32)
    );
    assert_eq!(
        fields[4].field_type,
        StructuralFieldType::IeeeFloat(psi_core::IeeeFloatFormat::Binary64)
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
