use super::*;
use crate::record::*;
use calling_conventions::{
    CallSignature, CallingPolicy, ValueShape, evaluate_ordinary_boundary_entry_plan,
};

fn nominal(path: &str) -> PackageReviewNominalIdentity {
    PackageReviewNominalIdentity {
        owner: PackageReviewNominalOwner::Package(
            semantic_vocabulary::PackageKeyIdentity::from_digest([0x73; 32]).unwrap(),
        ),
        path: path.to_owned(),
    }
}

fn value_type() -> PackageReviewTypeIdentity {
    PackageReviewTypeIdentity {
        canonical: "u64".to_owned(),
    }
}

fn application() -> PackagePolicyClosedConformanceApplication {
    PackagePolicyClosedConformanceApplication {
        declaration: nominal("Selected"),
        lifetime_arguments: Vec::new(),
        type_arguments: Vec::new(),
        const_arguments: Vec::new(),
        machine_arguments: Vec::new(),
        subject: Some(value_type()),
        trait_identity: nominal("Marker"),
        trait_lifetime_arguments: Vec::new(),
        trait_arguments: Vec::new(),
        rows: Vec::new(),
    }
}

#[test]
fn nested_conformance_lifetimes_stay_within_the_calling_telescope() {
    let mut policy = policy();
    policy.boundary_lifetime_parameter_count = 2;
    policy.requirement_lifetime_parameter_count = 1;
    let mut application = application();
    application.lifetime_arguments = vec![2, 0];
    application.trait_lifetime_arguments = vec![1];
    assert!(validate_application_lifetimes(&policy, &application).is_ok());
    application.lifetime_arguments.push(3);
    assert!(validate_application_lifetimes(&policy, &application).is_err());
    application.lifetime_arguments.pop();
    application.trait_lifetime_arguments.push(u32::MAX);
    assert!(validate_application_lifetimes(&policy, &application).is_err());
    policy.boundary_lifetime_parameter_count = u32::MAX;
    assert!(validate_application_lifetimes(&policy, &application).is_err());
}

fn windows_target() -> PackageReviewRepresentationTarget {
    PackageReviewRepresentationTarget {
        profile: PackageReviewRepresentationTargetProfile::WindowsX64,
        architecture: PackageReviewRepresentationArchitecture::X86_64,
        object_format: PackageReviewRepresentationObjectFormat::Coff,
        pointer_size: 8,
        pointer_alignment: 8,
    }
}

fn policy() -> PackagePolicyCallingPlan {
    let validated = evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::MicrosoftX64,
        &CallSignature {
            parameters: vec![ValueShape::integer(8, 8)],
            result: None,
        },
    )
    .unwrap();
    PackagePolicyCallingPlan {
        boundary_trait: nominal("Boundary"),
        boundary_arguments: Vec::new(),
        boundary_lifetime_parameter_count: 0,
        requirement: nominal("Boundary::call"),
        requirement_trait: nominal("Boundary"),
        requirement_arguments: Vec::new(),
        requirement_lifetime_arguments: Vec::new(),
        requirement_lifetime_parameter_count: 0,
        static_parameters: Vec::new(),
        target: windows_target(),
        shape_graph: PackageReviewBoundaryShapeGraph {
            shapes: vec![PackageReviewBoundaryShape {
                class: PackageReviewBoundaryShapeClass::Integer,
                byte_size: 8,
                alignment: 8,
            }],
            fields: Vec::new(),
            parameters: vec![0],
            result: None,
        },
        semantic_parameters: vec![PackagePolicyCallingParameter {
            name: "value".to_owned(),
            value_type: value_type(),
            is_mutable: false,
            is_const: false,
            shape_root: 0,
        }],
        semantic_result: None,
        native_parameters: vec![PackagePolicyNativeParameter {
            name: "value".to_owned(),
            origin: PackagePolicyNativeParameterOrigin::SemanticFormal {
                formal_ordinal: 0,
                shape_root: 0,
            },
        }],
        callbacks: PackagePolicyCallbacks {
            binders: Vec::new(),
            demands: Vec::new(),
            materializations: Vec::new(),
            layouts: Vec::new(),
        },
        opaque_uses: Vec::new(),
        physical: PackagePolicyPhysicalCallingContract::from_validated_plan(&validated),
    }
}

fn opaque(name: &str, policy: &PackagePolicyCallingPlan) -> PackagePolicyCallingOpaqueUse {
    PackagePolicyCallingOpaqueUse {
        opaque: nominal(name),
        carrier: nominal("Carrier"),
        selection_owner: nominal("build").owner,
        application: application(),
        origin: PackageReviewOpaqueRepresentationApplicationOrigin::NamedConformance,
        lifecycle: PackageReviewOpaqueRepresentationLifecycleDisposition::Inert,
        copy_disposition: PackageReviewOpaqueRepresentationCopyDisposition::PlacementOnly,
        occurrences: vec![PackageReviewOpaqueRepresentationOccurrence {
            carrier_shape_root: 0,
            role: PackageReviewOpaqueRepresentationMovementRole::Parameter {
                formal_ordinal: 0,
                native_ordinal: 0,
            },
            path: Vec::new(),
            placement: policy.physical.parameters[0].clone(),
        }],
    }
}

fn layout(name: &str, offset: u64) -> PackagePolicyCallbackLayout {
    let mut terminal_slot = application();
    terminal_slot.declaration = nominal(name);
    PackagePolicyCallbackLayout {
        formal_ordinal: 0,
        native_ordinal: 0,
        root_layout: PackagePolicyCallbackLayoutApplication {
            policy: nominal("Spread"),
            schema: value_type(),
            byte_size: 24,
            alignment: 8,
        },
        inline_field: None,
        terminal_slot,
        terminal_offset: offset,
        terminal_byte_size: 8,
        terminal_alignment: 8,
        composed_offset: offset,
    }
}

#[test]
fn exact_target_profiles_reject_crossed_coordinates_without_using_recovery_host() {
    use PackageReviewRepresentationArchitecture as Architecture;
    use PackageReviewRepresentationObjectFormat as ObjectFormat;
    use PackageReviewRepresentationTargetProfile as Profile;
    for (profile, architecture, object_format) in [
        (
            Profile::LinuxArm64,
            Architecture::Aarch64,
            ObjectFormat::Elf,
        ),
        (Profile::LinuxX64, Architecture::X86_64, ObjectFormat::Elf),
        (
            Profile::MacosArm64,
            Architecture::Aarch64,
            ObjectFormat::MachO,
        ),
        (
            Profile::WindowsX64,
            Architecture::X86_64,
            ObjectFormat::Coff,
        ),
        (Profile::UefiX64, Architecture::X86_64, ObjectFormat::Coff),
    ] {
        let valid = PackageReviewRepresentationTarget {
            profile,
            architecture,
            object_format,
            pointer_size: 8,
            pointer_alignment: 8,
        };
        assert!(target::validate(valid).is_ok());
        let mut wrong = valid;
        wrong.architecture = if architecture == Architecture::Aarch64 {
            Architecture::X86_64
        } else {
            Architecture::Aarch64
        };
        assert!(target::validate(wrong).is_err());
        wrong = valid;
        wrong.object_format = if object_format == ObjectFormat::Elf {
            ObjectFormat::Coff
        } else {
            ObjectFormat::Elf
        };
        assert!(target::validate(wrong).is_err());
        for pointer in [0, 1, 4, 16, u16::MAX] {
            wrong = valid;
            wrong.pointer_size = pointer;
            assert!(target::validate(wrong).is_err());
            wrong = valid;
            wrong.pointer_alignment = pointer;
            assert!(target::validate(wrong).is_err());
        }
    }
    for profile in [Profile::CrossPlatformCli, Profile::LocalUnchecked] {
        for architecture in [Architecture::Aarch64, Architecture::X86_64] {
            for object_format in [ObjectFormat::Elf, ObjectFormat::MachO, ObjectFormat::Coff] {
                assert!(
                    target::validate(PackageReviewRepresentationTarget {
                        profile,
                        architecture,
                        object_format,
                        pointer_size: 8,
                        pointer_alignment: 8,
                    })
                    .is_ok()
                );
            }
        }
        assert!(
            target::validate(PackageReviewRepresentationTarget {
                profile,
                pointer_size: 0,
                ..windows_target()
            })
            .is_err()
        );
    }
}

#[test]
fn callback_slots_cannot_overlap_under_distinct_nominal_names() {
    let first = layout("First", 8);
    let adjacent = layout("Second", 16);
    assert!(callbacks::validate_disjoint_layouts(&[first.clone(), adjacent]).is_ok());
    let duplicate = layout("Second", 8);
    assert!(callbacks::validate_disjoint_layouts(&[first.clone(), duplicate.clone()]).is_err());
    let partial = layout("Second", 12);
    assert!(callbacks::validate_disjoint_layouts(&[first.clone(), partial]).is_err());
    let mut other_parameter = duplicate;
    other_parameter.native_ordinal = 1;
    assert!(callbacks::validate_disjoint_layouts(&[first, other_parameter]).is_ok());
    assert!(callbacks::validate_disjoint_layouts(&[layout("Overflow", u64::MAX)]).is_err());
}

#[test]
fn opaque_occurrence_has_one_nominal_owner_and_bounded_catalog() {
    let mut policy = policy();
    policy.opaque_uses = vec![opaque("First", &policy)];
    assert!(policy.validate_canonical_structure().is_ok());
    policy.opaque_uses.push(opaque("Second", &policy));
    policy.opaque_uses.sort();
    assert_eq!(
        policy.validate_canonical_structure(),
        Err("calling opaque occurrence has more than one nominal owner")
    );
    let mut disjoint = policy.clone();
    let mut second_formal = disjoint.semantic_parameters[0].clone();
    second_formal.name = "second".to_owned();
    disjoint.semantic_parameters.push(second_formal);
    disjoint.shape_graph.parameters.push(0);
    disjoint
        .native_parameters
        .push(PackagePolicyNativeParameter {
            name: "second".to_owned(),
            origin: PackagePolicyNativeParameterOrigin::SemanticFormal {
                formal_ordinal: 1,
                shape_root: 0,
            },
        });
    let validated = evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::MicrosoftX64,
        &CallSignature {
            parameters: vec![ValueShape::integer(8, 8); 2],
            result: None,
        },
    )
    .unwrap();
    disjoint.physical = PackagePolicyPhysicalCallingContract::from_validated_plan(&validated);
    disjoint.opaque_uses[1].occurrences[0].role =
        PackageReviewOpaqueRepresentationMovementRole::Parameter {
            formal_ordinal: 1,
            native_ordinal: 1,
        };
    disjoint.opaque_uses[1].occurrences[0].placement = disjoint.physical.parameters[1].clone();
    assert!(disjoint.validate_canonical_structure().is_ok());
    policy.opaque_uses.truncate(1);
    policy.opaque_uses[0].opaque.owner = PackageReviewNominalOwner::Unresolved;
    assert!(policy.validate_canonical_structure().is_err());
    policy.opaque_uses[0].opaque = nominal("First");
    policy.opaque_uses[0].selection_owner = PackageReviewNominalOwner::Unresolved;
    assert!(policy.validate_canonical_structure().is_err());
    policy.opaque_uses[0].selection_owner = nominal("build").owner;
    let occurrence = policy.opaque_uses[0].occurrences[0].clone();
    policy.opaque_uses[0].occurrences = vec![occurrence; 257];
    assert_eq!(
        policy.validate_canonical_structure(),
        Err("calling opaque catalog exceeds normalized shape capacity")
    );
}

#[test]
fn malicious_indices_cycles_and_empty_nominals_reject_without_panicking() {
    assert!(policy().validate_canonical_structure().is_ok());
    let mutations: &[fn(&mut PackagePolicyCallingPlan)] = &[
        |policy| policy.requirement.path.clear(),
        |policy| policy.requirement_trait.owner = PackageReviewNominalOwner::Unresolved,
        |policy| policy.shape_graph.result = Some(u16::MAX),
        |policy| policy.shape_graph.parameters[0] = u16::MAX,
        |policy| {
            policy.shape_graph.shapes[0].class = PackageReviewBoundaryShapeClass::FixedArray {
                element: u16::MAX,
                length: 1,
            }
        },
        |policy| {
            policy.shape_graph.shapes[0].class = PackageReviewBoundaryShapeClass::FixedArray {
                element: 0,
                length: 1,
            }
        },
        |policy| {
            policy.shape_graph.shapes[0].class = PackageReviewBoundaryShapeClass::Record {
                first_field: u16::MAX,
                field_count: u16::MAX,
            }
        },
        |policy| {
            policy.native_parameters[0].origin =
                PackagePolicyNativeParameterOrigin::SemanticFormal {
                    formal_ordinal: u32::MAX,
                    shape_root: u16::MAX,
                }
        },
        |policy| {
            policy.native_parameters[0].origin =
                PackagePolicyNativeParameterOrigin::PrivateCallback {
                    binder_index: u32::MAX,
                    byte_size: 8,
                    alignment: 8,
                }
        },
        |policy| {
            policy.callbacks.binders = vec![PackagePolicyCallbackBinder {
                parameter: nominal("binder"),
                static_parameter_ordinal: u32::MAX,
                static_machine_ordinal: 0,
                requirement: nominal("Callback::call"),
            }]
        },
        |policy| policy.callbacks.layouts = vec![layout("Slot", 8); 33],
    ];
    for (index, mutate) in mutations.iter().enumerate() {
        let mut changed = policy();
        mutate(&mut changed);
        assert!(
            changed.validate_canonical_structure().is_err(),
            "malformed policy {index}"
        );
    }
}
