//! Field identities survive independent once-only scalar observations.
use super::*;
use abstract_operations::AbstractFunctionResult;
use semantic_vocabulary::EdgeId;

#[test]
fn field_observations_replay_parameter_field_offset_and_result() {
    for scalar in [
        integer(IntegerSign::Signed, 8),
        integer(IntegerSign::Unsigned, 64),
        ScalarType::Boolean,
    ] {
        for native in [
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
            NativeTarget::windows_x64(),
        ] {
            let (mut source, _, _) = fixture(native, scalar, false);
            let declaration = &mut source.structural_types.make_mut()[0];
            declaration.shape = StructuralTypeShape::Record {
                fields: [1, 2]
                    .map(|ordinal| StructuralFieldDeclaration {
                        id: StructuralFieldId::new(ordinal).unwrap(),
                        identity: format!("field{ordinal}"),
                        relevance: BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(scalar),
                    })
                    .to_vec(),
            };
            let function = &mut source.functions[0];
            function.parameters.clear();
            function.structural_parameters[0].access = StructuralAccess::SharedBorrow;
            let parameter = function.structural_parameters[0].clone();
            let read = |ordinal| {
                let psi_operation = OperationId::new(ordinal).unwrap();
                let result = ValueId::new(ordinal).unwrap();
                let field = StructuralFieldId::new(ordinal).unwrap();
                if scalar == ScalarType::Boolean {
                    AbstractOperation::BooleanStructuralField {
                        psi_operation,
                        result,
                        source: parameter.place,
                        field,
                    }
                } else {
                    AbstractOperation::IntegerStructuralField {
                        psi_operation,
                        result: AbstractResult {
                            value: result,
                            scalar_type: scalar,
                        },
                        source: parameter.place,
                        field,
                    }
                }
            };
            let result = AbstractResult {
                value: ValueId::new(3).unwrap(),
                scalar_type: scalar,
            };
            function.result = AbstractFunctionResult::Scalar(result);
            function.operations = vec![
                read(1),
                read(2),
                AbstractOperation::Return {
                    psi_edge: EdgeId::new(1).unwrap(),
                    result: result.value,
                    value: ValueId::new(2).unwrap(),
                    scalar_type: scalar,
                    cleanup_actions: Vec::new(),
                },
            ];
            let target = abstract_operations_to_target_operations::lower_to_target_operations(
                &source, native,
            )
            .unwrap();
            let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
                &source,
                FuelScheduleIdentity::new(1).unwrap(),
            )
            .unwrap();
            optimization_unit_semantics::validate_psi_optimization_unit(&unit)
                .unwrap_or_else(|error| panic!("{scalar:?} optimizer: {error:?}"));
            let legalized = legalize_target_operations(&target, &source, &unit).unwrap();
            validate_legalized_operations(&target, &source, &unit, legalized.plan().clone())
                .unwrap();
            let environment =
                register_environment::baseline_target_register_environment(native).unwrap();
            let constraints = crate::selection_constraints(&legalized, &environment);
            let selected = crate::select_instructions(
                &legalized,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
            .unwrap();
            assert_eq!(selected.plan().functions[0].memory_accesses.len(), 2);
            let mut changed = selected.plan().clone();
            let instruction = changed.functions[0].blocks[0]
                .instructions
                .iter_mut()
                .find(|instruction| {
                    instruction.provenance.operations == vec![OperationId::new(2).unwrap()]
                        && matches!(
                            instruction.kind,
                            selected_instructions::SelectedInstructionKind::Load8 { .. }
                                | selected_instructions::SelectedInstructionKind::Load64 { .. }
                        )
                })
                .expect("the second observation has its own exact load");
            match &mut instruction.kind {
                selected_instructions::SelectedInstructionKind::Load8 { byte_offset }
                | selected_instructions::SelectedInstructionKind::Load64 { byte_offset } => {
                    *byte_offset = 0
                }
                _ => unreachable!(),
            }
            assert!(
                crate::validate_selected_instructions(
                    &legalized,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                    changed
                )
                .is_err()
            );
            for mutation in ["field", "access", "source", "result"] {
                let mut changed = target.clone();
                let TargetUnitOperation::StructuralScalarFieldRead {
                    field,
                    source: argument,
                    result,
                    ..
                } = &mut changed.functions[0].graph.blocks[0].operations[1]
                else {
                    panic!("field observation");
                };
                match mutation {
                    "field" => *field = StructuralFieldId::new(1).unwrap(),
                    "access" => argument.access = StructuralAccess::MutableBorrow,
                    "source" => argument.place = PlaceId::new(99).unwrap(),
                    "result" => result.value = ValueId::new(1).unwrap(),
                    _ => unreachable!(),
                }
                assert!(
                    legalize_target_operations(&changed, &source, &unit).is_err(),
                    "{native:?} {scalar:?} {mutation}"
                );
                assert!(
                    validate_legalized_operations(
                        &changed,
                        &source,
                        &unit,
                        legalized.plan().clone()
                    )
                    .is_err(),
                    "{native:?} {scalar:?} {mutation}"
                );
            }
            let mut changed = legalized.plan().clone();
            let LegalizedScalarInstructionKind::StructuralScalarFieldRead { field, .. } =
                &mut changed.scalar_functions[0].blocks[0].instructions[1].kind
            else {
                panic!("field row");
            };
            *field = StructuralFieldId::new(1).unwrap();
            assert!(validate_legalized_operations(&target, &source, &unit, changed).is_err());
        }
    }
}
