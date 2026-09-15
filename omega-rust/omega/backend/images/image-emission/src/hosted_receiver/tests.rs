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
        assert!(receiver_pointer_matches(shape, placement));
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
                !receiver_pointer_matches(shape, &corrupted),
                "corrupted receiver: {corrupted:?}"
            );
        }
        let changed_shape = ValueShape::borrowed_reference(shape.byte_size + 1, shape.alignment);
        assert!(!receiver_pointer_matches(changed_shape, placement));
        assert!(!receiver_pointer_matches(
            ValueShape::integer(shape.byte_size, shape.alignment),
            placement
        ));
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
        assert!(physical_contract_matches(&physical_contract(
            requirement,
            source
        )));
    }
    let changed_source =
        program_entry_plan::ProgramEntryPhysicalContractPackageSourceDigest::from_package_source(
            target::ProgramEntryPhysicalContractPackage::MacosArm64,
            b"different target implementation",
        );
    assert!(!physical_contract_matches(&physical_contract(
        "accepted-package::MacosPhysicalEntry::enter",
        changed_source
    )));
}
