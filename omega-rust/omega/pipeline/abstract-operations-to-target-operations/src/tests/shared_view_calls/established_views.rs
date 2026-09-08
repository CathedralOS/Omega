//! Raw target projection retains a derived producer; proof admission is separate.
use super::*;

fn subslice_calls() -> AbstractOperationPlan {
    let mut plan = fixture();
    let caller = &mut plan.functions[0];
    let parameter = caller.structural_parameters[0].place;
    let structural_type = caller.structural_parameters[0].structural_type;
    let length = ValueId::new(20).unwrap();
    let place = PlaceId::new(21).unwrap();
    for operation in &mut caller.operations {
        if let AbstractOperation::CallStructuralScalar {
            structural_arguments,
            ..
        } = operation
        {
            structural_arguments[0].place = place;
        }
    }
    caller.operations.insert(
        0,
        AbstractOperation::ByteSequenceLength {
            psi_operation: OperationId::new(20).unwrap(),
            result: AbstractResult {
                value: length,
                scalar_type: caller.parameters[0].scalar_type,
            },
            source: parameter,
        },
    );
    caller.operations.insert(
        1,
        AbstractOperation::ByteSequenceSubslice {
            psi_operation: OperationId::new(21).unwrap(),
            result: terminal_psi::StructuralOperationResult {
                place,
                structural_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
                claims: Vec::new(),
            },
            source: parameter,
            start: caller.parameters[0].value,
            end: length,
            length,
            obligation: semantic_vocabulary::ObligationId::new(21).unwrap(),
        },
    );
    plan
}

#[test]
fn repeated_calls_retain_exact_subslice_producer_without_a_parameter_home() {
    let plan = subslice_calls();
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let lowered = lower_to_target_operations(&plan, target).unwrap();
        let TargetOperation::ReturnIntegerExpression { expression, .. } =
            &lowered.functions[0].operation
        else {
            panic!("scalar call result");
        };
        let TargetIntegerExpression::StructuralCall {
            structural_arguments,
            ..
        } = expression
        else {
            panic!("ordinary shared call");
        };
        assert_eq!(structural_arguments[0].place, PlaceId::new(21).unwrap());
        assert_eq!(
            structural_arguments[0].source,
            target_operations::TargetStructuralArgumentSource::EstablishedByteView {
                psi_operation: OperationId::new(21).unwrap(),
            }
        );
        assert_eq!(
            lowered.functions[0]
                .mixed_structural_scalar_abi
                .as_ref()
                .unwrap()
                .structural_parameters
                .len(),
            1
        );
        for mutation in 0..3 {
            let mut changed = plan.clone();
            let AbstractOperation::ByteSequenceSubslice { result, .. } =
                &mut changed.functions[0].operations[1]
            else {
                panic!("subslice fixture");
            };
            match mutation {
                0 => result.multiplicity = StructuralMultiplicity::Affine,
                1 => result.structural_type = StructuralTypeId::new(99).unwrap(),
                _ => result
                    .claims
                    .push(terminal_psi::StructuralResultClaimBinding {
                        claim: semantic_vocabulary::ClaimId::new(99).unwrap(),
                        path: Vec::new(),
                    }),
            }
            assert!(
                lower_to_target_operations(&changed, target).is_err(),
                "source mutation {mutation}"
            );
        }
    }
}
