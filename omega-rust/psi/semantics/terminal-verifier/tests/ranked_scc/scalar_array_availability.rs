//! Ranked backedges cannot re-establish an activation-owned array payload slot.

use super::*;

#[test]
fn ranked_backedge_rejects_array_reestablishment_but_not_preheader_construction() {
    for length in [0, 1] {
        let mut module = ranked_countdown();
        let array_type = id(1, StructuralTypeId::new);
        let leaf_type = id(2, StructuralTypeId::new);
        let place = id(20, PlaceId::new);
        let operation = id(20, OperationId::new);
        let scalar_type = module.machines[0].parameters[0].scalar_type;
        module.structural_types = vec![
            StructuralTypeDeclaration {
                id: array_type,
                identity: format!("[u32;{length}]"),
                shape: StructuralTypeShape::FixedArray {
                    element: leaf_type,
                    length,
                },
            },
            StructuralTypeDeclaration {
                id: leaf_type,
                identity: "u32".into(),
                shape: StructuralTypeShape::PrimitiveScalar(scalar_type),
            },
        ];
        let machine = &mut module.machines[0];
        machine.structural_places.push(StructuralPlaceDeclaration {
            id: place,
            kind: StructuralPlaceKind::OperationResult {
                producer: operation,
                structural_type: array_type,
            },
        });
        machine.blocks[0].operations.push(Operation {
            static_reach_binding: None,
            id: operation,
            result: OperationResult::Structural(terminal_psi::StructuralOperationResult {
                place,
                structural_type: array_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: vec![],
                projected_qualifications: vec![],
                claims: vec![],
            }),
            kind: OperationKind::EstablishScalarArray {
                elements: vec![id(1, ValueId::new); length as usize],
            },
        });
        validate_module_representation(&module)
            .expect("one preheader construction does not acquire loop disposal debt");

        // The existing unsigned countdown proves the backedge representation
        // before the new full-graph component check sees this repeated slot.
        let machine = &mut module.machines[0];
        let constructor = machine.blocks[0].operations.pop().unwrap();
        machine.blocks[2].operations.push(constructor);
        assert_eq!(
            validate_module_representation(&module),
            Err(ModuleError::ControlCycle(id(3, BlockId::new))),
        );
    }
}
