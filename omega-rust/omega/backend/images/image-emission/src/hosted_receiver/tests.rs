use super::{
    ProgramEntryPhysicalContractPlan, physical_contract_matches, receiver_pointer_matches,
    zero_valid_record_storage,
};
#[test]
fn zero_filled_receiver_byte_fields_require_owned_backing() {
    use semantic_vocabulary::{StructuralFieldId, StructuralTypeId};
    use terminal_psi::{
        BindingRelevance, ByteSequenceCarrier, StructuralFieldDeclaration, StructuralFieldType,
        StructuralTypeDeclaration, StructuralTypeShape,
    };
    let root = StructuralTypeId::new(1).unwrap();
    let child = StructuralTypeId::new(2).unwrap();
    let field = |field_type| StructuralFieldDeclaration {
        id: StructuralFieldId::new(1).unwrap(),
        identity: "bytes".into(),
        relevance: BindingRelevance::Relevant,
        field_type,
    };
    for carrier in [
        ByteSequenceCarrier::BoundedOwned { capacity: 0 },
        ByteSequenceCarrier::BoundedOwned { capacity: 3 },
        ByteSequenceCarrier::BoundedOwned { capacity: 9 },
        ByteSequenceCarrier::BorrowedView,
    ] {
        let declarations = [
            StructuralTypeDeclaration {
                id: root,
                identity: "outer".into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![field(StructuralFieldType::Structural(child))],
                },
            },
            StructuralTypeDeclaration {
                id: child,
                identity: "inner".into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![field(StructuralFieldType::ByteSequence(carrier))],
                },
            },
        ];
        let expected = matches!(carrier, ByteSequenceCarrier::BoundedOwned { .. });
        for selected in [root, child] {
            assert_eq!(
                zero_valid_record_storage(&declarations, selected, &mut Vec::new()),
                expected,
                "carrier {carrier:?}, root {selected:?}",
            );
        }
    }
}

#[test]
fn nested_receiver_storage_rejects_missing_cyclic_erased_and_zero_excluded_leaves() {
    use semantic_vocabulary::{
        BoundedIntegerType, IntegerSign, IntegerType, IntegerValue, ScalarType, StructuralFieldId,
        StructuralTypeId,
    };
    use terminal_psi::{
        BindingRelevance, StructuralFieldDeclaration, StructuralFieldType,
        StructuralTypeDeclaration, StructuralTypeShape,
    };
    let root = StructuralTypeId::new(1).unwrap();
    let child = StructuralTypeId::new(2).unwrap();
    let field = |field_type| StructuralFieldDeclaration {
        id: StructuralFieldId::new(1).unwrap(),
        identity: "value".into(),
        relevance: BindingRelevance::Relevant,
        field_type,
    };
    let declarations = vec![
        StructuralTypeDeclaration {
            id: root,
            identity: "outer".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![field(StructuralFieldType::Structural(child))],
            },
        },
        StructuralTypeDeclaration {
            id: child,
            identity: "inner".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![field(StructuralFieldType::Scalar(ScalarType::Boolean))],
            },
        },
    ];
    assert!(zero_valid_record_storage(
        &declarations,
        root,
        &mut Vec::new()
    ));
    assert!(!zero_valid_record_storage(
        &declarations[..1],
        root,
        &mut Vec::new()
    ));
    for field_type in [
        StructuralFieldType::Structural(root),
        StructuralFieldType::Erased {
            type_identity: "unestablished-service".into(),
        },
        StructuralFieldType::BoundedInteger(
            BoundedIntegerType::new(
                IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                IntegerValue::Signed(1),
                IntegerValue::Signed(9),
            )
            .unwrap(),
        ),
    ] {
        let mut changed = declarations.clone();
        changed[1].shape = StructuralTypeShape::Record {
            fields: vec![field(field_type)],
        };
        assert!(!zero_valid_record_storage(&changed, root, &mut Vec::new()));
    }
    let mut duplicated = declarations.clone();
    duplicated.push(declarations[1].clone());
    assert!(!zero_valid_record_storage(
        &duplicated,
        root,
        &mut Vec::new()
    ));
}

#[test]
fn nested_receiver_storage_follows_the_zero_tag_sum_case() {
    use semantic_vocabulary::{
        BoundedIntegerType, IntegerSign, IntegerType, IntegerValue, ScalarType, StructuralCaseId,
        StructuralFieldId, StructuralTypeId,
    };
    use terminal_psi::{
        BindingRelevance, StructuralCaseDeclaration, StructuralFieldDeclaration,
        StructuralFieldType, StructuralTypeDeclaration, StructuralTypeShape,
    };
    let root = StructuralTypeId::new(1).unwrap();
    let sum = StructuralTypeId::new(2).unwrap();
    let field = |field_type| StructuralFieldDeclaration {
        id: StructuralFieldId::new(1).unwrap(),
        identity: "value".into(),
        relevance: BindingRelevance::Relevant,
        field_type,
    };
    let case = |fields| StructuralCaseDeclaration {
        id: StructuralCaseId::new(1).unwrap(),
        identity: "Say".into(),
        fields,
    };
    let sum_declaration = |fields: Vec<StructuralFieldDeclaration>| StructuralTypeDeclaration {
        id: sum,
        identity: "sum".into(),
        shape: StructuralTypeShape::Sum {
            cases: vec![
                case(fields),
                StructuralCaseDeclaration {
                    id: StructuralCaseId::new(2).unwrap(),
                    identity: "Quiet".into(),
                    fields: vec![field(StructuralFieldType::Erased {
                        type_identity: "unestablished".into(),
                    })],
                },
            ],
        },
    };
    let declarations = |sum_shape: StructuralTypeDeclaration| {
        vec![
            StructuralTypeDeclaration {
                id: root,
                identity: "outer".into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![field(StructuralFieldType::Structural(sum))],
                },
            },
            sum_shape,
        ]
    };
    // A payloadless or zero-valid first case inhabits zero storage; later
    // cases — even erased or invalid ones — are never reached.
    assert!(zero_valid_record_storage(
        &declarations(sum_declaration(vec![])),
        root,
        &mut Vec::new()
    ));
    assert!(zero_valid_record_storage(
        &declarations(sum_declaration(vec![field(StructuralFieldType::Scalar(
            ScalarType::Boolean
        ))])),
        root,
        &mut Vec::new()
    ));
    for first_case_fields in [
        vec![field(StructuralFieldType::BoundedInteger(
            BoundedIntegerType::new(
                IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                IntegerValue::Signed(1),
                IntegerValue::Signed(9),
            )
            .unwrap(),
        ))],
        vec![field(StructuralFieldType::ByteSequence(
            terminal_psi::ByteSequenceCarrier::BorrowedView,
        ))],
        vec![StructuralFieldDeclaration {
            relevance: BindingRelevance::Erased,
            ..field(StructuralFieldType::Scalar(ScalarType::Boolean))
        }],
        vec![field(StructuralFieldType::Erased {
            type_identity: "nested-service".into(),
        })],
    ] {
        let label = format!("{first_case_fields:?}");
        assert!(
            !zero_valid_record_storage(
                &declarations(sum_declaration(first_case_fields)),
                root,
                &mut Vec::new()
            ),
            "first-case payload {label}"
        );
    }
    // A sum with no cases has no zero-storage inhabitant.
    let mut empty = sum_declaration(vec![]);
    empty.shape = StructuralTypeShape::Sum { cases: vec![] };
    assert!(!zero_valid_record_storage(
        &declarations(empty),
        root,
        &mut Vec::new()
    ));
}

#[test]
fn nested_receiver_storage_checks_array_elements_and_mixed_common_fields() {
    use semantic_vocabulary::{
        BoundedIntegerType, IntegerSign, IntegerType, IntegerValue, ScalarType, StructuralCaseId,
        StructuralFieldId, StructuralTypeId,
    };
    use terminal_psi::{
        BindingRelevance, StructuralCaseDeclaration, StructuralFieldDeclaration,
        StructuralFieldType, StructuralTypeDeclaration, StructuralTypeShape,
    };
    let root = StructuralTypeId::new(1).unwrap();
    let array = StructuralTypeId::new(2).unwrap();
    let element = StructuralTypeId::new(3).unwrap();
    let nested_array = StructuralTypeId::new(5).unwrap();
    let leaf = StructuralTypeId::new(6).unwrap();
    let mixed = StructuralTypeId::new(4).unwrap();
    let field = |field_type| StructuralFieldDeclaration {
        id: StructuralFieldId::new(1).unwrap(),
        identity: "value".into(),
        relevance: BindingRelevance::Relevant,
        field_type,
    };
    // Record element arrays and nested fixed arrays are zero-valid when the
    // complete element chain is; the element judgment is length-independent.
    for (length, element_shape) in [
        (
            2,
            StructuralTypeShape::Record {
                fields: vec![field(StructuralFieldType::Scalar(ScalarType::Integer(
                    IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                )))],
            },
        ),
        (0, StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean)),
        (
            3,
            StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BoundedOwned {
                capacity: 4,
            }),
        ),
        (
            2,
            StructuralTypeShape::FixedArray {
                element: leaf,
                length: 5,
            },
        ),
    ] {
        let mut declarations = vec![
            StructuralTypeDeclaration {
                id: root,
                identity: "outer".into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![field(StructuralFieldType::Structural(array))],
                },
            },
            StructuralTypeDeclaration {
                id: array,
                identity: "array".into(),
                shape: StructuralTypeShape::FixedArray { element, length },
            },
            StructuralTypeDeclaration {
                id: element,
                identity: "element".into(),
                shape: element_shape.clone(),
            },
        ];
        if let StructuralTypeShape::FixedArray { .. } = element_shape {
            // A fixed array of fixed arrays bottoms out in a scalar leaf.
            declarations[1].shape = StructuralTypeShape::FixedArray {
                element: nested_array,
                length,
            };
            declarations[2].id = nested_array;
            declarations.push(StructuralTypeDeclaration {
                id: leaf,
                identity: "leaf".into(),
                shape: StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
            });
        }
        assert!(
            zero_valid_record_storage(&declarations, root, &mut Vec::new()),
            "length {length} element {element_shape:?}"
        );
    }
    // Elements that zero cannot establish reject through the same chain.
    for element_shape in [
        StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean),
        StructuralTypeShape::ByteSequence(terminal_psi::ByteSequenceCarrier::BorrowedView),
        StructuralTypeShape::Record {
            fields: vec![field(StructuralFieldType::BoundedInteger(
                BoundedIntegerType::new(
                    IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                    IntegerValue::Signed(1),
                    IntegerValue::Signed(9),
                )
                .unwrap(),
            ))],
        },
    ] {
        let expected = matches!(
            element_shape,
            StructuralTypeShape::PrimitiveScalar(ScalarType::Boolean)
        );
        let declarations = vec![
            StructuralTypeDeclaration {
                id: root,
                identity: "outer".into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![field(StructuralFieldType::Structural(array))],
                },
            },
            StructuralTypeDeclaration {
                id: array,
                identity: "array".into(),
                shape: StructuralTypeShape::FixedArray { element, length: 0 },
            },
            StructuralTypeDeclaration {
                id: element,
                identity: "element".into(),
                shape: element_shape.clone(),
            },
        ];
        assert_eq!(
            zero_valid_record_storage(&declarations, root, &mut Vec::new()),
            expected,
            "element {element_shape:?}"
        );
    }
    // A cyclic element chain rejects even beneath an empty dimension.
    let cyclic = vec![
        StructuralTypeDeclaration {
            id: root,
            identity: "outer".into(),
            shape: StructuralTypeShape::Record {
                fields: vec![field(StructuralFieldType::Structural(array))],
            },
        },
        StructuralTypeDeclaration {
            id: array,
            identity: "array".into(),
            shape: StructuralTypeShape::FixedArray {
                element: array,
                length: 0,
            },
        },
    ];
    assert!(!zero_valid_record_storage(&cyclic, root, &mut Vec::new()));
    // A mixed shape composes common fields with the zero-tag case payload.
    let mixed_declaration =
        |common: Vec<StructuralFieldDeclaration>, case_fields| StructuralTypeDeclaration {
            id: mixed,
            identity: "mixed".into(),
            shape: StructuralTypeShape::Mixed {
                fields: common,
                cases: vec![
                    StructuralCaseDeclaration {
                        id: StructuralCaseId::new(1).unwrap(),
                        identity: "Empty".into(),
                        fields: case_fields,
                    },
                    StructuralCaseDeclaration {
                        id: StructuralCaseId::new(2).unwrap(),
                        identity: "Full".into(),
                        fields: vec![field(StructuralFieldType::Erased {
                            type_identity: "unestablished".into(),
                        })],
                    },
                ],
            },
        };
    let root_mixed = |mixed_declaration: StructuralTypeDeclaration| {
        vec![
            StructuralTypeDeclaration {
                id: root,
                identity: "outer".into(),
                shape: StructuralTypeShape::Record {
                    fields: vec![field(StructuralFieldType::Structural(mixed))],
                },
            },
            mixed_declaration,
        ]
    };
    assert!(zero_valid_record_storage(
        &root_mixed(mixed_declaration(
            vec![field(StructuralFieldType::Scalar(ScalarType::Boolean))],
            vec![]
        )),
        root,
        &mut Vec::new()
    ));
    for (common, case_fields) in [
        (
            vec![field(StructuralFieldType::BoundedInteger(
                BoundedIntegerType::new(
                    IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                    IntegerValue::Signed(1),
                    IntegerValue::Signed(9),
                )
                .unwrap(),
            ))],
            vec![],
        ),
        (
            vec![StructuralFieldDeclaration {
                relevance: BindingRelevance::Erased,
                ..field(StructuralFieldType::Scalar(ScalarType::Boolean))
            }],
            vec![],
        ),
        (
            vec![field(StructuralFieldType::Scalar(ScalarType::Boolean))],
            vec![field(StructuralFieldType::BoundedInteger(
                BoundedIntegerType::new(
                    IntegerType::new(IntegerSign::Signed, 32).unwrap(),
                    IntegerValue::Signed(1),
                    IntegerValue::Signed(9),
                )
                .unwrap(),
            ))],
        ),
    ] {
        let label = format!("common {common:?} case {case_fields:?}");
        assert!(
            !zero_valid_record_storage(
                &root_mixed(mixed_declaration(common, case_fields)),
                root,
                &mut Vec::new()
            ),
            "{label}"
        );
    }
}

#[test]
fn hosted_receiver_accepts_only_canonical_borrowed_pointer_placement() {
    use calling_conventions::{
        CallSignature, CallingPolicy, IndirectPointerLocation, MachineRegister, ValueLocation,
        ValuePlacement, ValueShape, evaluate_call_plan,
    };
    for shape in [
        ValueShape::borrowed_reference(0, 1),
        ValueShape::borrowed_reference(4, 4),
        ValueShape::borrowed_reference(32, 16),
    ] {
        let signature = CallSignature {
            parameters: vec![shape],
            result: None,
        };
        let plan = evaluate_call_plan(CallingPolicy::Aapcs64, &signature)
            .expect("canonical receiver pointer ABI");
        let placement = &plan.parameters[0];
        assert!(receiver_pointer_matches(
            shape,
            placement,
            MachineRegister::Aarch64X(0)
        ));
        // A Linux/System V placement register must never satisfy the Darwin
        // bridge's x0 custody, and vice versa.
        assert!(!receiver_pointer_matches(
            shape,
            placement,
            MachineRegister::X86Rdi
        ));
        let indirect = |pointer, copy_stack_byte_offset, byte_size, alignment| ValuePlacement {
            shape,
            locations: vec![ValueLocation::Indirect {
                pointer,
                copy_stack_byte_offset,
                byte_size,
                alignment,
            }],
        };
        let pointer = IndirectPointerLocation::Register(MachineRegister::Aarch64X(0));
        for corrupted in [
            indirect(
                IndirectPointerLocation::Register(MachineRegister::Aarch64X(1)),
                None,
                shape.byte_size,
                shape.alignment,
            ),
            indirect(
                IndirectPointerLocation::Stack {
                    stack_byte_offset: 0,
                    alignment: 8,
                },
                None,
                shape.byte_size,
                shape.alignment,
            ),
            indirect(pointer, Some(0), shape.byte_size, shape.alignment),
            indirect(pointer, None, shape.byte_size + 1, shape.alignment),
            indirect(pointer, None, shape.byte_size, shape.alignment * 2),
            ValuePlacement {
                shape,
                locations: Vec::new(),
            },
            ValuePlacement {
                shape,
                locations: vec![placement.locations[0]; 2],
            },
            ValuePlacement {
                shape,
                locations: vec![ValueLocation::Register {
                    register: MachineRegister::Aarch64X(0),
                    value_byte_offset: 0,
                    byte_size: 8,
                }],
            },
            ValuePlacement {
                shape: ValueShape::integer(shape.byte_size, shape.alignment),
                locations: placement.locations.clone(),
            },
        ] {
            assert!(
                !receiver_pointer_matches(shape, &corrupted, MachineRegister::Aarch64X(0)),
                "corrupted receiver: {corrupted:?}"
            );
        }
        let changed_shape = ValueShape::borrowed_reference(shape.byte_size + 1, shape.alignment);
        assert!(!receiver_pointer_matches(
            changed_shape,
            placement,
            MachineRegister::Aarch64X(0)
        ));
        assert!(!receiver_pointer_matches(
            ValueShape::integer(shape.byte_size, shape.alignment),
            placement,
            MachineRegister::Aarch64X(0)
        ));
    }
}

#[test]
fn windows_receiver_accepts_only_canonical_borrowed_pointer_placement() {
    use calling_conventions::{
        CallSignature, CallingPolicy, IndirectPointerLocation, MachineRegister, ValueLocation,
        ValuePlacement, ValueShape, evaluate_call_plan,
    };
    for shape in [
        ValueShape::borrowed_reference(0, 1),
        ValueShape::borrowed_reference(4, 4),
        ValueShape::borrowed_reference(32, 16),
    ] {
        let signature = CallSignature {
            parameters: vec![shape],
            result: None,
        };
        let plan = evaluate_call_plan(CallingPolicy::MicrosoftX64, &signature)
            .expect("canonical Microsoft x64 receiver pointer ABI");
        let placement = &plan.parameters[0];
        // Microsoft x64 passes the first argument pointer in rcx through an
        // indirect location retaining the referent geometry; a borrowed
        // reference carries no caller copy.
        assert!(receiver_pointer_matches(
            shape,
            placement,
            MachineRegister::X86Rcx
        ));
        // A System V placement register must never satisfy the Windows
        // bridge's rcx custody, and vice versa.
        assert!(!receiver_pointer_matches(
            shape,
            placement,
            MachineRegister::X86Rdi
        ));
        assert!(!receiver_pointer_matches(
            shape,
            placement,
            MachineRegister::Aarch64X(0)
        ));
        let indirect = |pointer, copy_stack_byte_offset, byte_size, alignment| ValuePlacement {
            shape,
            locations: vec![ValueLocation::Indirect {
                pointer,
                copy_stack_byte_offset,
                byte_size,
                alignment,
            }],
        };
        let pointer = IndirectPointerLocation::Register(MachineRegister::X86Rcx);
        for corrupted in [
            // A substituted register is a receiver substitution, not an alias.
            indirect(
                IndirectPointerLocation::Register(MachineRegister::X86Rdx),
                None,
                shape.byte_size,
                shape.alignment,
            ),
            indirect(
                IndirectPointerLocation::Stack {
                    stack_byte_offset: 0,
                    alignment: 8,
                },
                None,
                shape.byte_size,
                shape.alignment,
            ),
            indirect(pointer, Some(0), shape.byte_size, shape.alignment),
            indirect(pointer, None, shape.byte_size + 1, shape.alignment),
            indirect(pointer, None, shape.byte_size, shape.alignment * 2),
            ValuePlacement {
                shape,
                locations: Vec::new(),
            },
            ValuePlacement {
                shape,
                locations: vec![placement.locations[0]; 2],
            },
            ValuePlacement {
                shape,
                locations: vec![ValueLocation::Register {
                    register: MachineRegister::X86Rcx,
                    value_byte_offset: 0,
                    byte_size: 8,
                }],
            },
            ValuePlacement {
                shape: ValueShape::integer(shape.byte_size, shape.alignment),
                locations: placement.locations.clone(),
            },
        ] {
            assert!(
                !receiver_pointer_matches(shape, &corrupted, MachineRegister::X86Rcx),
                "corrupted receiver: {corrupted:?}"
            );
        }
    }
}

#[test]
fn linux_receiver_accepts_only_canonical_borrowed_pointer_placement() {
    use calling_conventions::{
        CallSignature, CallingPolicy, IndirectPointerLocation, MachineRegister, ValueLocation,
        ValuePlacement, ValueShape, evaluate_call_plan,
    };
    for shape in [
        ValueShape::borrowed_reference(0, 1),
        ValueShape::borrowed_reference(4, 4),
        ValueShape::borrowed_reference(32, 16),
    ] {
        let signature = CallSignature {
            parameters: vec![shape],
            result: None,
        };
        let plan = evaluate_call_plan(CallingPolicy::SystemVAMD64, &signature)
            .expect("canonical System V receiver pointer ABI");
        let placement = &plan.parameters[0];
        // System V passes the first integer argument in rdi through an
        // indirect location retaining the referent geometry.
        assert!(receiver_pointer_matches(
            shape,
            placement,
            MachineRegister::X86Rdi
        ));
        assert!(!receiver_pointer_matches(
            shape,
            placement,
            MachineRegister::Aarch64X(0)
        ));
        let indirect = |pointer, copy_stack_byte_offset, byte_size, alignment| ValuePlacement {
            shape,
            locations: vec![ValueLocation::Indirect {
                pointer,
                copy_stack_byte_offset,
                byte_size,
                alignment,
            }],
        };
        let pointer = IndirectPointerLocation::Register(MachineRegister::X86Rdi);
        for corrupted in [
            // A substituted register is a receiver substitution, not an alias.
            indirect(
                IndirectPointerLocation::Register(MachineRegister::X86Rsi),
                None,
                shape.byte_size,
                shape.alignment,
            ),
            indirect(
                IndirectPointerLocation::Stack {
                    stack_byte_offset: 0,
                    alignment: 8,
                },
                None,
                shape.byte_size,
                shape.alignment,
            ),
            indirect(pointer, Some(0), shape.byte_size, shape.alignment),
            indirect(pointer, None, shape.byte_size + 1, shape.alignment),
            indirect(pointer, None, shape.byte_size, shape.alignment * 2),
            ValuePlacement {
                shape,
                locations: Vec::new(),
            },
            ValuePlacement {
                shape,
                locations: vec![placement.locations[0]; 2],
            },
            ValuePlacement {
                shape,
                locations: vec![ValueLocation::Register {
                    register: MachineRegister::X86Rdi,
                    value_byte_offset: 0,
                    byte_size: 8,
                }],
            },
            ValuePlacement {
                shape: ValueShape::integer(shape.byte_size, shape.alignment),
                locations: placement.locations.clone(),
            },
        ] {
            assert!(
                !receiver_pointer_matches(shape, &corrupted, MachineRegister::X86Rdi),
                "corrupted receiver: {corrupted:?}"
            );
        }
    }
}

fn physical_contract(
    requirement: &str,
    source: program_entry_plan::ProgramEntryPhysicalContractPackageSourceDigest,
) -> ProgramEntryPhysicalContractPlan {
    let plan = program_entry_plan::exact_macos_arm64_physical_boundary_entry_plan();
    ProgramEntryPhysicalContractPlan::new(
        target::TargetProfile::MacosArm64.program_entry_slot(),
        requirement.into(),
        target::ProgramEntryPhysicalContractPackage::MacosArm64,
        source,
        0,
        [
            program_entry_plan::MACOS_ARM64_I32_TYPE_IDENTITY,
            program_entry_plan::MACOS_ARM64_ADDRESS_TYPE_IDENTITY,
            program_entry_plan::MACOS_ARM64_ADDRESS_TYPE_IDENTITY,
            program_entry_plan::MACOS_ARM64_ADDRESS_TYPE_IDENTITY,
        ]
        .into_iter()
        .map(str::to_owned)
        .collect(),
        program_entry_plan::MACOS_ARM64_I32_TYPE_IDENTITY.into(),
        plan.contract_report_fingerprint(),
        plan.plan().clone(),
    )
    .expect("well-shaped physical plan")
}

#[test]
fn hosted_physical_replay_keeps_source_bytes_for_package_qualified_requirements() {
    let source = program_entry_plan::exact_macos_arm64_physical_contract_package_source_digest();
    for requirement in [
        program_entry_plan::MACOS_ARM64_PHYSICAL_REQUIREMENT_IDENTITY,
        "accepted-package::MacosPhysicalEntry::enter",
    ] {
        assert!(physical_contract_matches(
            &physical_contract(requirement, source),
            target::NativeTarget::macos_arm64()
        ));
    }
    let changed_source =
        program_entry_plan::ProgramEntryPhysicalContractPackageSourceDigest::from_package_source(
            target::ProgramEntryPhysicalContractPackage::MacosArm64,
            b"different target implementation",
        );
    assert!(!physical_contract_matches(
        &physical_contract(
            "accepted-package::MacosPhysicalEntry::enter",
            changed_source
        ),
        target::NativeTarget::macos_arm64()
    ));
}

fn linux_physical_contract(
    requirement: &str,
    source: program_entry_plan::ProgramEntryPhysicalContractPackageSourceDigest,
) -> ProgramEntryPhysicalContractPlan {
    let plan = program_entry_plan::exact_linux_x86_64_physical_boundary_entry_plan();
    ProgramEntryPhysicalContractPlan::new(
        target::TargetProfile::LinuxX64.program_entry_slot(),
        requirement.into(),
        target::ProgramEntryPhysicalContractPackage::LinuxX86_64,
        source,
        0,
        [program_entry_plan::LINUX_X86_64_ADDRESS_TYPE_IDENTITY]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        program_entry_plan::LINUX_X86_64_I32_TYPE_IDENTITY.into(),
        plan.contract_report_fingerprint(),
        plan.plan().clone(),
    )
    .expect("well-shaped Linux physical plan")
}

#[test]
fn linux_hosted_physical_replay_keeps_source_bytes_for_package_qualified_requirements() {
    let source = program_entry_plan::exact_linux_x86_64_physical_contract_package_source_digest();
    for requirement in [
        program_entry_plan::LINUX_X86_64_PHYSICAL_REQUIREMENT_IDENTITY,
        "accepted-package::LinuxPhysicalEntry::enter",
    ] {
        assert!(physical_contract_matches(
            &linux_physical_contract(requirement, source),
            target::NativeTarget::linux_x64()
        ));
        // The exact Linux contract must not satisfy a different bridge target.
        assert!(!physical_contract_matches(
            &linux_physical_contract(requirement, source),
            target::NativeTarget::macos_arm64()
        ));
    }
    let changed_source =
        program_entry_plan::ProgramEntryPhysicalContractPackageSourceDigest::from_package_source(
            target::ProgramEntryPhysicalContractPackage::LinuxX86_64,
            b"different target implementation",
        );
    assert!(!physical_contract_matches(
        &linux_physical_contract(
            "accepted-package::LinuxPhysicalEntry::enter",
            changed_source
        ),
        target::NativeTarget::linux_x64()
    ));
    // A Darwin contract presented on the Linux target is a contract
    // substitution, not an alias.
    let darwin = physical_contract(
        program_entry_plan::MACOS_ARM64_PHYSICAL_REQUIREMENT_IDENTITY,
        program_entry_plan::exact_macos_arm64_physical_contract_package_source_digest(),
    );
    assert!(!physical_contract_matches(
        &darwin,
        target::NativeTarget::linux_x64()
    ));
}

fn linux_arm64_physical_contract(
    requirement: &str,
    source: program_entry_plan::ProgramEntryPhysicalContractPackageSourceDigest,
) -> ProgramEntryPhysicalContractPlan {
    let plan = program_entry_plan::exact_linux_arm64_physical_boundary_entry_plan();
    ProgramEntryPhysicalContractPlan::new(
        target::TargetProfile::LinuxArm64.program_entry_slot(),
        requirement.into(),
        target::ProgramEntryPhysicalContractPackage::LinuxArm64,
        source,
        0,
        [program_entry_plan::LINUX_ARM64_U64_TYPE_IDENTITY]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        program_entry_plan::LINUX_ARM64_I32_TYPE_IDENTITY.into(),
        plan.contract_report_fingerprint(),
        plan.plan().clone(),
    )
    .expect("well-shaped Linux ARM64 physical plan")
}

#[test]
fn linux_arm64_hosted_physical_replay_keeps_source_bytes_for_package_qualified_requirements() {
    let source = program_entry_plan::exact_linux_arm64_physical_contract_package_source_digest();
    for requirement in [
        program_entry_plan::LINUX_ARM64_PHYSICAL_REQUIREMENT_IDENTITY,
        "accepted-package::LinuxPhysicalEntry::enter",
    ] {
        assert!(physical_contract_matches(
            &linux_arm64_physical_contract(requirement, source),
            target::NativeTarget::linux_arm64()
        ));
        // The exact Linux ARM64 contract must not satisfy a different bridge.
        assert!(!physical_contract_matches(
            &linux_arm64_physical_contract(requirement, source),
            target::NativeTarget::linux_x64()
        ));
        assert!(!physical_contract_matches(
            &linux_arm64_physical_contract(requirement, source),
            target::NativeTarget::macos_arm64()
        ));
    }
    let changed_source =
        program_entry_plan::ProgramEntryPhysicalContractPackageSourceDigest::from_package_source(
            target::ProgramEntryPhysicalContractPackage::LinuxArm64,
            b"different target implementation",
        );
    assert!(!physical_contract_matches(
        &linux_arm64_physical_contract(
            "accepted-package::LinuxPhysicalEntry::enter",
            changed_source
        ),
        target::NativeTarget::linux_arm64()
    ));
    // An x86-64 contract presented on the ARM64 target is a contract
    // substitution, not an alias.
    let linux_x64 = linux_physical_contract(
        program_entry_plan::LINUX_X86_64_PHYSICAL_REQUIREMENT_IDENTITY,
        program_entry_plan::exact_linux_x86_64_physical_contract_package_source_digest(),
    );
    assert!(!physical_contract_matches(
        &linux_x64,
        target::NativeTarget::linux_arm64()
    ));
}

fn windows_physical_contract(
    requirement: &str,
    source: program_entry_plan::ProgramEntryPhysicalContractPackageSourceDigest,
) -> ProgramEntryPhysicalContractPlan {
    let plan = program_entry_plan::exact_windows_x86_64_physical_boundary_entry_plan();
    ProgramEntryPhysicalContractPlan::new(
        target::TargetProfile::WindowsX64.program_entry_slot(),
        requirement.into(),
        target::ProgramEntryPhysicalContractPackage::WindowsX64,
        source,
        0,
        Vec::new(),
        program_entry_plan::WINDOWS_X86_64_U32_TYPE_IDENTITY.into(),
        plan.contract_report_fingerprint(),
        plan.plan().clone(),
    )
    .expect("well-shaped Windows physical plan")
}

#[test]
fn windows_hosted_physical_replay_keeps_source_bytes_for_package_qualified_requirements() {
    let source = program_entry_plan::exact_windows_x86_64_physical_contract_package_source_digest();
    for requirement in [
        program_entry_plan::WINDOWS_X86_64_PHYSICAL_REQUIREMENT_IDENTITY,
        "accepted-package::WindowsProcessEntry::enter",
    ] {
        assert!(physical_contract_matches(
            &windows_physical_contract(requirement, source),
            target::NativeTarget::windows_x64()
        ));
        // The exact Windows contract must not satisfy a different bridge.
        assert!(!physical_contract_matches(
            &windows_physical_contract(requirement, source),
            target::NativeTarget::linux_x64()
        ));
        assert!(!physical_contract_matches(
            &windows_physical_contract(requirement, source),
            target::NativeTarget::macos_arm64()
        ));
    }
    let changed_source =
        program_entry_plan::ProgramEntryPhysicalContractPackageSourceDigest::from_package_source(
            target::ProgramEntryPhysicalContractPackage::WindowsX64,
            b"different target implementation",
        );
    assert!(!physical_contract_matches(
        &windows_physical_contract(
            "accepted-package::WindowsProcessEntry::enter",
            changed_source
        ),
        target::NativeTarget::windows_x64()
    ));
    // A Linux contract presented on the Windows target is a contract
    // substitution, not an alias.
    let linux = linux_physical_contract(
        program_entry_plan::LINUX_X86_64_PHYSICAL_REQUIREMENT_IDENTITY,
        program_entry_plan::exact_linux_x86_64_physical_contract_package_source_digest(),
    );
    assert!(!physical_contract_matches(
        &linux,
        target::NativeTarget::windows_x64()
    ));
}

fn hosted_receiver_binding(
    physical: ProgramEntryPhysicalContractPlan,
    ceiling_bytes: u64,
    receiver_byte_count: u64,
    receiver_alignment: u64,
) -> super::HostedReceiverBinding {
    let target = physical.target_slot().owner.native_target();
    super::HostedReceiverBinding {
        source: program_entry_plan::SelectedProgramEntrySourceSignature::from_checked_typed_entry(
            physical.target_slot(),
            symbols::SymbolHandle::from_arena_index(1),
            symbols::SymbolHandle::from_arena_index(2),
            "Main".into(),
            "main".into(),
            "Main::main".into(),
            program_entry_plan::ProgramEntrySourceReceiverSignature::ProvisionedMutable {
                normalized_type_identity: "named(name(Main))".into(),
            },
            Vec::new(),
        )
        .expect("well-shaped source signature"),
        physical,
        services: Vec::new(),
        demand: crate::StackDemand {
            psi: terminal_psi::TerminalPsiIdentity {
                vocabulary_marker: terminal_psi::VocabularyMarker::CURRENT,
                program_fingerprint: terminal_psi::SemanticFingerprint::from_bytes([7; 32]),
            },
            target,
            entry: semantic_vocabulary::MachineId::new(1).unwrap(),
            ceiling_bytes,
            stack_alignment: 16,
            contributing_machines: Default::default(),
            admitted_contribution_report_identities: Default::default(),
            admitted_contribution_commitments: Default::default(),
        },
        receiver_byte_count,
        receiver_alignment,
        cleanup_occupancy: false,
    }
}

#[test]
fn hosted_receiver_partitions_reserve_disjoint_aligned_residences() {
    let linux_source =
        program_entry_plan::exact_linux_x86_64_physical_contract_package_source_digest();
    let linux = |ceiling_bytes: u64, receiver_byte_count: u64, receiver_alignment: u64| {
        hosted_receiver_binding(
            linux_physical_contract(
                program_entry_plan::LINUX_X86_64_PHYSICAL_REQUIREMENT_IDENTITY,
                linux_source,
            ),
            ceiling_bytes,
            receiver_byte_count,
            receiver_alignment,
        )
    };
    for (existing, ceiling, byte_count, alignment) in [
        (0u64, 0, 0, 1),
        (3, 5, 17, 8),
        (48, 33, 64, 64),
        (1024, 4096, 4, 256),
    ] {
        let partitions = linux(ceiling, byte_count, alignment)
            .partitions(existing)
            .unwrap_or_else(|error| {
                panic!("{existing}/{ceiling}/{byte_count}/{alignment}: {error:?}")
            });
        let saved_end = partitions.saved_continuation_offset + 16;
        let stack_end = partitions.stack_offset + partitions.stack_byte_count;
        assert!(
            partitions.saved_continuation_offset >= existing
                && partitions.saved_continuation_offset % 16 == 0
                && saved_end <= partitions.stack_offset
                && partitions.stack_byte_count >= ceiling.max(16)
                && partitions.stack_byte_count % 16 == 0
                && stack_end <= partitions.receiver_offset
                && partitions.receiver_offset % alignment.max(16) == 0
                && partitions.receiver_byte_count == byte_count.max(1)
                && partitions.receiver_offset + partitions.receiver_byte_count
                    == partitions.end_offset,
            "{existing}/{ceiling}/{byte_count}/{alignment}: {partitions:?}"
        );
    }
    // The Windows caller owes shadow space plus the pushed return address on
    // top of the checked demand; the private stack partition carries it.
    let windows = hosted_receiver_binding(
        windows_physical_contract(
            program_entry_plan::WINDOWS_X86_64_PHYSICAL_REQUIREMENT_IDENTITY,
            program_entry_plan::exact_windows_x86_64_physical_contract_package_source_digest(),
        ),
        0,
        8,
        8,
    )
    .partitions(0)
    .unwrap();
    assert_eq!(windows.stack_byte_count, 64);
    // Occupancy that cannot be represented rejects instead of overflowing.
    assert!(linux(u64::MAX, 8, 8).partitions(0).is_err());
    assert!(linux(8, 8, 8).partitions(u64::MAX - 4).is_err());
    // A non-power-of-two receiver alignment has no valid residence.
    assert!(linux(8, 8, 24).partitions(0).is_err());
}
