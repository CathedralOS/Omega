use super::*;

#[test]
fn unit_fixed_array_call_selects_exact_forty_byte_native_placements() {
    let root = MachineId::new(1).unwrap();
    let callee = MachineId::new(2).unwrap();
    let element_type = StructuralTypeId::new(1).unwrap();
    let structural_type = StructuralTypeId::new(2).unwrap();
    let root_place = PlaceId::new(1).unwrap();
    let callee_place = PlaceId::new(2).unwrap();
    let u64_type =
        ScalarType::Integer(IntegerType::new(psi_core::IntegerSign::Unsigned, 64).unwrap());
    let structural_types = vec![
        StructuralTypeDeclaration {
            id: element_type,
            identity: "Acknowledgement".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![
                    StructuralFieldDeclaration {
                        id: StructuralFieldId::new(1).unwrap(),
                        identity: "value".into(),
                        relevance: psi_terminal::BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(u64_type),
                    },
                    StructuralFieldDeclaration {
                        id: StructuralFieldId::new(2).unwrap(),
                        identity: "proof".into(),
                        relevance: psi_terminal::BindingRelevance::Erased,
                        field_type: StructuralFieldType::Erased {
                            type_identity: "named(name(example::Evidence))".into(),
                        },
                    },
                ],
            },
        },
        StructuralTypeDeclaration {
            id: structural_type,
            identity: "[Acknowledgement; 5]".into(),
            shape: StructuralTypeShape::FixedArray {
                element: element_type,
                length: 5,
            },
        },
    ];
    let parameter = |place| StructuralParameterDeclaration {
        place,
        position: 0,
        is_self: false,
        structural_type,
        multiplicity: StructuralMultiplicity::Linear,
        access: StructuralAccess::Owned,
        qualifications: Vec::new(),
    };
    let unit_function = |machine, place, operations| AbstractFunction {
        machine,
        attachment: None,
        entry: BlockId::new(machine.get()).unwrap(),
        parameters: Vec::new(),
        structural_parameters: vec![parameter(place)],
        result: AbstractFunctionResult::Unit,
        entry_claims: Vec::new(),
        published_service_ceiling: Vec::new(),
        block_entries: vec![omega_abstract_operations::AbstractBlockEntry {
            block: BlockId::new(machine.get()).unwrap(),
            parameters: Vec::new(),
            operation_offset: 0,
        }],
        operations,
    };
    let plan = AbstractOperationPlan {
        psi: identity(),
        entry: root,
        structural_types,
        boundary_machines: Vec::new(),
        provider_candidates: Vec::new(),
        functions: vec![
            unit_function(
                root,
                root_place,
                vec![
                    AbstractOperation::CallUnit {
                        psi_operation: OperationId::new(1).unwrap(),
                        callee,
                        structural_arguments: vec![psi_terminal::StructuralArgument {
                            place: root_place,
                            access: StructuralAccess::Owned,
                            path: Vec::new(),
                        }],
                        claim_transfers: Vec::new(),
                    },
                    AbstractOperation::ReturnUnit {
                        psi_edge: EdgeId::new(1).unwrap(),
                        cleanup_actions: Vec::new(),
                    },
                ],
            ),
            unit_function(
                callee,
                callee_place,
                vec![AbstractOperation::ReturnUnit {
                    psi_edge: EdgeId::new(2).unwrap(),
                    cleanup_actions: Vec::new(),
                }],
            ),
        ],
    };

    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let lowered = lower_to_target_operations(&plan, target).unwrap();
        let TargetOperation::UnitBody(root) = &lowered.functions[0].operation else {
            panic!("root must remain Unit")
        };
        assert_eq!(root.parameters[0].shape, ValueShape::integer(40, 8));
        let TargetUnitOperation::Call { arguments, .. } = &root.operations[0] else {
            panic!("root must call helper")
        };
        assert!(arguments[0].path.is_empty());
        assert_eq!(arguments[0].shape, ValueShape::integer(40, 8));
    }

    let linux = lower_to_target_operations(&plan, NativeTarget::linux_x64()).unwrap();
    let TargetOperation::UnitBody(linux_root) = &linux.functions[0].operation else {
        panic!("root must remain Unit")
    };
    assert_eq!(linux_root.parameters[0].shape, ValueShape::integer(40, 8));
    assert_eq!(linux_root.parameters[0].placement.locations.len(), 5);
    assert!(
        linux_root.parameters[0]
            .placement
            .locations
            .iter()
            .enumerate()
            .all(|(index, location)| matches!(
                location,
                ValueLocation::Stack {
                    stack_byte_offset,
                    value_byte_offset,
                    byte_size: 8,
                    alignment: 8,
                } if *stack_byte_offset == index as u32 * 8
                    && *value_byte_offset == index as u16 * 8
            ))
    );
    let TargetUnitOperation::Call { arguments, .. } = &linux_root.operations[0] else {
        panic!("root must call helper")
    };
    assert_eq!(arguments[0].source, arguments[0].destination);

    let windows = lower_to_target_operations(&plan, NativeTarget::windows_x64()).unwrap();
    let TargetOperation::UnitBody(windows_root) = &windows.functions[0].operation else {
        panic!("root must remain Unit")
    };
    assert!(matches!(
        windows_root.parameters[0].placement.locations.as_slice(),
        [ValueLocation::Indirect {
            pointer: omega_calling_conventions::IndirectPointerLocation::Register(
                MachineRegister::X86Rcx
            ),
            byte_size: 40,
            alignment: 8,
            ..
        }]
    ));
    let TargetUnitOperation::Call { arguments, .. } = &windows_root.operations[0] else {
        panic!("root must call helper")
    };
    assert_eq!(arguments[0].source, arguments[0].destination);
}

#[test]
fn fixed_array_layout_repeats_padded_nested_elements_and_rejects_overflow() {
    let element_type = StructuralTypeId::new(1).unwrap();
    let inner_array_type = StructuralTypeId::new(2).unwrap();
    let outer_array_type = StructuralTypeId::new(3).unwrap();
    let oversized_array_type = StructuralTypeId::new(4).unwrap();
    let u64_type =
        ScalarType::Integer(IntegerType::new(psi_core::IntegerSign::Unsigned, 64).unwrap());
    let declarations = vec![
        StructuralTypeDeclaration {
            id: element_type,
            identity: "PaddedElement".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![
                    StructuralFieldDeclaration {
                        id: StructuralFieldId::new(1).unwrap(),
                        identity: "tag".into(),
                        relevance: psi_terminal::BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(ScalarType::Boolean),
                    },
                    StructuralFieldDeclaration {
                        id: StructuralFieldId::new(2).unwrap(),
                        identity: "value".into(),
                        relevance: psi_terminal::BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(u64_type),
                    },
                ],
            },
        },
        StructuralTypeDeclaration {
            id: inner_array_type,
            identity: "[PaddedElement; 2]".into(),
            shape: StructuralTypeShape::FixedArray {
                element: element_type,
                length: 2,
            },
        },
        StructuralTypeDeclaration {
            id: outer_array_type,
            identity: "[[PaddedElement; 2]; 3]".into(),
            shape: StructuralTypeShape::FixedArray {
                element: inner_array_type,
                length: 3,
            },
        },
        StructuralTypeDeclaration {
            id: oversized_array_type,
            identity: "[PaddedElement; 4096]".into(),
            shape: StructuralTypeShape::FixedArray {
                element: element_type,
                length: 4096,
            },
        },
    ];
    let declarations = declarations
        .iter()
        .map(|declaration| (declaration.id, declaration))
        .collect::<BTreeMap<_, _>>();

    let shape = structural_shape(
        outer_array_type,
        &declarations,
        &mut BTreeMap::new(),
        &mut BTreeSet::new(),
    )
    .unwrap();
    assert_eq!(shape, ValueShape::integer(96, 8));
    assert_eq!(
        structural_shape(
            oversized_array_type,
            &declarations,
            &mut BTreeMap::new(),
            &mut BTreeSet::new(),
        ),
        Err(LoweringError::StructuralTypeTooLarge(oversized_array_type))
    );
}

#[test]
fn metadata_only_boundary_requires_the_exact_preceding_port_realization() {
    use omega_target_operations::{
        MetadataOnlyPortRealization, ProviderExecutionBinding, ProviderPlanReportIdentity,
    };

    let machine = MachineId::new(1).unwrap();
    let boundary = BoundaryMachineId::new(1).unwrap();
    let port_operation = OperationId::new(1).unwrap();
    let settlement_operation = OperationId::new(2).unwrap();
    let service = psi_core::ServiceId::new(1).unwrap();
    let element_type = StructuralTypeId::new(1).unwrap();
    let array_type = StructuralTypeId::new(2).unwrap();
    let argument_place = PlaceId::new(1).unwrap();
    let boundary_place = PlaceId::new(2).unwrap();
    let u64_type =
        ScalarType::Integer(IntegerType::new(psi_core::IntegerSign::Unsigned, 64).unwrap());
    let provider_execution = ProviderExecutionBinding::from_execution_record(
        ProviderPlanReportIdentity::new(7).unwrap(),
        8,
        9,
        10,
        11,
    )
    .unwrap();
    let realization = MetadataOnlyPortRealization {
        effect_operation: port_operation,
        service,
        port: 0x20,
        value: 0x20,
    };
    let plan = AbstractOperationPlan {
        psi: identity(),
        entry: machine,
        structural_types: vec![
            StructuralTypeDeclaration {
                id: element_type,
                identity: "Acknowledgement".into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![StructuralFieldDeclaration {
                        id: StructuralFieldId::new(1).unwrap(),
                        identity: "value".into(),
                        relevance: psi_terminal::BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(u64_type),
                    }],
                },
            },
            StructuralTypeDeclaration {
                id: array_type,
                identity: "[Acknowledgement; 2]".into(),
                shape: StructuralTypeShape::FixedArray {
                    element: element_type,
                    length: 2,
                },
            },
        ],
        boundary_machines: vec![BoundaryMachineDeclaration {
            id: boundary,
            identity: "InterruptAcknowledgement::complete".into(),
            attachment: None,
            scalar_parameters: Vec::new(),
            structural_parameters: vec![StructuralParameterDeclaration {
                place: boundary_place,
                position: 0,
                is_self: false,
                structural_type: element_type,
                multiplicity: StructuralMultiplicity::Linear,
                access: StructuralAccess::Owned,
                qualifications: Vec::new(),
            }],
            result: None,
            requires: Vec::new(),
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: vec![service],
        }],
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine,
            attachment: None,
            entry: BlockId::new(1).unwrap(),
            parameters: Vec::new(),
            structural_parameters: vec![StructuralParameterDeclaration {
                place: argument_place,
                position: 0,
                is_self: false,
                structural_type: array_type,
                multiplicity: StructuralMultiplicity::Affine,
                access: StructuralAccess::Owned,
                qualifications: Vec::new(),
            }],
            result: AbstractFunctionResult::Unit,
            entry_claims: Vec::new(),
            published_service_ceiling: vec![service],
            block_entries: vec![omega_abstract_operations::AbstractBlockEntry {
                block: BlockId::new(1).unwrap(),
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: vec![
                AbstractOperation::PortWrite {
                    psi_operation: port_operation,
                    service,
                    port: 0x20,
                    value: 0x20,
                },
                AbstractOperation::BoundaryCall {
                    psi_operation: settlement_operation,
                    result: None,
                    boundary,
                    arguments: Vec::new(),
                    structural_arguments: vec![psi_terminal::StructuralArgument {
                        place: argument_place,
                        access: StructuralAccess::Owned,
                        path: vec![StructuralPathSegment::FixedIndex(1)],
                    }],
                    completion_claim_sources: Vec::new(),
                    completion_receipts: Vec::new(),
                },
                AbstractOperation::ReturnUnit {
                    psi_edge: EdgeId::new(1).unwrap(),
                    cleanup_actions: vec![psi_terminal::TerminalAffineCleanupAction::DiscardRoot(
                        argument_place,
                    )],
                },
            ],
        }],
    };
    let binding = BoundarySettlementBinding {
        boundary,
        provider_execution,
        realization: realization.into(),
    };
    let lowered =
        lower_to_target_operations_with_settlements(&plan, NativeTarget::linux_x64(), &[binding])
            .expect("exact effect evidence");
    let TargetOperation::UnitBody(body) = &lowered.functions[0].operation else {
        panic!("Unit body")
    };
    let TargetUnitOperation::BoundarySettlement {
        provider_execution: actual,
        realization: actual_realization,
        arguments,
        ..
    } = &body.operations[1]
    else {
        panic!("boundary settlement")
    };
    assert_eq!(*actual, provider_execution);
    assert_eq!(*actual_realization, realization.into());
    assert_eq!(
        arguments,
        &[psi_terminal::StructuralArgument {
            place: argument_place,
            access: StructuralAccess::Owned,
            path: vec![StructuralPathSegment::FixedIndex(1)],
        }]
    );

    let mut scalar_argument = plan.clone();
    let argument = ValueId::new(1).unwrap();
    scalar_argument.boundary_machines[0]
        .scalar_parameters
        .push(ScalarType::Boolean);
    scalar_argument.functions[0]
        .parameters
        .push(AbstractParameter {
            value: argument,
            scalar_type: ScalarType::Boolean,
        });
    let AbstractOperation::BoundaryCall { arguments, .. } =
        &mut scalar_argument.functions[0].operations[1]
    else {
        unreachable!("fixture contains a boundary call")
    };
    arguments.push(argument);
    assert_eq!(
        lower_to_target_operations_with_settlements(
            &scalar_argument,
            NativeTarget::linux_x64(),
            &[binding],
        ),
        Err(
            LoweringError::ScalarBoundaryArgumentsRequireNativeRealization {
                machine,
                operation: settlement_operation,
                boundary,
            }
        )
    );

    let wrong = BoundarySettlementBinding {
        realization: MetadataOnlyPortRealization {
            value: 0x21,
            ..realization
        }
        .into(),
        ..binding
    };
    assert_eq!(
        lower_to_target_operations_with_settlements(&plan, NativeTarget::linux_x64(), &[wrong],),
        Err(LoweringError::BoundaryRealizationMismatch(boundary))
    );

    let mut result_bearing = plan.clone();
    let result = AbstractResult {
        value: ValueId::new(1).unwrap(),
        scalar_type: ScalarType::Boolean,
    };
    result_bearing.boundary_machines[0].result = Some(result.scalar_type);
    let AbstractOperation::BoundaryCall {
        result: operation_result,
        ..
    } = &mut result_bearing.functions[0].operations[1]
    else {
        unreachable!("fixture contains a boundary call")
    };
    *operation_result = Some(result);
    assert_eq!(
        lower_to_target_operations_with_settlements(
            &result_bearing,
            NativeTarget::linux_x64(),
            &[binding],
        ),
        Err(
            LoweringError::ResultBearingBoundarySettlementRequiresNativeRealization {
                machine,
                operation: settlement_operation,
                boundary,
            }
        )
    );
}

#[test]
fn claim_completion_only_boundary_retains_two_linear_claims_without_physical_inputs() {
    use omega_abstract_operations::CompletionClaimSource;
    use omega_target_operations::{
        ClaimCompletionOnlyRealization, ProviderExecutionBinding, ProviderPlanReportIdentity,
    };
    use psi_core::{ClaimId, StructuralDomainId};
    use psi_terminal::{CompletionReceipt, EntryClaim};

    let machine = MachineId::new(31).unwrap();
    let boundary = BoundaryMachineId::new(31).unwrap();
    let extent = StructuralTypeId::new(31).unwrap();
    let boundary_place = PlaceId::new(31).unwrap();
    let image = PlaceId::new(32).unwrap();
    let storage = PlaceId::new(33).unwrap();
    let image_claim = ClaimId::new(31).unwrap();
    let storage_claim = ClaimId::new(32).unwrap();
    let granted = StructuralDomainId::new(31).unwrap();
    let image_settle = OperationId::new(31).unwrap();
    let storage_settle = OperationId::new(32).unwrap();
    let return_edge = EdgeId::new(31).unwrap();
    let provider_execution = ProviderExecutionBinding::from_execution_record(
        ProviderPlanReportIdentity::new(31).unwrap(),
        32,
        33,
        34,
        35,
    )
    .unwrap();
    let entry_claims = vec![
        EntryClaim {
            claim: image_claim,
            input: image,
            path: Vec::new(),
        },
        EntryClaim {
            claim: storage_claim,
            input: storage,
            path: Vec::new(),
        },
    ];
    let claim_sources = entry_claims
        .iter()
        .cloned()
        .map(|entry| CompletionClaimSource {
            claim: entry.claim,
            entry: Some(entry),
            content: None,
        })
        .collect::<Vec<_>>();
    let parameter = |place, position| StructuralParameterDeclaration {
        place,
        position,
        is_self: false,
        structural_type: extent,
        multiplicity: StructuralMultiplicity::Linear,
        access: StructuralAccess::Owned,
        qualifications: vec![granted],
    };
    let settle = |operation, place, claim| AbstractOperation::BoundaryCall {
        psi_operation: operation,
        result: None,
        boundary,
        arguments: Vec::new(),
        structural_arguments: vec![StructuralArgument {
            place,
            access: StructuralAccess::Owned,
            path: Vec::new(),
        }],
        completion_claim_sources: claim_sources.clone(),
        completion_receipts: vec![CompletionReceipt {
            claim,
            argument_index: 0,
        }],
    };
    let plan = AbstractOperationPlan {
        psi: identity(),
        entry: machine,
        structural_types: vec![StructuralTypeDeclaration {
            id: extent,
            identity: "Extent".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![
                    StructuralFieldDeclaration {
                        id: StructuralFieldId::new(31).unwrap(),
                        identity: "base".into(),
                        relevance: psi_terminal::BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                            IntegerType::address(64).unwrap(),
                        )),
                    },
                    StructuralFieldDeclaration {
                        id: StructuralFieldId::new(32).unwrap(),
                        identity: "length".into(),
                        relevance: psi_terminal::BindingRelevance::Relevant,
                        field_type: StructuralFieldType::Scalar(ScalarType::Integer(
                            IntegerType::new(psi_core::IntegerSign::Unsigned, 64).unwrap(),
                        )),
                    },
                ],
            },
        }],
        boundary_machines: vec![BoundaryMachineDeclaration {
            id: boundary,
            identity: "Extent::settle".into(),
            attachment: Some(extent),
            scalar_parameters: Vec::new(),
            structural_parameters: vec![StructuralParameterDeclaration {
                place: boundary_place,
                position: 0,
                is_self: true,
                structural_type: extent,
                multiplicity: StructuralMultiplicity::Linear,
                access: StructuralAccess::Owned,
                qualifications: vec![granted],
            }],
            result: None,
            requires: vec![psi_terminal::StructuralDomainRequirement {
                argument_index: 0,
                domain: granted,
            }],
            program_local_root_introductions: Vec::new(),
            content_guarantees: Vec::new(),
            published_service_ceiling: Vec::new(),
        }],
        provider_candidates: Vec::new(),
        functions: vec![AbstractFunction {
            machine,
            attachment: None,
            entry: BlockId::new(31).unwrap(),
            parameters: Vec::new(),
            structural_parameters: vec![parameter(image, 0), parameter(storage, 1)],
            result: AbstractFunctionResult::Unit,
            entry_claims,
            published_service_ceiling: Vec::new(),
            block_entries: vec![AbstractBlockEntry {
                block: BlockId::new(31).unwrap(),
                parameters: Vec::new(),
                operation_offset: 0,
            }],
            operations: vec![
                settle(image_settle, image, image_claim),
                settle(storage_settle, storage, storage_claim),
                AbstractOperation::ReturnUnit {
                    psi_edge: return_edge,
                    cleanup_actions: Vec::new(),
                },
            ],
        }],
    };
    let binding = BoundarySettlementBinding {
        boundary,
        provider_execution,
        realization: ClaimCompletionOnlyRealization.into(),
    };

    let lowered =
        lower_to_target_operations_with_settlements(&plan, NativeTarget::uefi_x64(), &[binding])
            .expect("claim completion is an evidence-bearing zero-input target settlement");
    let TargetOperation::UnitBody(body) = &lowered.functions[0].operation else {
        panic!("Unit body")
    };
    assert_eq!(body.operations.len(), 3);
    for (index, (operation, claim)) in
        [(image_settle, image_claim), (storage_settle, storage_claim)]
            .into_iter()
            .enumerate()
    {
        let TargetUnitOperation::BoundarySettlement {
            psi_operation,
            provider_execution: actual_provider,
            realization: BoundaryRealization::ClaimCompletionOnly(_),
            scalar_arguments,
            byte_sequence_arguments,
            completion_claim_sources,
            completion_receipts,
            ..
        } = &body.operations[index]
        else {
            panic!("claim-completion-only settlement")
        };
        assert_eq!(*psi_operation, operation);
        assert_eq!(*actual_provider, provider_execution);
        assert!(scalar_arguments.is_empty());
        assert!(byte_sequence_arguments.is_empty());
        assert_eq!(completion_claim_sources, &claim_sources);
        assert_eq!(
            completion_receipts,
            &[CompletionReceipt {
                claim,
                argument_index: 0,
            }]
        );
    }
    assert!(matches!(
        body.operations[2],
        TargetUnitOperation::Return { psi_edge, ref cleanup_actions }
            if psi_edge == return_edge && cleanup_actions.is_empty()
    ));

    let mut missing_receipt = plan.clone();
    let AbstractOperation::BoundaryCall {
        completion_receipts,
        ..
    } = &mut missing_receipt.functions[0].operations[0]
    else {
        unreachable!()
    };
    completion_receipts.clear();
    assert_eq!(
        lower_to_target_operations_with_settlements(
            &missing_receipt,
            NativeTarget::uefi_x64(),
            &[binding],
        ),
        Err(LoweringError::InvalidClaimCompletionOnlyShape {
            machine,
            operation: image_settle,
            boundary,
        })
    );
}
