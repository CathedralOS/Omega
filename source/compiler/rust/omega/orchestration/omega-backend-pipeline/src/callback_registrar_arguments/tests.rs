use super::*;
use crate::callback_private_relocations::{
    plan_callback_private_relocations, tests::fixture_with_destination,
};
use crate::callback_thunks::plan_callback_thunks;
use omega_abstract_operations::{AbstractHostCallNativeArgument, AbstractHostCallOccurrence};
use omega_backend_plan::replay_callback_registrar_argument_bindings;
use omega_calling_conventions::{LayoutPlanId, LayoutSlotId, callback_native_parameter_id};
use omega_platform_interface::{HostCall, HostCallArgument, HostCallFormalArgumentIdentity};
use psi_arena::HandleSpan;
use std::sync::Arc;

const REQUIREMENT: &str = "package::Registrar::register#exact";

pub(crate) fn exact_surface(
    placement: &BoundNominalCallbackPlacement,
) -> (HostCallPlan, AbstractBoundarySummary) {
    let requirement: Arc<str> = Arc::from(REQUIREMENT);
    let formal_count = placement
        .private_materialization
        .as_ref()
        .expect("private materialization")
        .registrar_boundary_entry_plan
        .call
        .parameters
        .len();
    let mut host_calls = HostCallPlan::default();
    let mut call = HostCall {
        source_site: Some(placement.site),
        registration_operation: placement.registration_operation,
        requirement_identity: Arc::clone(&requirement),
        source_key: omega_control_flow::StateKey {
            machine: placement.selected_machine,
            state: placement.selected_entry,
            segment_index: 0,
        },
        statement_index: 5,
        call_ordinal: 2,
        lowering: Handle::from_arena_index(7),
        has_result: true,
        ..HostCall::default()
    };
    host_calls
        .arguments
        .append_to_span(&mut call.arguments, HostCallArgument::default());
    for formal_ordinal in 0..u32::try_from(formal_count).unwrap() {
        host_calls.arguments.append_to_span(
            &mut call.arguments,
            HostCallArgument {
                formal: Some(HostCallFormalArgumentIdentity {
                    formal_ordinal,
                    native_parameter: callback_native_parameter_id(&requirement, formal_ordinal),
                }),
                ..HostCallArgument::default()
            },
        );
    }
    let source_call = host_calls.calls.insert(call.clone());

    let mut boundaries = AbstractBoundarySummary::default();
    let mut arguments = HandleSpan::empty();
    for formal_ordinal in 0..u32::try_from(formal_count).unwrap() {
        boundaries.host_call_arguments.append_to_span(
            &mut arguments,
            AbstractHostCallNativeArgument {
                formal_ordinal,
                native_parameter: Some(callback_native_parameter_id(&requirement, formal_ordinal)),
            },
        );
    }
    boundaries.host_calls.insert(AbstractHostCallOccurrence {
        source_call_index: source_call.arena_index(),
        source_call_generation: source_call.generation(),
        source_site: abstract_site(placement.site),
        registration_operation: placement.registration_operation,
        requirement_identity: requirement,
        source_key: call.source_key,
        statement_index: call.statement_index,
        call_ordinal: call.call_ordinal,
        lowering_index: call.lowering.arena_index(),
        lowering_generation: call.lowering.generation(),
        arguments,
    });
    (host_calls, boundaries)
}

fn abstract_site(site: NominalMachineUseSite) -> AbstractHostCallSourceSite {
    match site {
        NominalMachineUseSite::Statement(statement) => {
            AbstractHostCallSourceSite::Statement(statement)
        }
        NominalMachineUseSite::Expression(expression) => {
            AbstractHostCallSourceSite::Expression(expression)
        }
    }
}

pub(crate) fn exact_catalog(
    destination: NativePlace,
) -> (
    Vec<BoundNominalCallbackPlacement>,
    Arc<[CallbackThunkPlan]>,
    Arc<[CallbackPrivateRelocationDemand]>,
    HostCallPlan,
    AbstractBoundarySummary,
    Arc<[CallbackRegistrarArgumentBinding]>,
) {
    let (control_flow, placement) = fixture_with_destination(destination);
    let placements = vec![placement];
    let thunks = plan_callback_thunks(&control_flow, &placements).unwrap();
    let demands = plan_callback_private_relocations(&placements, &thunks).unwrap();
    let (host_calls, boundaries) = exact_surface(&placements[0]);
    let bindings =
        plan_callback_registrar_arguments(&placements, &thunks, &demands, &host_calls, &boundaries)
            .unwrap();
    (
        placements, thunks, demands, host_calls, boundaries, bindings,
    )
}

pub(crate) fn field_destination(slot_names: &[u64]) -> NativePlace {
    NativePlace::Field {
        parameter: callback_native_parameter_id(REQUIREMENT, 1),
        layout: LayoutPlanId::new(41).unwrap(),
        field_path: slot_names
            .iter()
            .map(|slot| LayoutSlotId::new(*slot).unwrap())
            .collect(),
    }
}

#[test]
fn binds_direct_and_nested_destinations_to_the_exact_native_argument() {
    for destination in [
        NativePlace::Parameter(callback_native_parameter_id(REQUIREMENT, 1)),
        field_destination(&[43, 47]),
    ] {
        let (placements, thunks, demands, host_calls, boundaries, bindings) =
            exact_catalog(destination.clone());
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].demand.destination, destination);
        assert_eq!(bindings[0].demand, demands[0]);
        let argument = boundaries
            .host_call_arguments
            .get(bindings[0].native_argument);
        assert_eq!(argument.formal_ordinal, 1);
        assert_eq!(
            argument.native_parameter,
            Some(callback_native_parameter_id(REQUIREMENT, 1))
        );
        replay_callback_registrar_argument_bindings(
            &placements,
            &thunks,
            &demands,
            &host_calls,
            &boundaries,
            &bindings,
        )
        .unwrap();
    }
}

#[test]
fn distinct_nested_destinations_may_share_one_exact_parameter_root() {
    let (control_flow, first) = fixture_with_destination(field_destination(&[43]));
    let (_, mut second) = fixture_with_destination(field_destination(&[47, 53]));
    second.static_machine_ordinal = 1;
    let placements = vec![first, second];
    let thunks = plan_callback_thunks(&control_flow, &placements).unwrap();
    let demands = plan_callback_private_relocations(&placements, &thunks).unwrap();
    let (host_calls, boundaries) = exact_surface(&placements[0]);
    let bindings =
        plan_callback_registrar_arguments(&placements, &thunks, &demands, &host_calls, &boundaries)
            .unwrap();

    assert_eq!(bindings.len(), 2);
    assert_eq!(bindings[0].host_call, bindings[1].host_call);
    assert_eq!(bindings[0].native_argument, bindings[1].native_argument);
    assert_ne!(
        bindings[0].demand.destination,
        bindings[1].demand.destination
    );

    let reordered = [bindings[1].clone(), bindings[0].clone()];
    assert!(
        replay_callback_registrar_argument_bindings(
            &placements,
            &thunks,
            &demands,
            &host_calls,
            &boundaries,
            &reordered,
        )
        .is_err()
    );
}

#[test]
fn replay_rejects_binding_cardinality_order_handle_and_path_drift() {
    let (placements, thunks, demands, host_calls, boundaries, bindings) =
        exact_catalog(field_destination(&[43, 47]));
    let replay = |candidate: &[CallbackRegistrarArgumentBinding]| {
        replay_callback_registrar_argument_bindings(
            &placements,
            &thunks,
            &demands,
            &host_calls,
            &boundaries,
            candidate,
        )
    };
    assert!(replay(&[]).is_err());
    assert!(replay(&[bindings[0].clone(), bindings[0].clone()]).is_err());

    let mut index_drift = bindings[0].clone();
    index_drift.demand_index = 1;
    assert!(replay(&[index_drift]).is_err());
    let mut occurrence_drift = bindings[0].clone();
    occurrence_drift.host_call = Handle::from_arena_index(29);
    assert!(replay(&[occurrence_drift]).is_err());
    let mut argument_drift = bindings[0].clone();
    argument_drift.native_argument = Handle::from_arena_index(31);
    assert!(replay(&[argument_drift]).is_err());
    let mut path_drift = bindings[0].clone();
    path_drift.demand.destination = field_destination(&[43, 53]);
    assert!(replay(&[path_drift]).is_err());
    let mut layout_drift = bindings[0].clone();
    if let NativePlace::Field { layout, .. } = &mut layout_drift.demand.destination {
        *layout = LayoutPlanId::new(59).unwrap();
    }
    assert!(replay(&[layout_drift]).is_err());
}

#[test]
fn replay_rejects_occurrence_target_coordinate_and_source_identity_drift() {
    let (placements, thunks, demands, host_calls, boundaries, bindings) = exact_catalog(
        NativePlace::Parameter(callback_native_parameter_id(REQUIREMENT, 1)),
    );
    let occurrence = bindings[0].host_call;
    let source_call = host_calls.calls.iter().next().unwrap().0;

    let replay_boundaries = |drift: &AbstractBoundarySummary| {
        replay_callback_registrar_argument_bindings(
            &placements,
            &thunks,
            &demands,
            &host_calls,
            drift,
            &bindings,
        )
    };

    let mut statement_drift = boundaries.clone();
    statement_drift
        .host_calls
        .get_mut(occurrence)
        .statement_index += 1;
    assert!(replay_boundaries(&statement_drift).is_err());
    let mut ordinal_drift = boundaries.clone();
    ordinal_drift.host_calls.get_mut(occurrence).call_ordinal += 1;
    assert!(replay_boundaries(&ordinal_drift).is_err());
    let mut lowering_drift = boundaries.clone();
    lowering_drift.host_calls.get_mut(occurrence).lowering_index += 1;
    assert!(replay_boundaries(&lowering_drift).is_err());

    let mut missing = boundaries.clone();
    missing.host_calls = psi_arena::Arena::new();
    assert!(replay_boundaries(&missing).is_err());
    let mut duplicate = boundaries.clone();
    let duplicate_row = duplicate.host_calls.get(occurrence).clone();
    duplicate.host_calls.insert(duplicate_row);
    assert!(replay_boundaries(&duplicate).is_err());

    let mut target_drift = boundaries.clone();
    target_drift
        .host_calls
        .get_mut(occurrence)
        .registration_operation = psi_symbols::SymbolHandle::from_arena_index(61);
    assert!(replay_boundaries(&target_drift).is_err());

    let mut source_drift = host_calls.clone();
    source_drift.calls.get_mut(source_call).requirement_identity = Arc::from("wrong::overload");
    assert!(
        replay_callback_registrar_argument_bindings(
            &placements,
            &thunks,
            &demands,
            &source_drift,
            &boundaries,
            &bindings,
        )
        .is_err()
    );

    let mut source_target_drift = host_calls.clone();
    source_target_drift
        .calls
        .get_mut(source_call)
        .registration_operation = psi_symbols::SymbolHandle::from_arena_index(67);
    assert!(
        replay_callback_registrar_argument_bindings(
            &placements,
            &thunks,
            &demands,
            &source_target_drift,
            &boundaries,
            &bindings,
        )
        .is_err()
    );

    let mut formal_drift = host_calls.clone();
    let source_arguments = formal_drift.calls.get(source_call).arguments;
    formal_drift
        .arguments
        .get_mut(span_handle(source_arguments, 2).unwrap())
        .formal
        .as_mut()
        .unwrap()
        .formal_ordinal = 0;
    assert!(
        replay_callback_registrar_argument_bindings(
            &placements,
            &thunks,
            &demands,
            &formal_drift,
            &boundaries,
            &bindings,
        )
        .is_err()
    );
}

#[test]
fn replay_rejects_native_argument_cardinality_order_and_identity_drift() {
    let (placements, thunks, demands, host_calls, boundaries, bindings) = exact_catalog(
        NativePlace::Parameter(callback_native_parameter_id(REQUIREMENT, 1)),
    );
    let occurrence = boundaries.host_calls.get(bindings[0].host_call);
    let native_arguments = boundaries
        .host_call_arguments
        .span(occurrence.arguments)
        .unwrap();
    let first = native_arguments[0];
    let second = native_arguments[1];

    let mut identity_drift = boundaries.clone();
    identity_drift
        .host_call_arguments
        .get_mut(bindings[0].native_argument)
        .native_parameter = Some(callback_native_parameter_id("wrong::overload", 1));
    assert!(
        replay_callback_registrar_argument_bindings(
            &placements,
            &thunks,
            &demands,
            &host_calls,
            &identity_drift,
            &bindings,
        )
        .is_err()
    );

    let mut order_drift = boundaries.clone();
    let first_handle = span_handle(occurrence.arguments, 0).unwrap();
    let second_handle = span_handle(occurrence.arguments, 1).unwrap();
    *order_drift.host_call_arguments.get_mut(first_handle) = second;
    *order_drift.host_call_arguments.get_mut(second_handle) = first;
    assert!(
        replay_callback_registrar_argument_bindings(
            &placements,
            &thunks,
            &demands,
            &host_calls,
            &order_drift,
            &bindings,
        )
        .is_err()
    );

    let mut cardinality_drift = boundaries.clone();
    cardinality_drift
        .host_calls
        .get_mut(bindings[0].host_call)
        .arguments = HandleSpan::empty();
    assert!(
        replay_callback_registrar_argument_bindings(
            &placements,
            &thunks,
            &demands,
            &host_calls,
            &cardinality_drift,
            &bindings,
        )
        .is_err()
    );
}
