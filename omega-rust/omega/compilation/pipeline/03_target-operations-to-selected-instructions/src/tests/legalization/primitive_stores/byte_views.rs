//! Byte-view reads and writes address backing, not the one/two-word descriptor.
use super::*;
use crate::tests::legalization::accept_referenced_obligations;
use selected_instructions::SelectedInstructionKind;
use semantic_vocabulary::ObligationId;
use terminal_psi::{ByteSequenceCarrier, StructuralPathSegment};

#[test]
fn primitive_byte_view_access_reconstructs_backing_and_rejects_forged_geometry() {
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        for carrier in [
            ByteSequenceCarrier::BoundedView {
                access: None,
                capacity: 48,
            },
            ByteSequenceCarrier::BorrowedView {
                access: Some(StructuralAccess::MutableBorrow),
            },
        ] {
            let scalar = integer(IntegerSign::Unsigned, 8);
            let (mut source, _, _) = fixture(native, scalar, true);
            source.structural_types.make_mut()[0].shape =
                StructuralTypeShape::ByteSequence(carrier);
            let function = &mut source.functions[0];
            function.structural_parameters[0].access = StructuralAccess::MutableBorrow;
            let selector = ValueId::new(10).unwrap();
            function.parameters.push(AbstractParameter {
                value: selector,
                scalar_type: integer(IntegerSign::Unsigned, 32),
            });
            for operation in &mut function.operations {
                if let AbstractOperation::WriteOnlyPrimitiveStore {
                    destination, path, ..
                } = operation
                {
                    *destination = function.structural_parameters[0].clone().into();
                    *path = vec![StructuralPathSegment::RuntimeIndex {
                        index: selector,
                        obligation: ObligationId::new(1).unwrap(),
                    }];
                }
            }
            let read = AbstractResult {
                value: ValueId::new(11).unwrap(),
                scalar_type: scalar,
            };
            let result = AbstractResult {
                value: ValueId::new(12).unwrap(),
                scalar_type: scalar,
            };
            function.result = abstract_operations::AbstractFunctionResult::Scalar(result);
            assert!(matches!(
                function.operations.pop(),
                Some(AbstractOperation::ReturnUnit { .. })
            ));
            function
                .operations
                .push(AbstractOperation::PrimitiveScalarRead {
                    psi_operation: OperationId::new(3).unwrap(),
                    result: read,
                    source: function.structural_parameters[0].place,
                    path: vec![StructuralPathSegment::RuntimeIndex {
                        index: selector,
                        obligation: ObligationId::new(2).unwrap(),
                    }],
                });
            function.operations.push(AbstractOperation::Return {
                psi_edge: semantic_vocabulary::EdgeId::new(1).unwrap(),
                result: result.value,
                value: read.value,
                scalar_type: scalar,
                cleanup_actions: Vec::new(),
            });
            let target = abstract_operations_to_target_operations::lower_to_target_operations(
                &source,
                abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
            )
            .unwrap();
            let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
                &source,
                FuelScheduleIdentity::new(1).unwrap(),
            )
            .unwrap();
            let unit = accept_referenced_obligations(unit);
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
            crate::validate_selected_instructions(
                &legalized,
                &constraints,
                environment.physical(),
                environment.constraints(),
                selected.plan().clone(),
            )
            .unwrap();

            let mut changed = legalized.plan().clone();
            let LegalizedScalarInstructionKind::WriteOnlyPrimitiveStore { indices, .. } =
                &mut legalized_store(&mut changed).kind
            else {
                unreachable!()
            };
            assert_eq!(indices[0].stride, 1);
            assert_eq!(
                indices[0].extent,
                match carrier {
                    ByteSequenceCarrier::BoundedView { capacity, .. } => Some(capacity),
                    _ => None,
                }
            );
            indices[0].stride = 8;
            assert!(validate_legalized_operations(&target, &source, &unit, changed).is_err());

            let mut changed = selected.plan().clone();
            let load = changed.functions[0]
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.instructions)
                .find(|instruction| {
                    matches!(
                        instruction.kind,
                        SelectedInstructionKind::Load64 { byte_offset: 0 }
                    )
                })
                .expect("the descriptor's backing pointer is loaded before the byte store");
            load.kind = SelectedInstructionKind::Load64 { byte_offset: 8 };
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

            let mut missing_bounds = unit.clone();
            missing_bounds.accepted_obligation_facts.clear();
            assert!(legalize_target_operations(&target, &source, &missing_bounds).is_err());

            let mut readonly = source.clone();
            readonly.functions[0].structural_parameters[0].access = StructuralAccess::SharedBorrow;
            let parameter = readonly.functions[0].structural_parameters[0].clone();
            for operation in &mut readonly.functions[0].operations {
                if let AbstractOperation::WriteOnlyPrimitiveStore { destination, .. } = operation {
                    *destination = parameter.clone().into();
                }
            }
            assert!(
                abstract_operations_to_target_operations::lower_to_target_operations(
                    &readonly,
                    abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
                )
                .is_err()
            );
        }
    }
}
