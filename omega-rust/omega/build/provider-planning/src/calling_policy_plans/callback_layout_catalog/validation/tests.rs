use super::super::{BoundaryCallbackInlineField, BoundaryCallbackLayoutEntry};
use super::*;
use crate::calling_policy_plans::materialized_boundary_signature_from_abi;
use calling_conventions::{CallSignature, LayoutPlanId, NativeParameterId, ValueShape};
use layout::{
    TargetClosedPlanLaidDataLayoutIdentity, TargetClosedPrivateCallbackDemand, TypeLayout,
};
use symbols::SymbolHandle;
use target::NativeTarget;
use typed_trees::typed_trees::{
    ClosedConformanceApplication, ClosedConformanceApplicationCommitment,
};

pub(in crate::calling_policy_plans) fn signature_fixture() -> MaterializedBoundarySignature {
    let target = NativeTarget::host();
    let pointer = target.pointer_size;
    let alignment = target.pointer_alignment;
    let shape = ValueShape::integer(pointer as u16, alignment as u16);
    let mut signature = materialized_boundary_signature_from_abi(&CallSignature {
        parameters: vec![shape],
        result: None,
    })
    .unwrap();
    let data_symbol = SymbolHandle::from_arena_index(127);
    let parameter = &mut signature.native_parameters[0];
    parameter.identity = NativeParameterId::new(107).unwrap();
    parameter.layout_data_symbol = data_symbol;
    let layout = LayoutPlanId::new(109).unwrap();
    let slot_identity = "fixture::CallbackSlot".into();
    let requirement_identity = "fixture::Callback::call".into();
    let terminal_slot = TargetClosedPrivateCallbackDemand {
        data_symbol,
        slot_identity,
        slot_application: ClosedConformanceApplication {
            arguments: Box::new([]),
            declaration: SymbolHandle::from_arena_index(131),
            trait_definition: SymbolHandle::from_arena_index(137),
            commitment: ClosedConformanceApplicationCommitment::from_digest([7; 32]),
            ..ClosedConformanceApplication::default()
        },
        layout_subject_identity: "fixture::Layout".into(),
        callback_requirement_identity: requirement_identity,
        layout,
        slot: callback_layout_slot_id(layout, "fixture::CallbackSlot"),
        requirement: callback_requirement_id("fixture::Callback::call"),
        offset: pointer,
        byte_size: pointer,
        alignment,
    };
    let demand = terminal_slot.native_demand(parameter.identity);
    signature
        .callback_layout_catalog
        .push(BoundaryCallbackLayoutEntry {
            formal_ordinal: 0,
            native_ordinal: 0,
            destination: demand.destination.clone(),
            root_layout: TargetClosedPlanLaidDataLayoutIdentity {
                data_symbol,
                data_identity: "fixture::Record".into(),
                layout_subject_identity: terminal_slot.layout_subject_identity.clone(),
                // Named root and legacy terminal layouts intentionally use different IDs.
                layout: LayoutPlanId::new(139).unwrap(),
                physical: TypeLayout {
                    size: 4 * pointer,
                    alignment,
                },
            },
            inline_field: None,
            terminal_slot,
            composed_offset: pointer,
        });
    signature.callback_demands.push(demand);
    signature
}

fn two_hop_fixture() -> MaterializedBoundarySignature {
    let mut signature = signature_fixture();
    let pointer = signature.native_target.pointer_size;
    let alignment = signature.native_target.pointer_alignment;
    let entry = &mut signature.callback_layout_catalog[0];
    let child_symbol = SymbolHandle::from_arena_index(149);
    entry.terminal_slot.data_symbol = child_symbol;
    entry.terminal_slot.layout_subject_identity = "fixture::ChildLayout".into();
    let field_identity = "fixture::Record::child";
    entry.inline_field = Some(BoundaryCallbackInlineField {
        symbol: SymbolHandle::from_arena_index(151),
        identity: field_identity.into(),
        offset: pointer,
        extent: 2 * pointer,
        alignment,
        child_layout: TargetClosedPlanLaidDataLayoutIdentity {
            data_symbol: child_symbol,
            data_identity: "fixture::Child".into(),
            layout_subject_identity: entry.terminal_slot.layout_subject_identity.clone(),
            layout: LayoutPlanId::new(157).unwrap(),
            physical: TypeLayout {
                size: 2 * pointer,
                alignment,
            },
        },
    });
    entry.composed_offset = 2 * pointer;
    entry.destination = NativePlace::Field {
        parameter: signature.native_parameters[0].identity,
        layout: entry.root_layout.layout,
        field_path: vec![
            callback_layout_field_slot_id(entry.root_layout.layout, field_identity),
            entry.terminal_slot.slot,
        ],
    };
    signature.callback_demands[0].destination = entry.destination.clone();
    signature
}

#[test]
fn direct_and_two_hop_catalogs_replay_with_distinct_root_and_terminal_layouts() {
    for signature in [signature_fixture(), two_hop_fixture()] {
        assert_ne!(
            signature.callback_layout_catalog[0].root_layout.layout,
            signature.callback_layout_catalog[0].terminal_slot.layout,
        );
        validate(&signature, &signature.callback_demands).unwrap();
    }
}

#[test]
fn named_catalog_preserves_existing_boundary_application_bytes() {
    use crate::calling_policy_plans::boundary_plan_application_identity;
    use calling_conventions::{CallingPolicy, evaluate_ordinary_boundary_entry_plan};

    for mut signature in [signature_fixture(), two_hop_fixture()] {
        let target = signature.native_target;
        let abi = CallSignature {
            parameters: vec![ValueShape::integer(
                u16::try_from(target.pointer_size).unwrap(),
                u16::try_from(target.pointer_alignment).unwrap(),
            )],
            result: None,
        };
        let validated =
            evaluate_ordinary_boundary_entry_plan(CallingPolicy::native_for_target(target), &abi)
                .unwrap();
        let retained = boundary_plan_application_identity(&signature, &validated);
        signature.callback_layout_catalog.clear();
        assert_eq!(
            retained,
            boundary_plan_application_identity(&signature, &validated),
            "named semantic custody is separate from the existing ABI byte vocabulary",
        );
        assert!(validate(&signature, &signature.callback_demands).is_err());
    }
}

#[test]
fn catalog_requires_every_field_demand_once_in_exact_order() {
    let mut signature = signature_fixture();
    let mut second = signature.callback_layout_catalog[0].clone();
    second.terminal_slot.slot_identity = "fixture::SecondSlot".into();
    second.terminal_slot.slot = callback_layout_slot_id(
        second.terminal_slot.layout,
        &second.terminal_slot.slot_identity,
    );
    second.terminal_slot.offset *= 2;
    second.composed_offset = second.terminal_slot.offset;
    let demand = second
        .terminal_slot
        .native_demand(signature.native_parameters[0].identity);
    second.destination = demand.destination.clone();
    signature.callback_layout_catalog.push(second);
    signature.callback_demands.push(demand);
    signature
        .callback_demands
        .sort_by(|left, right| left.destination.cmp(&right.destination));
    signature
        .callback_layout_catalog
        .sort_by(|left, right| left.destination.cmp(&right.destination));
    validate(&signature, &signature.callback_demands).unwrap();

    let mut changed = signature.clone();
    changed.callback_layout_catalog.pop();
    assert!(validate(&changed, &changed.callback_demands).is_err());
    let mut changed = signature.clone();
    changed.callback_layout_catalog.swap(0, 1);
    assert!(validate(&changed, &changed.callback_demands).is_err());
    let mut changed = signature.clone();
    changed.callback_layout_catalog[1] = changed.callback_layout_catalog[0].clone();
    assert!(validate(&changed, &changed.callback_demands).is_err());
    let mut changed = signature.clone();
    changed.callback_demands[1] = changed.callback_demands[0].clone();
    assert!(validate(&changed, &changed.callback_demands).is_err());
    let mut changed = signature.clone();
    changed.callback_demands.swap(0, 1);
    assert!(validate(&changed, &changed.callback_demands).is_err());
    assert!(validate(&signature, &signature.callback_demands[..1]).is_err());
}

#[test]
fn parameter_destinations_do_not_acquire_named_field_entries() {
    let mut signature = signature_fixture();
    signature.callback_demands[0].destination =
        NativePlace::Parameter(signature.native_parameters[0].identity);
    assert!(validate(&signature, &signature.callback_demands).is_err());
    signature.callback_layout_catalog.clear();
    validate(&signature, &signature.callback_demands).unwrap();
    signature.callback_demands.clear();
    validate(&signature, &[]).unwrap();
}

#[test]
fn catalog_rejects_native_and_semantic_parameter_drift() {
    let mutations: &[fn(&mut MaterializedBoundarySignature)] = &[
        |signature| signature.callback_layout_catalog[0].native_ordinal = u32::MAX,
        |signature| signature.callback_layout_catalog[0].formal_ordinal = u32::MAX,
        |signature| signature.native_parameters.clear(),
        |signature| signature.native_parameters[0].identity = NativeParameterId::new(163).unwrap(),
        |signature| signature.native_parameters[0].native_ordinal = 1,
        |signature| signature.native_parameters[0].layout_data_symbol = SymbolHandle::invalid(),
        |signature| {
            signature.native_parameters[0].layout_data_symbol = SymbolHandle::from_arena_index(167)
        },
        |signature| {
            signature.native_parameters[0].origin =
                BoundaryNativeParameterOrigin::SemanticFormal { formal_ordinal: 1 }
        },
        |signature| {
            signature.native_parameters[0].shape = BoundaryNativeParameterShape::Semantic(u16::MAX)
        },
        |signature| signature.parameters[0] = u16::MAX,
        |signature| {
            signature
                .native_parameters
                .push(signature.native_parameters[0])
        },
    ];
    for (index, mutate) in mutations.iter().enumerate() {
        let mut signature = signature_fixture();
        mutate(&mut signature);
        assert!(
            validate(&signature, &signature.callback_demands).is_err(),
            "mutation {index}"
        );
    }
}

#[test]
fn catalog_rejects_named_slot_path_and_geometry_drift_without_panicking() {
    let mutations: &[fn(&mut BoundaryCallbackLayoutEntry)] = &[
        |entry| entry.terminal_slot.slot_identity = "fixture::ForeignSlot".into(),
        |entry| entry.terminal_slot.callback_requirement_identity = "fixture::Foreign::call".into(),
        |entry| entry.terminal_slot.requirement = callback_requirement_id("fixture::Foreign::call"),
        |entry| entry.terminal_slot.slot_application.declaration = SymbolHandle::invalid(),
        |entry| entry.terminal_slot.slot_application.trait_definition = SymbolHandle::invalid(),
        |entry| {
            entry.terminal_slot.slot_application.commitment =
                ClosedConformanceApplicationCommitment::default()
        },
        |entry| entry.terminal_slot.layout_subject_identity = "fixture::ForeignLayout".into(),
        |entry| entry.terminal_slot.data_symbol = SymbolHandle::from_arena_index(173),
        |entry| entry.terminal_slot.offset = usize::MAX,
        |entry| entry.terminal_slot.byte_size = usize::MAX,
        |entry| entry.terminal_slot.alignment = 0,
        |entry| entry.root_layout.physical.size = 0,
        |entry| entry.root_layout.physical.alignment = 0,
        |entry| entry.root_layout.data_identity = "".into(),
        |entry| entry.composed_offset = usize::MAX,
        |entry| {
            let NativePlace::Field { field_path, .. } = &mut entry.destination else {
                unreachable!()
            };
            field_path.clear();
        },
    ];
    for fixture in [signature_fixture, two_hop_fixture] {
        for (index, mutate) in mutations.iter().enumerate() {
            let mut signature = fixture();
            mutate(&mut signature.callback_layout_catalog[0]);
            // Keep the outer demand join intact so inner association checks run.
            signature.callback_demands[0].destination =
                signature.callback_layout_catalog[0].destination.clone();
            assert!(
                validate(&signature, &signature.callback_demands).is_err(),
                "mutation {index}"
            );
        }
    }
}

#[test]
fn two_hop_catalog_rejects_child_field_and_composition_drift() {
    let mutations: &[fn(&mut BoundaryCallbackInlineField)] = &[
        |field| field.symbol = SymbolHandle::invalid(),
        |field| field.identity = "fixture::Record::foreign".into(),
        |field| field.offset = usize::MAX,
        |field| field.extent = usize::MAX,
        |field| field.alignment = 0,
        |field| field.child_layout.data_symbol = SymbolHandle::invalid(),
        |field| field.child_layout.data_symbol = SymbolHandle::from_arena_index(179),
        |field| field.child_layout.layout_subject_identity = "fixture::ForeignChild".into(),
        |field| field.child_layout.physical.size = 0,
        |field| field.child_layout.physical.alignment = 0,
    ];
    for (index, mutate) in mutations.iter().enumerate() {
        let mut signature = two_hop_fixture();
        mutate(
            signature.callback_layout_catalog[0]
                .inline_field
                .as_mut()
                .unwrap(),
        );
        assert!(
            validate(&signature, &signature.callback_demands).is_err(),
            "mutation {index}"
        );
    }
}
