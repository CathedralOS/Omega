use super::fixture_roster;
use crate::{
    ACTIVE_FAIL_CANARIES, CANARY_UMBRELLA_LOCK, CHECKED_ONLY_FAIL_CANARIES,
    CROSS_TARGET_FAIL_CANARIES, Command, check_canary, compile_canary_without_output,
    compile_canary_without_output_for_target, compile_native_canary_without_output,
    compile_reviewed_repository_fixture, compile_rooted_canary_for_native_host,
    compile_terminal_canary_without_output_for_target, executable_name, fail_canary, fs,
    pass_canary, run_bounded_canary_jobs,
};
use compiler::CheckedCompileRequest;

#[test]
fn runtime_ranked_accumulator_guarantee_exit_canary_runs() {
    // A ranked free loop returning its accumulator under `ensures result <=
    // previous`: the source exit prover admits the guarantee through a proved
    // header invariant over the re-entered parameter, the producer lowers the
    // cycle with its `Natural` certificate, and `descend(3, 9)` returns 1.
    let canary = pass_canary(fixture_roster::PROOFS_RUNTIME_RANKED_ACCUMULATOR_GUARANTEE_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-ranked-accumulator-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("ranked accumulator guarantee canary should compile from its authored root");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("ranked accumulator guarantee canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected descend(3, 9) to return 1 under its proved guarantee (exit 70), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn accumulator_guarantee_twins_reject_in_checked_semantics() {
    // The twins share the checked-only pass canary's conserved
    // `acc + remaining` claim but break one obligation each: the wrong-step
    // twin forwards `acc` unchanged (preservation fails), the unestablished
    // twin asks for `+ 1` the loop never establishes, and the unbounded twin
    // drops the declared entry ranges that certify the contract's arithmetic.
    // The first two keep the identical `Nat::Descending` cycle certificate, so
    // the rejection is the functional claim's and not the termination
    // answer's.
    for &name in [
        fixture_roster::PROOFS_ACCUMULATOR_GUARANTEE_WRONG_STEP_TWIN,
        fixture_roster::PROOFS_ACCUMULATOR_GUARANTEE_UNESTABLISHED_TWIN,
        fixture_roster::PROOFS_ACCUMULATOR_GUARANTEE_UNBOUNDED_FORMALS,
    ]
    .iter()
    {
        let canary = fail_canary(name);
        let expected = fs::read_to_string(canary.join("expected.txt"))
            .expect("accumulator twin should carry expected.txt");
        let diagnostics = compile_native_canary_without_output(&canary)
            .expect_err("a broken accumulation claim must reject before native emission");
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
            // Production-route entries refuse behind checked semantics, so a
            // Check stop would admit them; they take the Terminal-artifact
            // route at their bound target first, which reaches the lowering
            // wall without entering native realization.
            let production_cross_target = fixture_roster::CROSS_TARGET_PRODUCTION_FAIL_CANARIES
                .iter()
                .find_map(|(candidate, target)| (*candidate == canary_name).then_some(*target));
            let cross_target = CROSS_TARGET_FAIL_CANARIES
                .iter()
                .find_map(|(candidate, target)| (*candidate == canary_name).then_some(*target));
            match (production_cross_target, cross_target) {
                (Some(target), _) => {
                    compile_terminal_canary_without_output_for_target(&canary, target)
                }
                (None, Some(target)) => compile_canary_without_output_for_target(&canary, target),
                (None, None) => compile_native_canary_without_output(&canary),
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
    let canary = pass_canary(fixture_roster::WIRE_DECODE_REQUIREMENT_SURFACE);
    compile_canary_without_output(&canary)
        .expect("strict, projecting, and preserving decode requirements should compile");
}

#[test]
fn range_gated_establishment_canaries_compile() {
    for &name in fixture_roster::RANGE_GATED_ESTABLISHMENT_PASS_CANARIES {
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
    for &name in fixture_roster::RANGE_GATED_ESTABLISHMENT_FILE_FAIL_CANARIES {
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
    for &name in fixture_roster::DEFAULT_DOMAIN_MEMBERSHIP_PASS_CANARIES {
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
    for &name in fixture_roster::DEFAULT_DOMAIN_MEMBERSHIP_FILE_FAIL_CANARIES {
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
    let pass = pass_canary(fixture_roster::DEPENDENT_DATA_WHERE_STANDING_BOUND_EXIT);
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

    let fail = fail_canary(fixture_roster::DEPENDENT_DATA_WHERE_STANDING_BOUND_ABSENT_REJECTED);
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
    for &name in fixture_roster::DEFAULT_DOMAIN_MEASURE_PASS_CANARIES {
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

    for &name in fixture_roster::DEFAULT_DOMAIN_MEASURE_FILE_FAIL_CANARIES {
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

    for &name in fixture_roster::DEFAULT_DOMAIN_STALE_FACT_FAIL_CANARIES {
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
    let canary = pass_canary(fixture_roster::DEPENDENT_DATA_WHERE_PRODUCT_HYPOTHESIS);
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

    for &name in fixture_roster::DEFAULT_DOMAIN_PRODUCT_FAIL_CANARIES {
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
    for &name in fixture_roster::DEFAULT_DOMAIN_CORRELATION_PASS_CANARIES {
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

    for &name in fixture_roster::DEFAULT_DOMAIN_CORRELATION_FAIL_CANARIES {
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
    for &name in fixture_roster::COMMUTATIVE_SEMIRING_PASS_CANARIES {
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

    for &name in fixture_roster::COMMUTATIVE_SEMIRING_CHECKED_PASS_CANARIES {
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
    let accepted = pass_canary(fixture_roster::PROOFS_NAT_EXACT_SUBTRACTION_COMPILE);
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

    let rejected = fail_canary(fixture_roster::PROOFS_NAT_EXACT_SUBTRACTION_REQUIRES_ORDER);
    let diagnostics = check_canary(&rejected)
        .expect_err("bare Nat subtraction without its order fact must reject");
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains("cannot prove requires contract for call subtract")
            && combined.contains("used <= total"),
        "{} rejected with the wrong diagnostic:\n{combined}",
        rejected.display()
    );
}

#[test]
fn algebraic_normalization_requires_an_exact_licensed_conformance() {
    let licensed = pass_canary(fixture_roster::PROOFS_RING_REARRANGE_CORE_NAT);
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

    for &name in fixture_roster::ALGEBRAIC_NORMALIZATION_FAIL_CANARIES {
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
    let canary = pass_canary(fixture_roster::PROOFS_RING_IDENTITY_SLOT_BRIDGE_COMPILE);
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
    let canary = pass_canary(fixture_roster::PROOFS_INTEGER_MEASURED_NAT_INDUCTION_COMPILE);
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
    let canary =
        pass_canary(fixture_roster::TERMINATION_PROOF_NON_TAIL_JOINT_MACHINE_CYCLE_COMPILE);
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

    for &(name, expected) in fixture_roster::PROOF_JOINT_RANKING_FAIL_CANARIES {
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
    let canary = pass_canary(fixture_roster::FLOAT_FLOAT_TO_INT_EXACT_PROOFS_EXIT);
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

    for &name in fixture_roster::EXACT_FLOAT_TO_INT_FAIL_CANARIES {
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
    let canary = pass_canary(fixture_roster::ARITHMETIC_RUNTIME_FLOAT_MIN_MAX_ABS_CLAMP_EXIT);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("generic float builtins should compile to checked trees");
    // Core spells `F32::minimum`/`maximum`/`square_root` as top-level boundary
    // requirements, so the desugared builtin uses are retained as named
    // requirement uses; a named operator use would carry the same custody.
    let uses = checked
        .facts
        .operators
        .named_uses()
        .map(|operator_use| {
            (
                operator_use.expression,
                operator_use.provider_plan_report_fingerprint,
                operator_use.provider_plan_commitment,
            )
        })
        .chain(
            checked
                .facts
                .operators
                .named_requirement_uses()
                .map(|requirement_use| {
                    (
                        requirement_use.expression,
                        requirement_use.provider_plan_report_fingerprint,
                        requirement_use.provider_plan_commitment,
                    )
                }),
        )
        .filter(|(expression, _, _)| {
            matches!(
                checked.typed.expression_table.expression(*expression),
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
        uses.iter().all(|(_, fingerprint, _)| *fingerprint != 0),
        "every normalized float builtin must carry its exact selected ProviderPlan"
    );
    assert!(
        uses.iter().all(|(_, _, commitment)| !commitment.is_empty()),
        "every normalized float builtin must carry its exact selected ProviderPlan commitment"
    );
}

#[test]
fn dependent_embed_self_field_view_canary() {
    let canary = pass_canary(fixture_roster::DEPENDENT_EMBED_SELF_FIELD_VIEW);
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
