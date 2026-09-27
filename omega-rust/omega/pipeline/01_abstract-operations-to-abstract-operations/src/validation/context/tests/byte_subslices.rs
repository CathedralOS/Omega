//! Subslice result, bounds, source, and execution custody after optimization.

use semantic_vocabulary::StructuralTypeId;

use super::{
    AbstractOperation, OptimizationUnitValidationError, PlaceId, ValueId, byte_reads, id,
    refresh_identity, validate_transformed_psi_optimization_unit,
    validate_verified_psi_optimization_unit,
};

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
