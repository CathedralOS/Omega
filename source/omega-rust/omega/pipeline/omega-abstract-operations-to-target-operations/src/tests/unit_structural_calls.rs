//! Structural Unit-call layout and semantic-custody projection.

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
        projected_qualifications: Vec::new(),
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
                        requirement_obligations: vec![ObligationId::new(1).unwrap()],
                        crash_continuations: vec![CrashRouteBucket {
                            cause: CrashCause::Trap,
                            alternatives: vec![CrashRouteGuard::Truth],
                        }],
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
        let TargetUnitOperation::Call {
            arguments,
            requirement_obligations,
            crash_continuations,
            ..
        } = &root.operations[0]
        else {
            panic!("root must call helper")
        };
        assert!(arguments[0].path.is_empty());
        assert_eq!(arguments[0].shape, ValueShape::integer(40, 8));
        assert_eq!(requirement_obligations, &[ObligationId::new(1).unwrap()]);
        assert_eq!(
            crash_continuations,
            &[CrashRouteBucket {
                cause: CrashCause::Trap,
                alternatives: vec![CrashRouteGuard::Truth],
            }]
        );
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
