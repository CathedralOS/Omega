use super::fixture_roster;
use crate::{
    CanaryCompileProduct, CanaryCompileSpec, Command, compile, compile_canary_without_output,
    compile_reviewed_repository_fixture, compile_rooted_canary_for_native_host, executable_name,
    fail_canary, fs, interpret, pass_canary, repo_root,
};
use checked_interpreter::BuildMachineEntry;
use checked_interpreter::InterpretOptions;
use compiler::CheckedCompileRequest;

#[test]
fn runtime_generic_multiple_specializations_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_GENERIC_MULTIPLE_SPECIALIZATIONS_EXIT);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("multiple generic-machine specialization tuples should check");
    assert_eq!(
        checked
            .machine_specializations
            .iter()
            .filter(|specialization| {
                checked.machines().iter().any(|machine| {
                    machine.symbol == specialization.template
                        && machine.name.as_str() == "Main::pick"
                })
            })
            .count(),
        2
    );
    let interpreted = interpret(&checked, &[]);
    assert_eq!(interpreted.error, None);
    assert_eq!(interpreted.exit_code, 14);

    let build_dir =
        std::env::temp_dir().join(format!("omega-gen-multi-spec-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("multiple generic-machine specialization tuples should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("multiple generic-machine specializations should retain their executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("multiple generic-machine specialization canary should run");
    assert_eq!(
        output.status.code(),
        Some(14),
        "expected both concrete clones to materialize results (exit 14), got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_nominal_machine_parameter_satisfaction_exit_canary_runs() {
    // CALLBACK-PARAMETER-REQUIREMENT native witness: `register<machine
    // Selected>` admits only a selection carrying the authored
    // `satisfies Handler::call` row, and each specialization must retain that
    // exact selected entry so the rewritten `Selected(value)` call site
    // reaches its own target entry recipe. `chosen` returns its argument and
    // `constant` returns a distinct value; both are reachable only through
    // the binder, so a dropped or swapped selection exits with a value other
    // than 70.
    let canary = pass_canary(fixture_roster::RUNTIME_NOMINAL_MACHINE_PARAMETER_SATISFACTION_EXIT);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("nominal machine-parameter satisfaction canary should reach checked trees");

    let register_specializations = checked
        .machine_specializations
        .iter()
        .filter(|specialization| {
            checked.machines().iter().any(|machine| {
                machine.symbol == specialization.template && machine.name.as_str() == "register"
            })
        })
        .count();
    assert_eq!(
        register_specializations, 2,
        "each nominal selection should specialize `register` once"
    );
    let uses = &checked.facts.nominal_machine_uses.uses;
    assert_eq!(uses.len(), 2, "one nominal use per selection site");
    for selected_name in ["chosen", "constant"] {
        let selected_machine = checked
            .machines()
            .iter()
            .find(|machine| checked.symbols.display_path(machine.symbol, "::") == selected_name)
            .unwrap_or_else(|| panic!("selected machine `{selected_name}` should be retained"));
        let selected_entry = checked
            .machine_states(selected_machine)
            .first()
            .expect("selected machine should retain its entry")
            .symbol;
        let nominal_use = uses
            .iter()
            .find(|nominal_use| nominal_use.selected_machine == selected_machine.symbol)
            .unwrap_or_else(|| panic!("`{selected_name}` should record a nominal use"));
        assert_eq!(
            nominal_use.selected_entry, selected_entry,
            "`{selected_name}` must bind its machine entry, not a sibling state"
        );
        assert!(
            !nominal_use
                .published_requirement_envelope
                .contract_commitment
                .is_zero()
                && !nominal_use
                    .selected_actual_envelope
                    .contract_commitment
                    .is_zero()
                && nominal_use.refinement.selected_actual_report_fingerprint
                    == nominal_use
                        .selected_actual_envelope
                        .contract_report_fingerprint
                && nominal_use.refinement.selected_actual_commitment
                    == nominal_use.selected_actual_envelope.contract_commitment,
            "`{selected_name}` must carry its envelope refinement receipt"
        );
    }

    let interpreted = interpret(&checked, &[]);
    assert_eq!(
        interpreted.error, None,
        "reference execution should succeed"
    );
    assert_eq!(
        interpreted.exit_code, 70,
        "reference execution should dispatch each nominal selection to its exact entry"
    );

    let build_dir = std::env::temp_dir().join(format!(
        "omega-nominal-machine-parameter-satisfaction-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("nominal machine-parameter satisfaction canary should compile natively");
    let executable = compilation.checked_native_executable_path().expect(
        "nominal machine-parameter satisfaction canary should retain its executable receipt",
    );
    let output = Command::new(executable)
        .output()
        .expect("nominal machine-parameter satisfaction canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "each specialization's rewritten Selected(value) call must reach its own \
         selected entry recipe (exit 70); got {:?}\nstderr:\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_generic_enum_payload_exit_canary_runs() {
    // A monomorphized generic ENUM with a T-typed payload (`Maybe<i32 in Wrapping>`), constructed,
    // matched, and destructured natively -- the Option<T> shape. Its erased evidence payload
    // remains semantic but takes no runtime storage. Exit 70 via the material payload.
    let canary = pass_canary(fixture_roster::RUNTIME_GENERIC_ENUM_PAYLOAD_EXIT);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("generic enum payload canary should reach checked semantics");
    let interpreted = checked_interpreter::interpret_entry(
        &checked,
        BuildMachineEntry::Name("Main::main"),
        &[],
        InterpretOptions::default(),
    );
    assert_eq!(interpreted.error, None);
    assert_eq!(interpreted.exit_code, 70);

    let build_dir = std::env::temp_dir().join(format!("omega-gen-enum-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("generic enum payload canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("generic enum payload canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("generic enum payload canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a monomorphized generic enum payload to destructure natively (exit 70), got {:?}
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_generic_record_instance_exit_canary_runs() {
    // A monomorphized generic data instance (`Box<i32 in Wrapping>`) with native field access to
    // both the T-typed field and a concrete sibling: tag=30 + val=40 -> exit 70. Locks stage-1
    // generics monomorphization (recorded instance layout keyed by the definition symbol).
    let canary = pass_canary(fixture_roster::RUNTIME_GENERIC_RECORD_INSTANCE_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-gen-inst-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("generic record instance canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("generic record instance canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("generic record instance canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected native field access on a monomorphized generic instance (exit 70), got {:?}
{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_const_data_array_length_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_CONST_DATA_ARRAY_LENGTH_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-const-data-array-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("literal const data argument should specialize the array extent");
    let executable = compilation
        .checked_native_executable_path()
        .expect("const-data array canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("const data array canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected specialized `[i32; 4]` storage to round-trip 70, got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_const_data_forwarded_length_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_CONST_DATA_FORWARDED_LENGTH_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-const-data-forwarded-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("forwarded const data argument should specialize the nested array extent");
    let executable = compilation
        .checked_native_executable_path()
        .expect("forwarded const-data canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("forwarded const data array canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected forwarded `[i32; 4]` storage to round-trip 70, got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_const_data_multiple_instances_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_CONST_DATA_MULTIPLE_INSTANCES_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-const-data-multi-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("distinct const data instances should compile to distinct layouts");
    let executable = compilation
        .checked_native_executable_path()
        .expect("multiple const-data instances should retain their executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("multiple const data instance canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected two const-specialized buffers to sum to 70, got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_const_data_named_value_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_CONST_DATA_NAMED_VALUE_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-const-data-named-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("named integer const arguments should specialize generic data");
    let executable = compilation
        .checked_native_executable_path()
        .expect("named const-data canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("named const data argument canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected named const-specialized buffers to sum to 70, got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn structured_const_identity_and_rat_canonicality_canaries() {
    for &path in fixture_roster::STRUCTURED_CONST_PASS_CANARIES {
        let canary = pass_canary(path);
        compile_canary_without_output(&canary).unwrap_or_else(|diagnostics| {
            panic!(
                "structured const pass canary `{path}` should compile:\n{}",
                diagnostics
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        });
    }

    for &(path, expected) in fixture_roster::STRUCTURED_CONST_FAIL_CANARIES {
        let canary = fail_canary(path);
        let diagnostics = compile_canary_without_output(&canary)
            .expect_err("noncanonical structured const index should reject");
        let combined = diagnostics
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            combined.contains(expected),
            "structured const fail canary `{path}` should contain `{expected}`:\n{combined}"
        );
    }
}

#[test]
fn closed_indexed_domain_canaries() {
    let pass = pass_canary(fixture_roster::CLOSED_INDEXED_QUANTITY);
    compile_canary_without_output(&pass).unwrap_or_else(|diagnostics| {
        panic!(
            "closed indexed domain package should compile:\n{}",
            diagnostics
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        )
    });
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &pass.join("main.omg"),
        None,
    ))
    .expect("closed indexed qualifications should survive checked lowering");
    let uses = &checked.facts.qualifications.vacuous_uses;
    assert_eq!(
        uses.len(),
        4,
        "the concrete closed qualification, the open `retag_i64` template, \
         and both concrete generic instances should each be retained"
    );
    for use_fact in uses {
        let machine = checked
            .machines()
            .iter()
            .find(|machine| machine.symbol == use_fact.machine)
            .expect("qualification owner machine");
        let state = checked
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == use_fact.state)
            .expect("qualification owner state");
        let typed_trees::types::TypeReferenceNode::Constrained { constraints, .. } = checked
            .type_reference_table
            .type_reference(state.return_type)
        else {
            panic!("indexed qualification canary result should remain constrained");
        };
        // The bounded carrier may add a range constraint beside the declared
        // domain; the evidence check selects exactly the domain member.
        let domain_constraints = checked
            .type_reference_table
            .constraints(*constraints)
            .iter()
            .filter_map(|constraint| match constraint {
                typed_trees::types::TypeConstraintNode::Domain(domain) => Some(domain),
                _ => None,
            })
            .collect::<Vec<_>>();
        let [result_domain] = domain_constraints.as_slice() else {
            panic!("indexed qualification canary result should carry exactly one domain");
        };
        assert_eq!(
            use_fact.semantic_domain, result_domain.semantic_id,
            "vacuous-use evidence must retain the exact indexed instance"
        );
    }
    let retag = checked
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "retag_i64")
        .expect("retag template instance");
    let specializations = checked
        .machine_specializations
        .iter()
        .filter(|specialization| specialization.template == retag.symbol)
        .collect::<Vec<_>>();
    assert_eq!(specializations.len(), 2);
    assert!(
        specializations
            .iter()
            .all(|specialization| specialization.const_arguments.len() == 1)
    );
    assert_ne!(
        specializations[0].const_arguments,
        specializations[1].const_arguments
    );
    assert_ne!(
        specializations[0].report_fingerprint,
        specializations[1].report_fingerprint
    );
    let interpreted = interpret(&checked, &[]);
    assert_eq!(interpreted.error, None);
    assert_eq!(interpreted.exit_code, 70);

    let build_dir = std::env::temp_dir().join(format!(
        "omega-closed-indexed-qualification-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CanaryCompileSpec {
        root_path: pass.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("closed indexed generic conversion should compile natively");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("closed indexed generic conversion should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected destination-specialized retag to return exit 70, got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);

    for &path in fixture_roster::CLOSED_INDEXED_FAIL_CANARIES {
        let canary = fail_canary(path);
        let expected = fs::read_to_string(canary.join("expected.txt"))
            .expect("closed indexed domain fail canary should carry expected.txt");
        let diagnostics = compile_canary_without_output(&canary)
            .expect_err("invalid closed indexed domain should reject");
        let combined = diagnostics
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            combined.contains(expected.trim()),
            "closed indexed domain fail canary `{path}` should contain {:?}:\n{combined}",
            expected.trim()
        );
    }
}

#[test]
fn std_units_package_conversion_and_operator_canaries() {
    let source = fs::read_to_string(repo_root().join("source/library/std/units.omg"))
        .expect("read shipped units package");
    for published_name in [
        "Units::METER",
        "Units::KILOMETER",
        "Units::SECOND",
        "Units::METER_PER_SECOND",
        "Units::KILOMETER_PER_HOUR",
        "kilometers_to_meters_trapping_i64",
        "meters_to_kilometers_truncating_i64",
        "divide_f64_meters_by_seconds",
        "divide_f64_kilometers_by_hours",
        "kilometers_per_hour_to_meters_per_second_f64",
    ] {
        assert!(
            source.contains(published_name),
            "units package should publish `{published_name}`"
        );
    }

    let pass = pass_canary(fixture_roster::RUNTIME_STD_UNITS_EXIT);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &pass.join("main.omg"),
        None,
    ))
    .expect("shipped named units, conversions, and operators should check");
    assert!(
        checked
            .facts
            .index_compatibility
            .conditions
            .iter()
            .any(|condition| matches!(
                &condition.discharge,
                checked_trees::IndexCompatibilityDischarge::ClosedEvaluation
            )),
        "closed unit flows should retain their closed-evaluation verification condition"
    );
    let interpreted = interpret(&checked, &[]);
    assert_eq!(interpreted.error, None);
    assert_eq!(interpreted.exit_code, 70);

    let build_dir =
        std::env::temp_dir().join(format!("omega-std-units-package-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CanaryCompileSpec {
        root_path: pass.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        product: CanaryCompileProduct::NativeArtifactAndPublish,
    })
    .expect("shipped units package should compile natively");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("shipped units canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected units conversion/operator canary to exit 70, got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);

    let fail = fail_canary(fixture_roster::STD_UNITS_IMPLICIT_CROSS_INDEX);
    let expected = fs::read_to_string(fail.join("expected.txt"))
        .expect("imported cross-index fail canary should carry expected.txt");
    let diagnostics = compile_canary_without_output(&fail)
        .expect_err("kilometers must not flow into a meters parameter implicitly");
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains(expected.trim()),
        "imported cross-index diagnostic should contain {:?}:\n{combined}",
        expected.trim()
    );
}

#[test]
fn open_computed_quantity_result_canary_runs() {
    let canary = pass_canary(fixture_roster::OPEN_COMPUTED_QUANTITY_RESULT);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("generic computed index result should check");
    let selections = checked
        .open_index_normalizations
        .iter()
        .flat_map(|normalization| &normalization.operations)
        .collect::<Vec<_>>();
    assert!(!selections.is_empty());
    assert!(selections.iter().all(|selection| {
        selection
            .operation_contract_identity
            .contains("IndexAdd::add")
            && selection.algebra_requirement == "add"
            && selection.algebra_alias.as_deref() == Some("Canonical")
            && selection.provider.is_valid()
            && selection.algebra_trait.is_valid()
    }));
    assert!(
        checked
            .machine_specializations
            .iter()
            .any(|specialization| {
                specialization.template_contract_report_fingerprint != 0
                    && !specialization.template_contract_commitment.is_zero()
            })
    );
    assert!(
        checked
            .facts
            .index_compatibility
            .conditions
            .iter()
            .any(|condition| {
                condition.name.starts_with("index-equality:")
                    && matches!(
                        &condition.discharge,
                        checked_trees::IndexCompatibilityDischarge::LicensedNormalization {
                            operation_count
                        } if *operation_count > 0
                    )
            }),
        "computed result flow should retain its licensed-normalization verification condition: {:#?}",
        checked.facts.index_compatibility.conditions
    );
    let interpreted = interpret(&checked, &[]);
    assert_eq!(interpreted.error, None);
    assert_eq!(interpreted.exit_code, 70);

    let fail = fail_canary(fixture_roster::OPEN_INDEX_UNLICENSED_ALGEBRA);
    let expected = fs::read_to_string(fail.join("expected.txt"))
        .expect("unlicensed open-index canary should carry expected.txt");
    let diagnostics = compile_canary_without_output(&fail)
        .expect_err("an unproved index algebra must not license normalization");
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(combined.contains(expected.trim()), "{combined}");
}

#[test]
fn open_index_exact_local_fact_canary_runs() {
    let canary = pass_canary(fixture_roster::OPEN_INDEX_LOCAL_FACT);
    let checked = compile_reviewed_repository_fixture(CheckedCompileRequest::new(
        &canary.join("main.omg"),
        None,
    ))
    .expect("an exact active equality should discharge open index compatibility");
    let conditions = checked
        .facts
        .index_compatibility
        .conditions
        .iter()
        .filter(|condition| {
            matches!(
                &condition.discharge,
                checked_trees::IndexCompatibilityDischarge::EstablishedLocalFacts { .. }
            )
        })
        .collect::<Vec<_>>();
    assert!(
        conditions.len() >= 2,
        "both requires and call-ensures routes should retain evidence"
    );
    assert!(
        conditions
            .iter()
            .all(|condition| condition.actual_instance != condition.expected_instance)
    );
    assert!(
        conditions.iter().any(|condition| matches!(
            &condition.discharge,
            checked_trees::IndexCompatibilityDischarge::EstablishedLocalFacts { facts }
                if facts.len() == 2
        )),
        "a two-member index pack should retain both exact equality facts"
    );
    let evidence = conditions
        .iter()
        .flat_map(|condition| {
            let checked_trees::IndexCompatibilityDischarge::EstablishedLocalFacts { facts } =
                &condition.discharge
            else {
                unreachable!();
            };
            assert!(!facts.is_empty());
            facts
                .iter()
                .map(|fact| {
                    assert!(fact.is_valid());
                    checked.facts.semantic.facts.get(*fact)
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    // Under conformance-bound requirements every `a + 0 == 0`-shaped
    // equality transports as a declared `requires` hypothesis: no call can
    // mint the fact without a requires carrying it, so the discharge evidence
    // lives at the machines' State entry points rather than a call-ensures
    // axiom.
    assert!(
        evidence
            .iter()
            .all(|fact| { matches!(fact.point, facts::ProgramPoint::State { .. }) })
    );
    let interpreted = interpret(&checked, &[]);
    assert_eq!(interpreted.error, None);
    assert_eq!(interpreted.exit_code, 70);

    let fail = fail_canary(fixture_roster::OPEN_INDEX_UNESTABLISHED_EQUALITY);
    let expected = fs::read_to_string(fail.join("expected.txt"))
        .expect("unestablished index equality canary should carry expected.txt");
    let diagnostics = compile_canary_without_output(&fail)
        .expect_err("an ambient but inactive theorem must not discharge index compatibility");
    let combined = diagnostics
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        combined.contains(expected.trim())
            && combined.contains("exact local equality fact is required"),
        "unestablished equality diagnostic should be named and fail closed:\n{combined}"
    );
}

#[test]
fn runtime_const_data_expression_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_CONST_DATA_EXPRESSION_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-const-data-expression-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("closed integer const expressions should specialize generic data");
    let executable = compilation
        .checked_native_executable_path()
        .expect("const-data expression canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("const data expression canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected expression-specialized buffers to sum to 70, got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_const_data_symbolic_expression_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_CONST_DATA_SYMBOLIC_EXPRESSION_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-const-data-symbolic-expression-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("symbolic integer const expressions should specialize generic data");
    let executable = compilation
        .checked_native_executable_path()
        .expect("symbolic const-data expression should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("symbolic const data expression canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected symbolic-expression buffers to sum to 70, got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_const_data_machine_call_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_CONST_DATA_MACHINE_CALL_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-const-data-machine-call-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("const-evaluated machine calls should specialize generic data");
    let executable = compilation
        .checked_native_executable_path()
        .expect("const-data machine-call canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("const data machine-call canary should run");
    assert_eq!(output.status.code(), Some(70));
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_const_data_where_fact_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_CONST_DATA_WHERE_FACT_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-const-data-where-fact-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("const-only generic facts should discharge at instantiation");
    let executable = compilation
        .checked_native_executable_path()
        .expect("const-data where-fact canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("const-fact generic canary should run");
    assert_eq!(output.status.code(), Some(70));
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn domain_body_case_membership_with_default_arm_compiles() {
    compile_canary_without_output(&pass_canary(
        fixture_roster::MATCH_DEFAULT_SATISFIES_EXHAUSTIVENESS,
    ))
    .expect("a domain-body fact may name an implicit case domain");
}

#[test]
fn runtime_const_data_machine_fact_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_CONST_DATA_MACHINE_FACT_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-const-data-machine-fact-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("machine-backed const domain facts should discharge");
    let executable = compilation
        .checked_native_executable_path()
        .expect("const-data machine-fact canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("machine-backed const domain fact canary should run");
    assert_eq!(output.status.code(), Some(70));
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_signed_const_data_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_SIGNED_CONST_DATA_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-signed-const-data-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("signed const data arguments should specialize");
    let executable = compilation
        .checked_native_executable_path()
        .expect("signed const-data canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("signed const data canary should run");
    assert_eq!(output.status.code(), Some(70));
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_trait_default_dispatch_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_TRAIT_DEFAULT_DISPATCH_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-trait-default-dispatch-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("trait defaults and written overrides should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("trait-default dispatch canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("trait default dispatch canary should run");
    assert_eq!(output.status.code(), Some(70));
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_inherited_trait_default_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_INHERITED_TRAIT_DEFAULT_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-inherited-trait-default-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("inherited trait defaults should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("inherited trait-default canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("inherited trait default canary should run");
    assert_eq!(output.status.code(), Some(70));
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_generic_trait_default_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_GENERIC_TRAIT_DEFAULT_EXIT);
    let build_dir = std::env::temp_dir().join(format!(
        "omega-generic-trait-default-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("generic trait defaults should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("generic trait-default canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("generic trait default canary should run");
    assert_eq!(output.status.code(), Some(70));
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_const_container_methods_exit_canary_runs() {
    let canary = pass_canary(fixture_roster::RUNTIME_CONST_CONTAINER_METHODS_EXIT);
    let build_dir =
        std::env::temp_dir().join(format!("omega-const-container-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("const-specialized container methods should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("const-container methods canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("const container method canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected const-specialized methods to sum to 70, got {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_generic_two_instantiations_exit_canary_runs() {
    // Phase 1: TWO distinct instantiations of `Box<T>` (`Box<i32>` + `Box<bool>`)
    // coexist in one program with native field access on both -- the
    // per-instance monomorphization (pre-resolution desugar to distinct concrete
    // records) that replaces the layout builder's one-slot poison. exit 30.
    let canary = pass_canary(fixture_roster::RUNTIME_GENERIC_TWO_INSTANTIATIONS_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-gen-two-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("two-instantiation generic canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("two-instantiation generic canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("two-instantiation generic canary should run");
    assert_eq!(
        output.status.code(),
        Some(30),
        "expected two coexisting generic instances with native access (exit 30), got {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_min_max_guard_subject_hoist_exit_canary_runs() {
    // #26: a pure builtin (`min`/`max`) used directly as a guard SUBJECT is
    // hoisted into a temp automatically, so the guard compares a materialized
    // local. Builtins are effect-free, so the effectful-single-eval constraint
    // that reverted the general value-call hoist is satisfied by construction.
    // Discriminating (min=7, max=8 both match -> good, exit 70; wrong builtin
    // or vacuous guard -> bad, exit 71).
    let canary = pass_canary(fixture_roster::RUNTIME_MIN_MAX_GUARD_SUBJECT_HOIST_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-minguard-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("min/max guard-subject hoist canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("min/max guard-subject canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("min/max guard-subject hoist canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a hoisted pure-builtin guard subject to discriminate (exit 70), got {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_indexed_guard_true_false_pair_exit_canary_runs() {
    // #41 indexed half: `transition arr[i] > 5 { true -> false -> }` -- the
    // natural array-element branch. hoist_comparison_match_subject shares one
    // subject temp across arms (hoisting the read inside it), so the pair pairs
    // for exhaustiveness. Discriminating: arr[1]=20 > 5 true -> ok (70).
    let canary = pass_canary(fixture_roster::RUNTIME_INDEXED_GUARD_TRUE_FALSE_PAIR_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-idxpair-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("indexed guard true/false pair canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("indexed guard-pair canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("indexed guard true/false pair canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a shared-subject indexed guard pair to discriminate (exit 70), got {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_indexed_field_local_operand_exit_canary_runs() {
    // A local from a field read off a runtime-indexed element (`let a =
    // self.ps[self.i].x`) used as an arithmetic operand was rejected -- the local
    // alias-folded back to `arr[i].field`, which has no operand lowering. It now
    // keeps its slot (local_data_requires_storage recognizes a Member-off-a-
    // runtime-index initializer). Discriminating: ps[1].x=20 + ps[0].x=10 + 12 =
    // 42 -> 70; a dropped operand (read 0) would mismatch.
    let canary = pass_canary(fixture_roster::RUNTIME_INDEXED_FIELD_LOCAL_OPERAND_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-idxfield-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("indexed-field-local-operand canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("indexed-field operand canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("indexed-field-local-operand canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected an indexed-field local used as an operand to keep its slot (exit 70), got {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_indexed_local_bitwise_exit_canary_runs() {
    // Silent miscompile fixed (sibling of the compare case): `let t = arr[i]; let
    // m = t & 6` read m as 0 -- a bitwise operand didn't force the indexed-read
    // local's slot, so it alias-folded and dropped. is_bitwise_operator now counts
    // it. Discriminating: (20&6)+(20|1)+(20^4)+29 = 4+21+16+29 = 70; the miscompile
    // (operands read 0) would give 29 -> 71.
    let canary = pass_canary(fixture_roster::RUNTIME_INDEXED_LOCAL_BITWISE_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-idxbit-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("indexed-local-bitwise canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("indexed-local bitwise canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("indexed-local-bitwise canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected an indexed-read local used as a bitwise operand to read its slot (exit 70), got {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_indexed_local_compare_exit_canary_runs() {
    // Silent miscompile fixed: `let hi = arr[i]; let over: bool = hi > 5` read
    // `over` as a folded default (always false) -- the alias-fold substituted the
    // indexed-read local into the fenced `arr[i] > 5` form and silently produced
    // false. A comparison operand now keeps its slot (local_data_requires_storage
    // counts comparison operators), so the compare reads the slot. Discriminating
    // (over=true vs low_over=false -> exit 70; the miscompile made over=false -> 71).
    let canary = pass_canary(fixture_roster::RUNTIME_INDEXED_LOCAL_COMPARE_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-idxcmp-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("indexed-local-compare canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("indexed-local compare canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("indexed-local-compare canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected an indexed-read local used as a compare operand to read its slot (exit 70), got {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_min_guard_true_false_pair_exit_canary_runs() {
    // #41: a pure-builtin guard subject with a `{ true -> false -> }` PAIR. Each
    // arm re-lowers the subject to its own temp, so the pair stopped pairing for
    // exhaustiveness; hoist_comparison_match_subject shares one subject temp
    // across arms (keyed on the syntax subject handle). Discriminating: min=7
    // matches -> good (70); a wrong min or a failed pair would exit 71.
    let canary = pass_canary(fixture_roster::RUNTIME_MIN_GUARD_TRUE_FALSE_PAIR_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-minpair-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("min guard true/false pair canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("min guard-pair canary should retain its executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("min guard true/false pair canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "expected a shared-subject builtin guard pair to discriminate (exit 70), got {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}

#[test]
fn runtime_nested_generic_instantiations_exit_canary_runs() {
    // Phase 3: NESTED generic data. Pair<T> contains a Box<T> field, so Pair<i32>
    // needs Box<i32> synthesized too (and Pair<bool> -> Box<bool>). The desugar
    // runs to a fixpoint: synthesizing Pair<i32> emits a fresh Box<i32> spelling
    // the next round monomorphizes; generic template bodies are skipped so the
    // param-arg Box<T> is never mistaken for a concrete instance. exit 30.
    let canary = pass_canary(fixture_roster::RUNTIME_NESTED_GENERIC_INSTANTIATIONS_EXIT);
    let build_dir = std::env::temp_dir().join(format!("omega-gennest-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    let compilation = compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("nested generic instantiations canary should compile");
    let executable = compilation
        .checked_native_executable_path()
        .expect("nested generic instances should retain their executable receipt");
    let output = Command::new(executable)
        .output()
        .expect("nested generic instantiations canary should run");
    assert_eq!(
        output.status.code(),
        Some(30),
        "expected nested generic instances (Pair<i32>/Pair<bool> over Box<T>) to coexist (exit 30), got {:?}: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);
}
