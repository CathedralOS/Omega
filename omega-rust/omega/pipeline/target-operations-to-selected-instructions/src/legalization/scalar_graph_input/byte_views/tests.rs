use super::*;
use abstract_operations::{AbstractBoundaryResult, AbstractSuccessor};
use abstract_operations_to_target_operations::{
    AdmittedBoundaryExecution, AdmittedBoundarySettlement,
};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, EdgeId, FuelScheduleIdentity, IntegerValue, OperationId, PlaceId,
    StructuralPlaceKind, StructuralTypeId,
};
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralTypeDeclaration, StructuralTypeShape,
};

fn mixed_case_fixture() -> (
    AbstractOperationPlan,
    TargetOperationPlan,
    PsiOptimizationUnit,
) {
    let native = ::target::NativeTarget::linux_x64();
    let (mut plan, _, _) = crate::tests::legalization::structural_case::fixture(native);
    let structural_type = StructuralTypeId::new(50).unwrap();
    plan.structural_types
        .make_mut()
        .push(StructuralTypeDeclaration {
            id: structural_type,
            identity: "mutable bytes".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView),
        });
    let function = &mut plan.functions[0];
    function
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place: PlaceId::new(50).unwrap(),
            position: 0,
            is_self: false,
            structural_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::MutableBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    // Keep the payload live through a signed comparison after its byte-output
    // call. Both successors retain distinct semantic edges.
    function.operations.pop().unwrap();
    function.operations.extend([
        AbstractOperation::IntegerConstant {
            psi_operation: OperationId::new(3).unwrap(),
            result: ValueId::new(11).unwrap(),
            scalar_type: ScalarType::Integer(i32_type()),
            value: IntegerValue::Signed(-1),
        },
        AbstractOperation::IntegerLessThan {
            psi_operation: OperationId::new(4).unwrap(),
            result: ValueId::new(12).unwrap(),
            left: ValueId::new(10).unwrap(),
            right: ValueId::new(11).unwrap(),
        },
        AbstractOperation::Conditional {
            condition: ValueId::new(12).unwrap(),
            when_true: successor(4),
            when_false: successor(6),
        },
    ]);
    let settlements = [
        (
            1,
            target_operations::CompilerBuiltinExecution::HostedReadByte,
            target_operations::HostedReadByteRealization.into(),
        ),
        (
            2,
            target_operations::CompilerBuiltinExecution::HostedWriteByteI32,
            target_operations::HostedWriteByteI32Realization.into(),
        ),
    ]
    .map(
        |(boundary, execution, realization)| AdmittedBoundarySettlement {
            boundary: BoundaryMachineId::new(boundary).unwrap(),
            execution: AdmittedBoundaryExecution::CompilerBuiltin(execution),
            realization,
        },
    );
    let target = abstract_operations_to_target_operations::lower_to_target_operations_with_provider_executions(
        &plan, native, &settlements,
    ).unwrap();
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &plan,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    optimization_unit_semantics::validate_psi_optimization_unit(&unit).unwrap();
    (plan, target, unit)
}

fn successor(edge: u64) -> AbstractSuccessor {
    AbstractSuccessor {
        psi_edge: EdgeId::new(edge).unwrap(),
        target: BlockId::new(40).unwrap(),
        bindings: Vec::new(),
        structural_bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
    }
}

#[test]
fn borrowed_view_roster_composes_with_owned_case_and_signed_payload_comparison() {
    assert_ne!(
        crate::legalization_validator_identity(),
        optimization_core::OptimizationValidatorIdentity::from_canonical_bytes(
            b"omega.terminal-target-legalization-independent-replay.v41",
        )
    );
    let (plan, target, unit) = mixed_case_fixture();
    let legalized = crate::legalize_target_operations(&target, &plan, &unit).unwrap();
    crate::validate_legalized_operations(&target, &plan, &unit, legalized.plan().clone()).unwrap();
}

#[test]
fn borrowed_view_roster_rejects_changed_case_result_custody() {
    let (plan, target, unit) = mixed_case_fixture();
    for mutation in [
        "missing",
        "duplicate",
        "producer",
        "type",
        "place",
        "multiplicity",
    ] {
        let mut changed = unit.functions[0].clone();
        let result_position = changed
            .structural_places
            .iter()
            .position(|place| place.id == PlaceId::new(1).unwrap())
            .unwrap();
        match mutation {
            "missing" => {
                changed.structural_places.remove(result_position);
            }
            "duplicate" => changed
                .structural_places
                .push(changed.structural_places[result_position]),
            "producer" => {
                let StructuralPlaceKind::OperationResult { producer, .. } =
                    &mut changed.structural_places[result_position].kind
                else {
                    panic!("result");
                };
                *producer = OperationId::new(99).unwrap();
            }
            "type" => {
                let StructuralPlaceKind::OperationResult {
                    structural_type, ..
                } = &mut changed.structural_places[result_position].kind
                else {
                    panic!("result");
                };
                *structural_type = StructuralTypeId::new(50).unwrap();
            }
            "place" => changed.structural_places[result_position].id = PlaceId::new(99).unwrap(),
            "multiplicity" => {
                let AbstractOperation::BoundaryCall {
                    result: AbstractBoundaryResult::Structural(result),
                    ..
                } = &mut changed.blocks[0].nodes[0].operation
                else {
                    panic!("read");
                };
                result.multiplicity = StructuralMultiplicity::Linear;
            }
            _ => unreachable!(),
        }
        assert!(
            validate(
                &target.functions[0],
                &plan.functions[0],
                &changed,
                target.target,
                &plan
            )
            .is_err(),
            "{mutation}"
        );
    }
}
