//! Cyclic machines may establish an unrestricted scalar array on every
//! traversal: the shared cycle eligibility fence admits an
//! `EstablishScalarArray` whose result is claim-free and unrestricted —
//! payload replacement is defined for exactly that shape — whether the
//! construction sits in the preheader or inside the ranked component. An
//! affine result still fails the fence: re-establishing a claim-carrying
//! place has no defined semantics.

use super::{
    ModuleError, Operation, OperationId, OperationKind, OperationResult, PlaceId,
    StructuralMultiplicity, StructuralPlaceDeclaration, StructuralPlaceKind,
    StructuralTypeDeclaration, StructuralTypeId, StructuralTypeShape, ValueId, id,
    ranked_countdown, validate_module_representation,
};

fn module_with_array(
    block_position: usize,
    length: u64,
    multiplicity: StructuralMultiplicity,
) -> super::TerminalModule {
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
            multiplicity,
            qualifications: vec![],
            projected_qualifications: vec![],
            claims: vec![],
        }),
        kind: OperationKind::EstablishScalarArray {
            elements: vec![id(1, ValueId::new); length as usize],
        },
    });
    module
}

#[test]
fn cyclic_machine_admits_unrestricted_scalar_array_establishment() {
    for block_position in [0, 2] {
        for length in [0, 1] {
            // The natural countdown carries no special exemption: an
            // unrestricted claim-free establishment verifies in the
            // preheader and inside the ranked component alike.
            assert_eq!(
                validate_module_representation(&module_with_array(
                    block_position,
                    length,
                    StructuralMultiplicity::Unrestricted,
                )),
                Ok(()),
            );
        }
    }
}

#[test]
fn cyclic_machine_still_rejects_affine_scalar_array_establishment() {
    for length in [0, 1] {
        // Inside the ranked component an affine result is not the admitted
        // unrestricted shape — payload re-establishment is undefined for a
        // claim-carrying place — so the establishment's own result-shape
        // check rejects it before cycle eligibility is even consulted.
        assert_eq!(
            validate_module_representation(&module_with_array(
                2,
                length,
                StructuralMultiplicity::Affine,
            )),
            Err(ModuleError::ScalarArrayResultMismatch(id(
                20,
                OperationId::new
            ))),
        );
    }
}
