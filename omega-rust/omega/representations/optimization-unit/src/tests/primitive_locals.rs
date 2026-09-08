//! Canonical primitive-storage identities bind all executable operands.

use super::fixtures::{id, plan};
use crate::{recompute_psi_optimization_unit_identity, reconstruct_psi_optimization_unit_seed};
use abstract_operations::{AbstractOperation as O, AbstractResult};
use semantic_vocabulary::{
    FuelScheduleIdentity, OperationId, PlaceId, ScalarType, StructuralTypeId, ValueId,
};
use terminal_psi::{StructuralMultiplicity, StructuralOperationResult};

#[test]
fn primitive_storage_identity_binds_producer_place_type_and_value() {
    let value = AbstractResult {
        value: id(301, ValueId::new),
        scalar_type: ScalarType::Boolean,
    };
    let result = StructuralOperationResult {
        place: id(302, PlaceId::new),
        structural_type: id(303, StructuralTypeId::new),
        multiplicity: StructuralMultiplicity::Unrestricted,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
        claims: Vec::new(),
    };
    for operation in [
        O::EstablishPrimitiveLocal {
            psi_operation: id(304, OperationId::new),
            result: result.clone(),
            value,
        },
        O::PrimitiveLocalStore {
            psi_operation: id(305, OperationId::new),
            destination: result.place,
            value,
        },
        O::PrimitiveScalarRead {
            psi_operation: id(306, OperationId::new),
            source: result.place,
            result: value,
        },
    ] {
        let mut plan = plan();
        plan.functions[0].operations[0] = operation;
        let unit =
            reconstruct_psi_optimization_unit_seed(&plan, FuelScheduleIdentity::new(1).unwrap())
                .unwrap();
        for field in 0..4 {
            let mut changed = unit.clone();
            match &mut changed.functions[0].blocks[0].nodes[0].operation {
                O::EstablishPrimitiveLocal {
                    psi_operation,
                    result,
                    value,
                } => match field {
                    0 => *psi_operation = id(307, OperationId::new),
                    1 => result.place = id(308, PlaceId::new),
                    2 => result.structural_type = id(309, StructuralTypeId::new),
                    _ => value.value = id(310, ValueId::new),
                },
                O::PrimitiveLocalStore {
                    psi_operation,
                    destination,
                    value,
                } => match field {
                    0 => *psi_operation = id(307, OperationId::new),
                    1 => *destination = id(308, PlaceId::new),
                    2 => {
                        value.scalar_type = ScalarType::Integer(
                            semantic_vocabulary::IntegerType::new(
                                semantic_vocabulary::IntegerSign::Unsigned,
                                8,
                            )
                            .unwrap(),
                        )
                    }
                    _ => value.value = id(310, ValueId::new),
                },
                O::PrimitiveScalarRead {
                    psi_operation,
                    source,
                    result,
                } => match field {
                    0 => *psi_operation = id(307, OperationId::new),
                    1 => *source = id(308, PlaceId::new),
                    2 => {
                        result.scalar_type = ScalarType::Integer(
                            semantic_vocabulary::IntegerType::new(
                                semantic_vocabulary::IntegerSign::Unsigned,
                                8,
                            )
                            .unwrap(),
                        )
                    }
                    _ => result.value = id(310, ValueId::new),
                },
                _ => panic!("primitive operation"),
            }
            assert_ne!(
                unit.identity,
                recompute_psi_optimization_unit_identity(&changed)
            );
        }
    }
}
