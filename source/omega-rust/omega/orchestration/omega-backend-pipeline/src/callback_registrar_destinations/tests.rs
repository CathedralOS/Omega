use super::*;
use crate::callback_private_relocations::{
    plan_callback_private_relocations, tests::fixture_with_destination,
};
use crate::callback_registrar_arguments::{
    plan_callback_registrar_arguments,
    tests::{exact_catalog, exact_surface},
};
use crate::callback_thunks::plan_callback_thunks;
use omega_backend_plan::{
    CallbackRegistrarPhysicalDestinationKind, replay_callback_registrar_physical_destinations,
};
use omega_calling_conventions::{
    CallbackRequirementId, LayoutPlanId, LayoutSlotId, NativePlace, callback_native_parameter_id,
};
use omega_layout::{
    DataLayout, DataShape, FieldLayout, LayoutPlan, TargetClosedPlanLaidDataLayoutIdentity,
    TargetClosedPrivateCallbackDemand, TargetClosedTwoHopPrivateCallbackPath, TypeLayout,
    TypeLayoutDescriptor,
};
use psi_arena::Arena;
use psi_symbols::SymbolHandle;
use std::sync::Arc;

const REQUIREMENT: &str = "package::Registrar::register#exact";

pub(crate) fn field_destination(formal_ordinal: u32, slots: &[u64]) -> NativePlace {
    NativePlace::Field {
        parameter: callback_native_parameter_id(REQUIREMENT, formal_ordinal),
        layout: LayoutPlanId::new(41).unwrap(),
        field_path: slots
            .iter()
            .map(|slot| LayoutSlotId::new(*slot).unwrap())
            .collect(),
    }
}

pub(crate) fn closed_row(slot: u64, offset: usize) -> TargetClosedPrivateCallbackDemand {
    TargetClosedPrivateCallbackDemand {
        data_symbol: SymbolHandle::from_arena_index(71),
        slot_identity: Arc::from(format!("package::Layout::slot_{slot}")),
        layout_subject_identity: Arc::from("package::Layout"),
        callback_requirement_identity: Arc::from("package::Handler::call"),
        layout: LayoutPlanId::new(41).unwrap(),
        slot: LayoutSlotId::new(slot).unwrap(),
        requirement: CallbackRequirementId::new(13).unwrap(),
        offset,
        byte_size: 8,
        alignment: 8,
    }
}

pub(crate) fn layouts(rows: Vec<TargetClosedPrivateCallbackDemand>) -> LayoutPlan {
    let mut data_layouts = Arena::new();
    data_layouts.insert(DataLayout {
        symbol: SymbolHandle::from_arena_index(71),
        name: psi_checked_trees::name::Identifier::from("Layout"),
        layout: TypeLayout {
            size: 64,
            alignment: 8,
        },
        ..DataLayout::default()
    });
    LayoutPlan {
        data_layouts,
        fields: Arena::new(),
        bit_fields: Vec::new(),
        stored_integers: Vec::new(),
        repeated_fields: Vec::new(),
        machine_layouts: Arena::new(),
        variants: Arena::new(),
        private_callback_demands: rows,
        plan_laid_layout_identities: Vec::new(),
        two_hop_private_callback_paths: Vec::new(),
    }
}

pub(crate) fn nested_layouts() -> LayoutPlan {
    let root_symbol = SymbolHandle::from_arena_index(71);
    let child_symbol = SymbolHandle::from_arena_index(72);
    let field_symbol = SymbolHandle::from_arena_index(73);
    let root_layout = LayoutPlanId::new(41).unwrap();
    let child_layout = LayoutPlanId::new(42).unwrap();
    let terminal_layout = LayoutPlanId::new(43).unwrap();
    let root_physical = TypeLayout {
        size: 64,
        alignment: 8,
    };
    let child_physical = TypeLayout {
        size: 32,
        alignment: 8,
    };
    let mut fields = Arena::new();
    let field = fields.insert(FieldLayout {
        symbol: field_symbol,
        name: psi_checked_trees::name::Identifier::from("child"),
        offset: 8,
        type_symbol: child_symbol,
        type_name: Arc::from("Child"),
        type_descriptor: TypeLayoutDescriptor::Named {
            symbol: child_symbol,
            name: psi_checked_trees::name::Identifier::from("Child"),
        },
        layout: child_physical,
    });
    let root_identity = TargetClosedPlanLaidDataLayoutIdentity {
        data_symbol: root_symbol,
        data_identity: Arc::from("package::Root"),
        layout_subject_identity: Arc::from("package::RootLayout"),
        layout: root_layout,
        physical: root_physical,
    };
    let child_identity = TargetClosedPlanLaidDataLayoutIdentity {
        data_symbol: child_symbol,
        data_identity: Arc::from("package::Child"),
        layout_subject_identity: Arc::from("package::ChildLayout"),
        layout: child_layout,
        physical: child_physical,
    };
    let terminal = TargetClosedPrivateCallbackDemand {
        data_symbol: child_symbol,
        slot_identity: Arc::from("package::Child::callback"),
        layout_subject_identity: Arc::from("package::ChildLayout"),
        callback_requirement_identity: Arc::from("package::Handler::call"),
        layout: terminal_layout,
        slot: LayoutSlotId::new(47).unwrap(),
        requirement: CallbackRequirementId::new(13).unwrap(),
        offset: 16,
        byte_size: 8,
        alignment: 8,
    };
    let field_identity = Arc::<str>::from("package::Root::child");
    let field_slot =
        omega_calling_conventions::callback_layout_field_slot_id(root_layout, &field_identity);
    let path = TargetClosedTwoHopPrivateCallbackPath {
        root_layout_index: 0,
        root_layout: root_identity.clone(),
        field_symbol,
        field,
        field_layout: fields.get(field).clone(),
        field_identity,
        field_slot,
        field_relative_offset: 8,
        field_extent: 32,
        field_alignment: 8,
        child_layout_index: 1,
        child_layout: child_identity.clone(),
        terminal_demand_index: 0,
        terminal_demand: terminal.clone(),
        composed_offset: 24,
    };
    let mut data_layouts = Arena::new();
    data_layouts.insert(DataLayout {
        symbol: root_symbol,
        name: psi_checked_trees::name::Identifier::from("Root"),
        shape: DataShape::Record {
            fields: psi_arena::HandleSpan::from_parts(field, 1),
        },
        layout: root_physical,
    });
    data_layouts.insert(DataLayout {
        symbol: child_symbol,
        name: psi_checked_trees::name::Identifier::from("Child"),
        shape: DataShape::Record {
            fields: psi_arena::HandleSpan::empty(),
        },
        layout: child_physical,
    });
    LayoutPlan {
        data_layouts,
        fields,
        bit_fields: Vec::new(),
        stored_integers: Vec::new(),
        repeated_fields: Vec::new(),
        machine_layouts: Arena::new(),
        variants: Arena::new(),
        private_callback_demands: vec![terminal],
        plan_laid_layout_identities: vec![root_identity, child_identity],
        two_hop_private_callback_paths: vec![path],
    }
}

#[test]
fn retains_exact_register_and_stack_parameter_placements() {
    for (formal_ordinal, expected_stack) in [(1, false), (4, true)] {
        let destination = field_destination(formal_ordinal, &[43]);
        let (placements, thunks, demands, host_calls, boundaries, bindings) =
            exact_catalog(destination);
        let layouts = layouts(vec![closed_row(43, 8)]);
        let physical = plan_callback_registrar_physical_destinations(
            NativeTarget::windows_x64(),
            &placements,
            &thunks,
            &demands,
            &host_calls,
            &boundaries,
            &bindings,
            &layouts,
        )
        .unwrap();

        assert_eq!(physical[0].formal_ordinal, formal_ordinal);
        assert!(matches!(
            physical[0].kind,
            CallbackRegistrarPhysicalDestinationKind::Field { .. }
        ));
        assert_eq!(
            physical[0]
                .parameter_placement
                .locations
                .iter()
                .any(|location| matches!(
                    location,
                    omega_calling_conventions::ValueLocation::Stack { .. }
                )),
            expected_stack
        );
    }
}

#[test]
fn retains_synthetic_direct_parameter_without_layout_authority() {
    let (placements, thunks, demands, host_calls, boundaries, bindings) = exact_catalog(
        NativePlace::Parameter(callback_native_parameter_id(REQUIREMENT, 1)),
    );
    let physical = plan_callback_registrar_physical_destinations(
        NativeTarget::windows_x64(),
        &placements,
        &thunks,
        &demands,
        &host_calls,
        &boundaries,
        &bindings,
        &layouts(Vec::new()),
    )
    .unwrap();

    assert!(matches!(
        physical[0].kind,
        CallbackRegistrarPhysicalDestinationKind::Parameter
    ));
}

#[test]
fn retains_one_exact_target_closed_field_row_and_geometry() {
    let (placements, thunks, demands, host_calls, boundaries, bindings) =
        exact_catalog(field_destination(1, &[43]));
    let layouts = layouts(vec![closed_row(43, 8)]);
    let physical = plan_callback_registrar_physical_destinations(
        NativeTarget::windows_x64(),
        &placements,
        &thunks,
        &demands,
        &host_calls,
        &boundaries,
        &bindings,
        &layouts,
    )
    .unwrap();

    let CallbackRegistrarPhysicalDestinationKind::Field {
        layout_demand_index,
        layout_demand,
    } = &physical[0].kind
    else {
        panic!("field destination should retain target-closed geometry");
    };
    assert_eq!(*layout_demand_index, 0);
    assert_eq!(layout_demand, &layouts.private_callback_demands[0]);
    assert_eq!(layout_demand.offset, 8);
    assert_eq!(layout_demand.byte_size, 8);
    assert_eq!(layout_demand.alignment, 8);
}

#[test]
fn distinct_slots_may_share_one_exact_registrar_argument_root() {
    let (control_flow, first) = fixture_with_destination(field_destination(1, &[43]));
    let (_, mut second) = fixture_with_destination(field_destination(1, &[47]));
    second.static_machine_ordinal = 1;
    let placements = vec![first, second];
    let thunks = plan_callback_thunks(&control_flow, &placements).unwrap();
    let demands = plan_callback_private_relocations(&placements, &thunks).unwrap();
    let (host_calls, boundaries) = exact_surface(&placements[0]);
    let bindings =
        plan_callback_registrar_arguments(&placements, &thunks, &demands, &host_calls, &boundaries)
            .unwrap();
    let layouts = layouts(vec![closed_row(43, 8), closed_row(47, 16)]);
    let physical = plan_callback_registrar_physical_destinations(
        NativeTarget::windows_x64(),
        &placements,
        &thunks,
        &demands,
        &host_calls,
        &boundaries,
        &bindings,
        &layouts,
    )
    .unwrap();

    assert_eq!(physical.len(), 2);
    assert_eq!(physical[0].formal_ordinal, physical[1].formal_ordinal);
    assert_eq!(
        physical[0].parameter_placement,
        physical[1].parameter_placement
    );
    assert_ne!(physical[0].kind, physical[1].kind);
}

#[test]
fn replay_rejects_cardinality_ordinal_placement_and_layout_identity_drift() {
    let (placements, thunks, demands, host_calls, boundaries, bindings) =
        exact_catalog(field_destination(1, &[43]));
    let layouts = layouts(vec![closed_row(43, 8)]);
    let physical = plan_callback_registrar_physical_destinations(
        NativeTarget::windows_x64(),
        &placements,
        &thunks,
        &demands,
        &host_calls,
        &boundaries,
        &bindings,
        &layouts,
    )
    .unwrap();
    let replay = |candidate: &[omega_backend_plan::CallbackRegistrarPhysicalDestination]| {
        replay_callback_registrar_physical_destinations(
            NativeTarget::windows_x64(),
            &placements,
            &thunks,
            &demands,
            &host_calls,
            &boundaries,
            &bindings,
            &layouts,
            candidate,
        )
    };
    assert!(replay(&[]).is_err());
    assert!(replay(&[physical[0].clone(), physical[0].clone()]).is_err());
    assert!(
        replay_callback_registrar_physical_destinations(
            NativeTarget::macos_arm64(),
            &placements,
            &thunks,
            &demands,
            &host_calls,
            &boundaries,
            &bindings,
            &layouts,
            &physical,
        )
        .is_err()
    );

    let mut ordinal = physical[0].clone();
    ordinal.formal_ordinal = 4;
    assert!(replay(&[ordinal]).is_err());
    let mut placement = physical[0].clone();
    placement.parameter_placement.locations.clear();
    assert!(replay(&[placement]).is_err());
    let mut binding = physical[0].clone();
    binding.binding_index = 1;
    assert!(replay(&[binding]).is_err());

    for mutation in 0..7 {
        let mut identity = physical[0].clone();
        let CallbackRegistrarPhysicalDestinationKind::Field {
            layout_demand_index,
            layout_demand,
        } = &mut identity.kind
        else {
            unreachable!()
        };
        match mutation {
            0 => *layout_demand_index = 1,
            1 => layout_demand.layout = LayoutPlanId::new(83).unwrap(),
            2 => layout_demand.slot = LayoutSlotId::new(89).unwrap(),
            3 => layout_demand.requirement = CallbackRequirementId::new(97).unwrap(),
            4 => layout_demand.slot_identity = Arc::from("wrong::slot"),
            5 => layout_demand.layout_subject_identity = Arc::from("wrong::layout"),
            6 => layout_demand.callback_requirement_identity = Arc::from("wrong::requirement"),
            _ => unreachable!(),
        }
        assert!(replay(&[identity]).is_err());
    }
}

#[test]
fn replay_rejects_data_symbol_offset_extent_and_alignment_drift() {
    let (placements, thunks, demands, host_calls, boundaries, bindings) =
        exact_catalog(field_destination(1, &[43]));
    let valid_layouts = layouts(vec![closed_row(43, 8)]);
    let physical = plan_callback_registrar_physical_destinations(
        NativeTarget::windows_x64(),
        &placements,
        &thunks,
        &demands,
        &host_calls,
        &boundaries,
        &bindings,
        &valid_layouts,
    )
    .unwrap();

    for mutation in 0..4 {
        let mut drifted_layouts = valid_layouts.clone();
        let mut drifted_physical = physical[0].clone();
        let CallbackRegistrarPhysicalDestinationKind::Field { layout_demand, .. } =
            &mut drifted_physical.kind
        else {
            unreachable!()
        };
        match mutation {
            0 => {
                drifted_layouts.private_callback_demands[0].data_symbol =
                    SymbolHandle::from_arena_index(101);
                layout_demand.data_symbol = SymbolHandle::from_arena_index(101);
            }
            1 => {
                drifted_layouts.private_callback_demands[0].offset = 9;
                layout_demand.offset = 9;
            }
            2 => {
                drifted_layouts.private_callback_demands[0].byte_size = 4;
                layout_demand.byte_size = 4;
            }
            3 => {
                drifted_layouts.private_callback_demands[0].alignment = 4;
                layout_demand.alignment = 4;
            }
            _ => unreachable!(),
        }
        assert!(
            replay_callback_registrar_physical_destinations(
                NativeTarget::windows_x64(),
                &placements,
                &thunks,
                &demands,
                &host_calls,
                &boundaries,
                &bindings,
                &drifted_layouts,
                &[drifted_physical],
            )
            .is_err()
        );
    }
}

#[test]
fn planner_rejects_missing_duplicate_and_multisegment_layout_rows() {
    for candidate_layouts in [
        layouts(Vec::new()),
        layouts(vec![closed_row(43, 8), closed_row(43, 8)]),
    ] {
        let (placements, thunks, demands, host_calls, boundaries, bindings) =
            exact_catalog(field_destination(1, &[43]));
        assert!(
            plan_callback_registrar_physical_destinations(
                NativeTarget::windows_x64(),
                &placements,
                &thunks,
                &demands,
                &host_calls,
                &boundaries,
                &bindings,
                &candidate_layouts,
            )
            .is_err()
        );
    }

    let (placements, thunks, demands, host_calls, boundaries, bindings) =
        exact_catalog(field_destination(1, &[43, 47]));
    assert!(
        plan_callback_registrar_physical_destinations(
            NativeTarget::windows_x64(),
            &placements,
            &thunks,
            &demands,
            &host_calls,
            &boundaries,
            &bindings,
            &layouts(vec![closed_row(43, 8), closed_row(47, 16)]),
        )
        .is_err()
    );
}

#[test]
fn exact_two_hop_layout_path_reaches_physical_destination_and_rejects_tamper() {
    let layouts = nested_layouts();
    let path = &layouts.two_hop_private_callback_paths[0];
    let destination =
        field_destination(1, &[path.field_slot.get(), path.terminal_demand.slot.get()]);
    let (placements, thunks, demands, host_calls, boundaries, bindings) =
        exact_catalog(destination);
    let physical = plan_callback_registrar_physical_destinations(
        NativeTarget::windows_x64(),
        &placements,
        &thunks,
        &demands,
        &host_calls,
        &boundaries,
        &bindings,
        &layouts,
    )
    .unwrap();
    assert!(matches!(
        &physical[0].kind,
        CallbackRegistrarPhysicalDestinationKind::NestedField {
            path_demand_index: 0,
            path_demand,
        } if path_demand == path && path_demand.composed_offset == 24
    ));
    let replay = |candidate: &[omega_backend_plan::CallbackRegistrarPhysicalDestination],
                  candidate_layouts: &LayoutPlan| {
        replay_callback_registrar_physical_destinations(
            NativeTarget::windows_x64(),
            &placements,
            &thunks,
            &demands,
            &host_calls,
            &boundaries,
            &bindings,
            candidate_layouts,
            candidate,
        )
    };
    replay(&physical, &layouts).unwrap();

    for mutation in 0..6 {
        let mut drifted = physical[0].clone();
        let CallbackRegistrarPhysicalDestinationKind::NestedField {
            path_demand_index,
            path_demand,
        } = &mut drifted.kind
        else {
            unreachable!()
        };
        match mutation {
            0 => *path_demand_index = 1,
            1 => path_demand.field_slot = LayoutSlotId::new(91).unwrap(),
            2 => path_demand.terminal_demand.slot = LayoutSlotId::new(93).unwrap(),
            3 => path_demand.terminal_demand.requirement = CallbackRequirementId::new(95).unwrap(),
            4 => path_demand.composed_offset = 32,
            5 => path_demand.field_layout.offset = 16,
            _ => unreachable!(),
        }
        assert!(replay(&[drifted], &layouts).is_err());
    }

    let mut unrelated_field_layouts = layouts.clone();
    let unrelated = unrelated_field_layouts
        .fields
        .insert(unrelated_field_layouts.fields.get(path.field).clone());
    unrelated_field_layouts.two_hop_private_callback_paths[0].field = unrelated;
    let mut unrelated_field = physical[0].clone();
    let CallbackRegistrarPhysicalDestinationKind::NestedField { path_demand, .. } =
        &mut unrelated_field.kind
    else {
        unreachable!()
    };
    *path_demand = unrelated_field_layouts.two_hop_private_callback_paths[0].clone();
    assert!(replay(&[unrelated_field], &unrelated_field_layouts).is_err());

    let mut reference_layouts = layouts.clone();
    let field = path.field;
    let child_descriptor = reference_layouts.fields.get(field).type_descriptor.clone();
    reference_layouts.fields.get_mut(field).type_descriptor = TypeLayoutDescriptor::Reference {
        referee: Box::new(child_descriptor),
        is_mutable: false,
    };
    reference_layouts.two_hop_private_callback_paths[0].field_layout =
        reference_layouts.fields.get(field).clone();
    let mut reference = physical[0].clone();
    let CallbackRegistrarPhysicalDestinationKind::NestedField { path_demand, .. } =
        &mut reference.kind
    else {
        unreachable!()
    };
    *path_demand = reference_layouts.two_hop_private_callback_paths[0].clone();
    assert!(replay(&[reference], &reference_layouts).is_err());

    let mut child_bounds_layouts = layouts.clone();
    child_bounds_layouts.private_callback_demands[0].offset = 32;
    child_bounds_layouts.two_hop_private_callback_paths[0].terminal_demand =
        child_bounds_layouts.private_callback_demands[0].clone();
    child_bounds_layouts.two_hop_private_callback_paths[0].composed_offset = 40;
    let mut child_bounds = physical[0].clone();
    let CallbackRegistrarPhysicalDestinationKind::NestedField { path_demand, .. } =
        &mut child_bounds.kind
    else {
        unreachable!()
    };
    *path_demand = child_bounds_layouts.two_hop_private_callback_paths[0].clone();
    assert!(replay(&[child_bounds], &child_bounds_layouts).is_err());

    let mut missing = layouts.clone();
    missing.two_hop_private_callback_paths.clear();
    assert!(replay(&physical, &missing).is_err());
    let mut duplicate = layouts.clone();
    duplicate
        .two_hop_private_callback_paths
        .push(duplicate.two_hop_private_callback_paths[0].clone());
    assert!(replay(&physical, &duplicate).is_err());

    let mut duplicate_root_symbol = layouts.clone();
    let mut forged_root = duplicate_root_symbol.plan_laid_layout_identities[0].clone();
    forged_root.layout = LayoutPlanId::new(101).unwrap();
    duplicate_root_symbol
        .plan_laid_layout_identities
        .push(forged_root);
    assert!(replay(&physical, &duplicate_root_symbol).is_err());

    let mut duplicate_root_layout = layouts.clone();
    let mut forged_root = duplicate_root_layout.plan_laid_layout_identities[0].clone();
    forged_root.data_symbol = SymbolHandle::from_arena_index(101);
    duplicate_root_layout
        .plan_laid_layout_identities
        .push(forged_root);
    assert!(replay(&physical, &duplicate_root_layout).is_err());

    let mut duplicate_terminal_slot = layouts.clone();
    let mut forged_terminal = duplicate_terminal_slot.private_callback_demands[0].clone();
    forged_terminal.requirement = CallbackRequirementId::new(103).unwrap();
    duplicate_terminal_slot
        .private_callback_demands
        .push(forged_terminal);
    assert!(replay(&physical, &duplicate_terminal_slot).is_err());

    let mut colliding_first_hop = layouts.clone();
    let mut forged_path = colliding_first_hop.two_hop_private_callback_paths[0].clone();
    forged_path.field_symbol = SymbolHandle::from_arena_index(105);
    forged_path.terminal_demand.slot = LayoutSlotId::new(107).unwrap();
    colliding_first_hop
        .two_hop_private_callback_paths
        .push(forged_path);
    assert!(replay(&physical, &colliding_first_hop).is_err());

    let mut shared_first_hop = layouts.clone();
    let mut second_terminal = shared_first_hop.private_callback_demands[0].clone();
    second_terminal.slot = LayoutSlotId::new(109).unwrap();
    second_terminal.offset = 0;
    shared_first_hop
        .private_callback_demands
        .push(second_terminal.clone());
    let mut second_path = shared_first_hop.two_hop_private_callback_paths[0].clone();
    second_path.terminal_demand_index = 1;
    second_path.terminal_demand = second_terminal;
    second_path.composed_offset = second_path.field_relative_offset;
    shared_first_hop
        .two_hop_private_callback_paths
        .push(second_path);
    replay(&physical, &shared_first_hop).unwrap();

    let path = &layouts.two_hop_private_callback_paths[0];
    let reversed = field_destination(1, &[path.terminal_demand.slot.get(), path.field_slot.get()]);
    let (p, t, d, h, b, a) = exact_catalog(reversed);
    assert!(
        plan_callback_registrar_physical_destinations(
            NativeTarget::windows_x64(),
            &p,
            &t,
            &d,
            &h,
            &b,
            &a,
            &layouts,
        )
        .is_err()
    );
    let overwide = field_destination(
        1,
        &[
            path.field_slot.get(),
            path.terminal_demand.slot.get(),
            LayoutSlotId::new(97).unwrap().get(),
        ],
    );
    let (p, t, d, h, b, a) = exact_catalog(overwide);
    assert!(
        plan_callback_registrar_physical_destinations(
            NativeTarget::windows_x64(),
            &p,
            &t,
            &d,
            &h,
            &b,
            &a,
            &layouts,
        )
        .is_err()
    );
}
