//! Shared Unit graphs retain observations, Boolean homes and original reference calls.
use crate::{
    legalize_target_operations, select_instructions, validate_legalized_operations,
    validate_selected_instructions,
};
use abstract_operations::{
    AbstractBlockEntry, AbstractOperation, AbstractParameter, AbstractResult,
    AbstractStructuralCallResult, AbstractSuccessor,
};
use semantic_vocabulary::{
    BlockId, EdgeId, FuelScheduleIdentity, IntegerSign, IntegerType, MachineId, OperationId,
    PlaceId, ScalarType, StructuralTypeId, ValueId,
};
use target::NativeTarget;
use target_operations::{
    TargetBooleanExpression, TargetControlTerminator, TargetIntegerExpression,
    TargetScalarExpression, TargetUnitOperation,
};
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralArgument, StructuralMultiplicity,
    StructuralParameterDeclaration, StructuralTypeDeclaration, StructuralTypeShape,
};

fn fixture(
    native: NativeTarget,
) -> (
    abstract_operations::AbstractOperationPlan,
    target_operations::TargetOperationPlan,
    optimization_unit::PsiOptimizationUnit,
) {
    let (mut source, _, _) = crate::tests::fixtures::plain_unit::plain_unit_fixture();
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let place = PlaceId::new(10).unwrap();
    let structural_type = StructuralTypeId::new(10).unwrap();
    source
        .structural_types
        .make_mut()
        .push(StructuralTypeDeclaration {
            id: structural_type,
            identity: "shared-view".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: Some(terminal_psi::StructuralAccess::SharedBorrow),
            }),
        });
    let caller = &mut source.functions[0];
    caller.parameters = vec![AbstractParameter {
        value: ValueId::new(100).unwrap(),
        scalar_type,
    }];
    caller.structural_parameters = vec![StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::SharedBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }];
    let mut callee = caller.clone();
    callee.machine = MachineId::new(2).unwrap();
    callee.entry = BlockId::new(20).unwrap();
    callee.block_entries[0].block = callee.entry;
    callee.parameters[0].value = ValueId::new(200).unwrap();
    callee.structural_parameters[0].place = PlaceId::new(20).unwrap();
    callee.operations = vec![AbstractOperation::ReturnUnit {
        psi_edge: EdgeId::new(20).unwrap(),
        cleanup_actions: Vec::new(),
    }];
    caller.block_entries = [(1, 0), (2, 3), (3, 5)]
        .map(|(block, operation_offset)| AbstractBlockEntry {
            structural_parameters: Vec::new(),
            block: BlockId::new(block).unwrap(),
            parameters: Vec::new(),
            operation_offset,
        })
        .to_vec();
    let successor = |identity, target| AbstractSuccessor {
        structural_bindings: Vec::new(),
        psi_edge: EdgeId::new(identity).unwrap(),
        target: BlockId::new(target).unwrap(),
        bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    caller.operations = vec![
        AbstractOperation::ByteSequenceLength {
            psi_operation: OperationId::new(10).unwrap(),
            result: AbstractResult {
                value: ValueId::new(101).unwrap(),
                scalar_type,
            },
            source: place,
        },
        AbstractOperation::IntegerLessThan {
            psi_operation: OperationId::new(11).unwrap(),
            result: ValueId::new(102).unwrap(),
            left: ValueId::new(100).unwrap(),
            right: ValueId::new(101).unwrap(),
        },
        AbstractOperation::Conditional {
            condition: ValueId::new(102).unwrap(),
            when_true: successor(10, 2),
            when_false: successor(11, 3),
        },
        AbstractOperation::CallStructural {
            psi_operation: OperationId::new(12).unwrap(),
            callee: callee.machine,
            arguments: vec![ValueId::new(100).unwrap()],
            structural_arguments: vec![StructuralArgument {
                place,
                access: StructuralAccess::SharedBorrow,
                path: Vec::new(),
            }],
            dynamic_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
            result: AbstractStructuralCallResult::Unit,
        },
        AbstractOperation::ReturnUnit {
            psi_edge: EdgeId::new(12).unwrap(),
            cleanup_actions: Vec::new(),
        },
        AbstractOperation::ReturnUnit {
            psi_edge: EdgeId::new(13).unwrap(),
            cleanup_actions: Vec::new(),
        },
    ];
    source.functions.push(callee);
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
    (source, target, unit)
}

/// One borrowed view whose root arrives through control flow: a non-entry
/// block structural parameter lends the caller's own view to an ordinary call.
/// `block_access` is the block parameter's own access; the argument it lends
/// stays exclusive, so a shared root must not satisfy it.
fn block_parameter_source(
    block_access: StructuralAccess,
) -> abstract_operations::AbstractOperationPlan {
    let (mut source, _, _) = crate::tests::fixtures::plain_unit::plain_unit_fixture();
    let integer = IntegerType::new(IntegerSign::Unsigned, 64).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let place = PlaceId::new(10).unwrap();
    let lent = PlaceId::new(11).unwrap();
    let structural_type = StructuralTypeId::new(10).unwrap();
    source
        .structural_types
        .make_mut()
        .push(StructuralTypeDeclaration {
            id: structural_type,
            identity: "exclusive-view".into(),
            shape: StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BorrowedView {
                access: Some(terminal_psi::StructuralAccess::SharedBorrow),
            }),
        });
    let caller = &mut source.functions[0];
    caller.parameters = vec![AbstractParameter {
        value: ValueId::new(100).unwrap(),
        scalar_type,
    }];
    caller.structural_parameters = vec![StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: StructuralMultiplicity::Unrestricted,
        access: StructuralAccess::MutableBorrow,
        qualifications: Vec::new(),
        projected_qualifications: Vec::new(),
    }];
    let mut callee = caller.clone();
    callee.machine = MachineId::new(2).unwrap();
    callee.entry = BlockId::new(20).unwrap();
    callee.block_entries[0].block = callee.entry;
    callee.parameters[0].value = ValueId::new(200).unwrap();
    callee.structural_parameters[0].place = PlaceId::new(20).unwrap();
    callee.operations = vec![AbstractOperation::ReturnUnit {
        psi_edge: EdgeId::new(20).unwrap(),
        cleanup_actions: Vec::new(),
    }];
    caller.block_entries = vec![
        AbstractBlockEntry {
            structural_parameters: Vec::new(),
            block: BlockId::new(1).unwrap(),
            parameters: Vec::new(),
            operation_offset: 0,
        },
        AbstractBlockEntry {
            structural_parameters: vec![StructuralParameterDeclaration {
                place: lent,
                position: 0,
                is_self: false,
                structural_type,
                multiplicity: StructuralMultiplicity::Unrestricted,
                access: block_access,
                qualifications: Vec::new(),
                projected_qualifications: Vec::new(),
            }],
            block: BlockId::new(2).unwrap(),
            parameters: Vec::new(),
            operation_offset: 3,
        },
        AbstractBlockEntry {
            structural_parameters: Vec::new(),
            block: BlockId::new(3).unwrap(),
            parameters: Vec::new(),
            operation_offset: 5,
        },
    ];
    let successor = |identity, target, lend| AbstractSuccessor {
        structural_bindings: if lend {
            vec![abstract_operations::AbstractStructuralBinding {
                parameter: lent,
                argument: StructuralArgument {
                    place,
                    access: block_access,
                    path: Vec::new(),
                },
            }]
        } else {
            Vec::new()
        },
        psi_edge: EdgeId::new(identity).unwrap(),
        target: BlockId::new(target).unwrap(),
        bindings: Vec::new(),
        trivial_affine_discards: Vec::new(),
    };
    caller.operations = vec![
        AbstractOperation::ByteSequenceLength {
            psi_operation: OperationId::new(10).unwrap(),
            result: AbstractResult {
                value: ValueId::new(101).unwrap(),
                scalar_type,
            },
            source: place,
        },
        AbstractOperation::IntegerLessThan {
            psi_operation: OperationId::new(11).unwrap(),
            result: ValueId::new(102).unwrap(),
            left: ValueId::new(100).unwrap(),
            right: ValueId::new(101).unwrap(),
        },
        AbstractOperation::Conditional {
            condition: ValueId::new(102).unwrap(),
            when_true: successor(10, 2, true),
            when_false: successor(11, 3, false),
        },
        AbstractOperation::CallStructural {
            psi_operation: OperationId::new(12).unwrap(),
            callee: callee.machine,
            arguments: vec![ValueId::new(100).unwrap()],
            structural_arguments: vec![StructuralArgument {
                place: lent,
                access: StructuralAccess::MutableBorrow,
                path: Vec::new(),
            }],
            dynamic_arguments: Vec::new(),
            claim_transfers: Vec::new(),
            requirement_obligations: Vec::new(),
            crash_continuations: Vec::new(),
            result: AbstractStructuralCallResult::Unit,
        },
        AbstractOperation::ReturnUnit {
            psi_edge: EdgeId::new(12).unwrap(),
            cleanup_actions: Vec::new(),
        },
        AbstractOperation::ReturnUnit {
            psi_edge: EdgeId::new(13).unwrap(),
            cleanup_actions: Vec::new(),
        },
    ];
    source.functions.push(callee);
    source
}

/// An exclusive borrow reaches an ordinary call from a block parameter, not
/// only from the machine entrance: control flow establishes the same custody
/// the entrance publishes, and the argument retains that block-parameter
/// source through legalization and its independent replay. A shared root
/// cannot satisfy the exclusive argument, so the physical view alone never
/// authorizes the access.
#[test]
fn an_exclusive_view_lends_through_a_block_parameter_and_a_shared_root_rejects() {
    for native in [NativeTarget::linux_x64(), NativeTarget::linux_arm64()] {
        let source = block_parameter_source(StructuralAccess::MutableBorrow);
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
        let legal = legalize_target_operations(&target, &source, &unit).unwrap();
        validate_legalized_operations(&target, &source, &unit, legal.plan().clone()).unwrap();
        let argument = target.functions[0]
            .graph
            .blocks
            .iter()
            .flat_map(|block| &block.operations)
            .find_map(|operation| match operation {
                TargetUnitOperation::Call { arguments, .. } => arguments.first(),
                _ => None,
            })
            .expect("lent call argument");
        assert_eq!(
            argument.source,
            target_operations::TargetStructuralArgumentSource::BlockParameter {
                block: BlockId::new(2).unwrap(),
                place: PlaceId::new(11).unwrap(),
            },
            "{native:?}"
        );
        assert_eq!(
            argument.access,
            StructuralAccess::MutableBorrow,
            "{native:?}"
        );
        let shared = block_parameter_source(StructuralAccess::SharedBorrow);
        assert!(
            abstract_operations_to_target_operations::lower_to_target_operations(
                &shared,
                abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
            )
            .is_err(),
            "a shared block parameter cannot lend an exclusive argument: {native:?}"
        );
    }
}

#[test]
fn qualified_exclusive_parameters_keep_their_call_contract_and_native_address() {
    let mut source = block_parameter_source(StructuralAccess::MutableBorrow);
    let caller = &mut source.functions[0];
    caller.operations = caller.operations[3..5].to_vec();
    caller.block_entries.truncate(1);
    let domain = semantic_vocabulary::StructuralDomainId::new(1).unwrap();
    for function in &mut source.functions {
        for parameter in &mut function.structural_parameters {
            parameter.qualifications.push(domain);
        }
        for block in &mut function.block_entries {
            block.structural_parameters.clear();
        }
        for operation in &mut function.operations {
            match operation {
                AbstractOperation::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    when_true.structural_bindings.clear();
                    when_false.structural_bindings.clear();
                }
                AbstractOperation::CallStructural {
                    structural_arguments,
                    ..
                } => {
                    structural_arguments[0].place = function.structural_parameters[0].place;
                }
                _ => {}
            }
        }
    }
    let mut unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    unit.structural_domains = vec![terminal_psi::StructuralDomainDeclaration {
        id: domain,
        semantic_domain: semantic_vocabulary::DomainSemanticId::new(1).unwrap(),
        identity: "Utf8".into(),
        carrier: StructuralTypeId::new(10).unwrap(),
        byte_predicate: Some(terminal_psi::ByteSequencePredicate::ValidUtf8),
        content_projection: None,
        establishment_routes: Vec::new(),
    }]
    .into();
    unit.identity = optimization_unit::recompute_psi_optimization_unit_identity(&unit);
    optimization_unit_semantics::validate_psi_optimization_unit(&unit)
        .expect("the exact qualified call is admitted before native geometry");
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
        NativeTarget::windows_x64(),
    ] {
        let target = abstract_operations_to_target_operations::lower_to_target_operations(
            &source,
            abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
        )
        .expect("qualified descriptor transport does not change target layout");
        let legal = legalize_target_operations(&target, &source, &unit)
            .expect("parameter qualifications are not a new native carrier");
        validate_legalized_operations(&target, &source, &unit, legal.plan().clone())
            .expect("native replay rejoins the same qualified parameter");
    }
    let mut missing_predicate = unit.clone();
    missing_predicate.functions[0].structural_parameters[0]
        .qualifications
        .clear();
    missing_predicate.identity =
        optimization_unit::recompute_psi_optimization_unit_identity(&missing_predicate);
    assert!(
        optimization_unit_semantics::validate_psi_optimization_unit(&missing_predicate).is_err(),
        "native geometry cannot discharge a missing callee precondition"
    );
    let mut shared_source = unit;
    shared_source.functions[0].structural_parameters[0].access = StructuralAccess::SharedBorrow;
    shared_source.identity =
        optimization_unit::recompute_psi_optimization_unit_identity(&shared_source);
    assert!(
        optimization_unit_semantics::validate_psi_optimization_unit(&shared_source).is_err(),
        "the same address cannot authorize an exclusive call from shared access"
    );
}

#[test]
fn fused_binding_address_joins_replay_pointer_custody_on_all_hosted_targets() {
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
    ] {
        let mut source = block_parameter_source(access);
        let identity = source.functions[0].structural_parameters[0].structural_type;
        source
            .structural_types
            .make_mut()
            .iter_mut()
            .find(|declaration| declaration.id == identity)
            .unwrap()
            .shape = StructuralTypeShape::FusedServiceBinding;
        source.functions[0].structural_parameters[0].access = access;
        source.functions[1].structural_parameters[0].access = access;
        source.functions[0].operations[0] = AbstractOperation::IntegerConstant {
            psi_operation: OperationId::new(10).unwrap(),
            result: ValueId::new(101).unwrap(),
            scalar_type: source.functions[0].parameters[0].scalar_type,
            value: semantic_vocabulary::IntegerValue::Unsigned(4),
        };
        let AbstractOperation::CallStructural {
            structural_arguments,
            ..
        } = &mut source.functions[0].operations[3]
        else {
            panic!("joined call")
        };
        structural_arguments[0].access = access;
        for native in [
            NativeTarget::linux_x64(),
            NativeTarget::linux_arm64(),
            NativeTarget::macos_arm64(),
            NativeTarget::windows_x64(),
        ] {
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
            let legal = legalize_target_operations(&target, &source, &unit).unwrap();
            validate_legalized_operations(&target, &source, &unit, legal.plan().clone()).unwrap();
            let environment =
                register_environment::baseline_target_register_environment(native).unwrap();
            let constraints = crate::selection_constraints(&legal, &environment);
            let selected = select_instructions(
                &legal,
                &constraints,
                environment.physical(),
                environment.constraints(),
            )
            .unwrap();
            validate_selected_instructions(
                &legal,
                &constraints,
                environment.physical(),
                environment.constraints(),
                selected.plan().clone(),
            )
            .unwrap();
            for mutation in 0..3 {
                let mut changed = target.clone();
                let argument = changed.functions[0]
                    .graph
                    .blocks
                    .iter_mut()
                    .flat_map(|block| &mut block.operations)
                    .find_map(|operation| match operation {
                        TargetUnitOperation::Call { arguments, .. } => arguments.first_mut(),
                        _ => None,
                    })
                    .unwrap();
                assert_eq!(
                    argument.shape,
                    calling_conventions::ValueShape::borrowed_reference(0, 1)
                );
                assert!(!argument.destination.locations.is_empty());
                match mutation {
                    0 => argument.source_byte_offset = 1,
                    1 => argument.place = PlaceId::new(999).unwrap(),
                    _ => argument.access = StructuralAccess::Owned,
                }
                assert!(
                    legalize_target_operations(&changed, &source, &unit).is_err(),
                    "binding join mutation {mutation}: {native:?}"
                );
                assert!(
                    validate_legalized_operations(&changed, &source, &unit, legal.plan().clone())
                        .is_err()
                );
            }
        }
    }
}

#[test]
fn projected_address_join_calls_replay_exact_geometry_and_access() {
    check_projected_address_join(StructuralAccess::SharedBorrow, false);
}

#[test]
fn mutable_record_block_parameters_reuse_borrowed_entry_geometry() {
    check_projected_address_join(StructuralAccess::MutableBorrow, false);
}

#[test]
fn zeroed_local_records_lend_checked_address_joins() {
    for access in [
        StructuralAccess::SharedBorrow,
        StructuralAccess::MutableBorrow,
    ] {
        check_projected_address_join(access, true);
    }
}

fn check_projected_address_join(access: StructuralAccess, local_home: bool) {
    let mut source = block_parameter_source(access);
    let root = source.functions[0].structural_parameters[0].structural_type;
    let element = StructuralTypeId::new(11).unwrap();
    let scalar_type = source.functions[0].parameters[0].scalar_type;
    source
        .structural_types
        .make_mut()
        .iter_mut()
        .find(|declaration| declaration.id == root)
        .unwrap()
        .shape = StructuralTypeShape::Record {
        fields: [(1, "prefix"), (2, "value")]
            .map(
                |(identity, name)| terminal_psi::StructuralFieldDeclaration {
                    id: semantic_vocabulary::StructuralFieldId::new(identity).unwrap(),
                    identity: name.into(),
                    relevance: terminal_psi::BindingRelevance::Relevant,
                    field_type: terminal_psi::StructuralFieldType::Scalar(scalar_type),
                },
            )
            .to_vec(),
    };
    source
        .structural_types
        .make_mut()
        .push(StructuralTypeDeclaration {
            id: element,
            identity: "joined-element".into(),
            shape: StructuralTypeShape::PrimitiveScalar(scalar_type),
        });
    source.functions[0].structural_parameters[0].access = access;
    source.functions[1].structural_parameters[0].access = access;
    source.functions[1].structural_parameters[0].structural_type = element;
    source.functions[0].operations[0] = AbstractOperation::IntegerConstant {
        psi_operation: OperationId::new(10).unwrap(),
        result: ValueId::new(101).unwrap(),
        scalar_type,
        value: semantic_vocabulary::IntegerValue::Unsigned(4),
    };
    let AbstractOperation::CallStructural {
        structural_arguments,
        result: AbstractStructuralCallResult::Unit,
        ..
    } = &mut source.functions[0].operations[3]
    else {
        panic!("joined call")
    };
    structural_arguments[0].access = access;
    structural_arguments[0].path = vec![terminal_psi::StructuralPathSegment::Field("value".into())];

    if local_home {
        let caller = &mut source.functions[0];
        let home = caller.structural_parameters.remove(0).place;
        caller.operations.insert(
            0,
            AbstractOperation::EstablishZeroedStructural {
                psi_operation: OperationId::new(9).unwrap(),
                result: terminal_psi::StructuralOperationResult {
                    place: home,
                    structural_type: root,
                    multiplicity: StructuralMultiplicity::Affine,
                    qualifications: Vec::new(),
                    projected_qualifications: Vec::new(),
                    qualification_establishments: Vec::new(),
                    claims: Vec::new(),
                },
            },
        );
        for entry in caller.block_entries.iter_mut().skip(1) {
            entry.operation_offset += 1;
        }
        for operation in &mut caller.operations {
            if let AbstractOperation::ReturnUnit {
                cleanup_actions, ..
            } = operation
            {
                cleanup_actions.push(terminal_psi::TerminalAffineCleanupAction::DiscardRoot(home));
            }
        }
    }

    for native in [NativeTarget::macos_arm64(), NativeTarget::windows_x64()] {
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
        let legal = legalize_target_operations(&target, &source, &unit).unwrap();
        validate_legalized_operations(&target, &source, &unit, legal.plan().clone()).unwrap();
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = crate::selection_constraints(&legal, &environment);
        let selected = select_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        validate_selected_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
            selected.plan().clone(),
        )
        .unwrap();
        for mutation in 0..4 {
            let mut changed = target.clone();
            let argument = changed.functions[0]
                .graph
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.operations)
                .find_map(|operation| match operation {
                    TargetUnitOperation::Call { arguments, .. } => arguments.first_mut(),
                    _ => None,
                })
                .unwrap();
            assert_eq!(argument.source_byte_offset, 8);
            match mutation {
                0 => argument.source_byte_offset = 0,
                1 => argument.structural_type = root,
                2 => argument.path.clear(),
                _ => {
                    argument.access = if access == StructuralAccess::SharedBorrow {
                        StructuralAccess::MutableBorrow
                    } else {
                        StructuralAccess::SharedBorrow
                    };
                }
            }
            assert!(
                legalize_target_operations(&changed, &source, &unit).is_err(),
                "projected join mutation {mutation}, {access:?}, {native:?}"
            );
        }
    }
}

#[test]
fn shared_unit_graph_observations_and_boolean_homes_replay_on_all_hosted_targets() {
    for native in [
        NativeTarget::linux_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::windows_x64(),
        NativeTarget::macos_arm64(),
    ] {
        let (source, target, unit) = fixture(native);
        let legal = legalize_target_operations(&target, &source, &unit).unwrap();
        validate_legalized_operations(&target, &source, &unit, legal.plan().clone()).unwrap();
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = crate::selection_constraints(&legal, &environment);
        let selected = select_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
        )
        .unwrap();
        validate_selected_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
            selected.plan().clone(),
        )
        .unwrap();
        assert!(selected.plan().functions[0].local_storage_slots.is_empty());
        for mutation in 0..11 {
            let mut changed = target.clone();
            let graph = &mut changed.functions[0].graph;
            match mutation {
                0 => graph.parameters[0].access = StructuralAccess::Owned,
                1 => {
                    graph.parameters[0].placement.shape =
                        calling_conventions::ValueShape::integer(16, 8)
                }
                2 => {
                    let TargetUnitOperation::ScalarDefinition { result_home, .. } =
                        &mut graph.blocks[0].operations[0]
                    else {
                        panic!("length");
                    };
                    result_home.source_value = ValueId::new(999).unwrap();
                }
                3 => {
                    let TargetUnitOperation::ScalarDefinition {
                        expression:
                            TargetScalarExpression::Integer {
                                expression:
                                    TargetIntegerExpression::ByteSequenceLength { source, .. },
                                ..
                            },
                        ..
                    } = &mut graph.blocks[0].operations[0]
                    else {
                        panic!("length");
                    };
                    *source = PlaceId::new(999).unwrap();
                }
                4 => {
                    let TargetUnitOperation::ScalarDefinition {
                        expression:
                            TargetScalarExpression::Boolean(TargetBooleanExpression::IntegerLessThan {
                                right,
                                ..
                            }),
                        ..
                    } = &mut graph.blocks[0].operations[1]
                    else {
                        panic!("compare");
                    };
                    let TargetIntegerExpression::ScalarHome(home) = right.as_mut() else {
                        panic!("length home");
                    };
                    home.defining_operation = OperationId::new(999).unwrap();
                }
                5 => {
                    let TargetControlTerminator::Conditional {
                        condition: TargetBooleanExpression::ScalarHome(home),
                        ..
                    } = &mut graph.blocks[0].terminator
                    else {
                        panic!("Boolean home");
                    };
                    home.shape = calling_conventions::ValueShape::integer(8, 8);
                }
                6 => graph.blocks[0].operations.swap(0, 1),
                7 => {
                    graph.blocks[0].operations.pop();
                }
                8 | 9 => {
                    let TargetUnitOperation::ScalarDefinition { result_home, .. } =
                        &mut graph.blocks[0].operations[0]
                    else {
                        panic!("length");
                    };
                    if mutation == 8 {
                        result_home.scalar_type = ScalarType::Boolean;
                    } else {
                        result_home.shape = calling_conventions::ValueShape::integer(1, 1);
                    }
                }
                _ => {
                    let TargetControlTerminator::Conditional {
                        condition: TargetBooleanExpression::ScalarHome(home),
                        ..
                    } = &mut graph.blocks[0].terminator
                    else {
                        panic!("Boolean home");
                    };
                    home.source_value = ValueId::new(101).unwrap();
                }
            }
            assert!(
                legalize_target_operations(&changed, &source, &unit).is_err(),
                "target mutation {mutation}: {native:?}"
            );
            assert!(
                validate_legalized_operations(&changed, &source, &unit, legal.plan().clone())
                    .is_err()
            );
        }
        let mut changed = selected.plan().clone();
        changed.functions[0].calls.clear();
        assert!(
            validate_selected_instructions(
                &legal,
                &constraints,
                environment.physical(),
                environment.constraints(),
                changed
            )
            .is_err()
        );
    }
}
