use super::super::BoundaryCallbackLayoutEntry;
use super::{
    BoundaryNativeParameterOrigin, BoundaryNativeParameterShape, MaterializedBoundarySignature,
    NativePlace, validate,
};
use crate::calling_policy_plans::materialized_boundary_signature_from_abi;
use crate::calling_policy_plans::plan_computation::boundary_plan_application_identity;
use calling_conventions::{
    CallSignature, LayoutPlanId, NativeParameterId, ValueShape, callback_layout_field_slot_id,
    callback_layout_slot_id, callback_requirement_id,
};
use layout::{
    FieldLayout, TargetClosedPlanLaidDataLayoutIdentity, TargetClosedPrivateCallbackDemand,
    TargetClosedTwoHopPrivateCallbackPath, TypeLayout,
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
    let shape = ValueShape::integer(pointer as u32, alignment as u16);
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
    let layouts = [TargetClosedPlanLaidDataLayoutIdentity {
        data_symbol,
        data_identity: "fixture::Record".into(),
        layout_subject_identity: terminal_slot.layout_subject_identity.clone(),
        // Named root and legacy terminal layouts intentionally use different IDs.
        layout: LayoutPlanId::new(139).unwrap(),
        physical: TypeLayout {
            size: 4 * pointer,
            alignment,
        },
    }];
    signature
        .callback_layout_catalog
        .push(BoundaryCallbackLayoutEntry::direct(parameter, &layouts, &terminal_slot).unwrap());
    signature.callback_demands.push(demand);
    signature
}

fn two_hop_fixture() -> MaterializedBoundarySignature {
    let mut signature = signature_fixture();
    let pointer = signature.native_target.pointer_size;
    let alignment = signature.native_target.pointer_alignment;
    let entry = &signature.callback_layout_catalog[0];
    let child_symbol = SymbolHandle::from_arena_index(149);
    let mut terminal_demand = entry.terminal_slot().clone();
    terminal_demand.data_symbol = child_symbol;
    terminal_demand.layout_subject_identity = "fixture::ChildLayout".into();
    let field_identity = "fixture::Record::child";
    let path = TargetClosedTwoHopPrivateCallbackPath {
        root_layout_index: 0,
        root_layout: entry.root_layout().clone(),
        field_symbol: SymbolHandle::from_arena_index(151),
        field: arena::Handle::invalid(),
        field_layout: FieldLayout::default(),
        field_identity: field_identity.into(),
        field_slot: callback_layout_field_slot_id(entry.root_layout().layout, field_identity),
        field_relative_offset: pointer,
        field_extent: 2 * pointer,
        field_alignment: alignment,
        child_layout_index: 1,
        child_layout: TargetClosedPlanLaidDataLayoutIdentity {
            data_symbol: child_symbol,
            data_identity: "fixture::Child".into(),
            layout_subject_identity: terminal_demand.layout_subject_identity.clone(),
            layout: LayoutPlanId::new(157).unwrap(),
            physical: TypeLayout {
                size: 2 * pointer,
                alignment,
            },
        },
        terminal_demand_index: 0,
        terminal_demand,
        composed_offset: 2 * pointer,
    };
    let parameter = &signature.native_parameters[0];
    signature.callback_layout_catalog[0] =
        BoundaryCallbackLayoutEntry::two_hop(parameter, &path).unwrap();
    signature.callback_demands[0] = path.native_demand(parameter.identity);
    signature
}

#[test]
fn named_catalog_preserves_existing_boundary_application_bytes() {
    use calling_conventions::{CallingPolicy, evaluate_ordinary_boundary_entry_plan};

    for mut signature in [signature_fixture(), two_hop_fixture()] {
        let target = signature.native_target;
        let abi = CallSignature {
            parameters: vec![ValueShape::integer(
                u32::try_from(target.pointer_size).unwrap(),
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
    let entry = &signature.callback_layout_catalog[0];
    let mut terminal = entry.terminal_slot().clone();
    terminal.slot_identity = "fixture::SecondSlot".into();
    terminal.slot = callback_layout_slot_id(terminal.layout, &terminal.slot_identity);
    terminal.offset *= 2;
    let layouts = [entry.root_layout().clone()];
    let parameter = &signature.native_parameters[0];
    let demand = terminal.native_demand(parameter.identity);
    let second = BoundaryCallbackLayoutEntry::direct(parameter, &layouts, &terminal).unwrap();
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
fn catalog_replays_backend_path_and_target_geometry() {
    for signature in [signature_fixture(), two_hop_fixture()] {
        validate(&signature, &signature.callback_demands).unwrap();
        let mut changed = signature.clone();
        let NativePlace::Field { field_path, .. } =
            &mut changed.callback_layout_catalog[0].destination
        else {
            unreachable!()
        };
        field_path.clear();
        changed.callback_demands[0].destination =
            changed.callback_layout_catalog[0].destination.clone();
        assert!(
            validate(&changed, &changed.callback_demands)
                .unwrap_err()
                .contains("exact ordered field path")
        );

        let mut changed = signature;
        changed.native_target.pointer_size *= 2;
        assert!(
            validate(&changed, &changed.callback_demands)
                .unwrap_err()
                .contains("target pointer geometry")
        );
    }
}
