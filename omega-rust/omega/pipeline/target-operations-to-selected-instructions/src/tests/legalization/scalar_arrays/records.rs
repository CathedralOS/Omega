use super::*;

fn record_fixture() -> (
    abstract_operations::AbstractOperationPlan,
    target_operations::TargetOperationPlan,
    optimization_unit::PsiOptimizationUnit,
) {
    let (mut source, _, _) = fixture(3);
    let fields = (1..=3)
        .map(|ordinal| semantic_vocabulary::StructuralFieldId::new(ordinal).unwrap())
        .collect::<Vec<_>>();
    source.structural_types.make_mut()[0].shape = StructuralTypeShape::Record {
        fields: fields
            .iter()
            .enumerate()
            .map(
                |(ordinal, field)| terminal_psi::StructuralFieldDeclaration {
                    id: *field,
                    identity: format!("field{ordinal}"),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: terminal_psi::StructuralFieldType::Scalar(ScalarType::Boolean),
                },
            )
            .collect(),
    };
    let O::EstablishScalarArray {
        psi_operation,
        result,
        elements,
    } = source.functions[0].operations[2].clone()
    else {
        panic!("array fixture");
    };
    source.functions[0].operations[2] = O::EstablishScalarRecord {
        psi_operation,
        result,
        fields: fields
            .into_iter()
            .zip(elements)
            .map(|(field, value)| terminal_psi::ScalarRecordFieldValue { field, value })
            .collect(),
    };
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
    (source, target, unit)
}

#[test]
fn scalar_record_target_replays_exact_field_value_and_home_correspondence() {
    let (source, target, unit) = record_fixture();
    let legalized = legalize_target_operations(&target, &source, &unit).unwrap();
    validate_legalized_operations(&target, &source, &unit, legalized.plan().clone()).unwrap();
    for mutation in 0..5 {
        let mut changed = target.clone();
        let TargetUnitOperation::EstablishScalarRecord {
            fields,
            result_home,
            psi_operation,
        } = &mut changed.functions[0].graph.blocks[0].operations[2]
        else {
            panic!("record");
        };
        match mutation {
            0 => fields[0].value = fields[1].value,
            1 => fields.swap(0, 1),
            2 => {
                fields.pop();
            }
            3 => {
                result_home.layout =
                    TargetStructuralHomeLayout::Aggregate(ValueShape::integer(4, 1))
            }
            4 => *psi_operation = OperationId::new(99).unwrap(),
            _ => unreachable!(),
        }
        assert!(
            legalize_target_operations(&changed, &source, &unit).is_err(),
            "target mutation {mutation}"
        );
        assert!(
            validate_legalized_operations(&changed, &source, &unit, legalized.plan().clone())
                .is_err(),
            "target replay mutation {mutation}"
        );
    }
    for mutation in 0..4 {
        let mut changed = legalized.plan().clone();
        let instruction = changed.scalar_functions[0].blocks[0]
            .instructions
            .iter_mut()
            .find(|instruction| matches!(instruction.kind, K::EstablishScalarRecord { .. }))
            .unwrap();
        let K::EstablishScalarRecord {
            fields,
            result,
            shape,
        } = &mut instruction.kind
        else {
            panic!("record");
        };
        match mutation {
            0 => fields[0].value = fields[1].value,
            1 => fields.swap(0, 1),
            2 => *shape = ValueShape::integer(4, 1),
            3 => result.place = PlaceId::new(99).unwrap(),
            _ => unreachable!(),
        }
        assert!(
            validate_legalized_operations(&target, &source, &unit, changed).is_err(),
            "legalized mutation {mutation}"
        );
    }
}
