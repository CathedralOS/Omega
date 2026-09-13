use super::*;

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
