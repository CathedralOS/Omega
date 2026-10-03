//! Atomic legalization and independent replay preserve one event, including
//! its address, ordering, operands and instruction-observed result.
use abstract_operations::{
    AbstractAtomicEvent, AbstractAtomicReadModifyWrite, AbstractOperation, AbstractOperationPlan,
    AbstractResult, AtomicModificationAfter, AtomicReadsFrom,
};
use legalized_operations::{LegalizedOperationPlan, LegalizedScalarInstructionKind};
use semantic_vocabulary::{
    FuelScheduleIdentity, IntegerSign, IntegerType, IntegerValue, OperationId, PlaceId, ScalarType,
    StructuralFieldId, StructuralTypeId, ValueId,
};
use target::NativeTarget;
use target_operations::{TargetOperationPlan, TargetUnitOperation};
use terminal_psi::{
    AtomicAccessEvent, MemoryOrdering, StructuralAccess, StructuralFieldDeclaration,
    StructuralFieldType, StructuralMultiplicity, StructuralParameterDeclaration,
    StructuralPathSegment, StructuralTypeDeclaration, StructuralTypeShape,
};

fn fixture(
    native: NativeTarget,
    nested: bool,
) -> (
    AbstractOperationPlan,
    TargetOperationPlan,
    optimization_unit::PsiOptimizationUnit,
) {
    let (mut source, _, _) = crate::tests::fixtures::plain_unit::plain_unit_fixture();
    let integer = IntegerType::new(IntegerSign::Unsigned, 32).unwrap();
    let scalar_type = ScalarType::Integer(integer);
    let root = StructuralTypeId::new(1).unwrap();
    let field = StructuralFieldId::new(2).unwrap();
    let place = PlaceId::new(1).unwrap();
    let value = ValueId::new(1).unwrap();
    let prior = AbstractResult {
        value: ValueId::new(2).unwrap(),
        scalar_type,
    };
    source
        .structural_types
        .make_mut()
        .push(StructuralTypeDeclaration {
            id: root,
            identity: "Counter".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![
                    StructuralFieldDeclaration {
                        id: StructuralFieldId::new(1).unwrap(),
                        identity: "padding".into(),
                        relevance: terminal_psi::BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                            IntegerType::new(IntegerSign::Unsigned, 8).unwrap(),
                        )),
                    },
                    StructuralFieldDeclaration {
                        id: field,
                        identity: "value".into(),
                        relevance: terminal_psi::BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Atomic(scalar_type),
                    },
                ],
            },
        });
    let mut path = Vec::new();
    let parameter_type = if nested {
        let wrapper = StructuralTypeId::new(2).unwrap();
        source
            .structural_types
            .make_mut()
            .push(StructuralTypeDeclaration {
                id: wrapper,
                identity: "Wrapper".into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![StructuralFieldDeclaration {
                        id: StructuralFieldId::new(3).unwrap(),
                        identity: "counter".into(),
                        relevance: terminal_psi::BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Structural(root),
                    }],
                },
            });
        path.push(StructuralPathSegment::Field("counter".into()));
        wrapper
    } else {
        root
    };
    let function = &mut source.functions[0];
    function.attachment = Some(parameter_type);
    function
        .structural_parameters
        .push(StructuralParameterDeclaration {
            place,
            position: 0,
            is_self: true,
            structural_type: parameter_type,
            multiplicity: StructuralMultiplicity::Unrestricted,
            access: StructuralAccess::MutableBorrow,
            qualifications: Vec::new(),
            projected_qualifications: Vec::new(),
        });
    let location = abstract_operations::atomic::AbstractAtomicLocation {
        root: place,
        path,
        field,
    };
    let store = OperationId::new(2).unwrap();
    let mut operations = vec![
        AbstractOperation::IntegerConstant {
            psi_operation: OperationId::new(1).unwrap(),
            result: value,
            scalar_type,
            value: IntegerValue::Unsigned(5),
        },
        AbstractOperation::AtomicEvent {
            psi_operation: store,
            event: AbstractAtomicEvent::Store {
                location: location.clone(),
                ordering: MemoryOrdering::NoOrdering,
                value,
            },
            reads_from: None,
            modification_after: Some(AtomicModificationAfter::InitialResidency),
        },
        AbstractOperation::AtomicEvent {
            psi_operation: OperationId::new(3).unwrap(),
            event: AbstractAtomicEvent::ReadModifyWrite {
                location: location.clone(),
                ordering: MemoryOrdering::ReceivePublish,
                operation: AbstractAtomicReadModifyWrite::FetchAdd,
                operand: value,
                prior,
            },
            reads_from: Some(AtomicReadsFrom::Write { operation: store }),
            modification_after: Some(AtomicModificationAfter::Write { operation: store }),
        },
        // An ordinary following store consumes the event's observed prior.
        AbstractOperation::StructuralScalarFieldStore {
            range_obligation: None,
            psi_operation: OperationId::new(4).unwrap(),
            destination: function.structural_parameters[0].clone().into(),
            path: location.path,
            field,
            value: prior,
        },
    ];
    operations.append(&mut function.operations);
    function.operations = operations;
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
    optimization_unit_semantics::validate_psi_optimization_unit(&unit).unwrap();
    (source, target, unit)
}

#[test]
fn shared_atomic_cells_keep_authority_through_selection() {
    atomic_cells_keep_authority_through_selection(StructuralAccess::SharedBorrow);
}

#[test]
fn owned_atomic_cells_keep_addressable_storage_through_selection() {
    atomic_cells_keep_authority_through_selection(StructuralAccess::Owned);
}

fn atomic_cells_keep_authority_through_selection(access: StructuralAccess) {
    let native = NativeTarget::macos_arm64();
    let (mut source, _, _) = fixture(native, true);
    let function = &mut source.functions[0];
    function.structural_parameters[0].access = access;
    // Atomic-only uses must retain storage independently of the ordinary store
    // which would otherwise force a captured pointer or addressable home.
    function.operations.retain(|operation| {
        !matches!(
            operation,
            AbstractOperation::StructuralScalarFieldStore { .. }
        )
    });
    let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
        &source,
        FuelScheduleIdentity::new(1).unwrap(),
    )
    .unwrap();
    optimization_unit_semantics::validate_psi_optimization_unit(&unit).unwrap();
    let target = abstract_operations_to_target_operations::lower_to_target_operations(
        &source,
        abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
    )
    .unwrap();
    let legal = crate::legalize_target_operations(&target, &source, &unit).unwrap();
    crate::validate_legalized_operations(&target, &source, &unit, legal.plan().clone()).unwrap();
    let environment = register_environment::baseline_target_register_environment(native).unwrap();
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
}

#[test]
fn atomic_legalization_preserves_nested_location_and_observed_prior() {
    for native in [
        NativeTarget::macos_arm64(),
        NativeTarget::linux_arm64(),
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
    ] {
        for nested in [false, true] {
            let (source, target, unit) = fixture(native, nested);
            let legal = crate::legalize_target_operations(&target, &source, &unit).unwrap();
            crate::validate_legalized_operations(&target, &source, &unit, legal.plan().clone())
                .unwrap();
            let events = legal.plan().scalar_functions[0].blocks[0]
                .instructions
                .iter()
                .filter(|row| {
                    matches!(
                        row.kind,
                        LegalizedScalarInstructionKind::AtomicAccess { .. }
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(events.len(), 2);
            assert!(events[0].result.is_none());
            assert_eq!(events[1].result.unwrap().value, ValueId::new(2).unwrap());
            for row in events {
                let LegalizedScalarInstructionKind::AtomicAccess {
                    location,
                    field_byte_offset,
                    ..
                } = &row.kind
                else {
                    unreachable!()
                };
                assert_eq!(*field_byte_offset, 4);
                assert_eq!(location.path.len(), usize::from(nested));
            }
        }
    }
}

fn legalized_fetch(
    plan: &mut LegalizedOperationPlan,
) -> &mut legalized_operations::LegalizedScalarInstruction {
    plan.scalar_functions[0]
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find(|row| {
            matches!(
                row.kind,
                LegalizedScalarInstructionKind::AtomicAccess {
                    event: AtomicAccessEvent::ReadModifyWrite { .. },
                    ..
                }
            )
        })
        .unwrap()
}

#[test]
fn atomic_selection_preserves_observed_result_and_rejects_changed_memory_custody() {
    use selected_instructions::{AtomicOperation, SelectedInstructionKind};
    for native in [
        NativeTarget::macos_arm64(),
        NativeTarget::linux_arm64(),
        NativeTarget::linux_x64(),
    ] {
        let (source, target, unit) = fixture(native, true);
        let legal = crate::legalize_target_operations(&target, &source, &unit).unwrap();
        let environment =
            register_environment::baseline_target_register_environment(native).unwrap();
        let constraints = crate::selection_constraints(&legal, &environment);
        let selected = crate::select_instructions(
            &legal,
            &constraints,
            environment.physical(),
            environment.constraints(),
        );
        // macos_arm64 and x86-64 publish atomic constraint keys; linux_arm64
        // still publishes none, so its selection must refuse the event.
        if native == NativeTarget::linux_arm64() {
            assert!(
                selected.is_err(),
                "target has no admitted atomic realization"
            );
            continue;
        }
        let selected = selected.unwrap();
        let function = &selected.plan().functions[0];
        let position = function.blocks[0].instructions.iter().position(|instruction| matches!(instruction.kind, SelectedInstructionKind::AtomicAccess { form } if form.operation == AtomicOperation::FetchAdd)).unwrap();
        let instruction = &function.blocks[0].instructions[position];
        let footprints = function
            .memory_accesses
            .iter()
            .filter(|access| access.instruction == instruction.id)
            .collect::<Vec<_>>();
        assert_eq!(
            footprints.len(),
            2,
            "one event has both read and write reach"
        );
        assert!(
            footprints
                .iter()
                .all(|access| access.byte_offset == 4 && access.byte_count == 4)
        );
        for mutation in 0..5 {
            let mut changed = selected.plan().clone();
            let function = &mut changed.functions[0];
            let instruction = &mut function.blocks[0].instructions[position];
            match mutation {
                0 => {
                    let SelectedInstructionKind::AtomicAccess { form } = &mut instruction.kind
                    else {
                        unreachable!()
                    };
                    form.operation = AtomicOperation::Swap;
                }
                1 => {
                    instruction.operands[2].virtual_register =
                        instruction.operands[1].virtual_register
                }
                2 => {
                    function
                        .memory_accesses
                        .iter_mut()
                        .find(|access| access.instruction == instruction.id)
                        .unwrap()
                        .byte_count = 8
                }
                3 => function
                    .memory_accesses
                    .retain(|access| access.instruction != instruction.id),
                _ => instruction.kind = SelectedInstructionKind::Load32 { byte_offset: 4 },
            }
            assert!(
                crate::validate_selected_instructions(
                    &legal,
                    &constraints,
                    environment.physical(),
                    environment.constraints(),
                    changed
                )
                .is_err(),
                "mutation {mutation}"
            );
        }
    }
}

#[test]
fn atomic_replay_rejects_location_order_operand_and_observed_result_substitution() {
    let (source, target, unit) = fixture(NativeTarget::macos_arm64(), true);
    let legal = crate::legalize_target_operations(&target, &source, &unit).unwrap();
    let identity = legalized_operations::legalized_operation_plan_identity(legal.plan());
    for mutation in [
        "offset", "field", "path", "access", "width", "ordering", "operand", "result",
    ] {
        let mut changed = legal.plan().clone();
        let row = legalized_fetch(&mut changed);
        let LegalizedScalarInstructionKind::AtomicAccess {
            location,
            field,
            field_byte_offset,
            scalar_type,
            event,
            ..
        } = &mut row.kind
        else {
            unreachable!()
        };
        let AtomicAccessEvent::ReadModifyWrite {
            ordering, operand, ..
        } = event
        else {
            unreachable!()
        };
        match mutation {
            "offset" => *field_byte_offset += 4,
            "field" => *field = StructuralFieldId::new(1).unwrap(),
            "path" => location.path.clear(),
            "access" => location.access = StructuralAccess::WriteOnlyBorrow,
            "width" => *scalar_type = IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
            "ordering" => *ordering = MemoryOrdering::NoOrdering,
            "operand" => *operand = ValueId::new(2).unwrap(),
            _ => row.result.as_mut().unwrap().value = ValueId::new(1).unwrap(),
        }
        assert_ne!(
            legalized_operations::legalized_operation_plan_identity(&changed),
            identity,
            "{mutation}"
        );
        assert!(
            crate::validate_legalized_operations(&target, &source, &unit, changed).is_err(),
            "{mutation}"
        );
    }
    for mutation in ["offset", "ordering", "result", "operands"] {
        let mut changed = target.clone();
        let row = changed.functions[0].graph.blocks[0]
            .operations
            .iter_mut()
            .find(|row| {
                matches!(
                    row,
                    TargetUnitOperation::AtomicAccess {
                        result_home: Some(_),
                        ..
                    }
                )
            })
            .unwrap();
        let TargetUnitOperation::AtomicAccess {
            field_byte_offset,
            event,
            result_home,
            operands,
            ..
        } = row
        else {
            unreachable!()
        };
        match mutation {
            "offset" => *field_byte_offset += 4,
            "ordering" => {
                *event = AtomicAccessEvent::Load {
                    ordering: MemoryOrdering::NoOrdering,
                }
            }
            "result" => result_home.as_mut().unwrap().source_value = ValueId::new(1).unwrap(),
            _ => operands.clear(),
        }
        assert!(
            crate::legalize_target_operations(&changed, &source, &unit).is_err(),
            "{mutation}"
        );
        assert!(
            crate::validate_legalized_operations(&changed, &source, &unit, legal.plan().clone())
                .is_err(),
            "{mutation}"
        );
    }
}

#[test]
fn atomic_load_swap_exchange_and_fetch_families_keep_their_event_identity() {
    use AbstractAtomicReadModifyWrite as Fetch;
    for family in 0..8 {
        let native = NativeTarget::macos_arm64();
        let (mut source, _, _) = fixture(native, false);
        let AbstractOperation::AtomicEvent {
            event,
            modification_after,
            ..
        } = &mut source.functions[0].operations[2]
        else {
            unreachable!()
        };
        let AbstractAtomicEvent::ReadModifyWrite {
            location,
            operand,
            prior,
            ..
        } = event.clone()
        else {
            unreachable!()
        };
        *event = match family {
            0 => {
                *modification_after = None;
                AbstractAtomicEvent::Load {
                    location,
                    ordering: MemoryOrdering::Receive,
                    result: prior,
                }
            }
            1 => AbstractAtomicEvent::Swap {
                location,
                ordering: MemoryOrdering::GlobalOrder,
                value: operand,
                prior,
            },
            2 => AbstractAtomicEvent::CompareExchange {
                location,
                success: MemoryOrdering::ReceivePublish,
                failure: MemoryOrdering::Receive,
                expected: operand,
                replacement: operand,
                observed: prior,
            },
            _ => AbstractAtomicEvent::ReadModifyWrite {
                location,
                ordering: MemoryOrdering::NoOrdering,
                operation: [
                    Fetch::FetchAdd,
                    Fetch::FetchSub,
                    Fetch::FetchAnd,
                    Fetch::FetchOr,
                    Fetch::FetchXor,
                ][family - 3],
                operand,
                prior,
            },
        };
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
        let legal = crate::legalize_target_operations(&target, &source, &unit).unwrap();
        crate::validate_legalized_operations(&target, &source, &unit, legal.plan().clone())
            .unwrap();
        let row = &legal.plan().scalar_functions[0].blocks[0].instructions[2];
        let LegalizedScalarInstructionKind::AtomicAccess { event, .. } = row.kind else {
            panic!("atomic event")
        };
        assert_eq!(row.result.unwrap().value, prior.value);
        let mut operands = Vec::new();
        row.visit_scalar_operands(|value| operands.push(value));
        assert_eq!(operands, event.operands());
    }
}

#[test]
fn transferred_atomic_location_survives_native_selection_and_replay() {
    use abstract_operations::{AbstractBlockEntry, AbstractStructuralBinding};
    use selected_instructions::{AtomicOperation, SelectedInstructionKind};
    use semantic_vocabulary::{BlockId, EdgeId, StructuralPlaceKind};
    use terminal_psi::StructuralArgument;

    for native in [NativeTarget::macos_arm64(), NativeTarget::linux_x64()] {
        let (mut source, _, _) = fixture(native, true);
        let function = &mut source.functions[0];
        let incoming = function.structural_parameters[0].clone();
        let body = BlockId::new(2).unwrap();
        let mut arrival = incoming.clone();
        arrival.place = PlaceId::new(2).unwrap();
        arrival.is_self = false;
        let AbstractOperation::AtomicEvent { event, .. } = &mut function.operations[2] else {
            unreachable!("fetch follows initial store");
        };
        let AbstractAtomicEvent::ReadModifyWrite { location, .. } = event else {
            unreachable!("fetch event");
        };
        location.root = arrival.place;
        let AbstractOperation::StructuralScalarFieldStore { destination, .. } =
            &mut function.operations[3]
        else {
            unreachable!("store consumes observed prior");
        };
        *destination = arrival.clone().into();
        function.operations.insert(
            2,
            AbstractOperation::Jump {
                psi_edge: EdgeId::new(2).unwrap(),
                target: body,
                bindings: Vec::new(),
                structural_bindings: vec![AbstractStructuralBinding {
                    parameter: arrival.place,
                    argument: StructuralArgument {
                        place: incoming.place,
                        path: Vec::new(),
                        access: incoming.access,
                    },
                }],
                trivial_affine_discards: Vec::new(),
                residual_affine_discards: Vec::new(),
            },
        );
        function.block_entries.push(AbstractBlockEntry {
            block: body,
            operation_offset: 3,
            parameters: Vec::new(),
            structural_parameters: vec![arrival.clone()],
        });
        let unit = optimization_unit::reconstruct_psi_optimization_unit_seed(
            &source,
            FuelScheduleIdentity::new(1).unwrap(),
        )
        .unwrap();
        assert!(
            unit.functions[0]
                .structural_places
                .iter()
                .any(|place| place.id == incoming.place
                    && matches!(
                        place.kind,
                        StructuralPlaceKind::Parameter {
                            position: 0,
                            is_self: true
                        }
                    ))
        );
        assert!(
            unit.functions[0]
                .structural_places
                .iter()
                .any(|place| place.id == arrival.place
                    && matches!(place.kind,
                StructuralPlaceKind::BlockParameter { block, position: 0 } if block == body))
        );
        optimization_unit_semantics::validate_psi_optimization_unit(&unit).unwrap();
        let target = abstract_operations_to_target_operations::lower_to_target_operations(
            &source,
            abstract_operations_to_target_operations::TargetLoweringRequest::new(native),
        )
        .expect("atomic event uses its exact state arrival");
        let legal = crate::legalize_target_operations(&target, &source, &unit).unwrap();
        crate::validate_legalized_operations(&target, &source, &unit, legal.plan().clone())
            .unwrap();
        let mut inspected = legal.plan().clone();
        let fetch = legalized_fetch(&mut inspected);
        let LegalizedScalarInstructionKind::AtomicAccess {
            location,
            field_byte_offset,
            ..
        } = &fetch.kind
        else {
            unreachable!("fetch event");
        };
        assert_eq!(location.place, arrival.place);
        assert_eq!(location.access, StructuralAccess::MutableBorrow);
        assert_eq!(
            location.path,
            vec![StructuralPathSegment::Field("counter".into())]
        );
        assert_eq!(*field_byte_offset, 4);
        assert_eq!(fetch.result.unwrap().value, ValueId::new(2).unwrap());
        for substitution in ["root", "access", "offset"] {
            let mut changed = legal.plan().clone();
            let LegalizedScalarInstructionKind::AtomicAccess {
                location,
                field_byte_offset,
                ..
            } = &mut legalized_fetch(&mut changed).kind
            else {
                unreachable!("fetch event");
            };
            match substitution {
                "root" => location.place = incoming.place,
                "access" => location.access = StructuralAccess::SharedBorrow,
                _ => *field_byte_offset = 0,
            }
            assert!(
                crate::validate_legalized_operations(&target, &source, &unit, changed).is_err(),
                "{substitution}"
            );
        }
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
        let function = &selected.plan().functions[0];
        let fetch = function.blocks.iter().flat_map(|block| &block.instructions)
            .find(|instruction| matches!(instruction.kind, SelectedInstructionKind::AtomicAccess { form } if form.operation == AtomicOperation::FetchAdd))
            .expect("one selected atomic fetch");
        let accesses = function
            .memory_accesses
            .iter()
            .filter(|access| access.instruction == fetch.id)
            .collect::<Vec<_>>();
        assert_eq!(accesses.len(), 2);
        assert!(accesses.iter().all(|access| access.place == arrival.place
            && access.byte_offset == 4
            && access.byte_count == 4));
        let mut changed = selected.plan().clone();
        changed.functions[0]
            .memory_accesses
            .iter_mut()
            .find(|access| access.instruction == fetch.id)
            .unwrap()
            .place = incoming.place;
        assert!(
            crate::validate_selected_instructions(
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
