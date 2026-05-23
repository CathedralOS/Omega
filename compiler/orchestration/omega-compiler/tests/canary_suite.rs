use omega_compiler::{CompileOptions, compile};
use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn contract_canary_visualizes_flow_contract_summaries() {
    let canary = repo_root()
        .join("canaries/pass")
        .join("contracts_domain_membership_surface");
    let main_path = canary.join("main.omg");
    let build_dir =
        std::env::temp_dir().join(format!("omega-contract-canary-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("contract canary should compile with visual artifacts");

    let state_graph = fs::read_to_string(build_dir.join("06_state_graph.html"))
        .expect("state graph visualization should be written");
    let control_flow = fs::read_to_string(build_dir.join("07_control_flow.html"))
        .expect("control flow visualization should be written");
    let checked_trees = fs::read_to_string(build_dir.join("05_checked_trees.html"))
        .expect("checked tree visualization should be written");

    assert!(
        state_graph.contains("contract call #1.0 requires 1 ensures 1"),
        "state graph should show propagated contract call summaries"
    );
    assert!(
        control_flow.contains("contract call #1.0 requires 1 ensures 1"),
        "control flow should show propagated contract call summaries"
    );
    assert!(
        checked_trees.contains("requires contract self in Player::Valid"),
        "checked semantic facts should expose requires as a domain-membership fact"
    );
    assert!(
        checked_trees.contains("ensures contract self in Player::Alive"),
        "checked semantic facts should expose ensures as a domain-membership fact"
    );
    assert!(
        checked_trees.contains("place: self"),
        "checked semantic facts should retain a readable place for self membership"
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn pass_canaries_compile() {
    for canary_name in ACTIVE_PASS_CANARIES {
        let canary = repo_root().join("canaries/pass").join(canary_name);
        let main_path = canary.join("main.omg");
        let options = CompileOptions {
            root_path: main_path.clone(),
            build_dir: None,
            target_name: None,
            write_output: false,
        };

        if let Err(diagnostics) = compile(options) {
            panic!(
                "expected pass canary {} to compile, but got diagnostics:\n{}",
                canary.display(),
                diagnostics
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            );
        }
    }
}

#[test]
fn fail_canaries_reject_with_expected_diagnostic_fragment() {
    for canary_name in ACTIVE_FAIL_CANARIES {
        let canary = repo_root().join("canaries/fail").join(canary_name);
        let main_path = canary.join("main.omg");
        let expected_path = canary.join("expected.txt");
        let expected_fragment = fs::read_to_string(&expected_path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", expected_path.display()))
            .trim()
            .to_owned();
        let options = CompileOptions {
            root_path: main_path.clone(),
            build_dir: None,
            target_name: None,
            write_output: false,
        };

        let diagnostics = match compile(options) {
            Ok(report) => {
                panic!(
                    "expected fail canary {} to reject, but it compiled successfully: {}",
                    canary.display(),
                    report.summary()
                )
            }
            Err(diagnostics) => diagnostics,
        };
        let combined = diagnostics
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");

        assert!(
            combined.contains(&expected_fragment),
            "fail canary {} did not contain expected fragment {:?}\nactual diagnostics:\n{}",
            canary.display(),
            expected_fragment,
            combined
        );
    }
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("compiler crate should live under compiler/orchestration/omega-compiler")
        .to_path_buf()
}

const ACTIVE_PASS_CANARIES: &[&str] = &[
    "bounded_arithmetic_return",
    "bounded_float_division",
    "bounded_slice_index_max",
    "bounded_literal_named_constraints",
    "bounded_max_call",
    "bounded_member_guard_transition",
    "boundary_trait_effects_host_call",
    "composite_field_guard_dispatch",
    "composite_range_guard_dispatch",
    "contracts_domain_membership_surface",
    "domain_import_valid",
    "entry_surface_receiver_paths",
    "mutable_output_host_call",
    "nested_machine_continuation",
    "runtime_alias_integer_write",
    "runtime_alias_field_integer",
    "runtime_alias_field_binary",
    "runtime_alias_string_write",
    "runtime_alias_text_builder_write",
    "runtime_arithmetic_guard",
    "runtime_arithmetic_value",
    "runtime_call_guard",
    "runtime_branching_helper_guard",
    "runtime_branching_helper_local_guard_value",
    "runtime_branching_helper_string",
    "runtime_branching_helper_struct",
    "runtime_branching_helper_value",
    "runtime_branch_enemy_reward_shape",
    "runtime_call_value",
    "runtime_call_enum_field_value",
    "runtime_call_enum_field_with_args",
    "runtime_call_enum_field_with_mut_arg",
    "runtime_call_enum_sequence",
    "runtime_call_enum_value",
    "runtime_guarded_leaf_ordering_call",
    "runtime_contained_call_value",
    "runtime_contained_reward_table_roll_item",
    "runtime_nested_branch_assignment_prelude_value",
    "runtime_nested_branch_prelude_value",
    "runtime_nested_branch_value",
    "runtime_dispatch_helper_local_alias_add",
    "runtime_dispatch_local_index_binary_write",
    "runtime_indexed_alias_field_binary",
    "runtime_indexed_text_builder_write",
    "runtime_modulo_value",
    "runtime_multi_assignment_value_calls",
    "runtime_reward_table_roll_item_shape",
    "runtime_text_storage",
    "runtime_transition_subject_call_guard",
    "runtime_transition_argument_call_value",
    "std_option_storage_write",
    "std_option_surface",
    "trait_composition_satisfies",
    "trait_declaration_bundle",
    "trait_satisfies_machine_signature",
];

const ACTIVE_FAIL_CANARIES: &[&str] = &[
    "assign_immutable_parameter",
    "borrow_duplicate_mut",
    "borrow_mut_literal",
    "bounded_guarded_increment_unproven",
    "bounded_guarded_subtraction_unproven",
    "bounded_index_max_unproven",
    "bounded_match_guard_unproven",
    "domain_import_cycle",
    "domain_import_unknown",
    "domain_import_wrong_target",
    "domain_non_boolean_fact",
    "runtime_helper_ordering_return",
    "trait_composition_missing_requirement",
    "trait_requirement_cycle",
    "trait_requires_unknown",
    "trait_satisfies_missing_machine",
    "trait_satisfies_parameter_mismatch",
    "trait_satisfies_unknown",
    "trait_unknown_signature_type",
];
