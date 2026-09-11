//! A repeated and nested Unit closure with a Boolean, runtime index and borrowed view.
use abstract_operations::{AbstractOperation, AbstractParameter};
use semantic_vocabulary::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, MachineId, OperationId,
    PlaceId, ScalarType, StructuralTypeId, ValueId,
};
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralArgument, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
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
    let targeted =
        abstract_operations_to_target_operations::lower_to_target_operations(&plan, native)
            .unwrap();
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &plan,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    (plan, targeted, unit)
}
