//! Subslice result, bounds, source, and execution custody after optimization.

use super::*;

#[test]
fn surviving_subslice_cannot_reuse_proof_after_endpoint_drift() {
    let verified = byte_reads::verified_byte_operation(true);
    validate_verified_psi_optimization_unit(&verified).unwrap();
    let (input, baseline) = verified.into_parts();
    for operand in 0..2 {
        let mut changed = baseline.clone();
        let node = &mut changed.functions[0].blocks[1].nodes[0];
        let AbstractOperation::ByteSequenceSubslice { start, end, .. } = &mut node.operation else {
            panic!("subslice")
        };
        if operand == 0 {
            *start = id(5, ValueId::new);
        } else {
            *end = id(5, ValueId::new);
        }
        node.uses[operand].value = id(5, ValueId::new);
        refresh_identity(&mut changed);
        // Both replacement endpoints are dominating u64 parameters. The
        // immutable Terminal operation, not scalar typing, rejects the forgery.
        optimization_unit_semantics::validate_psi_optimization_unit(&changed).unwrap();
        assert!(matches!(
            validate_transformed_psi_optimization_unit(&input, &changed),
            Err(OptimizationUnitValidationError::OperationObligationOwnerMismatch { .. })
        ));
    }
}

#[test]
fn subslice_requires_exact_length_result_contract_and_fuel() {
    let verified = byte_reads::verified_byte_operation(true);
    validate_verified_psi_optimization_unit(&verified).unwrap();
    let (input, baseline) = verified.into_parts();
    for mutation in 0..9 {
        let mut changed = baseline.clone();
        let node = &mut changed.functions[0].blocks[1].nodes[0];
        let AbstractOperation::ByteSequenceSubslice { result, length, .. } = &mut node.operation
        else {
            panic!("subslice")
        };
        match mutation {
            0 => {
                *length = id(5, ValueId::new);
                node.uses[2].value = *length;
            }
            1 => result.multiplicity = terminal_psi::StructuralMultiplicity::Affine,
            2 => result.place = id(1, PlaceId::new),
            3 => result.structural_type = id(9, StructuralTypeId::new),
            4 => node.fuel.clear(),
            5 => node.uses.truncate(2),
            6 => changed.functions[0].facts.clear(),
            7 => changed.accepted_obligation_facts.clear(),
            8 => {
                let block = &mut changed.functions[0].blocks[1];
                block.nodes.swap(0, 1);
                // Reconcile scalar metadata so descriptor availability, not
                // stale instruction positions, is the rejected contract.
                for (position, node) in block.nodes.iter_mut().enumerate() {
                    for definition in &mut node.definitions {
                        definition.site = optimization_unit::ValueDefinitionSite::Node {
                            block: block.id,
                            node: position as u32,
                        };
                    }
                    for used in &mut node.uses {
                        used.node = position as u32;
                    }
                }
            }
            _ => panic!("bounded mutations"),
        }
        refresh_identity(&mut changed);
        assert!(
            validate_transformed_psi_optimization_unit(&input, &changed).is_err(),
            "mutation {mutation}"
        );
        if mutation == 0 {
            assert!(matches!(
                optimization_unit_semantics::validate_psi_optimization_unit(&changed),
                Err(OptimizationUnitValidationError::InvalidByteSequenceSubslice { .. })
            ));
        }
        if mutation == 8 {
            let actual = optimization_unit_semantics::validate_psi_optimization_unit(&changed);
            assert!(
                matches!(
                    actual,
                    Err(OptimizationUnitValidationError::StructuralPlaceNotAvailable { .. })
                ),
                "unexpected reordered descriptor result: {actual:?}"
            );
        }
    }
}

#[test]
fn subslice_and_length_cannot_substitute_another_immutable_source() {
    let verified = byte_reads::verified_byte_operation(true);
    let (input, mut changed) = verified.into_parts();
    let function = &mut changed.functions[0];
    let AbstractOperation::ByteSequenceLength { source, .. } =
        &mut function.blocks[0].nodes[0].operation
    else {
        panic!("source length")
    };
    *source = id(2, PlaceId::new);
    let AbstractOperation::ByteSequenceSubslice { source, .. } =
        &mut function.blocks[1].nodes[0].operation
    else {
        panic!("subslice")
    };
    *source = id(2, PlaceId::new);
    refresh_identity(&mut changed);
    optimization_unit_semantics::validate_psi_optimization_unit(&changed).unwrap();
    assert!(validate_transformed_psi_optimization_unit(&input, &changed).is_err());
}

#[test]
fn subslice_length_witness_must_dominate_even_for_the_identical_source() {
    let verified = byte_reads::verified_byte_operation(true);
    let (input, mut changed) = verified.into_parts();
    let nodes = &mut changed.functions[0].blocks[1].nodes;
    let AbstractOperation::ByteSequenceSubslice { length, .. } = &mut nodes[0].operation else {
        panic!("subslice")
    };
    *length = id(6, ValueId::new);
    nodes[0].uses[2].value = id(6, ValueId::new);
    let AbstractOperation::ByteSequenceLength { source, .. } = &mut nodes[1].operation else {
        panic!("later length")
    };
    *source = id(1, PlaceId::new);
    refresh_identity(&mut changed);
    assert!(matches!(
        optimization_unit_semantics::validate_psi_optimization_unit(&changed),
        Err(OptimizationUnitValidationError::UseBeforeDefinition { .. })
    ));
    assert!(validate_transformed_psi_optimization_unit(&input, &changed).is_err());
}

#[test]
fn measured_subslice_cannot_replace_its_descriptor_with_another_valid_view() {
    let verified = byte_reads::verified_byte_operation(true);
    validate_verified_psi_optimization_unit(&verified).unwrap();
    let (input, mut changed) = verified.into_parts();
    let AbstractOperation::ByteSequenceLength { source, .. } =
        &mut changed.functions[0].blocks[1].nodes[1].operation
    else {
        panic!("subslice length")
    };
    *source = id(1, PlaceId::new);
    refresh_identity(&mut changed);
    // The replacement has the same immutable byte-view type and is available.
    // Only the original descriptor/length join authorizes its retained equation.
    optimization_unit_semantics::validate_psi_optimization_unit(&changed).unwrap();
    assert!(matches!(
        validate_transformed_psi_optimization_unit(&input, &changed),
        Err(OptimizationUnitValidationError::StructuralCatalogMismatch { .. })
    ));
}
