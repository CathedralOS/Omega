//! Fixture identities shared with the executing owner and corpus inventory.

pub const BOUNDARY_EQUALITY_RECAST_WITNESS_COMPILE: &str =
    "dependent/boundary_equality_recast_witness_compile";
pub const WIRE_COMPATIBILITY_PRESERVATION_UNMET: &str =
    "wire/wire_compatibility_preservation_unmet";
pub const SELECTED_EMPTY_COMPONENT: &str = "terminal_psi/selected_empty_component";
pub const EXPLICIT_PROGRAM_ENTRY_BINDING: &str = "build/explicit_program_entry_binding";
pub const FREE_STANDING_MACHINE_HELPER_COMPILE: &str = "calls/free_standing_machine_helper_compile";
pub const LINEAR_TRANSFER_AND_CONSUME: &str = "ownership/linear_transfer_and_consume";
pub const BOUNDARY_TRAIT_EFFECTS_HOST_CALL: &str = "traits/boundary_trait_effects_host_call";
pub const BOUNDARY_DATA_OPAQUE_CONTRACT: &str = "proofs/boundary_data_opaque_contract";
pub const WIRE_CROSS_ERA_TYPE_CHANGE_MIGRATION: &str = "wire/wire_cross_era_type_change_migration";
pub const WIRE_COMPATIBILITY_DEMAND_REPORT: &str = "wire/wire_compatibility_demand_report";
pub const LINEAR_TRANSPARENT_RECORD_FRONTIER: &str = "ownership/linear_transparent_record_frontier";
pub const LINEAR_STATE_CALL_HANDOFF: &str = "ownership/linear_state_call_handoff";
pub const LINEAR_TRANSITION_NESTED_CALL_HANDOFF: &str =
    "ownership/linear_transition_nested_call_handoff";
pub const LINEAR_REPEATED_TRANSITION_CALL_HANDOFF: &str =
    "ownership/linear_repeated_transition_call_handoff";
pub const LINEAR_BOUNDARY_ENTRY_HANDOFF: &str = "ownership/linear_boundary_entry_handoff";
pub const LINEAR_LIVE_ACROSS_CALL_CONTINUATION: &str =
    "ownership/linear_live_across_call_continuation";
pub const LINEAR_FRESH_STATE_CALL_RESULT_HANDOFF: &str =
    "ownership/linear_fresh_state_call_result_handoff";
pub const LINEAR_TRANSPARENT_RECORD_STATE_RESULT: &str =
    "ownership/linear_transparent_record_state_result";
pub const LINEAR_AGGREGATE_STATE_RESULT: &str = "ownership/linear_aggregate_state_result";
pub const FLOAT_MEANING_CORE_SURFACE: &str = "core/float_meaning_core_surface";
pub const UNAPPROVED_HOST_CALL: &str = "capabilities/unapproved_host_call";

pub const PASS_CANARIES: &[&str] = &[
    BOUNDARY_EQUALITY_RECAST_WITNESS_COMPILE,
    SELECTED_EMPTY_COMPONENT,
    EXPLICIT_PROGRAM_ENTRY_BINDING,
    FREE_STANDING_MACHINE_HELPER_COMPILE,
    LINEAR_TRANSFER_AND_CONSUME,
    BOUNDARY_TRAIT_EFFECTS_HOST_CALL,
    BOUNDARY_DATA_OPAQUE_CONTRACT,
    WIRE_CROSS_ERA_TYPE_CHANGE_MIGRATION,
    WIRE_COMPATIBILITY_DEMAND_REPORT,
    LINEAR_TRANSPARENT_RECORD_FRONTIER,
    LINEAR_STATE_CALL_HANDOFF,
    LINEAR_TRANSITION_NESTED_CALL_HANDOFF,
    LINEAR_REPEATED_TRANSITION_CALL_HANDOFF,
    LINEAR_BOUNDARY_ENTRY_HANDOFF,
    LINEAR_LIVE_ACROSS_CALL_CONTINUATION,
    LINEAR_FRESH_STATE_CALL_RESULT_HANDOFF,
    LINEAR_TRANSPARENT_RECORD_STATE_RESULT,
    LINEAR_AGGREGATE_STATE_RESULT,
    FLOAT_MEANING_CORE_SURFACE,
];

pub const FAIL_CANARIES: &[&str] = &[WIRE_COMPATIBILITY_PRESERVATION_UNMET, UNAPPROVED_HOST_CALL];

pub const CHECKED_CAPABILITY_PASS_CANARIES: &[&str] = &[
    "capabilities/uses_caller_folder",
    "capabilities/uses_caller_capability_requires",
];

pub const SIGNED_RAT_PASS_CANARIES: &[&str] = &[
    "proofs/rat_metric_compile",
    "proofs/signed_rat_metric_compile",
    "proofs/cauchy_predicates_compile",
];

pub const CAPABILITY_VERB_PASS_CANARIES: &[(&str, &str)] = &[
    ("capabilities/acquires_filesystem_authority", "acquires"),
    ("capabilities/stores_capability", "stores"),
];

pub const CAPABILITY_FLOW_PASS_CANARIES: &[(&str, &[(&str, &str, &str)])] = &[
    (
        "capabilities/acquires_through_helper_return",
        // The second line shows the verb traveling a further call level: the
        // entry machine acquires through the mid-level helper, which acquired
        // through the boundary-touching helper.
        &[
            ("Backup::stage", "acquires", "Vault::pick"),
            ("Main::main", "acquires", "Backup::stage"),
        ],
    ),
    (
        "capabilities/derives_through_helper",
        &[("Worker::open_main_log", "derives", "Worker::open_log")],
    ),
];
