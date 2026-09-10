//! Aggregate and primitive places retain independent producer and type custody.

use super::*;

#[test]
fn scalar_array_roster_composes_exact_primitive_local_producers() {
    let (mut source, _, _) = fixture(3);
    let local = PlaceId::new(3).unwrap();
    source.functions[0].operations.insert(
        2,
        O::EstablishPrimitiveLocal {
            psi_operation: OperationId::new(4).unwrap(),
            result: terminal_psi::StructuralOperationResult {
                place: local,
                structural_type: StructuralTypeId::new(2).unwrap(),
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            },
            value: abstract_operations::AbstractResult {
                value: ValueId::new(1).unwrap(),
                scalar_type: ScalarType::Boolean,
            },
        },
    );
    let target = abstract_operations_to_target_operations::lower_to_target_operations(
        &source,
        target::NativeTarget::linux_x64(),
    )
    .unwrap();
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    optimization_unit_semantics::validate_psi_optimization_unit(&unit).unwrap();
    let legalized = legalize_target_operations(&target, &source, &unit).unwrap();
    validate_legalized_operations(&target, &source, &unit, legalized.plan().clone()).unwrap();

    for mutation in ["producer", "type", "missing declaration"] {
        let mut changed = unit.clone();
        let function = &mut changed.functions[0];
        let declaration = function
            .structural_places
            .iter_mut()
            .find(|declaration| declaration.id == local)
            .unwrap();
        match mutation {
            "producer" => {
                let semantic_vocabulary::StructuralPlaceKind::OperationResult { producer, .. } =
                    &mut declaration.kind
                else {
                    panic!("primitive local producer");
                };
                *producer = OperationId::new(99).unwrap();
            }
            "type" => {
                let semantic_vocabulary::StructuralPlaceKind::OperationResult {
                    structural_type,
                    ..
                } = &mut declaration.kind
                else {
                    panic!("primitive local type");
                };
                *structural_type = StructuralTypeId::new(1).unwrap();
            }
            "missing declaration" => {
                function.declared_places.remove(&local);
            }
            _ => unreachable!(),
        }
        assert!(
            legalize_target_operations(&target, &source, &changed).is_err(),
            "{mutation}"
        );
        assert!(
            validate_legalized_operations(&target, &source, &changed, legalized.plan().clone())
                .is_err(),
            "{mutation}"
        );
    }
}
