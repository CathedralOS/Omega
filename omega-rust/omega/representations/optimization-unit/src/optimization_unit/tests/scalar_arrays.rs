//! Array identity includes ordered leaves even when derived metadata is unchanged.

use super::fixtures::{id, plan};
use crate::{recompute_psi_optimization_unit_identity, reconstruct_psi_optimization_unit_seed};
use abstract_operations::AbstractOperation;
use semantic_vocabulary::{FuelScheduleIdentity, OperationId, PlaceId, StructuralTypeId, ValueId};
use terminal_psi::{StructuralMultiplicity, StructuralOperationResult};

#[test]
fn array_identity_binds_order_count_producer_and_complete_result() {
    let mut plan = plan();
    plan.functions[0].operations[0] = AbstractOperation::EstablishScalarArray {
        psi_operation: id(301, OperationId::new),
        result: StructuralOperationResult {
            place: id(302, PlaceId::new),
            structural_type: id(303, StructuralTypeId::new),
            multiplicity: StructuralMultiplicity::Unrestricted,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
            claims: Vec::new(),
        },
        elements: vec![
            id(304, ValueId::new),
            id(305, ValueId::new),
            id(304, ValueId::new),
        ],
    };
    let unit = reconstruct_psi_optimization_unit_seed(&plan, FuelScheduleIdentity::new(1).unwrap())
        .unwrap();
    for mutation in 0..7 {
        let mut changed = unit.clone();
        let AbstractOperation::EstablishScalarArray {
            psi_operation,
            result,
            elements,
        } = &mut changed.functions[0].blocks[0].nodes[0].operation
        else {
            panic!("array operation");
        };
        match mutation {
            0 => elements.swap(0, 1),
            1 => {
                elements.pop();
            }
            2 => elements.clear(),
            3 => *psi_operation = id(306, OperationId::new),
            4 => result.place = id(307, PlaceId::new),
            5 => result.structural_type = id(308, StructuralTypeId::new),
            6 => result.multiplicity = StructuralMultiplicity::Affine,
            _ => unreachable!(),
        }
        assert_ne!(
            unit.identity,
            recompute_psi_optimization_unit_identity(&changed),
            "mutation {mutation}"
        );
    }
}
