use crate::assignment::shared::{
    CallSignature, CallingPolicy, MachineId, ScalarParameterLocation, TargetOperationPlan, ValueId,
    ValueLocation, evaluate_call_plan,
};
use crate::{AssignmentError, assign_registers};
use omega_assigned_target_operations::{AssignedBooleanControl, AssignedOperation};
use omega_target::NativeTarget;
use omega_target_operations::{
    TargetBooleanControl, TargetConditionalBooleanArm, TargetFunction, TargetOperation,
    TargetStructuralParameter, TerminalPsiProvenance,
};
use psi_core::{EdgeId, PlaceId, StructuralTypeId};
use psi_terminal::{
    SemanticFingerprint, StructuralMultiplicity, TerminalAffineCleanupAction, TerminalPsiIdentity,
    VocabularyMarker,
};

#[test]
fn three_leaf_boolean_cleanup_assignment_retains_exact_edges() {
    for target in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let plan = boolean_cleanup_plan(target);
        let assigned = assign_registers(&plan).expect("assign bounded Boolean cleanup");
        let AssignedOperation::BooleanControlWithCleanup {
            control,
            structural_parameters,
            cleanup_actions,
            ..
        } = &assigned.functions[0].operation
        else {
            panic!("fixture must retain its Boolean cleanup carrier")
        };
        assert_eq!(structural_parameters.len(), 1);
        assert_eq!(cleanup_actions.len(), 1);
        let AssignedBooleanControl::Conditional {
            when_true,
            when_false,
            ..
        } = control
        else {
            panic!("root decision must survive assignment")
        };
        let AssignedBooleanControl::Conditional {
            when_true: nested_true,
            when_false: nested_false,
            ..
        } = when_true.control.as_ref()
        else {
            panic!("true arm must retain the nested decision")
        };
        assert!(matches!(
            nested_true.control.as_ref(),
            AssignedBooleanControl::ReturnImmediate {
                psi_return_edge,
                ..
            } if *psi_return_edge == EdgeId::new(10).unwrap()
        ));
        assert!(matches!(
            nested_false.control.as_ref(),
            AssignedBooleanControl::ReturnParameter {
                psi_return_edge,
                ..
            } if *psi_return_edge == EdgeId::new(11).unwrap()
        ));
        assert!(matches!(
            when_false.control.as_ref(),
            AssignedBooleanControl::ReturnNotParameter {
                psi_return_edge,
                ..
            } if *psi_return_edge == EdgeId::new(12).unwrap()
        ));
    }
}

#[test]
fn finite_boolean_cleanup_accepts_two_leaf_and_wider_trees() {
    let mut two_leaf = boolean_cleanup_plan(NativeTarget::linux_x64());
    let TargetOperation::BooleanControlWithCleanup { control, .. } =
        &mut two_leaf.functions[0].operation
    else {
        unreachable!()
    };
    let TargetBooleanControl::Conditional { when_true, .. } = control else {
        unreachable!()
    };
    when_true.control = Box::new(boolean_immediate_return(13));
    assign_registers(&two_leaf).expect("assign two-leaf Boolean cleanup");

    let mut wider = boolean_cleanup_plan(NativeTarget::linux_x64());
    let location = boolean_cleanup_condition_location(&wider);
    let TargetOperation::BooleanControlWithCleanup { control, .. } =
        &mut wider.functions[0].operation
    else {
        unreachable!()
    };
    let TargetBooleanControl::Conditional { when_true, .. } = control else {
        unreachable!()
    };
    let TargetBooleanControl::Conditional {
        when_true: nested_true,
        ..
    } = when_true.control.as_mut()
    else {
        unreachable!()
    };
    nested_true.control = Box::new(TargetBooleanControl::Conditional {
        condition_source: ValueId::new(1).unwrap(),
        condition_parameter_index: 0,
        condition_location: location,
        when_true: boolean_arm(20, boolean_immediate_return(20)),
        when_false: boolean_arm(21, boolean_immediate_return(21)),
    });
    assign_registers(&wider).expect("assign wider Boolean cleanup");
}

#[test]
fn finite_boolean_cleanup_requires_distinct_return_edges() {
    let mut plan = boolean_cleanup_plan(NativeTarget::linux_x64());
    let TargetOperation::BooleanControlWithCleanup { control, .. } =
        &mut plan.functions[0].operation
    else {
        unreachable!()
    };
    let TargetBooleanControl::Conditional { when_true, .. } = control else {
        unreachable!()
    };
    let TargetBooleanControl::Conditional { when_false, .. } = when_true.control.as_mut() else {
        unreachable!()
    };
    when_false.control = Box::new(boolean_immediate_return(10));
    assert!(matches!(
        assign_registers(&plan),
        Err(AssignmentError::UnsupportedScalarCleanup(_))
    ));
}

#[test]
fn finite_boolean_cleanup_rejects_misaligned_cleanup_signature() {
    let mut plan = boolean_cleanup_plan(NativeTarget::linux_x64());
    let TargetOperation::BooleanControlWithCleanup {
        cleanup_actions, ..
    } = &mut plan.functions[0].operation
    else {
        unreachable!()
    };
    cleanup_actions.clear();
    assert!(matches!(
        assign_registers(&plan),
        Err(AssignmentError::UnsupportedScalarCleanup(_))
    ));
}

fn boolean_cleanup_plan(target: NativeTarget) -> TargetOperationPlan {
    let scalar_shape = omega_calling_conventions::ValueShape::integer(1, 1);
    let structural_shape = omega_calling_conventions::ValueShape::integer(8, 8);
    let call_plan = evaluate_call_plan(
        CallingPolicy::native_for_target(target),
        &CallSignature {
            parameters: vec![scalar_shape, structural_shape],
            result: Some(scalar_shape),
        },
    )
    .expect("bounded Boolean cleanup ABI");
    let [ValueLocation::Register { register, .. }] = call_plan.parameters[0].locations.as_slice()
    else {
        panic!("first Boolean input must have one direct register home")
    };
    let condition_location = ScalarParameterLocation::Register(*register);
    let nested = TargetBooleanControl::Conditional {
        condition_source: ValueId::new(1).unwrap(),
        condition_parameter_index: 0,
        condition_location,
        when_true: boolean_arm(4, boolean_immediate_return(10)),
        when_false: boolean_arm(
            5,
            TargetBooleanControl::ReturnParameter {
                psi_return_edge: EdgeId::new(11).unwrap(),
                source_value: ValueId::new(1).unwrap(),
                parameter_index: 0,
                location: condition_location,
            },
        ),
    };
    let place = PlaceId::new(1).unwrap();
    TargetOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([7; 32]),
        },
        target,
        entry: MachineId::new(1).unwrap(),
        functions: vec![TargetFunction {
            fixed_integer_scalar_abi: None,
            mixed_structural_scalar_abi: None,
            machine: MachineId::new(1).unwrap(),
            attachment: None,
            provenance: TerminalPsiProvenance {
                operations: Vec::new(),
                edges: (1..=5)
                    .chain(10..=12)
                    .map(|edge| EdgeId::new(edge).unwrap())
                    .collect(),
            },
            operation: TargetOperation::BooleanControlWithCleanup {
                control: TargetBooleanControl::Conditional {
                    condition_source: ValueId::new(1).unwrap(),
                    condition_parameter_index: 0,
                    condition_location,
                    when_true: boolean_arm(2, nested),
                    when_false: boolean_arm(
                        3,
                        TargetBooleanControl::ReturnNotParameter {
                            psi_return_edge: EdgeId::new(12).unwrap(),
                            source_value: ValueId::new(1).unwrap(),
                            parameter_index: 0,
                            location: condition_location,
                        },
                    ),
                },
                structural_types: Vec::new(),
                call_plan: call_plan.clone(),
                structural_parameters: vec![TargetStructuralParameter {
                    place,
                    structural_type: StructuralTypeId::new(1).unwrap(),
                    multiplicity: StructuralMultiplicity::Affine,
                    access: psi_terminal::StructuralAccess::Owned,
                    projected_qualifications: Vec::new(),
                    shape: structural_shape,
                    placement: call_plan.parameters[1].clone(),
                }],
                cleanup_actions: vec![TerminalAffineCleanupAction::DiscardRoot(place)],
            },
        }],
    }
}

fn boolean_arm(edge: u64, control: TargetBooleanControl) -> TargetConditionalBooleanArm {
    TargetConditionalBooleanArm {
        psi_edge: EdgeId::new(edge).unwrap(),
        control: Box::new(control),
    }
}

fn boolean_immediate_return(edge: u64) -> TargetBooleanControl {
    TargetBooleanControl::ReturnImmediate {
        psi_return_edge: EdgeId::new(edge).unwrap(),
        source_value: ValueId::new(edge).unwrap(),
        value: edge % 2 == 0,
    }
}

fn boolean_cleanup_condition_location(plan: &TargetOperationPlan) -> ScalarParameterLocation {
    let TargetOperation::BooleanControlWithCleanup { control, .. } = &plan.functions[0].operation
    else {
        unreachable!()
    };
    let TargetBooleanControl::Conditional {
        condition_location, ..
    } = control
    else {
        unreachable!()
    };
    *condition_location
}
