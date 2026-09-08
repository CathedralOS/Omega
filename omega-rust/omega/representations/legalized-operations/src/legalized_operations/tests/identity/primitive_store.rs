//! Raw identity controls do not confer primitive-store admission.
use super::*;

fn primitive_store_plan() -> LegalizedOperationPlan {
    let mut plan = scalar_call_unit_plan();
    let row = &mut plan.scalar_functions[0].blocks[0].instructions[0];
    row.result = None;
    row.kind = LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore {
        destination: StructuralParameterDeclaration {
            place: id(501),
            position: 0,
            is_self: false,
            structural_type: id(502),
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::WriteOnlyBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        },
        value: abstract_operations::AbstractResult {
            value: id(503),
            scalar_type: ScalarType::Integer(IntegerType::new(IntegerSign::Signed, 32).unwrap()),
        },
        byte_size: 4,
    };
    plan
}

#[test]
fn primitive_store_identity_binds_destination_source_type_and_width() {
    let plan = primitive_store_plan();
    let identity = legalized_operation_plan_identity(&plan);
    assert_eq!(identity, legalized_operation_plan_identity(&plan.clone()));
    for mutation in 0..11 {
        let mut changed = plan.clone();
        let LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore {
            destination,
            value,
            byte_size,
        } = &mut changed.scalar_functions[0].blocks[0].instructions[0].kind
        else {
            panic!("primitive store fixture")
        };
        match mutation {
            0 => destination.place = id(999),
            1 => destination.position = 1,
            2 => destination.is_self = true,
            3 => destination.structural_type = id(999),
            4 => destination.multiplicity = StructuralMultiplicity::Linear,
            5 => destination.access = StructuralAccess::MutableBorrow,
            6 => destination.qualifications.push(id(999)),
            7 => value.value = id(999),
            8 => {
                value.scalar_type =
                    ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 32).unwrap())
            }
            9 => destination.projected_qualifications.push(
                terminal_psi::StructuralPathQualification {
                    path: vec![StructuralPathSegment::FixedIndex(1)],
                    domain: id(998),
                },
            ),
            _ => *byte_size = 8,
        }
        assert_ne!(
            identity,
            legalized_operation_plan_identity(&changed),
            "primitive store mutation {mutation}"
        );
    }
    let mut ieee = plan.clone();
    let LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore { value, .. } =
        &mut ieee.scalar_functions[0].blocks[0].instructions[0].kind
    else {
        panic!("primitive store fixture")
    };
    value.scalar_type = ScalarType::IeeeFloat(semantic_vocabulary::IeeeFloatFormat::Binary32);
    assert_identity_drift(identity, &ieee);
}

#[test]
fn primitive_store_references_its_input_and_is_not_a_field_store() {
    let plan = primitive_store_plan();
    assert!(plan.scalar_functions[0].references_value(id(503)));
    assert!(!plan.scalar_functions[0].references_value(id(999)));
    let mut field = plan.clone();
    let row = &mut field.scalar_functions[0].blocks[0].instructions[0];
    let LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore {
        destination,
        value,
        byte_size,
    } = row.kind.clone()
    else {
        panic!("primitive store fixture")
    };
    row.kind = LegalizedScalarInstructionKind::StructuralScalarFieldStore {
        destination,
        value,
        byte_size,
        path: Vec::new(),
        field: id(504),
        byte_offset: 0,
    };
    assert_identity_drift(legalized_operation_plan_identity(&plan), &field);
    let field_identity = legalized_operation_plan_identity(&field);
    let LegalizedScalarInstructionKind::StructuralScalarFieldStore { destination, .. } =
        &mut field.scalar_functions[0].blocks[0].instructions[0].kind
    else {
        panic!("field store fixture")
    };
    destination
        .projected_qualifications
        .push(terminal_psi::StructuralPathQualification {
            path: vec![StructuralPathSegment::FixedIndex(1)],
            domain: id(998),
        });
    assert_identity_drift(field_identity, &field);
}
