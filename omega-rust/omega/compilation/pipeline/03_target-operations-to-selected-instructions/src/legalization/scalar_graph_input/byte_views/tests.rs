use super::super::{PsiOptimizationUnit, TargetOperationPlan, ValueId};
use super::{AbstractOperation, AbstractOperationPlan, validate};
use crate::legalization::scalar_graph_input::i32_type;
use abstract_operations::{
    AbstractBoundaryResult, AbstractStructuralCallResult, AbstractSuccessor,
};
use abstract_operations_to_target_operations::{
    AdmittedBoundaryExecution, AdmittedBoundarySettlement,
};
use semantic_vocabulary::{
    BlockId, BoundaryMachineId, EdgeId, FuelScheduleIdentity, IntegerValue, OperationId, PlaceId,
    ScalarType, StructuralPlaceKind, StructuralTypeId,
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
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: Some(terminal_psi::StructuralAccess::SharedBorrow),
            }),
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
    let target = abstract_operations_to_target_operations::lower_to_target_operations(
        &plan,
        abstract_operations_to_target_operations::TargetLoweringRequest {
            target: native,
            settlements: &settlements,
            installation: None,
            native_callbacks: &[],
        },
    )
    .unwrap();
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

fn literal_order_fixture(
    native: ::target::NativeTarget,
    scalar_return: bool,
) -> (
    AbstractOperationPlan,
    TargetOperationPlan,
    PsiOptimizationUnit,
) {
    use abstract_operations::{
        AbstractBlockEntry, AbstractFunction, AbstractFunctionResult, AbstractResult,
    };
    use semantic_vocabulary::MachineId;
    use terminal_psi::{
        SemanticFingerprint, StructuralPlaceDeclaration, TerminalPsiIdentity, VocabularyMarker,
    };

    let machine = MachineId::new(1).unwrap();
    let block = BlockId::new(1).unwrap();
    let structural_type = StructuralTypeDeclaration {
        id: StructuralTypeId::new(1).unwrap(),
        identity: "literal-order::Bytes".into(),
        shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
            access: Some(StructuralAccess::SharedBorrow),
        }),
    };
    let scalar_type = ScalarType::Integer(super::super::u64_type());
    let result = ValueId::new(3).unwrap();
    let mut operations = Vec::new();
    // Place IDs and source declaration ordinals are independent coordinates.
    // The canonical place roster will retain ordinal 1 before ordinal 0.
    for (ordinal, place, bytes) in [(0, 3, vec![97, 98, 99]), (1, 2, Vec::new())] {
        let place = StructuralPlaceDeclaration {
            id: PlaceId::new(place).unwrap(),
            kind: StructuralPlaceKind::ByteSequenceLiteral {
                declaration_ordinal: ordinal,
                structural_type: structural_type.id,
            },
        };
        operations.push(AbstractOperation::EstablishByteSequenceLiteral {
            psi_operation: OperationId::new(u64::from(ordinal) * 2 + 1).unwrap(),
            place,
            structural_type: structural_type.clone(),
            bytes,
            qualifications: Vec::new(),
        });
        operations.push(AbstractOperation::ByteSequenceLength {
            psi_operation: OperationId::new(u64::from(ordinal) * 2 + 2).unwrap(),
            result: AbstractResult {
                value: ValueId::new(u64::from(ordinal) + 1).unwrap(),
                scalar_type,
            },
            source: place.id,
        });
    }
    operations.push(if scalar_return {
        AbstractOperation::Return {
            psi_edge: EdgeId::new(1).unwrap(),
            result,
            value: ValueId::new(2).unwrap(),
            scalar_type,
            cleanup_actions: Vec::new(),
        }
    } else {
        AbstractOperation::ReturnUnit {
            psi_edge: EdgeId::new(1).unwrap(),
            cleanup_actions: Vec::new(),
        }
    });
    let plan = AbstractOperationPlan {
        psi: TerminalPsiIdentity {
            vocabulary_marker: VocabularyMarker::CURRENT,
            program_fingerprint: SemanticFingerprint::from_bytes([0x71; 32]),
        },
        entry: machine,
        structural_types: vec![structural_type.clone()].into(),
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine,
            attachment: None,
            entry: block,
            parameters: Vec::new(),
            structural_parameters: vec![StructuralParameterDeclaration {
                place: PlaceId::new(1).unwrap(),
                position: 0,
                is_self: false,
                structural_type: structural_type.id,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: StructuralAccess::SharedBorrow,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            }],
            result: if scalar_return {
                AbstractFunctionResult::Scalar(AbstractResult {
                    value: result,
                    scalar_type,
                })
            } else {
                AbstractFunctionResult::Unit
            },
            entry_claims: Vec::new(),
            published_service_ceiling: Vec::new(),
            block_entries: vec![AbstractBlockEntry {
                block,
                structural_parameters: Vec::new(),
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations,
        }],
    };
    let target = abstract_operations_to_target_operations::lower_to_target_operations(
        &plan,
        abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
    )
    .unwrap();
    let mut unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &plan,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    // Recreate the place-ID roster published by the native source pipeline;
    // seed reconstruction alone retains operation encounter order.
    unit.functions[0]
        .structural_places
        .sort_by_key(|place| place.id);
    unit.functions[0].declared_places = unit.functions[0]
        .structural_places
        .iter()
        .map(|place| place.id)
        .collect();
    unit.identity = optimization_unit::recompute_psi_optimization_unit_identity(&unit);
    optimization_unit_semantics::validate_psi_optimization_unit(&unit).unwrap();
    (plan, target, unit)
}

#[test]
fn borrowed_float_parameters_compose_with_scalar_results() {
    for native in [
        ::target::NativeTarget::windows_x64(),
        ::target::NativeTarget::linux_x64(),
        ::target::NativeTarget::macos_arm64(),
    ] {
        for access in [
            StructuralAccess::SharedBorrow,
            StructuralAccess::MutableBorrow,
        ] {
            let (mut plan, _, _) = literal_order_fixture(native, true);
            let structural_type = StructuralTypeId::new(2).unwrap();
            plan.structural_types
                .make_mut()
                .push(StructuralTypeDeclaration {
                    id: structural_type,
                    identity: "borrowed-float::f64".into(),
                    shape: StructuralTypeShape::PrimitiveScalar(ScalarType::IeeeFloat(
                        semantic_vocabulary::IeeeFloatFormat::Binary64,
                    )),
                });
            let parameter = &mut plan.functions[0].structural_parameters[0];
            parameter.structural_type = structural_type;
            parameter.access = access;
            let target = abstract_operations_to_target_operations::lower_to_target_operations(
                &plan,
                abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
            )
            .unwrap();
            let mut unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
                &plan,
                FuelScheduleIdentity::new(1).unwrap(),
            )
            .unwrap();
            unit.functions[0]
                .structural_places
                .sort_by_key(|place| place.id);
            unit.functions[0].declared_places = unit.functions[0]
                .structural_places
                .iter()
                .map(|place| place.id)
                .collect();
            unit.identity = optimization_unit::recompute_psi_optimization_unit_identity(&unit);
            optimization_unit_semantics::validate_psi_optimization_unit(&unit).unwrap();
            let legal = crate::legalize_target_operations(&target, &plan, &unit).unwrap();
            crate::validate_legalized_operations(&target, &plan, &unit, legal.plan().clone())
                .unwrap();

            // Removing a result-dependent carrier whitelist must not admit a
            // changed referent, access mode, ownership contract or ABI location.
            for mutation in ["type", "access", "multiplicity", "placement"] {
                let mut changed = target.clone();
                let abi = changed.functions[0]
                    .mixed_structural_scalar_abi
                    .as_mut()
                    .unwrap();
                let parameter = &mut abi.structural_parameters[0];
                match mutation {
                    "type" => parameter.structural_type = StructuralTypeId::new(1).unwrap(),
                    "access" => parameter.access = StructuralAccess::Owned,
                    "multiplicity" => parameter.multiplicity = StructuralMultiplicity::Linear,
                    "placement" => parameter.placement.locations.clear(),
                    _ => unreachable!(),
                }
                assert!(
                    validate(
                        &changed.functions[0],
                        &plan.functions[0],
                        &unit.functions[0],
                        native,
                        &plan
                    )
                    .is_err(),
                    "{mutation}"
                );
                assert!(
                    crate::legalize_target_operations(&changed, &plan, &unit).is_err(),
                    "{mutation}"
                );
                assert!(
                    crate::validate_legalized_operations(
                        &changed,
                        &plan,
                        &unit,
                        legal.plan().clone()
                    )
                    .is_err(),
                    "{mutation}"
                );
            }
        }
    }
}

#[test]
fn borrowed_parameters_compose_with_float_results_and_reject_changed_abi() {
    use abstract_operations::{AbstractFunctionResult, AbstractParameter, AbstractResult};
    use semantic_vocabulary::IeeeFloatFormat;

    for native in [
        ::target::NativeTarget::windows_x64(),
        ::target::NativeTarget::linux_x64(),
        ::target::NativeTarget::linux_arm64(),
        ::target::NativeTarget::macos_arm64(),
    ] {
        for format in [IeeeFloatFormat::Binary32, IeeeFloatFormat::Binary64] {
            let (mut plan, _, _) = literal_order_fixture(native, true);
            let scalar_type = ScalarType::IeeeFloat(format);
            let input = ValueId::new(1).unwrap();
            let result = ValueId::new(2).unwrap();
            let function = &mut plan.functions[0];
            function.parameters = vec![AbstractParameter {
                value: input,
                scalar_type,
            }];
            function.result = AbstractFunctionResult::Scalar(AbstractResult {
                value: result,
                scalar_type,
            });
            function.operations = vec![AbstractOperation::Return {
                psi_edge: EdgeId::new(1).unwrap(),
                result,
                value: input,
                scalar_type,
                cleanup_actions: Vec::new(),
            }];
            // Exercise both ends: reference argument transport is independent
            // of the floating-point return register selected for this call.
            let mut caller = function.clone();
            caller.machine = semantic_vocabulary::MachineId::new(2).unwrap();
            caller.entry = BlockId::new(2).unwrap();
            caller.block_entries[0].block = caller.entry;
            let call_result = ValueId::new(3).unwrap();
            caller.operations = vec![
                AbstractOperation::CallStructural {
                    psi_operation: OperationId::new(1).unwrap(),
                    result: AbstractStructuralCallResult::Scalar(AbstractResult {
                        value: call_result,
                        scalar_type,
                    }),
                    callee: function.machine,
                    arguments: vec![input],
                    structural_arguments: vec![terminal_psi::StructuralArgument {
                        place: caller.structural_parameters[0].place,
                        access: StructuralAccess::SharedBorrow,
                        path: Vec::new(),
                    }],
                    dynamic_arguments: Vec::new(),
                    claim_transfers: Vec::new(),
                    requirement_obligations: Vec::new(),
                    crash_continuations: Vec::new(),
                },
                AbstractOperation::Return {
                    psi_edge: EdgeId::new(2).unwrap(),
                    result,
                    value: call_result,
                    scalar_type,
                    cleanup_actions: Vec::new(),
                },
            ];
            plan.entry = caller.machine;
            plan.functions.push(caller);
            let target = abstract_operations_to_target_operations::lower_to_target_operations(
                &plan,
                abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
            )
            .unwrap();
            abstract_operations_to_target_operations::validate_abstract_to_target_translation(
                &plan, native, &target,
            )
            .unwrap();
            let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
                &plan,
                FuelScheduleIdentity::new(1).unwrap(),
            )
            .unwrap();
            optimization_unit_semantics::validate_psi_optimization_unit(&unit).unwrap();
            let legal = crate::legalize_target_operations(&target, &plan, &unit).unwrap();
            crate::validate_legalized_operations(&target, &plan, &unit, legal.plan().clone())
                .unwrap();
            let environment =
                register_environment::baseline_target_register_environment(native).unwrap();
            let constraints = crate::selection_constraints(&legal, &environment);
            let selected = crate::select_instructions(
                &legal,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
            .unwrap();
            crate::validate_selected_instructions(
                &legal,
                &constraints,
                environment.physical(),
                environment.constraints(),
                selected.plan().clone(),
            )
            .unwrap();

            for mutation in ["result value", "result type", "result placement"] {
                let mut changed = target.clone();
                let result = &mut changed.functions[0]
                    .mixed_structural_scalar_abi
                    .as_mut()
                    .unwrap()
                    .result;
                match mutation {
                    "result value" => result.value = input,
                    "result type" => result.scalar_type = ScalarType::Boolean,
                    "result placement" => result.placement.locations.clear(),
                    _ => unreachable!(),
                }
                assert!(abstract_operations_to_target_operations::validate_abstract_to_target_translation(&plan, native, &changed).is_err(), "{mutation}");
                assert!(
                    crate::legalize_target_operations(&changed, &plan, &unit).is_err(),
                    "{mutation}"
                );
                assert!(
                    crate::validate_legalized_operations(
                        &changed,
                        &plan,
                        &unit,
                        legal.plan().clone()
                    )
                    .is_err(),
                    "{mutation}"
                );
            }
        }
    }
}

#[test]
fn literal_ordinals_are_independent_of_place_roster_order() {
    for native in [
        ::target::NativeTarget::linux_x64(),
        ::target::NativeTarget::macos_arm64(),
    ] {
        for scalar_return in [false, true] {
            let (plan, target, unit) = literal_order_fixture(native, scalar_return);
            let ordinals = unit.functions[0]
                .structural_places
                .iter()
                .filter_map(|place| match place.kind {
                    StructuralPlaceKind::ByteSequenceLiteral {
                        declaration_ordinal,
                        ..
                    } => Some(declaration_ordinal),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(ordinals, [1, 0]);
            let legal = crate::legalize_target_operations(&target, &plan, &unit).unwrap();
            crate::validate_legalized_operations(&target, &plan, &unit, legal.plan().clone())
                .unwrap();
            let environment =
                register_environment::baseline_target_register_environment(native).unwrap();
            let constraints = crate::selection_constraints(&legal, &environment);
            let selected = crate::select_instructions(
                &legal,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
            .unwrap();
            assert_eq!(selected.plan().functions[0].local_storage_slots.len(), 2);
            crate::validate_selected_instructions(
                &legal,
                &constraints,
                environment.physical(),
                environment.constraints(),
                selected.plan().clone(),
            )
            .unwrap();
        }
    }
}

#[test]
fn literal_ordinal_density_and_exact_producer_custody_still_reject() {
    let (plan, target, unit) = literal_order_fixture(::target::NativeTarget::macos_arm64(), false);
    let legal = crate::legalize_target_operations(&target, &plan, &unit).unwrap();
    let source = PlaceId::new(3).unwrap();
    for mutation in [
        "duplicate ordinal",
        "sparse ordinal",
        "producer ordinal",
        "producer type",
        "missing producer",
        "duplicate producer",
    ] {
        let mut changed = unit.clone();
        let function = &mut changed.functions[0];
        match mutation {
            "duplicate ordinal" | "sparse ordinal" => {
                let ordinal = if mutation == "duplicate ordinal" {
                    1
                } else {
                    u32::MAX
                };
                let declaration = function
                    .structural_places
                    .iter_mut()
                    .find(|place| place.id == source)
                    .unwrap();
                let StructuralPlaceKind::ByteSequenceLiteral {
                    declaration_ordinal,
                    ..
                } = &mut declaration.kind
                else {
                    panic!("literal declaration")
                };
                *declaration_ordinal = ordinal;
                let AbstractOperation::EstablishByteSequenceLiteral { place, .. } =
                    &mut function.blocks[0].nodes[0].operation
                else {
                    panic!("literal producer")
                };
                *place = *declaration;
            }
            "producer ordinal" => {
                let AbstractOperation::EstablishByteSequenceLiteral { place, .. } =
                    &mut function.blocks[0].nodes[0].operation
                else {
                    panic!("literal producer")
                };
                let StructuralPlaceKind::ByteSequenceLiteral {
                    declaration_ordinal,
                    ..
                } = &mut place.kind
                else {
                    panic!("literal declaration")
                };
                *declaration_ordinal = 1;
            }
            "producer type" => {
                let AbstractOperation::EstablishByteSequenceLiteral {
                    structural_type, ..
                } = &mut function.blocks[0].nodes[0].operation
                else {
                    panic!("literal producer")
                };
                structural_type.id = StructuralTypeId::new(99).unwrap();
            }
            "missing producer" => {
                function.blocks[0].nodes.remove(0);
            }
            "duplicate producer" => {
                let producer = function.blocks[0].nodes[0].clone();
                function.blocks[0].nodes.push(producer);
            }
            _ => unreachable!(),
        }
        if !matches!(mutation, "duplicate ordinal" | "sparse ordinal") {
            assert!(
                super::super::literals::declaration_producer(function, source).is_none(),
                "{mutation}"
            );
        }
        changed.identity = optimization_unit::recompute_psi_optimization_unit_identity(&changed);
        assert!(
            optimization_unit_semantics::validate_psi_optimization_unit(&changed).is_err(),
            "{mutation}"
        );
        assert!(
            crate::legalize_target_operations(&target, &plan, &changed).is_err(),
            "{mutation}"
        );
        let mut proposed = legal.plan().clone();
        proposed.optimization_unit = changed.identity;
        assert!(
            crate::validate_legalized_operations(&target, &plan, &changed, proposed).is_err(),
            "{mutation}"
        );
    }
}
