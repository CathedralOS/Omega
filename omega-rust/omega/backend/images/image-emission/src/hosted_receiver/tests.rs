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
