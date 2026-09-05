use super::budgets::fixture_elements;
use super::*;
use crate::record::*;
use omega_calling_conventions::{
    CallSignature, CallingPolicy, ValueShape, evaluate_ordinary_boundary_entry_plan,
};
use psi_core::PackageKeyIdentity;

fn nominal(path: &str) -> PackageReviewNominalIdentity {
    PackageReviewNominalIdentity {
        owner: PackageReviewNominalOwner::Package(
            PackageKeyIdentity::from_digest([7; 32]).unwrap(),
        ),
        path: path.to_owned(),
    }
}

fn value_type(canonical: &str) -> PackageReviewTypeIdentity {
    PackageReviewTypeIdentity {
        canonical: canonical.to_owned(),
    }
}

fn conformance() -> PackagePolicyClosedConformanceApplication {
    PackagePolicyClosedConformanceApplication {
        declaration: nominal("Chosen"),
        lifetime_arguments: Vec::new(),
        type_arguments: vec![value_type("u64")],
        const_arguments: Vec::new(),
        machine_arguments: Vec::new(),
        subject: Some(value_type("Carrier")),
        trait_identity: nominal("Representation"),
        trait_lifetime_arguments: Vec::new(),
        trait_arguments: vec![value_type("Opaque")],
        rows: Vec::new(),
    }
}

pub(super) fn fixture() -> PackagePolicyCallingPlan {
    let native = evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::SystemVAMD64,
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
        target: PackageReviewRepresentationTarget {
            profile: PackageReviewRepresentationTargetProfile::LinuxX64,
            architecture: PackageReviewRepresentationArchitecture::X86_64,
            object_format: PackageReviewRepresentationObjectFormat::Elf,
            pointer_size: 8,
            pointer_alignment: 8,
        },
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
            name: "value".into(),
            value_type: value_type("u64"),
            is_mutable: false,
            is_const: false,
            shape_root: 0,
        }],
        semantic_result: None,
        native_parameters: vec![PackagePolicyNativeParameter {
            name: "value".into(),
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
        physical: PackagePolicyPhysicalCallingContract::from_validated_plan(&native),
    }
}

pub(in crate::encoding::recovery::policy) fn complete_fixture() -> PackagePolicyCallingPlan {
    let mut policy = fixture();
    let native = evaluate_ordinary_boundary_entry_plan(
        CallingPolicy::SystemVAMD64,
        &CallSignature {
            parameters: vec![ValueShape::integer(8, 8); 3],
            result: None,
        },
    )
    .unwrap();
    policy.physical = PackagePolicyPhysicalCallingContract::from_validated_plan(&native);
    policy.shape_graph.shapes.push(PackageReviewBoundaryShape {
        class: PackageReviewBoundaryShapeClass::Reference,
        byte_size: 8,
        alignment: 8,
    });
    policy.shape_graph.parameters.push(1);
    policy
        .semantic_parameters
        .push(PackagePolicyCallingParameter {
            name: "context".into(),
            value_type: value_type("&Context"),
            is_mutable: false,
            is_const: false,
            shape_root: 1,
        });
    policy.native_parameters.push(PackagePolicyNativeParameter {
        name: "context".into(),
        origin: PackagePolicyNativeParameterOrigin::SemanticFormal {
            formal_ordinal: 1,
            shape_root: 1,
        },
    });
    policy.native_parameters.push(PackagePolicyNativeParameter {
        name: "callback".into(),
        origin: PackagePolicyNativeParameterOrigin::PrivateCallback {
            binder_index: 0,
            byte_size: 8,
            alignment: 8,
        },
    });
    policy.static_parameters.push(PackageReviewTypeParameter {
        kind: PackageReviewTypeParameterKind::Machine(
            PackageReviewMachineParameterContract::Nominal {
                trait_identity: nominal("Callback"),
                requirement_identity: nominal("Callback::call"),
            },
        ),
        bounds: PackageReviewDataProperties {
            multiplicity: psi_language_semantics::Multiplicity::Unrestricted,
            carry: None,
        },
    });
    policy.callbacks.binders.push(PackagePolicyCallbackBinder {
        parameter: nominal("Boundary::call::callback"),
        static_parameter_ordinal: 0,
        static_machine_ordinal: 0,
        requirement: nominal("Callback::call"),
    });
    let root = PackagePolicyCallbackLayoutApplication {
        policy: nominal("ContextLayout"),
        schema: value_type("Context"),
        byte_size: 32,
        alignment: 8,
    };
    policy.callbacks.layouts = vec![
        PackagePolicyCallbackLayout {
            formal_ordinal: 1,
            native_ordinal: 1,
            root_layout: root.clone(),
            inline_field: None,
            terminal_slot: conformance(),
            terminal_offset: 0,
            terminal_byte_size: 8,
            terminal_alignment: 8,
            composed_offset: 0,
        },
        PackagePolicyCallbackLayout {
            formal_ordinal: 1,
            native_ordinal: 1,
            root_layout: root,
            inline_field: Some(PackagePolicyCallbackInlineField {
                field: nominal("Context::child"),
                offset: 8,
                extent: 16,
                alignment: 8,
                child_layout: PackagePolicyCallbackLayoutApplication {
                    policy: nominal("ChildLayout"),
                    schema: value_type("Child"),
                    byte_size: 16,
                    alignment: 8,
                },
            }),
            terminal_slot: conformance(),
            terminal_offset: 8,
            terminal_byte_size: 8,
            terminal_alignment: 8,
            composed_offset: 16,
        },
    ];
    for destination in [
        PackagePolicyCallbackDestination::Parameter { native_ordinal: 2 },
        PackagePolicyCallbackDestination::Field {
            native_ordinal: 1,
            layout_index: 0,
        },
        PackagePolicyCallbackDestination::Field {
            native_ordinal: 1,
            layout_index: 1,
        },
    ] {
        policy.callbacks.demands.push(PackagePolicyCallbackDemand {
            destination: destination.clone(),
            requirement: nominal("Callback::call"),
        });
        policy
            .callbacks
            .materializations
            .push(PackagePolicyCallbackMaterialization {
                binder_index: 0,
                destination,
            });
    }
    policy.opaque_uses.push(PackagePolicyCallingOpaqueUse {
        opaque: nominal("Opaque"),
        carrier: nominal("Carrier"),
        selection_owner: nominal("build").owner,
        application: conformance(),
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
    });
    policy
}

fn recover(bytes: &[u8]) -> Result<PackagePolicyCallingPlan, Error> {
    PackagePolicyCallingPlan::recover_canonical(bytes, PackagePolicyRecoveryLimits::default())
}

#[test]
fn complete_callback_and_opaque_children_round_trip_without_nested_envelopes() {
    for policy in [fixture(), complete_fixture()] {
        let bytes = policy.canonical_bytes().unwrap();
        assert_eq!(recover(&bytes).unwrap(), policy);
        assert_eq!(recover(&bytes).unwrap().canonical_bytes().unwrap(), bytes);
        for magic in [
            crate::encoding::CONFORMANCE_POLICY_MAGIC,
            crate::encoding::PHYSICAL_CALLING_POLICY_MAGIC,
        ] {
            assert!(!bytes.windows(magic.len()).any(|window| window == magic));
        }
    }
}

#[test]
fn all_truncations_versions_and_trailing_bytes_reject() {
    let bytes = complete_fixture().canonical_bytes().unwrap();
    for end in 0..bytes.len() {
        assert!(recover(&bytes[..end]).is_err(), "prefix {end}");
    }
    let mut changed = bytes.clone();
    changed[0] ^= 1;
    assert_eq!(recover(&changed), Err(Error::UnsupportedVersion));
    let mut changed = bytes.clone();
    changed[CALLING_POLICY_MAGIC.len()] = 255;
    assert_eq!(recover(&changed), Err(Error::UnsupportedVersion));
    let mut changed = bytes;
    changed.push(0);
    assert_eq!(recover(&changed), Err(Error::TrailingBytes));
}

#[test]
fn nested_components_share_aggregate_byte_and_allocation_budgets() {
    let complete = complete_fixture();
    let bytes = complete.canonical_bytes().unwrap();
    for limits in [
        PackagePolicyRecoveryLimits::new(
            bytes.len() - 1,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
        ),
        PackagePolicyRecoveryLimits::new(usize::MAX, 1, usize::MAX, usize::MAX, usize::MAX),
        PackagePolicyRecoveryLimits::new(usize::MAX, usize::MAX, 1, usize::MAX, usize::MAX),
        PackagePolicyRecoveryLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX),
    ] {
        assert!(PackagePolicyCallingPlan::recover_canonical(&bytes, limits).is_err());
    }
    // The fixture retains one type argument in each of three conformance
    // applications. A single aggregate budget must account for all of them.
    let minimal_policy = fixture();
    let minimal = minimal_policy.canonical_bytes().unwrap();
    let base_elements = fixture_elements(&minimal_policy);
    let full_elements = fixture_elements(&complete);
    assert!(full_elements > base_elements);
    let limits = PackagePolicyRecoveryLimits::new(
        usize::MAX,
        usize::MAX,
        base_elements,
        usize::MAX,
        usize::MAX,
    );
    assert!(PackagePolicyCallingPlan::recover_canonical(&minimal, limits).is_ok());
    assert_eq!(
        PackagePolicyCallingPlan::recover_canonical(&bytes, limits),
        Err(Error::ElementLimitExceeded)
    );
    for (encoded, expected, elements) in [
        (&minimal, &minimal_policy, base_elements),
        (&bytes, &complete, full_elements),
    ] {
        let exact = PackagePolicyRecoveryLimits::new(
            encoded.len(),
            usize::MAX,
            elements,
            usize::MAX,
            usize::MAX,
        );
        assert_eq!(
            &PackagePolicyCallingPlan::recover_canonical(encoded, exact).unwrap(),
            expected
        );
        let short = PackagePolicyRecoveryLimits::new(
            encoded.len(),
            usize::MAX,
            elements - 1,
            usize::MAX,
            usize::MAX,
        );
        assert_eq!(
            PackagePolicyCallingPlan::recover_canonical(encoded, short),
            Err(Error::ElementLimitExceeded)
        );
    }
}

#[test]
fn every_semantic_child_changes_complete_policy_bytes() {
    let original = complete_fixture();
    let baseline = original.canonical_bytes().unwrap();
    let mutations: &[fn(&mut PackagePolicyCallingPlan)] = &[
        |p| p.boundary_trait = nominal("OtherBoundary"),
        |p| p.boundary_arguments.push(value_type("Other")),
        |p| p.requirement = nominal("Boundary::other"),
        |p| p.requirement_trait = nominal("OtherOwner"),
        |p| p.requirement_arguments.push(value_type("Other")),
        |p| p.semantic_parameters[0].value_type = value_type("Opaque"),
        |p| p.semantic_parameters[0].is_mutable = true,
        |p| p.callbacks.binders[0].parameter = nominal("Boundary::call::other"),
        |p| {
            for layout in &mut p.callbacks.layouts {
                layout.root_layout.policy = nominal("OtherLayout");
            }
        },
        |p| {
            for layout in &mut p.callbacks.layouts {
                layout.root_layout.schema = value_type("OtherSchema");
            }
        },
        |p| p.callbacks.layouts[0].terminal_slot.declaration = nominal("OtherSlot"),
        |p| p.callbacks.layouts[1].inline_field.as_mut().unwrap().field = nominal("Context::other"),
        |p| {
            p.callbacks.layouts[1]
                .inline_field
                .as_mut()
                .unwrap()
                .child_layout
                .policy = nominal("OtherChildLayout")
        },
        |p| p.opaque_uses[0].opaque = nominal("OtherOpaque"),
        |p| p.opaque_uses[0].carrier = nominal("OtherCarrier"),
        |p| {
            p.opaque_uses[0].selection_owner = PackageReviewNominalOwner::Package(
                PackageKeyIdentity::from_digest([8; 32]).unwrap(),
            )
        },
        |p| {
            p.opaque_uses[0]
                .application
                .type_arguments
                .push(value_type("Other"))
        },
        |p| {
            p.opaque_uses[0].copy_disposition =
                PackageReviewOpaqueRepresentationCopyDisposition::CheckedSemanticCopy
        },
        |p| p.physical.shadow_bytes = 32,
    ];
    for (index, mutate) in mutations.iter().enumerate() {
        let mut changed = original.clone();
        mutate(&mut changed);
        let bytes = changed
            .canonical_bytes()
            .unwrap_or_else(|error| panic!("mutation {index}: {error}"));
        assert_ne!(bytes, baseline, "mutation {index}");
        assert_eq!(recover(&bytes).unwrap(), changed);
    }
}

#[test]
fn invalid_cross_component_coordinates_reject_before_publication() {
    let mutations: &[fn(&mut PackagePolicyCallingPlan)] = &[
        |p| p.native_parameters[0].name = "other".into(),
        |p| p.shape_graph.parameters[0] = u16::MAX,
        |p| p.semantic_parameters[0].shape_root = u16::MAX,
        |p| p.callbacks.binders[0].static_parameter_ordinal = u32::MAX,
        |p| p.callbacks.materializations[0].binder_index = u32::MAX,
        |p| {
            p.callbacks.demands[1].destination = PackagePolicyCallbackDestination::Field {
                native_ordinal: 1,
                layout_index: u32::MAX,
            }
        },
        |p| p.callbacks.layouts[1].composed_offset = u64::MAX,
        |p| p.opaque_uses[0].occurrences[0].carrier_shape_root = u16::MAX,
        |p| p.callbacks.layouts.swap(0, 1),
        |p| p.opaque_uses.push(p.opaque_uses[0].clone()),
    ];
    for (index, mutate) in mutations.iter().enumerate() {
        let mut policy = complete_fixture();
        mutate(&mut policy);
        assert!(policy.canonical_bytes().is_err(), "mutation {index}");
    }
}
