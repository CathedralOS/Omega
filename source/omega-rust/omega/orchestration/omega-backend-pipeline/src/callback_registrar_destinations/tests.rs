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
use omega_layout::{DataLayout, LayoutPlan, TargetClosedPrivateCallbackDemand, TypeLayout};
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
