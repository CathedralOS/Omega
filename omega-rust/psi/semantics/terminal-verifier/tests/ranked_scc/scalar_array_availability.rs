//! Cyclic machines cannot establish an activation-owned array payload slot
//! anywhere: the shared cycle eligibility fence admits only the enumerated
//! operation families, whether the construction sits in the preheader or
//! inside the ranked component.

use super::{
    BlockId, ModuleError, Operation, OperationId, OperationKind, OperationResult, PlaceId,
    StructuralMultiplicity, StructuralPlaceDeclaration, StructuralPlaceKind,
    StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape, ValueId, id,
    ranked_countdown, validate_module_representation,
};
#[test]
fn cyclic_machine_rejects_scalar_array_establishment_at_the_eligibility_fence() {
    for block_position in [0, 2] {
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
            machine.blocks[block_position].operations.push(Operation {
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
            // The natural countdown carries no special exemption: every cyclic
            // machine answers the same eligibility fence before its ranking
            // record is even consulted.
            assert_eq!(
                validate_module_representation(&module),
                Err(ModuleError::ControlCycle(id(2, BlockId::new))),
            );
        }
    }
}
