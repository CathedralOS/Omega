use super::{
    ResultBoundaryHandler, UNIT_AFFINE_CONSTRUCTION_PREFIX_SOURCE,
    UNIT_AFFINE_DEEPER_CONSTRUCTION_PREFIX_SOURCE, UNIT_AFFINE_DEEPEST_CONSTRUCTION_PREFIX_SOURCE,
    UNIT_AFFINE_EIGHT_CONSTRUCTION_PREFIX_SOURCE, UNIT_AFFINE_EIGHTEEN_CONSTRUCTION_PREFIX_SOURCE,
    UNIT_AFFINE_ELEVEN_CONSTRUCTION_PREFIX_SOURCE, UNIT_AFFINE_FIFTEEN_CONSTRUCTION_PREFIX_SOURCE,
    UNIT_AFFINE_FOURTEEN_CONSTRUCTION_PREFIX_SOURCE, UNIT_AFFINE_LOCAL_SOURCE,
    UNIT_AFFINE_NINE_CONSTRUCTION_PREFIX_SOURCE, UNIT_AFFINE_NINETEEN_CONSTRUCTION_PREFIX_SOURCE,
    UNIT_AFFINE_SEVEN_CONSTRUCTION_PREFIX_SOURCE, UNIT_AFFINE_SEVENTEEN_CONSTRUCTION_PREFIX_SOURCE,
    UNIT_AFFINE_SIXTEEN_CONSTRUCTION_PREFIX_SOURCE, UNIT_AFFINE_TEN_CONSTRUCTION_PREFIX_SOURCE,
    UNIT_AFFINE_THIRTEEN_CONSTRUCTION_PREFIX_SOURCE, UNIT_AFFINE_TWELVE_CONSTRUCTION_PREFIX_SOURCE,
    UNIT_AFFINE_TWENTY_CONSTRUCTION_PREFIX_SOURCE,
    UNIT_AFFINE_TWENTY_FIVE_CONSTRUCTION_PREFIX_SOURCE,
    UNIT_AFFINE_TWENTY_FOUR_CONSTRUCTION_PREFIX_SOURCE,
    UNIT_AFFINE_TWENTY_ONE_CONSTRUCTION_PREFIX_SOURCE,
    UNIT_AFFINE_TWENTY_SIX_CONSTRUCTION_PREFIX_SOURCE,
    UNIT_AFFINE_TWENTY_THREE_CONSTRUCTION_PREFIX_SOURCE,
    UNIT_AFFINE_TWENTY_TWO_CONSTRUCTION_PREFIX_SOURCE,
    UNIT_AFFINE_WIDER_CONSTRUCTION_PREFIX_SOURCE, checked_result_boundary_source,
};
use proof_admission::AdmissionProfile;
use source_files_to_tokens::Lexer;
use symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
use syntax_trees_to_symbol_resolved_trees::{ResolutionRequest, resolve};
use terminal_codec::{decode_module, encode_module, encode_proof_section};
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::AcceptTerminalEffects;
use terminal_interpreter::TerminalStructuralInputs;
use terminal_interpreter::{
    TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus, TerminalInterpretError,
    TerminalScalarValue, TerminalStructuralValue,
};
use terminal_psi::Terminator;
use tokens_to_syntax_trees::parse_syntax_trees;
use typed_trees_to_checked_trees::CheckingRequest;
use typed_trees_to_checked_trees::lower_typed_trees;

#[test]
fn source_unit_retains_ordered_empty_affine_local_cleanup() {
    let tokens = Lexer::new(UNIT_AFFINE_LOCAL_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled()).expect("check");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::cleanup")
        .expect("bounded Unit local lowering");
    let machine = lowered.semantic_module.machines.first().expect("machine");
    let locals = machine
        .structural_places
        .iter()
        .filter(|place| {
            matches!(
                place.kind,
                semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal { .. }
            )
        })
        .map(|place| place.id)
        .collect::<Vec<_>>();
    assert_eq!(locals.len(), 2);
    assert!(matches!(
        machine.blocks[0].operations.as_slice(),
        [
            terminal_psi::Operation {
                kind: terminal_psi::OperationKind::EstablishTrivialAffineLocal { destination: first },
                ..
            },
            terminal_psi::Operation {
                kind: terminal_psi::OperationKind::EstablishTrivialAffineLocal { destination: second },
                ..
            }
        ] if [*first, *second] == locals.as_slice()
    ));
    let terminal_psi::Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &machine.blocks[0].terminator
    else {
        panic!("Unit return")
    };
    assert_eq!(
        trivial_affine_discards,
        &locals
            .iter()
            .rev()
            .copied()
            .chain(
                machine
                    .structural_parameters
                    .iter()
                    .rev()
                    .map(|parameter| parameter.place)
            )
            .collect::<Vec<_>>()
    );

    let semantic = encode_module(&lowered.semantic_module).expect("Unit semantics encode");
    assert_eq!(
        decode_module(&semantic).expect("Unit semantics decode"),
        lowered.semantic_module,
        "the codec retains explicit local establishment and cleanup custody"
    );
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("independent verifier reconstructs Unit local cleanup");

    let mut missing_declaration = lowered.semantic_module.clone();
    missing_declaration.machines[0]
        .structural_places
        .retain(|place| place.id != locals[0]);
    assert!(
        terminal_verifier::validate_module_representation(&missing_declaration).is_err(),
        "an establishment operation cannot outlive its typed local declaration"
    );

    let mut reordered_declarations = lowered.semantic_module.clone();
    for place in &mut reordered_declarations.machines[0].structural_places {
        if let semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
            declaration_ordinal,
            ..
        } = &mut place.kind
        {
            *declaration_ordinal = 1 - *declaration_ordinal;
        }
    }
    assert!(
        terminal_verifier::validate_module_representation(&reordered_declarations).is_err(),
        "local declarations are canonical source-order custody"
    );

    let mut reordered_cleanup = lowered.semantic_module.clone();
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &mut reordered_cleanup.machines[0].blocks[0].terminator
    else {
        unreachable!()
    };
    trivial_affine_discards.swap(0, 1);
    assert!(
        terminal_verifier::validate_module_representation(&reordered_cleanup).is_err(),
        "reordered cleanup is not a valid terminal module"
    );

    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("Unit proof encodes");
    let arguments = machine
        .structural_parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| TerminalStructuralValue {
            opaque_identity: 0xaff1 + index as u64,
            structural_type: parameter.structural_type,
            qualifications: parameter.qualifications.clone(),
            path: Vec::new(),
        })
        .collect::<Vec<_>>();
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &arguments,
            ..Default::default()
        },
    )
    .expect("Unit affine-local artifact starts");
    let mut meter = TerminalFuelMeter::with_allowance(0);
    for expected_usage in 0..3 {
        assert!(matches!(
            execution
                .resume(&mut meter, &mut AcceptTerminalEffects)
                .unwrap(),
            TerminalExecutionStatus::SponsorExhausted(_)
        ));
        assert_eq!(meter.usage().total_units(), expected_usage);
        meter.replenish(1).unwrap();
    }
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(meter.usage().total_units(), 3);
}

#[test]
fn source_unit_construction_prefix_reaches_verified_interpreted_terminal_psi() {
    let tokens = Lexer::new(UNIT_AFFINE_CONSTRUCTION_PREFIX_SOURCE)
        .tokenize()
        .expect("tokenize");
    let syntax = parse_syntax_trees(&tokens).expect("parse");
    let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
    let typed = lower_symbol_resolved_trees(&resolved).expect("type");
    let checked = lower_typed_trees(typed, &CheckingRequest::settled()).expect("check");
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::cleanup_prefix")
        .expect("bounded construction prefix lowering");
    let machine = &lowered.semantic_module.machines[0];
    let locals = machine
        .structural_places
        .iter()
        .filter_map(|place| match place.kind {
            semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal,
                structural_type,
                construction: Some(construction),
            } => Some((place.id, declaration_ordinal, structural_type, construction)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(locals.len(), 2);
    assert!(
        locals
            .iter()
            .enumerate()
            .all(|(index, (_, ordinal, _, construction))| {
                usize::try_from(*ordinal) == Ok(index)
                    && usize::try_from(construction.index) == Ok(index)
            })
    );
    assert_eq!(
        locals[0].3.root_structural_type,
        locals[1].3.root_structural_type
    );
    assert!(matches!(
        lowered
            .semantic_module
            .structural_types
            .iter()
            .find(|declaration| declaration.id == locals[0].3.root_structural_type)
            .expect("construction root declaration")
            .shape,
        terminal_psi::StructuralTypeShape::FixedArray { element, length: 3 }
            if element == locals[0].2
    ));
    assert!(matches!(
        machine.blocks[0].operations.as_slice(),
        [
            terminal_psi::Operation {
                kind: terminal_psi::OperationKind::EstablishTrivialAffineLocal { destination: first },
                ..
            },
            terminal_psi::Operation {
                kind: terminal_psi::OperationKind::EstablishTrivialAffineLocal { destination: second },
                ..
            }
        ] if [*first, *second] == [locals[0].0, locals[1].0]
    ));
    let Terminator::ReturnUnit {
        trivial_affine_discards,
        ..
    } = &machine.blocks[0].terminator
    else {
        panic!("construction-prefix cleanup must return Unit")
    };
    assert_eq!(trivial_affine_discards, &[locals[1].0, locals[0].0]);

    let semantic = encode_module(&lowered.semantic_module).expect("construction semantics encode");
    assert_eq!(decode_module(&semantic).unwrap(), lowered.semantic_module);
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("verifier reconstructs exact construction prefix and cleanup");

    let mut wrong_index = lowered.semantic_module.clone();
    let semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
        construction: Some(construction),
        ..
    } = &mut wrong_index.machines[0].structural_places[0].kind
    else {
        unreachable!()
    };
    construction.index = 1;
    assert!(terminal_verifier::validate_module_representation(&wrong_index).is_err());

    let mut wrong_root = lowered.semantic_module.clone();
    let root = locals[0].3.root_structural_type;
    let terminal_psi::StructuralTypeShape::FixedArray { length, .. } = &mut wrong_root
        .structural_types
        .iter_mut()
        .find(|declaration| declaration.id == root)
        .expect("construction root declaration")
        .shape
    else {
        unreachable!()
    };
    *length = 4;
    assert!(terminal_verifier::validate_module_representation(&wrong_root).is_err());

    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("construction proof encodes");
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
    )
    .expect("construction-prefix artifact starts");
    let mut meter = TerminalFuelMeter::with_allowance(0);
    for expected_usage in 0..3 {
        assert!(matches!(
            execution
                .resume(&mut meter, &mut AcceptTerminalEffects)
                .unwrap(),
            TerminalExecutionStatus::SponsorExhausted(_)
        ));
        assert_eq!(meter.usage().total_units(), expected_usage);
        meter.replenish(1).unwrap();
    }
    assert_eq!(
        execution
            .resume(&mut meter, &mut AcceptTerminalEffects)
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
    );
    assert_eq!(meter.usage().total_units(), 3);
}

#[test]
fn wider_construction_prefixes_replay_codec_order_mutations_and_exact_fuel() {
    for (source, prefix_length, root_length) in [
        (UNIT_AFFINE_WIDER_CONSTRUCTION_PREFIX_SOURCE, 3_usize, 4_u64),
        (
            UNIT_AFFINE_DEEPER_CONSTRUCTION_PREFIX_SOURCE,
            4_usize,
            5_u64,
        ),
        (
            UNIT_AFFINE_DEEPEST_CONSTRUCTION_PREFIX_SOURCE,
            5_usize,
            6_u64,
        ),
        (UNIT_AFFINE_SEVEN_CONSTRUCTION_PREFIX_SOURCE, 6_usize, 7_u64),
        (UNIT_AFFINE_EIGHT_CONSTRUCTION_PREFIX_SOURCE, 7_usize, 8_u64),
        (UNIT_AFFINE_NINE_CONSTRUCTION_PREFIX_SOURCE, 8_usize, 9_u64),
        (UNIT_AFFINE_TEN_CONSTRUCTION_PREFIX_SOURCE, 9_usize, 10_u64),
        (
            UNIT_AFFINE_ELEVEN_CONSTRUCTION_PREFIX_SOURCE,
            10_usize,
            11_u64,
        ),
        (
            UNIT_AFFINE_TWELVE_CONSTRUCTION_PREFIX_SOURCE,
            11_usize,
            12_u64,
        ),
        (
            UNIT_AFFINE_THIRTEEN_CONSTRUCTION_PREFIX_SOURCE,
            12_usize,
            13_u64,
        ),
        (
            UNIT_AFFINE_FOURTEEN_CONSTRUCTION_PREFIX_SOURCE,
            13_usize,
            14_u64,
        ),
        (
            UNIT_AFFINE_FIFTEEN_CONSTRUCTION_PREFIX_SOURCE,
            14_usize,
            15_u64,
        ),
        (
            UNIT_AFFINE_SIXTEEN_CONSTRUCTION_PREFIX_SOURCE,
            15_usize,
            16_u64,
        ),
        (
            UNIT_AFFINE_SEVENTEEN_CONSTRUCTION_PREFIX_SOURCE,
            16_usize,
            17_u64,
        ),
        (
            UNIT_AFFINE_EIGHTEEN_CONSTRUCTION_PREFIX_SOURCE,
            17_usize,
            18_u64,
        ),
        (
            UNIT_AFFINE_NINETEEN_CONSTRUCTION_PREFIX_SOURCE,
            18_usize,
            19_u64,
        ),
        (
            UNIT_AFFINE_TWENTY_CONSTRUCTION_PREFIX_SOURCE,
            19_usize,
            20_u64,
        ),
        (
            UNIT_AFFINE_TWENTY_ONE_CONSTRUCTION_PREFIX_SOURCE,
            20_usize,
            21_u64,
        ),
        (
            UNIT_AFFINE_TWENTY_TWO_CONSTRUCTION_PREFIX_SOURCE,
            21_usize,
            22_u64,
        ),
        (
            UNIT_AFFINE_TWENTY_THREE_CONSTRUCTION_PREFIX_SOURCE,
            22_usize,
            23_u64,
        ),
        (
            UNIT_AFFINE_TWENTY_FOUR_CONSTRUCTION_PREFIX_SOURCE,
            23_usize,
            24_u64,
        ),
        (
            UNIT_AFFINE_TWENTY_FIVE_CONSTRUCTION_PREFIX_SOURCE,
            24_usize,
            25_u64,
        ),
        (
            UNIT_AFFINE_TWENTY_SIX_CONSTRUCTION_PREFIX_SOURCE,
            25_usize,
            26_u64,
        ),
    ] {
        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = resolve(ResolutionRequest::new(&syntax)).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let checked = lower_typed_trees(typed, &CheckingRequest::settled()).expect("check");
        let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::cleanup_prefix")
            .unwrap_or_else(|error| {
                panic!("construction prefix of length {prefix_length} failed to lower: {error:?}")
            });
        let machine = &lowered.semantic_module.machines[0];
        let locals = machine
            .structural_places
            .iter()
            .filter_map(|place| match place.kind {
                semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
                    declaration_ordinal,
                    structural_type,
                    construction: Some(construction),
                } => Some((place.id, declaration_ordinal, structural_type, construction)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(locals.len(), prefix_length);
        assert!(
            locals.iter().enumerate().all(
                |(index, (_, ordinal, _, construction))| usize::try_from(*ordinal) == Ok(index)
                    && usize::try_from(construction.index) == Ok(index)
                    && construction.root_structural_type == locals[0].3.root_structural_type
            )
        );
        assert!(matches!(
            lowered
                .semantic_module
                .structural_types
                .iter()
                .find(|declaration| declaration.id == locals[0].3.root_structural_type)
                .expect("construction root declaration")
                .shape,
            terminal_psi::StructuralTypeShape::FixedArray { element, length }
                if element == locals[0].2 && length == root_length
        ));
        assert_eq!(machine.blocks[0].operations.len(), prefix_length);
        assert!(machine.blocks[0].operations.iter().enumerate().all(
            |(index, operation)| matches!(
                operation.kind,
                terminal_psi::OperationKind::EstablishTrivialAffineLocal { destination }
                    if destination == locals[index].0
            )
        ));
        let Terminator::ReturnUnit {
            trivial_affine_discards,
            ..
        } = &machine.blocks[0].terminator
        else {
            panic!("construction-prefix cleanup must return Unit")
        };
        assert_eq!(
            trivial_affine_discards,
            &locals.iter().rev().map(|local| local.0).collect::<Vec<_>>()
        );

        let semantic =
            encode_module(&lowered.semantic_module).expect("construction semantics encode");
        assert_eq!(decode_module(&semantic).unwrap(), lowered.semantic_module);
        terminal_verifier::verify_module(
            &lowered.semantic_module,
            &lowered.proof_bundle,
            &AdmissionProfile::default(),
        )
        .expect("verifier reconstructs wider construction prefix and cleanup");

        let mut reordered_cleanup = lowered.semantic_module.clone();
        let Terminator::ReturnUnit {
            trivial_affine_discards,
            ..
        } = &mut reordered_cleanup.machines[0].blocks[0].terminator
        else {
            unreachable!()
        };
        trivial_affine_discards.swap(0, 1);
        assert!(terminal_verifier::validate_module_representation(&reordered_cleanup).is_err());

        let mut missing_establishment = lowered.semantic_module.clone();
        missing_establishment.machines[0].blocks[0]
            .operations
            .remove(1);
        assert!(terminal_verifier::validate_module_representation(&missing_establishment).is_err());

        let mut wrong_index = lowered.semantic_module.clone();
        let semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
            construction: Some(construction),
            ..
        } = &mut wrong_index.machines[0].structural_places[2].kind
        else {
            unreachable!()
        };
        construction.index = 1;
        assert!(terminal_verifier::validate_module_representation(&wrong_index).is_err());

        let mut redirected_root = lowered.semantic_module.clone();
        let semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
            construction: Some(construction),
            ..
        } = &mut redirected_root.machines[0].structural_places[prefix_length - 1].kind
        else {
            unreachable!()
        };
        construction.root_structural_type = locals[0].2;
        assert!(terminal_verifier::validate_module_representation(&redirected_root).is_err());

        let mut wrong_root_length = lowered.semantic_module.clone();
        let root = locals[0].3.root_structural_type;
        let terminal_psi::StructuralTypeShape::FixedArray { length, .. } = &mut wrong_root_length
            .structural_types
            .iter_mut()
            .find(|declaration| declaration.id == root)
            .expect("construction root declaration")
            .shape
        else {
            unreachable!()
        };
        *length = root_length - 1;
        assert!(terminal_verifier::validate_module_representation(&wrong_root_length).is_err());

        let mut wider_root_length = lowered.semantic_module.clone();
        let terminal_psi::StructuralTypeShape::FixedArray { length, .. } = &mut wider_root_length
            .structural_types
            .iter_mut()
            .find(|declaration| declaration.id == root)
            .expect("construction root declaration")
            .shape
        else {
            unreachable!()
        };
        *length = root_length + 1;
        assert!(terminal_verifier::validate_module_representation(&wider_root_length).is_err());

        if root_length == 26 {
            let mut fenced_successor = lowered.semantic_module.clone();
            let machine = &mut fenced_successor.machines[0];
            let mut successor_place = *machine
                .structural_places
                .last()
                .expect("construction-prefix place");
            successor_place.id = semantic_vocabulary::PlaceId::new(
                machine
                    .structural_places
                    .iter()
                    .map(|place| place.id.get())
                    .max()
                    .expect("construction-prefix place")
                    + 1,
            )
            .expect("successor place");
            let semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
                declaration_ordinal,
                construction: Some(construction),
                ..
            } = &mut successor_place.kind
            else {
                unreachable!()
            };
            *declaration_ordinal = 25;
            construction.index = 25;

            let block = &mut machine.blocks[0];
            let mut successor_operation = block
                .operations
                .last()
                .expect("construction-prefix establishment")
                .clone();
            successor_operation.id = semantic_vocabulary::OperationId::new(
                block
                    .operations
                    .iter()
                    .map(|operation| operation.id.get())
                    .max()
                    .expect("construction-prefix operation")
                    + 1,
            )
            .expect("successor operation");
            let terminal_psi::OperationKind::EstablishTrivialAffineLocal { destination } =
                &mut successor_operation.kind
            else {
                unreachable!()
            };
            *destination = successor_place.id;
            block.operations.push(successor_operation);
            let Terminator::ReturnUnit {
                trivial_affine_discards,
                ..
            } = &mut block.terminator
            else {
                unreachable!()
            };
            trivial_affine_discards.insert(0, successor_place.id);
            machine.structural_places.push(successor_place);

            let terminal_psi::StructuralTypeShape::FixedArray { length, .. } =
                &mut fenced_successor
                    .structural_types
                    .iter_mut()
                    .find(|declaration| declaration.id == root)
                    .expect("construction root declaration")
                    .shape
            else {
                unreachable!()
            };
            *length = 27;
            assert!(terminal_verifier::validate_module_representation(&fenced_successor).is_err());
        }

        let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .expect("construction proof encodes");
        let mut execution = TerminalExecution::start_artifact(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[],
            TerminalStructuralInputs::default(),
        )
        .expect("wider construction-prefix artifact starts");
        let mut meter = TerminalFuelMeter::with_allowance(0);
        for expected_usage in 0..prefix_length + 1 {
            assert!(matches!(
                execution
                    .resume(&mut meter, &mut AcceptTerminalEffects)
                    .unwrap(),
                TerminalExecutionStatus::SponsorExhausted(_)
            ));
            assert_eq!(
                meter.usage().total_units(),
                u64::try_from(expected_usage).expect("bounded fuel usage")
            );
            meter.replenish(1).unwrap();
        }
        assert_eq!(
            execution
                .resume(&mut meter, &mut AcceptTerminalEffects)
                .unwrap(),
            TerminalExecutionStatus::Complete(TerminalExecutionResult::Unit)
        );
        assert_eq!(
            meter.usage().total_units(),
            u64::try_from(prefix_length + 1).expect("bounded fuel usage")
        );
    }
}

#[test]
fn result_bearing_boundary_receipt_verifies_and_commits_only_after_success() {
    let checked = checked_result_boundary_source();
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "Root::enter")
        .expect("result-bearing boundary custody should lower");
    let module = &lowered.semantic_module;
    assert_eq!(module.boundary_machines.len(), 1);
    assert_eq!(
        module.boundary_machines[0].result,
        terminal_psi::BoundaryMachineResult::Scalar(semantic_vocabulary::ScalarType::Boolean)
    );
    let operation = &module.machines[0].blocks[0].operations[0];
    assert!(operation.result.scalar().is_some());
    let terminal_psi::OperationKind::BoundaryCall {
        completion_receipts,
        ..
    } = &operation.kind
    else {
        panic!("result-bearing call must remain a terminal boundary operation")
    };
    assert_eq!(completion_receipts.len(), 1);

    let semantic = encode_module(module).expect("result boundary semantics encode");
    assert_eq!(
        decode_module(&semantic).expect("result boundary semantics decode"),
        *module
    );
    let proof = encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
        .expect("result boundary proof encodes");
    terminal_verifier::verify_module(module, &lowered.proof_bundle, &AdmissionProfile::default())
        .expect("result-bearing boundary custody verifies");
    let mut mismatched_result = module.clone();
    mismatched_result.boundary_machines[0].result = terminal_psi::BoundaryMachineResult::Unit;
    assert!(matches!(
        terminal_verifier::validate_module_representation(&mismatched_result),
        Err(terminal_verifier::ModuleError::BoundaryCallResultMismatch {
            operation: rejected,
            ..
        }) if rejected == operation.id
    ));
    let parameter = &module.machines[0].structural_parameters[0];
    let mut execution = TerminalExecution::start_artifact(
        &semantic,
        &proof,
        &AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs {
            arguments: &[TerminalStructuralValue {
                opaque_identity: 0x005e_771e,
                structural_type: parameter.structural_type,
                qualifications: Vec::new(),
                path: Vec::new(),
            }],
            ..Default::default()
        },
    )
    .expect("result boundary artifact starts");
    let initial_claims = execution.live_claim_frontier().collect::<Vec<_>>();
    assert_eq!(initial_claims.len(), 1);
    let mut meter = TerminalFuelMeter::unbounded();
    let mut rejecting = ResultBoundaryHandler { reject: true };
    assert!(matches!(
        execution.resume(&mut meter, &mut rejecting),
        Err(TerminalInterpretError::EffectRejected { operation: rejected, .. })
            if rejected == operation.id
    ));
    assert_eq!(
        execution.live_claim_frontier().collect::<Vec<_>>(),
        initial_claims
    );
    assert!(execution.effects().is_empty());

    let mut accepting = ResultBoundaryHandler { reject: false };
    assert_eq!(
        execution
            .resume(&mut meter, &mut accepting)
            .expect("accepted boundary result resumes"),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
            TerminalScalarValue::Boolean(true)
        ))
    );
    assert_eq!(execution.live_claim_frontier().count(), 0);
    assert_eq!(execution.effects().len(), 1);
}
