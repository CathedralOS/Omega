//! Receiving validation binds each derived descriptor to its checked producer.
use super::*;
use legalized_operations::LegalizedScalarInstructionKind;
use target_operations::{
    TargetByteView, TargetIntegerControl, TargetIntegerExpression, TargetOperation,
};
use target_operations_to_selected_instructions::{
    legalize_target_operations, validate_legalized_operations,
};

#[test]
fn subslice_replay_rejects_substituted_derivation() {
    let module = subslice::suffix_module();
    let proof = subslice::suffix_proof(&module);
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let compiled = byte_view_target(&module, &proof, target);
        let native = compiled.target_operations();
        let abstracted = compiled.optimized().plan();
        let unit = compiled.optimized().unit();
        let admitted = legalize_target_operations(native, abstracted, unit).unwrap();
        for corruption in 0..9 {
            let mut proposed = admitted.plan().clone();
            let row = proposed.scalar_functions[0]
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.instructions)
                .find(|row| {
                    matches!(
                        row.kind,
                        LegalizedScalarInstructionKind::ByteSequenceSubslice { .. }
                    )
                })
                .unwrap();
            let LegalizedScalarInstructionKind::ByteSequenceSubslice {
                result,
                source,
                start,
                end,
                length,
                obligation,
                accepted_fact,
            } = &mut row.kind
            else {
                unreachable!()
            };
            match corruption {
                0 => result.place = *source,
                1 => *source = result.place,
                2 => *start = *end,
                3 => *end = *start,
                4 => *length = *start,
                5 => *obligation = ObligationId::new(99).unwrap(),
                6 => {
                    *accepted_fact =
                        optimization_core::AcceptedObligationFactIdentity::from_bytes([0; 32])
                }
                7 => {
                    result.structural_type = semantic_vocabulary::StructuralTypeId::new(99).unwrap()
                }
                8 => row.fuel.clear(),
                _ => unreachable!(),
            }
            assert!(
                validate_legalized_operations(native, abstracted, unit, proposed).is_err(),
                "subslice substitution {corruption} on {target:?}"
            );
        }
    }
}

fn derived_view(control: &mut TargetIntegerControl) -> &mut TargetByteView {
    match control {
        TargetIntegerControl::Return {
            expression: TargetIntegerExpression::ByteSequenceLength { view, .. },
            ..
        } => view,
        TargetIntegerControl::Conditional { when_true, .. }
        | TargetIntegerControl::ConditionalExpression { when_true, .. } => {
            derived_view(&mut when_true.control)
        }
        _ => panic!("suffix target has a derived length return"),
    }
}

#[test]
fn subslice_receiving_entrance_rejects_substituted_target_view() {
    let module = subslice::suffix_module();
    let compiled = byte_view_target(
        &module,
        &subslice::suffix_proof(&module),
        NativeTarget::host(),
    );
    for corruption in 0..6 {
        let mut native = compiled.target_operations().clone();
        let TargetOperation::ReturnIntegerExpressionConditionalControl { when_true, .. } =
            &mut native.functions[0].operation
        else {
            panic!("guarded suffix")
        };
        let TargetByteView::Subslice {
            psi_operation,
            place,
            source,
            start,
            end,
            length,
            obligation,
        } = derived_view(&mut when_true.control)
        else {
            panic!("derived view")
        };
        match corruption {
            0 => *psi_operation = OperationId::new(99).unwrap(),
            1 => *place = PlaceId::new(3).unwrap(),
            2 => {
                let TargetByteView::Parameter { place, .. } = source.as_mut() else {
                    unreachable!()
                };
                *place = PlaceId::new(99).unwrap();
            }
            3 => *start = end.clone(),
            4 => *length = ValueId::new(10).unwrap(),
            5 => *obligation = ObligationId::new(99).unwrap(),
            _ => unreachable!(),
        }
        assert!(
            legalize_target_operations(
                &native,
                compiled.optimized().plan(),
                compiled.optimized().unit()
            )
            .is_err(),
            "target substitution {corruption}"
        );
    }
}
