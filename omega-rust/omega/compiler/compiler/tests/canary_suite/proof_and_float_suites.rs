use super::*;

#[test]
fn fail_canaries_reject_with_expected_diagnostic_fragment() {
    // COLLECT-ALL, not first-panic: one regressed member must not exempt the
    // rest of the fail corpus from its check (the serial-umbrella masking
    // pattern -- every conversion so far has found something hiding). Local
    // iteration may select a focused subset with OMEGA_FAIL_CANARY_FILTER;
    // CI's unset default still checks the complete corpus.
    let _umbrella = CANARY_UMBRELLA_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut failures: Vec<String> = Vec::new();
    let filter = std::env::var("OMEGA_FAIL_CANARY_FILTER").ok();
    let mut selected = 0usize;

    let selected_by_filter = |canary_name: &&str| {
        filter.as_deref().is_none_or(|filter| {
            filter
                .split(',')
                .map(str::trim)
                .any(|candidate| !candidate.is_empty() && canary_name.contains(candidate))
        })
    };
    let evaluate = |canary_name: &str, checked_only: bool| {
        let canary = fail_canary(canary_name);
        let expected_path = canary.join("expected.txt");
        let expected_fragment = fs::read_to_string(&expected_path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", expected_path.display()))
            .replace("\r\n", "\n")
            .replace('\r', "\n")
            .trim()
            .to_owned();

        let result = if checked_only {
            check_canary(&canary).map(|()| "checked semantics".to_owned())
        } else {
            let cross_target = CROSS_TARGET_FAIL_CANARIES
                .iter()
                .find_map(|(candidate, target)| (*candidate == canary_name).then_some(*target));
            match cross_target {
                Some(target) => compile_canary_without_output_for_target(&canary, target),
                None => compile_native_canary_without_output(&canary),
            }
            .map(|report| report.summary())
        };
        let diagnostics = match result {
            Ok(summary) => {
                return Some(format!(
                    "{} compiled successfully (expected a rejection): {}",
                    canary.display(),
                    summary
                ));
            }
            Err(diagnostics) => diagnostics,
        };
        let combined = diagnostics
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");

        if !combined.contains(&expected_fragment) {
            Some(format!(
                "{} missing expected fragment {:?}\nactual diagnostics:\n{}",
                canary.display(),
                expected_fragment,
                combined
            ))
        } else {
            None
        }
    };

    let checked_only = CHECKED_ONLY_FAIL_CANARIES
        .iter()
        .copied()
        .filter(selected_by_filter)
        .collect::<Vec<_>>();
    selected += checked_only.len();
    failures.extend(
        run_bounded_canary_jobs(&checked_only, |canary_name| evaluate(canary_name, true))
            .into_iter()
            .flatten(),
    );
    let active = ACTIVE_FAIL_CANARIES
        .iter()
        .copied()
        .filter(selected_by_filter)
        .collect::<Vec<_>>();
    selected += active.len();
    failures.extend(
        run_bounded_canary_jobs(&active, |canary_name| evaluate(canary_name, false))
            .into_iter()
            .flatten(),
    );

    assert!(
        filter.is_none() || selected > 0,
        "OMEGA_FAIL_CANARY_FILTER matched no active fail canaries"
    );
    assert!(
        failures.is_empty(),
        "{} fail canary(ies) drifted:\n\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

#[test]
fn decode_requirement_surface_compiles() {
    let canary = pass_canary("wire/decode_requirement_surface");
    compile_canary_without_output(&canary)
        .expect("strict, projecting, and preserving decode requirements should compile");
}

#[test]
fn range_gated_establishment_canaries_compile() {
    for name in [
        "dependent/range_sugar_gated_construction_compile",
        "dependent/nested_gated_construction_compile",
        "dependent/zero_case_absorbs_nested_gate_compile",
        "dependent/range_gated_machine_establishment_compile",
        "dependent/data_where_cross_state_establish",
        "dependent/data_where_callee_establishes",
        "dependent/data_where_multistate_callee",
        "dependent/data_where_gated_literal_proves",
    ] {
        let canary = pass_canary(name);
        check_canary(&canary).unwrap_or_else(|diagnostics| {
            panic!(
                "{} failed:\n{}",
                canary.display(),
                diagnostics
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        });
    }
}

#[test]
fn range_gated_establishment_canaries_reject_unsafe_uses() {
    for name in [
        "arithmetic/zii_range_excludes_zero_rejected",
        "range/element_range_zero_excluded",
        "dependent/range_sugar_gated_field_omitted_rejected",
        "dependent/nested_gated_field_omitted_rejected",
        "dependent/data_where_gated_machine_unestablished_rejected",
    ] {
        let canary = fail_canary(name);
        let expected = fs::read_to_string(canary.join("expected.txt"))
            .expect("range-gated fail canary should carry expected.txt");
        let diagnostics =
            check_canary(&canary).expect_err("unsafe range-gated use should be rejected");
        let combined = diagnostics
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            combined.contains(expected.trim()),
            "{} missing expected fragment {:?}:\n{}",
            canary.display(),
            expected.trim(),
            combined
        );
    }
}

#[test]
fn default_domain_membership_canaries_compile() {
    for name in [
        "dependent/data_where_membership_literal_compile",
        "dependent/data_where_membership_window_restored_compile",
        "dependent/data_where_membership_zero_valid_compile",
    ] {
        let canary = pass_canary(name);
        check_canary(&canary).unwrap_or_else(|diagnostics| {
            panic!(
                "{} failed:\n{}",
                canary.display(),
                diagnostics
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        });
    }
}

#[test]
fn default_domain_membership_canaries_reject_invalid_claims() {
    for name in [
        "dependent/data_where_membership_literal_rejected",
        "dependent/data_where_membership_carrier_mismatch_rejected",
        "dependent/data_where_ambiguous_domain_short_name_rejected",
    ] {
        let canary = fail_canary(name);
        let expected = fs::read_to_string(canary.join("expected.txt"))
            .expect("membership fail canary should carry expected.txt");
        let diagnostics = check_canary(&canary)
            .expect_err("invalid default-domain membership should be rejected");
        let combined = diagnostics
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            combined.contains(expected.trim()),
            "{} missing expected fragment {:?}:\n{}",
            canary.display(),
            expected.trim(),
            combined
        );
    }
}

#[test]
fn default_domain_standing_bound_canaries() {
    let pass = pass_canary("dependent/data_where_standing_bound_exit");
    compile_canary_without_output(&pass).unwrap_or_else(|diagnostics| {
        panic!(
            "{} failed:\n{}",
            pass.display(),
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });

    let fail = fail_canary("dependent/data_where_standing_bound_absent_rejected");
    let expected = fs::read_to_string(fail.join("expected.txt"))
        .expect("standing-bound fail canary should carry expected.txt");
    let diagnostics = compile_canary_without_output(&fail)
        .expect_err("arithmetic without the standing bound should reject");
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains(expected.trim()),
        "{} missing expected fragment {:?}:\n{}",
        fail.display(),
        expected.trim(),
        combined
    );
}

#[test]
fn default_domain_measure_and_symbolic_canaries() {
    for name in [
        "dependent/data_where_length_construction_compile",
        "dependent/data_where_length_window_compile",
        "dependent/data_where_length_zero_valid_compile",
        "dependent/data_where_symbolic_equal_construction_compile",
        "dependent/data_where_symbolic_equal_window_compile",
        "dependent/data_where_capacity_measure_compile",
    ] {
        let canary = pass_canary(name);
        check_canary(&canary).unwrap_or_else(|diagnostics| {
            panic!(
                "{} failed:\n{}",
                canary.display(),
                diagnostics
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        });
    }

    for name in [
        "dependent/data_where_length_mismatch_rejected",
        "dependent/data_where_capacity_mismatch_rejected",
    ] {
        let canary = fail_canary(name);
        let expected = fs::read_to_string(canary.join("expected.txt"))
            .expect("default-domain fail canary should carry expected.txt");
        let diagnostics = compile_canary_without_output(&canary)
            .expect_err("invalid default-domain measure should reject");
        let combined = diagnostics
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            combined.contains(expected.trim()),
            "{} missing expected fragment {:?}:\n{}",
            canary.display(),
            expected.trim(),
            combined
        );
    }

    for name in [
        "dependent/data_where_param_write_unproven",
        "dependent/data_where_cross_state_unknown_refuses",
        "dependent/data_where_symbolic_correlation_stale_rejected",
        "dependent/data_where_invariant_window_unclosed_rejected",
    ] {
        let canary = fail_canary(name);
        assert!(
            check_canary(&canary).is_err(),
            "{} unexpectedly compiled; symbolic facts must not survive unrelated writes or state boundaries",
            canary.display()
        );
    }
}

#[test]
fn default_domain_product_hypothesis_canary() {
    let canary = pass_canary("dependent/data_where_product_hypothesis");
    check_canary(&canary).unwrap_or_else(|diagnostics| {
        panic!(
            "{} failed:\n{}",
            canary.display(),
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });

    for name in [
        "dependent/data_where_gated_machine_unestablished_rejected",
        "dependent/data_where_read_before_establish",
        "dependent/data_where_invariant_window_unclosed_rejected",
    ] {
        let canary = fail_canary(name);
        assert!(
            check_canary(&canary).is_err(),
            "{} unexpectedly compiled; calls must not establish an invalid or open default domain",
            canary.display()
        );
    }
}

#[test]
fn default_domain_symbolic_correlation_canaries() {
    for name in [
        "dependent/data_where_symbolic_affine_window_compile",
        "dependent/data_where_commutative_correlation_compile",
        "dependent/data_where_flow_proven_construction_compile",
    ] {
        let canary = pass_canary(name);
        check_canary(&canary).unwrap_or_else(|diagnostics| {
            panic!(
                "{} failed:\n{}",
                canary.display(),
                diagnostics
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        });
    }

    for name in [
        "dependent/data_where_symbolic_correlation_stale_rejected",
        "dependent/data_where_cross_state_unknown_refuses",
    ] {
        let canary = fail_canary(name);
        assert!(
            check_canary(&canary).is_err(),
            "{} unexpectedly compiled; symbolic correlations must remain state-local",
            canary.display()
        );
    }
}

#[test]
fn commutative_semiring_core_canaries() {
    for name in [
        "proofs/ring_rearrange_core_nat",
        "traits/ring_requirement_satisfies_exit",
    ] {
        let canary = pass_canary(name);
        compile_canary_without_output(&canary).unwrap_or_else(|diagnostics| {
            panic!(
                "{} failed:\n{}",
                canary.display(),
                diagnostics
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        });
    }

    for name in [
        "proofs/polynomial_expand_core_nat",
        "proofs/proof_nat_structural_lemmas",
    ] {
        let canary = pass_canary(name);
        check_canary(&canary).unwrap_or_else(|diagnostics| {
            panic!(
                "{} failed to reach checked semantics:\n{}",
                canary.display(),
                diagnostics
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        });
    }
}

#[test]
fn exact_nat_subtraction_requires_a_prior_order_fact() {
    let accepted = pass_canary("proofs/nat_exact_subtraction_compile");
    check_canary(&accepted).unwrap_or_else(|diagnostics| {
        panic!(
            "{} failed to reach checked semantics:\n{}",
            accepted.display(),
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });

    let rejected = fail_canary("proofs/nat_exact_subtraction_requires_order");
    let diagnostics = check_canary(&rejected)
        .expect_err("bare Nat subtraction without its order fact must reject");
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("cannot prove `used <= total`")
            && combined.contains("`Nat::subtract` (spelled `-`)"),
        "{} rejected with the wrong diagnostic:\n{combined}",
        rejected.display()
    );
}

#[test]
fn algebraic_normalization_requires_an_exact_licensed_conformance() {
    let licensed = pass_canary("proofs/ring_rearrange_core_nat");
    check_canary(&licensed).unwrap_or_else(|diagnostics| {
        panic!(
            "{} failed to reach checked semantics with its selected conformance:\n{}",
            licensed.display(),
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });

    for name in [
        "proofs/ring_rearrange_unlicensed_rejected",
        "proofs/ring_rearrange_false_shuffle_rejected",
    ] {
        let canary = fail_canary(name);
        let diagnostics = check_canary(&canary)
            .expect_err("unlicensed or unequal normalization must reject in checked semantics");
        let combined = diagnostics
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            combined.contains("no entailment tier judges yet"),
            "{} rejected with the wrong diagnostic:\n{combined}",
            canary.display()
        );
    }
}

#[test]
fn ring_identity_slot_bridge_canary_compiles() {
    let canary = pass_canary("proofs/ring_identity_slot_bridge_compile");
    compile_canary_without_output(&canary).unwrap_or_else(|diagnostics| {
        panic!(
            "{} failed:\n{}",
            canary.display(),
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });
}

#[test]
fn integer_measured_nat_induction_canary_compiles() {
    let canary = pass_canary("proofs/integer_measured_nat_induction_compile");
    check_canary(&canary).unwrap_or_else(|diagnostics| {
        panic!(
            "{} failed to reach checked semantics:\n{}",
            canary.display(),
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });
}

#[test]
fn proof_joint_scc_ranking_canaries_reach_checked_semantics() {
    let canary = pass_canary("termination/proof_non_tail_joint_machine_cycle_compile");
    check_canary(&canary).unwrap_or_else(|diagnostics| {
        panic!(
            "{} failed to reach checked semantics:\n{}",
            canary.display(),
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });

    for (name, expected) in [
        (
            "termination/proof_joint_machine_cycle_nondecreasing",
            "does not structurally decrease",
        ),
        (
            "termination/proof_joint_machine_cycle_unmeasured",
            "unmeasured proof machine",
        ),
    ] {
        let canary = fail_canary(name);
        let diagnostics = check_canary(&canary)
            .expect_err("an invalid proof-machine SCC must reject in checked semantics");
        let combined = diagnostics
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            combined.contains(expected),
            "{} rejected with the wrong diagnostic:\n{combined}",
            canary.display()
        );
    }
}

#[test]
fn exact_float_to_int_proof_canaries() {
    let canary = pass_canary("float/float_to_int_exact_proofs_exit");
    compile_canary_without_output(&canary).unwrap_or_else(|diagnostics| {
        panic!(
            "{} failed:\n{}",
            canary.display(),
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });

    for name in [
        "arithmetic/float_cast_unproven_rejected",
        "arithmetic/float_to_int_exact_unproven",
    ] {
        let canary = fail_canary(name);
        assert!(
            compile_canary_without_output(&canary).is_err(),
            "{} unexpectedly compiled; exact float-to-int needs non-NaN range evidence",
            canary.display()
        );
    }
}

#[test]
fn generic_float_builtins_retain_exact_provider_evidence() {
    let canary = pass_canary("arithmetic/runtime_float_min_max_abs_clamp_exit");
    let checked = compile_to_checked(&canary.join("main.omg"), None)
        .expect("generic float builtins should compile to checked trees");
    let uses = checked
        .facts
        .operators
        .named_uses()
        .filter(|operator_use| {
            matches!(
                checked
                    .typed
                    .expression_table
                    .expression(operator_use.expression),
                typed_trees::expression::ExpressionNode::Call(call)
                    if matches!(call.target.as_str(), "min" | "max" | "sqrt")
            )
        })
        .collect::<Vec<_>>();
    assert!(
        uses.len() >= 5,
        "direct and desugared min/max uses must be retained"
    );
    assert!(
        uses.iter()
            .all(|operator_use| operator_use.provider_plan_report_fingerprint != 0),
        "every normalized float builtin must carry its exact selected ProviderPlan"
    );
    assert!(
        uses.iter()
            .all(|operator_use| !operator_use.provider_plan_commitment.is_empty()),
        "every normalized float builtin must carry its exact selected ProviderPlan commitment"
    );
}

#[test]
fn runtime_float_min_max_abs_clamp_exit_canary_runs() {
    // Float min/max on SSE (maxsd/minsd), plus abs/clamp over floats which
    // desugar to them: max(3,7)+min(3,7)+abs(-12)+clamp(300,0,200) = 222.
    let canary = pass_canary("arithmetic/runtime_float_min_max_abs_clamp_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-float-minmax-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("float min/max/abs/clamp canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("float min/max/abs/clamp canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected float max/min/abs/clamp to sum to 222 (exit 70); exit 71 = maxsd/minsd \
         disagreed with the interpreter. got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_shared_ref_param_member_exit_canary_runs() {
    // Shared &Struct param (non-boundary): content-spill convention; the callee
    // reads a=7 and b=35 through the ref -> got=42 (the exit). A pointee
    // misresolution dereferences the spilled content and crashes (0xC0000005).
    let canary = pass_canary("calls/runtime_shared_ref_param_member_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-sharedref-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("shared-ref param canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("shared-ref param canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("shared-ref param canary should run");

    assert_eq!(
        output.status.code(),
        Some(42),
        "expected a+b=42 through the shared &Struct param (exit 42); a crash/garbage =          the pointee resolver treated the content spill as a pointer. got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_shared_ref_param_large_deref_exit_canary_runs() {
    // The DEREF twin of the content-spill canary above (bug 2026-07-12): a
    // shared &Struct param whose referee is LARGER than a pointer holds a real
    // pointer (it cannot be content-spilled into the 8-byte slot), so a
    // local-init member read (`let v = r.value`, Cathedral's `let bs =
    // table.boot_services` shape) MUST dereference. Reading the slot inline
    // instead fetched garbage -- Cathedral's M2 boot dispatched get_memory_map
    // through it and #UD'd under QEMU. value=42 at offset 16 -> exit 42.
    let canary = pass_canary("calls/runtime_shared_ref_param_large_deref_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-sharedref-large-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("large shared-ref deref canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("large shared-ref deref canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("large shared-ref deref canary should run");

    assert_eq!(
        output.status.code(),
        Some(42),
        "expected value=42 dereferenced through the large shared &Struct param (exit 42); \
         an inline (non-deref) read of the pointer-sized slot fetches garbage. got {:?}
stderr:
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_large_shared_ref_direct_assignment_exit_canary_runs() {
    let canary = pass_canary("calls/runtime_large_shared_ref_direct_assignment_exit");
    let build_dir = std::env::temp_dir().join(format!(
        "omega-large-shared-ref-direct-assignment-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("large shared-ref direct-assignment canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("large shared-ref direct-assignment canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("large shared-ref direct-assignment canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "large shared-ref argument/assignment lost address identity; stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_same_type_contained_direct_fields_exit_canary_runs() {
    // The SOUND pattern for two same-type contained machines: DIRECT field access
    // (not method calls, which alias to the first field of the type). a -> 13,
    // b -> 21 independently -> exit 70.
    let canary = pass_canary("calls/runtime_same_type_contained_direct_fields_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-sametype-direct-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("same-type contained direct-fields canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("same-type direct-fields canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("same-type contained direct-fields canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected two same-type contained machines to be INDEPENDENT via direct field \
         access (a=13, b=21 -> exit 70); exit 71 = they aliased. got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_sum_field_store_payload_exit_canary_runs() {
    // Regression for the sum-type field-store payload-offset miscompile: storing
    // Tx::Transfer{to:3, amount:40} into a field then matching must read to=3,
    // amount=40 -> exit 70 (before the fix: to=40, amount=0).
    let canary = pass_canary("control_flow/runtime_sum_field_store_payload_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-sumfield-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("sum-field-store payload canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("sum-field-store payload canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("sum-field-store payload canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected field-stored Tx::Transfer to read to=3, amount=40 (exit 70); exit 71 = the \
         payload offset shifted (write not variant-tagged). got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_argmax_index_exit_canary_runs() {
    // argmax over [4,15,8,42,16,23]: the maximum 42 is at index 3 -> exit 70;
    // a wrong index-capture -> exit 71.
    let canary = pass_canary("collections/runtime_argmax_index_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-argmax-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("argmax canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("argmax canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("argmax canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected argmax of [4,15,8,42,16,23] to be index 3 (exit 70); exit 71 = wrong \
         index capture. got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_bracket_matcher_stack_exit_canary_runs() {
    // Stack-based bracket matcher over "([)]" (mis-nested). Correct verdict is
    // UNBALANCED -> exit 70; a count-only or broken matcher -> exit 71.
    let canary = pass_canary("collections/runtime_bracket_matcher_stack_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-bracket-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("bracket matcher canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("bracket matcher canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("bracket matcher canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the stack matcher to detect mis-nesting in \"([)]\" (exit 70); exit 71 = \
         it accepted the mismatch. got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_palindrome_two_pointer_exit_canary_runs() {
    // Two-pointer palindrome over [1,2,3,4,1] (NOT a palindrome): must detect the
    // arr[1]=2 vs arr[3]=4 mismatch -> exit 70; missing it -> exit 71.
    let canary = pass_canary("collections/runtime_palindrome_two_pointer_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-palindrome-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("palindrome two-pointer canary should compile from its authored root");

    let executable = compilation
        .checked_native_executable_path()
        .expect("palindrome two-pointer canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("palindrome two-pointer canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected the two-pointer scan to DETECT arr[1]=2 != arr[3]=4 (exit 70); exit 71 = \
         it missed the mismatch. got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_cross_array_indexed_guard_compare_exit_canary_runs() {
    // PROBE: `a[i] < b[j]` (two different arrays, both runtime indices) in a
    // guard. a[1]=20 < b[3]=4 is FALSE; reverse TRUE -> exit 70.
    let canary = pass_canary("collections/runtime_cross_array_indexed_guard_compare_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-cross-idx-guard-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("cross-array indexed guard-compare canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("cross-array indexed guard canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("cross-array indexed guard-compare canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a[1]=20 < b[3]=4 FALSE and reverse TRUE (exit 70); exit 71 = base/index \
         confusion across the two arrays. got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_dual_indexed_guard_equality_exit_canary_runs() {
    // PROBE: `arr[i] == arr[j]` (equality op, both runtime indices) in a guard.
    // arr[0]=10 == arr[3]=10 TRUE; arr[0]=10 == arr[1]=20 FALSE -> exit 70.
    let canary = pass_canary("collections/runtime_dual_indexed_guard_equality_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-dual-idx-eq-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("dual-indexed guard-equality canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("dual-indexed guard-equality canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("dual-indexed guard-equality canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected arr[0]==arr[3] TRUE and arr[0]==arr[1] FALSE (exit 70); exit 71 = the \
         equality compared the wrong elements. got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_dual_indexed_guard_compare_exit_canary_runs() {
    // PROBE: `arr[i] < arr[j]` (both runtime indices) in a guard. arr[1]=20 <
    // arr[3]=40 is TRUE -> exit 70. Exit 71 = silent miscompile.
    let canary = pass_canary("collections/runtime_dual_indexed_guard_compare_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-dual-idx-guard-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("dual-indexed guard-compare canary should compile");

    let executable = compilation
        .checked_native_executable_path()
        .expect("dual-indexed guard-compare canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("dual-indexed guard-compare canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected arr[1]=20 < arr[3]=40 to be TRUE (exit 70); exit 71 = the guard \
         compared the wrong elements (silent miscompile). got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_float_running_min_max_fold_exit_canary_runs() {
    // A RUNNING float min/max fold across state edges: `self.lo = min(self.lo,
    // self.cur)` reads an accumulator field, feeds minsd/maxsd, and writes it
    // back each iteration (the constant-operand canary never reaches this
    // field-read-then-write-back path). Over [5, 2, 8, 3]: lo->2, hi->8, sum 10.
    let canary = pass_canary("arithmetic/runtime_float_running_min_max_fold_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-float-minmax-fold-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("float running min/max fold canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("float running min/max fold canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected running float min/max fold over [5,2,8,3] to give lo=2,hi=8,sum=10 \
         (exit 70); exit 71 = the field-accumulator min/max fold disagreed. got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_clamp_desugar_exit_canary_runs() {
    // `clamp(x, lo, hi)` = `min(max(x, lo), hi)`: 300->255, -5->0, 128->128.
    let canary = pass_canary("arithmetic/runtime_clamp_desugar_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-clamp-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("clamp desugar canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("clamp desugar canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected clamp above/below/within to give 255/0/128 (exit 70); got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_clamp_narrowing_exit_canary_runs() {
    // clamp's [0,100] result interval flows to the decision-17 narrowing check,
    // so `self.i8 = clamp(self.i32, 0, 100)` compiles AND the backend stores the
    // clamped value: clamp(300,0,100)=100 lands in i8, read back = exit 100.
    let canary = pass_canary("arithmetic/runtime_clamp_narrowing_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-clamp-narrow-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("clamp narrowing canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("clamp narrowing canary should run");

    assert_eq!(
        output.status.code(),
        Some(100),
        "expected clamp(300,0,100)=100 stored in i8 (exit 100); got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_negative_float_to_int_exit_canary_runs() {
    // Negative f64 -> i32 truncates toward zero: -3.7 -> -3, -100.0 -> -100.
    // Guards on the results and exits 70 (a positive code) so the assertion is
    // robust to shells that mangle negative process exit codes.
    let canary = pass_canary("arithmetic/runtime_negative_float_to_int_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-negfloat-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("negative float->int canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("negative float->int canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected -3.7->-3 and -100.0->-100 (exit 70); got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn float_to_int_policy_canary_runs() {
    let canary = pass_canary("float/float_to_int_policy_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-float-int-policy-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("float-to-int policy canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("float-to-int policy canary should run");
    assert_eq!(
        output.status.code(),
        Some(77),
        "expected target-width saturation and NaN->0 (exit 77); got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn float_to_int_exact_proofs_canary_runs() {
    let canary = pass_canary("float/float_to_int_exact_proofs_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-float-int-exact-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("proven Exact float-to-int canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("proven Exact float-to-int canary should run");
    assert_eq!(output.status.code(), Some(78));
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn float_to_int_trapping_canaries_abort() {
    for name in [
        "float/float_to_int_trapping_nan_traps",
        "float/float_to_int_trapping_overflow_traps",
    ] {
        let canary = pass_canary(name);
        let leaf = name.rsplit('/').next().unwrap_or("trap");
        let build_dir =
            std::env::temp_dir().join(format!("omega-float-int-{leaf}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&build_dir);
        compile_rooted_canary_for_native_host(&canary, build_dir.clone())
            .expect("Trapping float-to-int canary should compile");
        let output = Command::new(build_dir.join(executable_name()))
            .output()
            .expect("Trapping float-to-int canary should start");
        assert!(
            !output.status.success(),
            "{name} reached exit(70) instead of trapping"
        );
        assert_ne!(
            output.status.code(),
            Some(70),
            "{name} reached its post-operation sentinel instead of trapping"
        );
        let _ = fs::remove_dir_all(&build_dir);
    }
}

#[test]
fn float_saturating_arithmetic_canary_runs() {
    let canary = pass_canary("float/float_saturating_arithmetic_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-float-saturating-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("Saturating float arithmetic canary should compile");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("Saturating float arithmetic canary should run");
    assert_eq!(
        output.status.code(),
        Some(77),
        "expected overflow-only f32 saturation (exit 77); got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn float_trapping_arithmetic_canaries_abort() {
    for name in [
        "float/float_trapping_overflow_traps",
        "float/float_trapping_divide_zero_traps",
        "float/float_trapping_invalid_traps",
        "float/float_trapping_propagated_nan_traps",
        "float/float_trapping_propagated_infinity_traps",
    ] {
        let canary = pass_canary(name);
        let leaf = name.rsplit('/').next().unwrap_or("trap");
        let build_dir =
            std::env::temp_dir().join(format!("omega-float-{leaf}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&build_dir);
        compile_rooted_canary_for_native_host(&canary, build_dir.clone())
            .expect("Trapping float arithmetic canary should compile from its authored root");
        let output = Command::new(build_dir.join(executable_name()))
            .output()
            .expect("Trapping float arithmetic canary should start");
        assert!(
            !output.status.success(),
            "{name} reached exit(70) instead of trapping"
        );
        let _ = fs::remove_dir_all(&build_dir);
    }
}

#[test]
fn runtime_sqrt_builtin_exit_canary_runs() {
    // `sqrt(x)` unary float builtin: f64 sqrt(64)=8, f32 sqrt(9)=3, via the
    // native sqrtsd/sqrtss path (both operands = x on the binary SSE lane).
    let canary = pass_canary("arithmetic/runtime_sqrt_builtin_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-sqrt-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("sqrt builtin canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("sqrt builtin canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected sqrt(64.0)=8.0 and sqrt(9.0f32)=3.0 (exit 70); got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_abs_desugar_exit_canary_runs() {
    // `abs(x)` desugars to `max(x, 0 - x)` (frontend-only; min/max are binary
    // builtins). abs(-70)=70 and abs(12)=12; exit 70 confirms both.
    let canary = pass_canary("arithmetic/runtime_abs_desugar_exit");
    let build_dir = std::env::temp_dir().join(format!("omega-abs-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("abs desugar canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("abs desugar canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected abs(-70)=70 and abs(12)=12 (exit 70); got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_float_self_compare_nan_exit_canary_runs() {
    // The canonical isNaN idiom `f != f` (and its `f == f` complement) on a
    // NaN operand: TRUE/FALSE per IEEE. Was silently folded to constants by
    // the untyped reflexive fold; the fold is TYPE-GATED now and float
    // self-compares lower to the real ucomis* runtime compare.
    let canary = pass_canary("arithmetic/runtime_float_self_compare_nan_exit");
    let build_dir =
        std::env::temp_dir().join(format!("omega-nan-self-compare-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);

    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("NaN self-compare canary should compile");

    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("NaN self-compare canary should run");

    assert_eq!(
        output.status.code(),
        Some(70),
        "expected `f != f` TRUE and `f == f` FALSE for NaN (exit 70); exit 71 = a reflexive \
         fold collapsed the float self-compare to a constant. got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_total_order_satisfiers_exit_canary_runs() {
    // F6: the explicit f32/f64 total orders are named trait satisfiers, and a
    // generic consumer selects their concrete machine symbols statically. Raw
    // NaN payloads and signed zero distinguish this from arithmetic `<`.
    let canary = pass_canary("float/runtime_total_order_satisfiers_exit");
    let main_path = canary.join("main.omg");

    let checked = compile_to_checked(&main_path, None)
        .expect("total-order satisfier canary should compile to checked trees");
    let interpreted = interpret(&checked, &[]);
    assert_eq!(
        interpreted.error, None,
        "interpreter should support the total-order satisfier path"
    );
    assert_eq!(
        interpreted.exit_code, 70,
        "interpreter should preserve the complete f32/f64 total-order edge set"
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-total-float-order-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile(CanaryCompileSpec {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("total-order satisfier canary should compile natively");

    let executable = compilation
        .checked_native_executable_path()
        .expect("total-order satisfier canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("total-order satisfier canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected IEEE totalOrder over NaNs/infinities/signed zero (exit 70), got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);

    // Keep both native instruction-selection families on the same static
    // satisfier path even when the host can execute only one of them.
    for target in ["linux_x86_64", "linux_arm64"] {
        let cross_dir = std::env::temp_dir().join(format!(
            "omega-total-float-order-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&cross_dir);
        compiler::compile(
            CompileRequest::new(CompilerOptions {
                root_path: main_path.clone(),
                build_dir: Some(cross_dir.clone()),
                target_name: Some(target.into()),
            })
            .with_requested_product(RequestedCompileProduct::NativeArtifact)
            .with_artifact_policy(ArtifactEmissionPolicy::OutputOnly),
        )
        .unwrap_or_else(|error| {
            panic!("total-order satisfier canary should cross-compile for {target}: {error:?}")
        });
        let _ = fs::remove_dir_all(&cross_dir);
    }
}

#[test]
fn build_runtime_float_semantics_twins_agree() {
    const DIFFERENTIAL_SUITE_ID: &str = "omega.float.hardware.macos_arm64.semantic-edge-twins.v1";
    const DIFFERENTIAL_COVERAGE: &[&str] = &[
        "one zero-argument semantic machine at build time and runtime",
        "binary32/binary64 nearest-even ties",
        "subnormal underflow and finite overflow",
        "signed zero, infinity, NaN, and partial comparisons",
        "minimum/maximum, classification, and square root",
        "directed arithmetic and directed FMA",
        "fused versus separately rounded multiply-add",
    ];
    // Provider-plan identity now retains exact package provenance for every
    // schema, requirement owner, provider type, and origin. Builtin float
    // providers carry the explicit unbound identity; the companion stability
    // invariant lives with the float plan suite.
    const EXPECTED_DIFFERENTIAL_RESULT_IDENTITY: u64 = 0xc8d6_5187_ebc2_db51;

    let canary = pass_canary("float/build_runtime_semantics_twins");
    let main_path = canary.join("main.omg");
    let checked = compile_to_checked(&main_path, None)
        .expect("float semantic twins should compile and evaluate their array length");

    let mut selected_intrinsics = std::collections::BTreeSet::new();
    let mut selected_plan_identities = Vec::new();
    for provider_plan_report_fingerprint in checked
        .facts
        .operators
        .resolved_uses()
        .map(|operator_use| operator_use.provider_plan_report_fingerprint)
        .chain(
            checked
                .facts
                .operators
                .named_uses()
                .map(|operator_use| operator_use.provider_plan_report_fingerprint),
        )
    {
        let Some(plan) = checked
            .selected_provider_plans()
            .plan_by_report_fingerprint(provider_plan_report_fingerprint)
        else {
            continue;
        };
        let [row] = plan.rows.as_slice() else {
            continue;
        };
        let effects::provider_plan::ProviderBinding::CompilerIntrinsic { machine: name } =
            &row.binding
        else {
            continue;
        };
        selected_intrinsics.insert(name.clone());
        selected_plan_identities.push(plan.report_fingerprint());
    }
    selected_plan_identities.sort_unstable();
    selected_plan_identities.dedup();
    assert_eq!(
        selected_intrinsics.len(),
        56,
        "{DIFFERENTIAL_SUITE_ID} must bind every operation/format edge used by the twin"
    );
    assert_eq!(
        selected_plan_identities.len(),
        56,
        "{DIFFERENTIAL_SUITE_ID} must retain one exact plan per selected intrinsic"
    );

    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.error, None,
        "the runtime half of the float semantic twins should interpret"
    );
    assert_eq!(
        outcome.exit_code, 70,
        "build-time and runtime f32/f64 edge families should agree"
    );

    let build_dir = std::env::temp_dir().join(format!(
        "omega-float-semantic-edge-twins-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CanaryCompileSpec {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("float semantic twins should compile natively");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("float semantic twins should run natively");
    assert_eq!(
        output.status.code(),
        Some(70),
        "native runtime must agree with the build-time twin; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);

    let scratch = std::env::temp_dir().join(format!(
        "omega-float-semantic-edge-twins-linux-arm64-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let source_dir = scratch.join("src");
    fs::create_dir_all(&source_dir).expect("semantic-edge cross-target source directory");
    fs::copy(canary.join("main.omg"), source_dir.join("main.omg"))
        .expect("copy semantic-edge twin canary");
    fs::write(
        source_dir.join("build.omg"),
        "\
         machine build(builder: &mut Build) {\n\
             builder.application(\"float-semantic-edge-twins\");\n\
             builder.roots.bind(linux_arm64::ProgramEntry, Main::main);\n\
         }\n",
    )
    .expect("write semantic-edge build source");
    compile(CanaryCompileSpec {
        root_path: source_dir.join("main.omg"),
        build_dir: Some(scratch.join("out")),
        target_name: Some("linux_arm64".to_owned()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .unwrap_or_else(|diagnostics| {
        panic!("float semantic twins should compile for linux_arm64: {diagnostics:#?}")
    });
    let _ = fs::remove_dir_all(&scratch);

    let result_identity = retained_float_differential_result_identity(
        DIFFERENTIAL_SUITE_ID,
        "macos_arm64",
        DIFFERENTIAL_COVERAGE,
        &selected_intrinsics,
        &selected_plan_identities,
        &outcome,
        &output,
        &["linux_arm64"],
    );
    assert_eq!(
        result_identity, EXPECTED_DIFFERENTIAL_RESULT_IDENTITY,
        "{DIFFERENTIAL_SUITE_ID} result changed ({result_identity:#018x}); validate the exact plans, build/runtime edge corpus, interpreter/native results, and cross-target build before refreshing the retained identity"
    );
}

#[test]
fn linux_arm64_float_semantic_edge_twin_retains_artifact_evidence() {
    const SUITE_ID: &str = "omega.float.hardware.linux_arm64.semantic-edge-twin.v1";
    const COVERAGE: &[&str] = &[
        "one zero-argument semantic machine at build time and runtime",
        "binary32/binary64 nearest-even ties",
        "subnormal underflow and finite overflow",
        "signed zero, infinity, NaN, and partial comparisons",
        "minimum/maximum, classification, and square root",
        "directed arithmetic and directed FMA",
        "fused versus separately rounded multiply-add",
    ];
    const EXPECTED_PLAN_COUNT: usize = 56;
    const EXPECTED_BUILD_ARTIFACT_IDENTITY: u64 = 0xfc80_03e9_bf11_3370;
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    const EXPECTED_HOSTED_EXECUTION_IDENTITY: u64 = 0xf9bd_4b4f_c0cb_1bbb;

    fn retain(hash: &mut u64, bytes: &[u8]) {
        for byte in (bytes.len() as u64)
            .to_le_bytes()
            .into_iter()
            .chain(bytes.iter().copied())
        {
            *hash ^= u64::from(byte);
            *hash = hash.wrapping_mul(0x100000001b3);
        }
    }

    let canary = pass_canary("float/build_runtime_semantics_twins");
    let scratch = std::env::temp_dir().join(format!(
        "omega-linux-arm64-float-semantic-edge-twin-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    let source_dir = scratch.join("src");
    fs::create_dir_all(&source_dir).expect("Linux AArch64 semantic-edge source directory");
    fs::copy(canary.join("main.omg"), source_dir.join("main.omg"))
        .expect("copy Linux AArch64 semantic-edge twin canary");
    fs::write(
        source_dir.join("build.omg"),
        "\
         machine build(builder: &mut Build) {\n\
             builder.application(\"linux-arm64-float-semantic-edge-twin\");\n\
             builder.roots.bind(linux_arm64::ProgramEntry, Main::main);\n\
         }\n",
    )
    .expect("write Linux AArch64 semantic-edge build source");
    let main_path = source_dir.join("main.omg");

    let checked = compile_to_checked(&main_path, Some("linux_arm64"))
        .expect("Linux AArch64 float twin should compile and evaluate its array length");
    let mut selected_intrinsics = std::collections::BTreeSet::new();
    let mut selected_plan_identities = Vec::new();
    for provider_plan_report_fingerprint in checked
        .facts
        .operators
        .resolved_uses()
        .map(|operator_use| operator_use.provider_plan_report_fingerprint)
        .chain(
            checked
                .facts
                .operators
                .named_uses()
                .map(|operator_use| operator_use.provider_plan_report_fingerprint),
        )
    {
        let Some(plan) = checked
            .selected_provider_plans()
            .plan_by_report_fingerprint(provider_plan_report_fingerprint)
        else {
            continue;
        };
        let [row] = plan.rows.as_slice() else {
            continue;
        };
        let effects::provider_plan::ProviderBinding::CompilerIntrinsic { machine } = &row.binding
        else {
            continue;
        };
        selected_intrinsics.insert(machine.clone());
        selected_plan_identities.push(plan.report_fingerprint());
    }
    selected_plan_identities.sort_unstable();
    selected_plan_identities.dedup();
    assert_eq!(selected_intrinsics.len(), EXPECTED_PLAN_COUNT);
    assert_eq!(selected_plan_identities.len(), EXPECTED_PLAN_COUNT);
    assert!(
        selected_intrinsics
            .iter()
            .any(|intrinsic| intrinsic.contains("fused_multiply_add"))
    );
    assert!(
        selected_intrinsics
            .iter()
            .any(|intrinsic| intrinsic.contains("toward_"))
    );

    let interpreted = interpret(&checked, &[]);
    assert_eq!(interpreted.error, None);
    assert_eq!(interpreted.exit_code, 70);

    let first_build = compile(CanaryCompileSpec {
        root_path: main_path.clone(),
        build_dir: Some(scratch.join("first-build")),
        target_name: Some("linux_arm64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("float twin should produce an exact linux_arm64 image");
    let first_path = first_build
        .checked_native_executable_path()
        .expect("linux_arm64 cross-build must retain its executable receipt");
    let image_bytes = fs::read(first_path).expect("read retained linux_arm64 image bytes");
    assert!(!image_bytes.is_empty());
    assert_eq!(image_bytes.get(..4), Some(b"\x7fELF".as_slice()));
    assert_eq!(
        image_bytes.get(18..20),
        Some([0xb7, 0x00].as_slice()),
        "retained ELF image must name the AArch64 machine"
    );

    let second_build = compile(CanaryCompileSpec {
        root_path: main_path.clone(),
        build_dir: Some(scratch.join("second-build")),
        target_name: Some("linux_arm64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("float twin should reproduce its exact linux_arm64 image");
    let second_bytes = fs::read(
        second_build
            .checked_native_executable_path()
            .expect("second linux_arm64 cross-build must retain its executable receipt"),
    )
    .expect("read second linux_arm64 image bytes");
    assert_eq!(image_bytes, second_bytes);

    let mut build_artifact_identity = 0xcbf29ce484222325_u64;
    retain(&mut build_artifact_identity, SUITE_ID.as_bytes());
    retain(
        &mut build_artifact_identity,
        b"evidence:cross-build-exact-image",
    );
    for category in COVERAGE {
        retain(&mut build_artifact_identity, category.as_bytes());
    }
    for intrinsic in &selected_intrinsics {
        retain(&mut build_artifact_identity, intrinsic.as_bytes());
    }
    for identity in &selected_plan_identities {
        retain(&mut build_artifact_identity, &identity.to_le_bytes());
    }
    retain(
        &mut build_artifact_identity,
        &interpreted.exit_code.to_le_bytes(),
    );
    retain(&mut build_artifact_identity, &interpreted.stdout);
    retain(&mut build_artifact_identity, &interpreted.stderr);
    retain(&mut build_artifact_identity, b"target-profile:linux_arm64");
    retain(&mut build_artifact_identity, b"architecture:aarch64");
    retain(&mut build_artifact_identity, b"object-format:elf");
    retain(
        &mut build_artifact_identity,
        &(target::NativeTarget::linux_arm64().pointer_size as u64).to_le_bytes(),
    );
    retain(
        &mut build_artifact_identity,
        &(target::NativeTarget::linux_arm64().pointer_alignment as u64).to_le_bytes(),
    );
    retain(
        &mut build_artifact_identity,
        &fs::read(&main_path).expect("read retained float semantic-edge source bytes"),
    );
    retain(
        &mut build_artifact_identity,
        &fs::read(source_dir.join("build.omg")).expect("read retained Linux AArch64 build binding"),
    );
    retain(&mut build_artifact_identity, &image_bytes);
    assert_eq!(
        build_artifact_identity, EXPECTED_BUILD_ARTIFACT_IDENTITY,
        "{SUITE_ID} build/artifact identity changed ({build_artifact_identity:#018x}); validate the exact plans, interpreter result, target binding, and reproducible image before refreshing it",
    );

    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        let output = Command::new(first_path)
            .output()
            .expect("hosted linux_arm64 float twin should execute its retained image");
        assert_eq!(output.status.code(), Some(70));
        let mut execution_identity = build_artifact_identity;
        retain(&mut execution_identity, b"evidence:hosted-native-execution");
        retain(
            &mut execution_identity,
            &output.status.code().unwrap_or_default().to_le_bytes(),
        );
        retain(&mut execution_identity, &output.stdout);
        retain(&mut execution_identity, &output.stderr);
        assert_eq!(
            execution_identity, EXPECTED_HOSTED_EXECUTION_IDENTITY,
            "{SUITE_ID} hosted execution identity changed ({execution_identity:#018x})",
        );
    }

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn linux_x64_baseline_float_semantic_edge_twin_retains_artifact_evidence() {
    const SUITE_ID: &str = "omega.float.hardware.linux_x64.baseline-semantic-edge-twin.v1";
    const COVERAGE: &[&str] = &[
        "one zero-argument baseline semantic machine at build time and runtime",
        "binary32/binary64 nearest-even add/subtract/multiply/divide",
        "subnormal underflow and finite overflow",
        "signed zero, infinities, NaNs, and partial comparisons",
        "minimum/maximum, classify/predicates, square root, and negate",
        "separately rounded multiply-then-add",
    ];
    const EXPECTED_PLAN_COUNT: usize = 36;
    const EXPECTED_BUILD_ARTIFACT_IDENTITY: u64 = 0xa237_8240_2355_e2c9;
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    const EXPECTED_HOSTED_EXECUTION_IDENTITY: u64 = 0x895e_d190_164e_b67e;

    fn retain(hash: &mut u64, bytes: &[u8]) {
        for byte in (bytes.len() as u64)
            .to_le_bytes()
            .into_iter()
            .chain(bytes.iter().copied())
        {
            *hash ^= u64::from(byte);
            *hash = hash.wrapping_mul(0x100000001b3);
        }
    }

    let canary = pass_canary("float/build_runtime_semantics_twins_x86_baseline");
    let main_path = canary.join("main.omg");
    let scratch = std::env::temp_dir().join(format!(
        "omega-x86-baseline-float-semantic-edge-twin-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);

    let checked = compile_to_checked(&main_path, Some("linux_x86_64"))
        .expect("baseline x86 float twin should compile and evaluate its array length");
    let mut selected_intrinsics = std::collections::BTreeSet::new();
    let mut selected_plan_identities = Vec::new();
    for provider_plan_report_fingerprint in checked
        .facts
        .operators
        .resolved_uses()
        .map(|operator_use| operator_use.provider_plan_report_fingerprint)
        .chain(
            checked
                .facts
                .operators
                .named_uses()
                .map(|operator_use| operator_use.provider_plan_report_fingerprint),
        )
    {
        let Some(plan) = checked
            .selected_provider_plans()
            .plan_by_report_fingerprint(provider_plan_report_fingerprint)
        else {
            continue;
        };
        let [row] = plan.rows.as_slice() else {
            continue;
        };
        let effects::provider_plan::ProviderBinding::CompilerIntrinsic { machine } = &row.binding
        else {
            continue;
        };
        selected_intrinsics.insert(machine.clone());
        selected_plan_identities.push(plan.report_fingerprint());
    }
    selected_plan_identities.sort_unstable();
    selected_plan_identities.dedup();
    assert_eq!(selected_intrinsics.len(), EXPECTED_PLAN_COUNT);
    assert_eq!(selected_plan_identities.len(), EXPECTED_PLAN_COUNT);
    assert!(selected_intrinsics.iter().all(|intrinsic| {
        !intrinsic.contains("toward_") && !intrinsic.contains("fused_multiply_add")
    }));

    let interpreted = interpret(&checked, &[]);
    assert_eq!(interpreted.error, None);
    assert_eq!(interpreted.exit_code, 70);

    let first_build = compile(CanaryCompileSpec {
        root_path: main_path.clone(),
        build_dir: Some(scratch.join("first-build")),
        target_name: Some("linux_x86_64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("baseline x86 float twin should produce an exact linux_x64 image");
    let first_path = first_build
        .checked_native_executable_path()
        .expect("linux_x64 cross-build must retain its executable receipt");
    let image_bytes = fs::read(first_path).expect("read retained linux_x64 image bytes");
    assert!(!image_bytes.is_empty());
    assert_eq!(image_bytes.get(..4), Some(b"\x7fELF".as_slice()));
    assert_eq!(
        image_bytes.get(18..20),
        Some([0x3e, 0x00].as_slice()),
        "retained ELF image must name the x86-64 machine"
    );

    let second_build = compile(CanaryCompileSpec {
        root_path: main_path.clone(),
        build_dir: Some(scratch.join("second-build")),
        target_name: Some("linux_x86_64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("baseline x86 float twin should reproduce its exact linux_x64 image");
    let second_bytes = fs::read(
        second_build
            .checked_native_executable_path()
            .expect("second linux_x64 cross-build must retain its executable receipt"),
    )
    .expect("read second linux_x64 image bytes");
    assert_eq!(image_bytes, second_bytes);

    let mut build_artifact_identity = 0xcbf29ce484222325_u64;
    retain(&mut build_artifact_identity, SUITE_ID.as_bytes());
    retain(
        &mut build_artifact_identity,
        b"evidence:cross-build-exact-image",
    );
    for category in COVERAGE {
        retain(&mut build_artifact_identity, category.as_bytes());
    }
    for intrinsic in &selected_intrinsics {
        retain(&mut build_artifact_identity, intrinsic.as_bytes());
    }
    for identity in &selected_plan_identities {
        retain(&mut build_artifact_identity, &identity.to_le_bytes());
    }
    retain(
        &mut build_artifact_identity,
        &interpreted.exit_code.to_le_bytes(),
    );
    retain(&mut build_artifact_identity, &interpreted.stdout);
    retain(&mut build_artifact_identity, &interpreted.stderr);
    retain(&mut build_artifact_identity, b"target-profile:linux_x86_64");
    retain(&mut build_artifact_identity, b"architecture:x86_64");
    retain(&mut build_artifact_identity, b"object-format:elf");
    retain(
        &mut build_artifact_identity,
        &(target::NativeTarget::linux_x64().pointer_size as u64).to_le_bytes(),
    );
    retain(
        &mut build_artifact_identity,
        &(target::NativeTarget::linux_x64().pointer_alignment as u64).to_le_bytes(),
    );
    retain(
        &mut build_artifact_identity,
        &fs::read(&main_path).expect("read retained x86 baseline source bytes"),
    );
    retain(
        &mut build_artifact_identity,
        &fs::read(canary.join("build.omg")).expect("read retained linux_x64 build binding"),
    );
    retain(&mut build_artifact_identity, &image_bytes);
    assert_eq!(
        build_artifact_identity, EXPECTED_BUILD_ARTIFACT_IDENTITY,
        "{SUITE_ID} build/artifact identity changed ({build_artifact_identity:#018x}); validate the exact baseline plans, interpreter result, target binding, and reproducible image before refreshing it",
    );

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        let output = Command::new(first_path)
            .output()
            .expect("hosted linux_x64 baseline twin should execute its retained image");
        assert_eq!(output.status.code(), Some(70));
        let mut execution_identity = build_artifact_identity;
        retain(&mut execution_identity, b"evidence:hosted-native-execution");
        retain(
            &mut execution_identity,
            &output.status.code().unwrap_or_default().to_le_bytes(),
        );
        retain(&mut execution_identity, &output.stdout);
        retain(&mut execution_identity, &output.stderr);
        assert_eq!(
            execution_identity, EXPECTED_HOSTED_EXECUTION_IDENTITY,
            "{SUITE_ID} hosted execution identity changed ({execution_identity:#018x})",
        );
    }

    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn windows_x64_baseline_float_semantic_edge_twin_retains_artifact_evidence() {
    const SUITE_ID: &str = "omega.float.hardware.windows_x64.baseline-semantic-edge-twin.v1";
    const COVERAGE: &[&str] = &[
        "one zero-argument baseline semantic machine at build time and runtime",
        "binary32/binary64 nearest-even add/subtract/multiply/divide",
        "subnormal underflow and finite overflow",
        "signed zero, infinities, NaNs, and partial comparisons",
        "minimum/maximum, classify/predicates, square root, and negate",
        "separately rounded multiply-then-add",
    ];
    const EXPECTED_PLAN_COUNT: usize = 36;
    const EXPECTED_BUILD_ARTIFACT_IDENTITY: u64 = 0x0551_4042_2e6f_f7b1;
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    const EXPECTED_HOSTED_EXECUTION_IDENTITY: u64 = 0xa36f_0003_a672_28d6;

    fn retain(hash: &mut u64, bytes: &[u8]) {
        for byte in (bytes.len() as u64)
            .to_le_bytes()
            .into_iter()
            .chain(bytes.iter().copied())
        {
            *hash ^= u64::from(byte);
            *hash = hash.wrapping_mul(0x100000001b3);
        }
    }

    let canary = pass_canary("float/build_runtime_semantics_twins_windows_x64");
    let main_path = canary.join("main.omg");
    let scratch = std::env::temp_dir().join(format!(
        "omega-windows-x64-baseline-float-semantic-edge-twin-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);

    let checked = compile_to_checked(&main_path, Some("windows_x86_64"))
        .expect("baseline Windows x64 float twin should compile and evaluate its array length");
    let mut selected_intrinsics = std::collections::BTreeSet::new();
    let mut selected_plan_identities = Vec::new();
    for provider_plan_report_fingerprint in checked
        .facts
        .operators
        .resolved_uses()
        .map(|operator_use| operator_use.provider_plan_report_fingerprint)
        .chain(
            checked
                .facts
                .operators
                .named_uses()
                .map(|operator_use| operator_use.provider_plan_report_fingerprint),
        )
    {
        let Some(plan) = checked
            .selected_provider_plans()
            .plan_by_report_fingerprint(provider_plan_report_fingerprint)
        else {
            continue;
        };
        let [row] = plan.rows.as_slice() else {
            continue;
        };
        let effects::provider_plan::ProviderBinding::CompilerIntrinsic { machine } = &row.binding
        else {
            continue;
        };
        selected_intrinsics.insert(machine.clone());
        selected_plan_identities.push(plan.report_fingerprint());
    }
    selected_plan_identities.sort_unstable();
    selected_plan_identities.dedup();
    assert_eq!(selected_intrinsics.len(), EXPECTED_PLAN_COUNT);
    assert_eq!(selected_plan_identities.len(), EXPECTED_PLAN_COUNT);
    assert!(selected_intrinsics.iter().all(|intrinsic| {
        !intrinsic.contains("toward_") && !intrinsic.contains("fused_multiply_add")
    }));

    let interpreted = interpret(&checked, &[]);
    assert_eq!(interpreted.error, None);
    assert_eq!(interpreted.exit_code, 70);

    let first_build = compile(CanaryCompileSpec {
        root_path: main_path.clone(),
        build_dir: Some(scratch.join("first-build")),
        target_name: Some("windows_x86_64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("baseline Windows x64 float twin should produce an exact windows_x64 image");
    let first_path = first_build
        .checked_native_executable_path()
        .expect("windows_x64 cross-build must retain its executable receipt");
    let image_bytes = fs::read(first_path).expect("read retained windows_x64 image bytes");
    assert_eq!(image_bytes.get(..2), Some(b"MZ".as_slice()));
    let pe_offset = u32::from_le_bytes(
        image_bytes
            .get(0x3c..0x40)
            .expect("PE image must retain its DOS e_lfanew field")
            .try_into()
            .expect("PE e_lfanew field has an exact four-byte width"),
    ) as usize;
    assert_eq!(
        image_bytes.get(pe_offset..pe_offset + 4),
        Some(b"PE\0\0".as_slice()),
        "retained image must carry the PE signature at e_lfanew"
    );
    assert_eq!(
        image_bytes.get(pe_offset + 4..pe_offset + 6),
        Some([0x64, 0x86].as_slice()),
        "retained PE image must name the AMD64 machine"
    );

    let second_build = compile(CanaryCompileSpec {
        root_path: main_path.clone(),
        build_dir: Some(scratch.join("second-build")),
        target_name: Some("windows_x86_64".into()),
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("baseline Windows x64 float twin should reproduce its exact windows_x64 image");
    let second_bytes = fs::read(
        second_build
            .checked_native_executable_path()
            .expect("second windows_x64 cross-build must retain its executable receipt"),
    )
    .expect("read second windows_x64 image bytes");
    assert_eq!(image_bytes, second_bytes);

    let mut build_artifact_identity = 0xcbf29ce484222325_u64;
    retain(&mut build_artifact_identity, SUITE_ID.as_bytes());
    retain(
        &mut build_artifact_identity,
        b"evidence:cross-build-exact-image",
    );
    for category in COVERAGE {
        retain(&mut build_artifact_identity, category.as_bytes());
    }
    for intrinsic in &selected_intrinsics {
        retain(&mut build_artifact_identity, intrinsic.as_bytes());
    }
    for identity in &selected_plan_identities {
        retain(&mut build_artifact_identity, &identity.to_le_bytes());
    }
    retain(
        &mut build_artifact_identity,
        &interpreted.exit_code.to_le_bytes(),
    );
    retain(&mut build_artifact_identity, &interpreted.stdout);
    retain(&mut build_artifact_identity, &interpreted.stderr);
    retain(
        &mut build_artifact_identity,
        b"target-profile:windows_x86_64",
    );
    retain(&mut build_artifact_identity, b"architecture:x86_64");
    retain(&mut build_artifact_identity, b"object-format:coff");
    retain(
        &mut build_artifact_identity,
        &(target::NativeTarget::windows_x64().pointer_size as u64).to_le_bytes(),
    );
    retain(
        &mut build_artifact_identity,
        &(target::NativeTarget::windows_x64().pointer_alignment as u64).to_le_bytes(),
    );
    retain(
        &mut build_artifact_identity,
        &fs::read(&main_path).expect("read retained Windows x64 baseline source bytes"),
    );
    retain(
        &mut build_artifact_identity,
        &fs::read(canary.join("build.omg")).expect("read retained windows_x64 build binding"),
    );
    retain(&mut build_artifact_identity, &image_bytes);
    assert_eq!(
        build_artifact_identity, EXPECTED_BUILD_ARTIFACT_IDENTITY,
        "{SUITE_ID} build/artifact identity changed ({build_artifact_identity:#018x}); validate the exact baseline plans, interpreter result, target binding, and reproducible image before refreshing it",
    );

    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        let output = Command::new(first_path)
            .output()
            .expect("hosted windows_x64 baseline twin should execute its retained image");
        assert_eq!(output.status.code(), Some(70));
        let mut execution_identity = build_artifact_identity;
        retain(&mut execution_identity, b"evidence:hosted-native-execution");
        retain(
            &mut execution_identity,
            &output.status.code().unwrap_or_default().to_le_bytes(),
        );
        retain(&mut execution_identity, &output.stdout);
        retain(&mut execution_identity, &output.stderr);
        assert_eq!(
            execution_identity, EXPECTED_HOSTED_EXECUTION_IDENTITY,
            "{SUITE_ID} hosted execution identity changed ({execution_identity:#018x})",
        );
    }

    let _ = fs::remove_dir_all(&scratch);
}
