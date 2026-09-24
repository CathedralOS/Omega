//! A repeated and nested Unit closure with a Boolean, runtime index and borrowed view.
use abstract_operations::{AbstractOperation, AbstractParameter};
use semantic_vocabulary::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, MachineId, OperationId,
    PlaceId, ScalarType, StructuralTypeId, ValueId,
};
use terminal_psi::{
    BindingRelevance, ByteSequenceCarrier, StructuralAccess, StructuralArgument,
    StructuralCaseDeclaration, StructuralFieldDeclaration, StructuralFieldType,
    StructuralMultiplicity, StructuralParameterDeclaration, StructuralTypeDeclaration,
    StructuralTypeShape, TerminalAffineCleanupAction,
};

pub(in crate::tests) fn fixture(
    native: target::NativeTarget,
) -> (
    abstract_operations::AbstractOperationPlan,
    target_operations::TargetOperationPlan,
    optimization_unit::PsiOptimizationUnit,
) {
    let (mut plan, _, _) = super::plain_unit::plain_unit_fixture();
    let structural_type = StructuralTypeId::new(1).unwrap();
    plan.structural_types
        .make_mut()
        .push(StructuralTypeDeclaration {
            id: structural_type,
            identity: "Bytes".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView),
        });
    let skeleton = plan.functions[0].clone();
    plan.functions.clear();
    for machine_number in 1..=3 {
        let mut function = skeleton.clone();
        function.machine = MachineId::new(machine_number).unwrap();
        function.entry = BlockId::new(machine_number).unwrap();
        function.block_entries[0].block = function.entry;
        let index = ValueId::new(machine_number * 10).unwrap();
        let condition = ValueId::new(machine_number * 10 + 1).unwrap();
        let place = PlaceId::new(machine_number).unwrap();
        function.parameters = vec![
            AbstractParameter {
                value: index,
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                ),
            },
            AbstractParameter {
                value: condition,
                scalar_type: ScalarType::Boolean,
            },
        ];
        function.structural_parameters = vec![StructuralParameterDeclaration {
            place,
            position: 0,
            is_self: false,
            structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::SharedBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }];
        function.operations.clear();
        let count = match machine_number {
            1 => 3,
            2 => 1,
            _ => 0,
        };
        for call_number in 0..count {
            function.operations.push(AbstractOperation::CallUnit {
                psi_operation: OperationId::new(machine_number * 10 + call_number).unwrap(),
                callee: MachineId::new(machine_number + 1).unwrap(),
                arguments: vec![index, condition],
                structural_arguments: vec![StructuralArgument {
                    place,
                    access: StructuralAccess::SharedBorrow,
                    path: Vec::new(),
                }],
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            });
        }
        function.operations.push(AbstractOperation::ReturnUnit {
            psi_edge: EdgeId::new(machine_number).unwrap(),
            cleanup_actions: Vec::new(),
        });
        plan.functions.push(function);
    }
    let targeted = abstract_operations_to_target_operations::lower_to_target_operations(
        &plan,
        abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
    )
    .unwrap();
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &plan,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    (plan, targeted, unit)
}

/// A borrowed mixed carrier: the caller lends its `SharedBorrow` mixed
/// parameter verbatim, the callee accepts the same borrowed view.
pub(in crate::tests) fn mixed_borrowed_fixture(
    native: target::NativeTarget,
) -> (
    abstract_operations::AbstractOperationPlan,
    target_operations::TargetOperationPlan,
    optimization_unit::PsiOptimizationUnit,
) {
    mixed_fixture(native, StructuralAccess::SharedBorrow)
}

/// An owned mixed carrier: the caller forwards its `Owned` affine mixed
/// parameter into the call and the callee discards it.
pub(in crate::tests) fn mixed_owned_fixture(
    native: target::NativeTarget,
) -> (
    abstract_operations::AbstractOperationPlan,
    target_operations::TargetOperationPlan,
    optimization_unit::PsiOptimizationUnit,
) {
    mixed_fixture(native, StructuralAccess::Owned)
}

fn mixed_fixture(
    native: target::NativeTarget,
    access: StructuralAccess,
) -> (
    abstract_operations::AbstractOperationPlan,
    target_operations::TargetOperationPlan,
    optimization_unit::PsiOptimizationUnit,
) {
    let (mut plan, _, _) = super::plain_unit::plain_unit_fixture();
    let structural_type = StructuralTypeId::new(1).unwrap();
    let field = |id, identity: &str| StructuralFieldDeclaration {
        id: semantic_vocabulary::StructuralFieldId::new(id).unwrap(),
        identity: identity.into(),
        relevance: BindingRelevance::Relevant,
        field_type: StructuralFieldType::Scalar(ScalarType::Integer(
            IntegerType::new(IntegerSign::Signed, 32).unwrap(),
        )),
    };
    plan.structural_types
        .make_mut()
        .push(StructuralTypeDeclaration {
            id: structural_type,
            identity: "test::MixedCarrier".into(),
            shape: StructuralTypeShape::Mixed {
                fields: vec![field(1, "test::common")],
                cases: vec![
                    StructuralCaseDeclaration {
                        id: semantic_vocabulary::StructuralCaseId::new(1).unwrap(),
                        identity: "test::Empty".into(),
                        fields: Vec::new(),
                    },
                    StructuralCaseDeclaration {
                        id: semantic_vocabulary::StructuralCaseId::new(2).unwrap(),
                        identity: "test::Present".into(),
                        fields: vec![field(2, "test::payload")],
                    },
                ],
            },
        });
    let skeleton = plan.functions[0].clone();
    plan.functions.clear();
    for machine_number in 1..=2 {
        let mut function = skeleton.clone();
        function.machine = MachineId::new(machine_number).unwrap();
        function.entry = BlockId::new(machine_number).unwrap();
        function.block_entries[0].block = function.entry;
        let index = ValueId::new(machine_number * 10).unwrap();
        let condition = ValueId::new(machine_number * 10 + 1).unwrap();
        let place = PlaceId::new(machine_number).unwrap();
        function.parameters = vec![
            AbstractParameter {
                value: index,
                scalar_type: ScalarType::Integer(
                    IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                ),
            },
            AbstractParameter {
                value: condition,
                scalar_type: ScalarType::Boolean,
            },
        ];
        let owned = access == StructuralAccess::Owned;
        function.structural_parameters = vec![StructuralParameterDeclaration {
            place,
            position: 0,
            is_self: false,
            structural_type,
            multiplicity: if owned {
                StructuralMultiplicity::Affine
            } else {
                StructuralMultiplicity::Unrestricted
            },
            access,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        }];
        function.operations.clear();
        if machine_number == 1 {
            function.operations.push(AbstractOperation::CallUnit {
                psi_operation: OperationId::new(10).unwrap(),
                callee: MachineId::new(2).unwrap(),
                arguments: vec![index, condition],
                structural_arguments: vec![StructuralArgument {
                    place,
                    access,
                    path: Vec::new(),
                }],
                claim_transfers: Vec::new(),
                requirement_obligations: Vec::new(),
                crash_continuations: Vec::new(),
            });
        }
        function.operations.push(AbstractOperation::ReturnUnit {
            psi_edge: EdgeId::new(machine_number).unwrap(),
            cleanup_actions: if owned && machine_number == 2 {
                vec![TerminalAffineCleanupAction::DiscardRoot(place)]
            } else {
                Vec::new()
            },
        });
        plan.functions.push(function);
    }
    let targeted = abstract_operations_to_target_operations::lower_to_target_operations(
        &plan,
        abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
    )
    .unwrap();
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &plan,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    (plan, targeted, unit)
}
