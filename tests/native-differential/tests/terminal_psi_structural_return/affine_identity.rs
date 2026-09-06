//! Plain affine identity returns use the existing assigned structural-return route.

use super::*;

fn artifact(wide: bool, array: bool) -> (Vec<u8>, Vec<u8>) {
    let source = if array && wide {
        "data Token { value: u64; }
         data Root {} machine Root::forward(value: [Token; 2]) -> [Token; 2] { value }"
    } else if array {
        "data Token { value: u64; }
         data Root {} machine Root::forward(value: [Token; 1]) -> [Token; 1] { value }"
    } else if wide {
        "data Token { value: u64; }
         data Pair { left: Token; right: Token; }
         data Root {} machine Root::forward(value: Pair) -> Pair { value }"
    } else {
        "data Token { value: u64; }
         data Root {} machine Root::forward(value: Token) -> Token { value }"
    };
    encode_identity(source)
}

fn encode_identity(source: &str) -> (Vec<u8>, Vec<u8>) {
    let tokens = Lexer::new(source).tokenize().unwrap();
    let syntax = parse_syntax_trees(&tokens).unwrap();
    let resolved = lower_syntax_trees(&syntax).unwrap();
    let typed = lower_symbol_resolved_trees(&resolved).unwrap();
    let checked = lower_typed_trees(typed).unwrap();
    let lowered = lower_machine(&checked, "Root::forward").expect("authored affine identity");
    (
        encode_module(&lowered.semantic_module).unwrap(),
        encode_proof_bundle(&lowered.proof_bundle).unwrap(),
    )
}

#[test]
fn plain_affine_identity_reaches_native_publication_and_host_execution() {
    check_identity(false, false);
}

#[test]
fn wide_affine_identity_preserves_both_direct_aggregate_fragments() {
    check_identity(true, false);
}

#[test]
fn fixed_array_affine_identity_uses_the_same_native_return_contract() {
    check_identity(false, true);
    check_identity(true, true);
}

#[test]
fn scalar_prefixed_affine_identity_retains_its_native_contract() {
    let (semantic, proof) = encode_identity(
        "data Token { value: u64; } data Root {}
         machine Root::forward(side: u64, value: Token) -> Token { value }",
    );
    let plan = lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default()).unwrap();
    for case in target_cases() {
        let target = lower_to_target_operations(&plan, case.target).unwrap();
        let TargetOperation::ReturnStructuralParameter {
            scalar_parameters, ..
        } = &target.functions[0].operation
        else {
            panic!("existing scalar-prefixed identity route")
        };
        assert_eq!(scalar_parameters.len(), 1);
        let assigned = assign_registers(&target).unwrap();
        let emitted = emit_machine_code(&assigned).unwrap();
        let object = build_object_artifact(&emitted).unwrap();
        let image = emit_executable_image(&object, 3).unwrap();
        let installation =
            build_installation_record(&image, ProfileDecisionId::new(1).unwrap()).unwrap();
        validate_installation_record(&installation, &image).unwrap();
        assert_eq!(
            decode_installation_record(&encode_installation_record(&installation).unwrap()),
            Ok(installation)
        );
    }
}

fn check_identity(wide: bool, array: bool) {
    let (semantic, proof) = artifact(wide, array);
    let module = decode_module(&semantic).unwrap();
    let proof_bundle = terminal_codec::decode_proof_bundle(&proof).unwrap();
    verify_module(&module, &proof_bundle, &AdmissionProfile::default())
        .expect("independent Terminal verification");
    let [machine] = module.machines.as_slice() else {
        panic!("one identity machine")
    };
    let TerminalMachineResult::Structural(signature) = &machine.result else {
        panic!("structural result")
    };
    assert_eq!(
        signature.multiplicity,
        terminal_psi::StructuralMultiplicity::Affine
    );
    assert!(signature.qualifications.is_empty());
    assert!(signature.projected_qualifications.is_empty());
    assert!(machine.entry_claims.is_empty());
    let Terminator::ReturnStructural {
        edge,
        source,
        returned_claims,
        trivial_affine_discards,
        ..
    } = &machine.blocks[0].terminator
    else {
        panic!("whole input return")
    };
    assert_eq!(*source, machine.structural_parameters[0].place);
    assert!(returned_claims.is_empty());
    assert!(trivial_affine_discards.is_empty());
    assert!(machine.blocks[0].operations.is_empty());
    let plan = lower_artifact_sections(&semantic, &proof, &AdmissionProfile::default()).unwrap();
    assert!(matches!(plan.functions[0].operations.as_slice(),
        [AbstractOperation::ReturnStructural { source: actual, returned_claims, .. }]
        if actual == source && returned_claims.is_empty()));

    for case in target_cases() {
        if wide && case.policy == CallingPolicy::MicrosoftX64 {
            assert!(
                matches!(lower_to_target_operations(&plan, case.target),
                    Err(abstract_operations_to_target_operations::LoweringError::UnsupportedStructuralReturnPlacement(actual))
                    if actual == machine.id),
                "Microsoft's indirect 16-byte result remains outside the direct-fragment slice"
            );
            continue;
        }
        let target_plan = lower_to_target_operations(&plan, case.target)
            .unwrap_or_else(|error| panic!("{:?}, wide {wide}, target: {error:?}", case.target));
        let TargetOperation::ReturnStructuralParameter {
            source: target_source,
            result,
            shape,
            source_placement,
            result_placement,
            returned_claims,
            psi_edge,
            ..
        } = &target_plan.functions[0].operation
        else {
            panic!("existing structural identity route")
        };
        assert_eq!(target_source, &machine.structural_parameters[0]);
        assert_eq!(result, signature);
        assert_eq!(*psi_edge, *edge);
        assert!(returned_claims.is_empty());
        assert_eq!(*shape, ValueShape::integer(if wide { 16 } else { 8 }, 8));
        assert_eq!(source_placement.locations.len(), if wide { 2 } else { 1 });
        assert_eq!(result_placement.locations.len(), if wide { 2 } else { 1 });
        for placement in [source_placement, result_placement] {
            for (position, location) in placement.locations.iter().enumerate() {
                assert!(
                    matches!(location, ValueLocation::Register { value_byte_offset, byte_size: 8, .. }
                    if usize::from(*value_byte_offset) == position * 8)
                );
            }
        }
        if !wide {
            assert_direct_register_placement(source_placement, case.parameter);
            assert_direct_register_placement(result_placement, case.result);
        }
        for mutation in 0..3 {
            let mut changed = target_plan.clone();
            let TargetOperation::ReturnStructuralParameter {
                source,
                result,
                parameters,
                returned_claims,
                result_placement,
                ..
            } = &mut changed.functions[0].operation
            else {
                unreachable!()
            };
            match mutation {
                0 => {
                    source.place = result.place;
                    parameters[0].place = result.place;
                }
                1 => returned_claims.push(semantic_vocabulary::ClaimId::new(90_001).unwrap()),
                2 => result_placement.locations.clear(),
                _ => unreachable!(),
            }
            assert!(
                assign_registers(&changed).is_err(),
                "assignment mutation {mutation}"
            );
        }
        let assigned = assign_registers(&target_plan).expect("identity assignment");
        assert!(matches!(&assigned.functions[0].operation,
            AssignedOperation::ReturnStructuralParameter { source: actual, returned_claims, .. }
            if actual == target_source && returned_claims.is_empty()));
        let emitted = emit_machine_code(&assigned).expect("identity emission");
        let function = &emitted.functions[0];
        assert_eq!(function.provenance.edges, [*edge]);
        assert!(function.provenance.operations.is_empty());
        let custody = function
            .structural_return
            .as_ref()
            .expect("structural return evidence");
        assert_eq!(custody.source, *target_source);
        assert_eq!(custody.result, *signature);
        assert_eq!(custody.source_placement, *source_placement);
        assert_eq!(custody.result_placement, *result_placement);
        assert!(custody.returned_claims.is_empty());
        assert!(custody.trivial_affine_discards.is_empty());
        if !wide {
            assert_eq!(function.bytes, case.bytes);
        }
        for mutation in 0..3 {
            let mut changed = emitted.clone();
            let returned = changed.functions[0].structural_return.as_mut().unwrap();
            match mutation {
                0 => {
                    returned.source.place = returned.result.place;
                    returned.parameters[0].place = returned.result.place;
                }
                1 => returned
                    .returned_claims
                    .push(semantic_vocabulary::ClaimId::new(90_001).unwrap()),
                2 => returned.result_placement.locations.clear(),
                _ => unreachable!(),
            }
            assert!(
                build_object_artifact(&changed).is_err(),
                "return evidence mutation {mutation}"
            );
        }
        let object = build_object_artifact(&emitted).expect("affine identity object");
        assert_eq!(
            object.entry_function().structural_return.as_ref(),
            Some(custody)
        );
        let container = emit_object_container(&object);
        assert_eq!(container.psi, plan.psi);
        assert_eq!(container.output.text_bytes, function.bytes.len());
        let image = emit_executable_image(&object, 3).expect("identity image");
        image_emission::validate_executable_image(&object, &image).unwrap();
        let installation = build_installation_record(&image, ProfileDecisionId::new(1).unwrap())
            .expect("identity installation");
        assert_eq!(installation.structural_returns()[0].returned, *custody);
        validate_installation_record(&installation, &image).unwrap();
        let encoded = encode_installation_record(&installation).unwrap();
        assert_eq!(
            decode_installation_record(&encoded),
            Ok(installation.clone())
        );

        // Locate the exact canonical return row, then change only its edge.
        let mut prefix = machine.id.get().to_le_bytes().to_vec();
        prefix.extend_from_slice(&edge.get().to_le_bytes());
        prefix.extend_from_slice(&0_u32.to_le_bytes()); // scalar parameters
        prefix.extend_from_slice(&1_u32.to_le_bytes()); // structural parameters
        prefix.extend_from_slice(&source.get().to_le_bytes());
        let offsets = encoded
            .windows(prefix.len())
            .enumerate()
            .filter_map(|(offset, bytes)| (bytes == prefix).then_some(offset))
            .collect::<Vec<_>>();
        let [offset] = offsets.as_slice() else {
            panic!("unique installed return row: {offsets:?}")
        };
        let mut changed = encoded.clone();
        changed[offset + 8..offset + 16].copy_from_slice(&90_002_u64.to_le_bytes());
        if let Ok(decoded) = decode_installation_record(&changed) {
            assert!(
                validate_installation_record(&decoded, &image).is_err(),
                "foreign return edge cannot bind the image"
            );
        }
        #[cfg(unix)]
        if !wide && case.target == NativeTarget::host() {
            assert!(host_structural_round_trip(
                &function.bytes,
                OPAQUE_REGION_IDENTITY,
                0
            ));
        }
    }
}
