use super::*;

fn optional_intrinsic_diagnostic_label(
    checked: &omega_compiler::CheckedCompilation,
    plan: &omega_effects::provider_plan::ProviderPlan,
) -> Option<String> {
    let mut operators = checked.typed.operators().iter().filter(|operator| {
        psi_typed_trees::operator::boundary_operator_requirement_identity(&checked.typed, operator)
            == plan.schema.trait_name
    });
    let operator = operators.next()?;
    assert!(
        operators.next().is_none(),
        "selected intrinsic plan must resolve one exact boundary operator"
    );
    omega_compiler::compiler_intrinsic_diagnostic_label(&checked.typed, operator)
}

fn selected_intrinsic_diagnostic_label(
    checked: &omega_compiler::CheckedCompilation,
    plan: &omega_effects::provider_plan::ProviderPlan,
) -> String {
    optional_intrinsic_diagnostic_label(checked, plan)
        .expect("selected float intrinsic must have a structured diagnostic label")
}

// Binding-site operator selection (chapter 8): the signature `requires`
// statically selects the domain-owned `+` meaning, and checked evidence records
// that choice without consulting flow facts.
#[test]
fn domain_operator_selection_records_signature_domain_meaning_as_evidence() {
    let canary = pass_canary("domains/domain_operator_proven_fact_selects_meaning");
    let checked = omega_compiler::compile_to_checked(&canary.join("main.omg"), None)
        .expect("signature-selected domain canary should compile to checked trees");

    let selected_domain_uses = checked
        .facts
        .operators
        .resolved_uses()
        .filter(|operator_use| operator_use.spelling == psi_language_core::OperatorSpelling::Add)
        .filter_map(|operator_use| checked.facts.operators.selected_candidate(operator_use))
        .filter(|candidate| candidate.is_domain_owned())
        .count();
    assert!(
        selected_domain_uses > 0,
        "expected the signature's `Quantity::Additive` selection to choose the domain-owned `+` meaning \
         and record it in the operator evidence"
    );
}

#[test]
fn float_operator_spellings_record_named_core_identities() {
    let canary = pass_canary("operators/float_operator_identities");
    let checked = omega_compiler::compile_to_checked(&canary.join("main.omg"), None)
        .expect("core float operation identities should compile");

    let selected_names: Vec<String> = checked
        .facts
        .operators
        .resolved_uses()
        .filter_map(|operator_use| checked.facts.operators.selected_candidate(operator_use))
        .filter_map(|candidate| {
            checked
                .typed
                .operators()
                .iter()
                .find(|operator| operator.symbol == candidate.operator_symbol)
        })
        .map(|operator| {
            checked
                .typed
                .operator_path_members(operator.name)
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::")
        })
        .collect();

    for required in [
        "Float::add",
        "Float::divide",
        "Float::greater",
        "Float::equal",
    ] {
        assert!(
            selected_names.iter().any(|name| name == required),
            "expected primitive float spelling to select `{required}`, got {selected_names:?}"
        );
    }

    let operator_name = |operator: &psi_typed_trees::operator::OperatorDefinition| {
        checked
            .typed
            .operator_path_members(operator.name)
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>()
            .join("::")
    };
    for required in [
        "F64::fused_multiply_add",
        "F32::classify",
        "F64::add_toward_positive",
    ] {
        let operator = checked
            .typed
            .operators()
            .iter()
            .find(|operator| operator_name(operator) == required)
            .unwrap_or_else(|| panic!("missing source-visible float requirement `{required}`"));
        assert!(
            operator.is_boundary,
            "`{required}` is a primitive carrier boundary requirement"
        );
        assert!(
            !checked.typed.operator_contracts(operator).is_empty(),
            "`{required}` must publish equality against FloatSemantics"
        );
    }
    let semantic_fma = checked
        .typed
        .operators()
        .iter()
        .find(|operator| operator_name(operator) == "FloatSemantics::fused_multiply_add")
        .expect("source-visible executable FMA semantic identity");
    assert!(
        !semantic_fma.is_boundary,
        "FloatSemantics is pure core computation, not a target boundary"
    );
}

#[test]
fn float_provider_plan_identities_ignore_arena_and_display_perturbations() {
    fn float_plan_snapshot(checked: &omega_compiler::CheckedCompilation) -> Vec<(String, u64)> {
        checked
            .selected_provider_plans()
            .plans()
            .iter()
            .filter(|plan| plan.name.starts_with("FloatNativeProvider::satisfies::"))
            .map(|plan| (plan.name.clone(), plan.identity_fingerprint()))
            .collect()
    }

    let canary = pass_canary("operators/float_operator_identities");
    let baseline = omega_compiler::compile_to_checked(&canary.join("main.omg"), None)
        .expect("baseline float provider plans should check");
    let baseline_snapshot = float_plan_snapshot(&baseline);
    assert!(
        !baseline_snapshot.is_empty(),
        "float identity invariant requires selected native provider plans"
    );
    for plan in baseline
        .selected_provider_plans()
        .plans()
        .iter()
        .filter(|plan| plan.name.starts_with("FloatNativeProvider::satisfies::"))
    {
        let mut renamed_display = plan.clone();
        renamed_display.origin_package = "non-semantic display perturbation".to_owned();
        assert_eq!(
            renamed_display.identity_fingerprint(),
            plan.identity_fingerprint(),
            "readable origin labels must not enter exact float plan identity"
        );
    }

    let scratch = std::env::temp_dir().join(format!(
        "omega-float-plan-identity-perturbation-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    fs::create_dir_all(&scratch).expect("float plan identity scratch directory");
    let source = fs::read_to_string(canary.join("main.omg")).expect("read float plan canary");
    let source = source.replacen(
        "use omega::language::core::float_operations;",
        "use omega::language::core::option;\nuse omega::language::core::float_operations;",
        1,
    );
    let source = source.replacen(
        "data Main {",
        "data ArenaPadding {\n    value: i32;\n}\n\ndata Main {",
        1,
    );
    fs::write(scratch.join("main.omg"), source).expect("write perturbed float plan source");
    fs::copy(canary.join("build.omg"), scratch.join("build.omg"))
        .expect("copy float plan build configuration");

    let perturbed = omega_compiler::compile_to_checked(&scratch.join("main.omg"), None)
        .expect("arena-perturbed float provider plans should check");
    assert_eq!(
        float_plan_snapshot(&perturbed),
        baseline_snapshot,
        "unrelated imported and local declarations must not leak arena coordinates into float plan identity"
    );
    let _ = fs::remove_dir_all(&scratch);
}

#[test]
fn migrated_float_provider_plans_are_selected_for_every_native_target() {
    const MIGRATED_REQUIREMENTS: &[&str] = &[
        "Float::add",
        "Float::subtract",
        "Float::multiply",
        "Float::divide",
        "Float::equal",
        "Float::not_equal",
        "Float::less",
        "Float::less_or_equal",
        "Float::greater",
        "Float::greater_or_equal",
        "F32::minimum",
        "F64::minimum",
        "F32::maximum",
        "F64::maximum",
        "F32::negate",
        "F64::negate",
        "F32::square_root",
        "F64::square_root",
        "F32::is_nan",
        "F64::is_nan",
        "F32::is_finite",
        "F64::is_finite",
        "F32::is_infinite",
        "F64::is_infinite",
        "F32::is_normal",
        "F64::is_normal",
        "F32::is_subnormal",
        "F64::is_subnormal",
        "F32::classify",
        "F64::classify",
        "F32::multiply_then_add",
        "F64::multiply_then_add",
        "F32::fused_multiply_add",
        "F64::fused_multiply_add",
        "F32::fused_multiply_add_toward_zero",
        "F64::fused_multiply_add_toward_zero",
        "F32::fused_multiply_add_toward_positive",
        "F64::fused_multiply_add_toward_positive",
        "F32::fused_multiply_add_toward_negative",
        "F64::fused_multiply_add_toward_negative",
        "F32::add_toward_zero",
        "F64::add_toward_zero",
        "F32::add_toward_positive",
        "F64::add_toward_positive",
        "F32::add_toward_negative",
        "F64::add_toward_negative",
        "F32::subtract_toward_zero",
        "F64::subtract_toward_zero",
        "F32::subtract_toward_positive",
        "F64::subtract_toward_positive",
        "F32::subtract_toward_negative",
        "F64::subtract_toward_negative",
        "F32::multiply_toward_zero",
        "F64::multiply_toward_zero",
        "F32::multiply_toward_positive",
        "F64::multiply_toward_positive",
        "F32::multiply_toward_negative",
        "F64::multiply_toward_negative",
        "F32::divide_toward_zero",
        "F64::divide_toward_zero",
        "F32::divide_toward_positive",
        "F64::divide_toward_positive",
        "F32::divide_toward_negative",
        "F64::divide_toward_negative",
        "F32::square_root_toward_zero",
        "F64::square_root_toward_zero",
        "F32::square_root_toward_positive",
        "F64::square_root_toward_positive",
        "F32::square_root_toward_negative",
        "F64::square_root_toward_negative",
        "F32::from_f64",
        "F64::from_f32",
        "F32::from_i8",
        "F32::from_i16",
        "F32::from_i32",
        "F32::from_i64",
        "F32::from_u8",
        "F32::from_u16",
        "F32::from_u32",
        "F32::from_u64",
        "F64::from_i8",
        "F64::from_i16",
        "F64::from_i32",
        "F64::from_i64",
        "F64::from_u8",
        "F64::from_u16",
        "F64::from_u32",
        "F64::from_u64",
        "I8::from_f32",
        "I8::from_f64",
        "I16::from_f32",
        "I16::from_f64",
        "I32::from_f32",
        "I32::from_f64",
        "I64::from_f32",
        "I64::from_f64",
        "U8::from_f32",
        "U8::from_f64",
        "U16::from_f32",
        "U16::from_f64",
        "U32::from_f32",
        "U32::from_f64",
        "U64::from_f32",
        "U64::from_f64",
    ];
    let canary = pass_canary("operators/float_operator_identities");
    for target in ["windows_x64", "linux_x64", "linux_arm64", "macos_arm64"] {
        let checked = omega_compiler::compile_to_checked(&canary.join("main.omg"), Some(target))
            .unwrap_or_else(|diagnostics| {
                panic!("core float provider plans should check for {target}: {diagnostics:#?}")
            });
        let operator_path = |operator: &psi_typed_trees::operator::OperatorDefinition| {
            checked
                .typed
                .operator_path_members(operator.name)
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::")
        };
        let expected_intrinsic = |operator: &psi_typed_trees::operator::OperatorDefinition| {
            let path = operator_path(operator);
            if !MIGRATED_REQUIREMENTS.contains(&path.as_str()) {
                return None;
            }
            if path.contains("::fused_multiply_add")
                && !matches!(target, "linux_arm64" | "macos_arm64")
            {
                // Generic x86-64 remains SSE2-baseline. Its FMA requirement
                // must wait for a feature-qualified or checked software plan.
                return None;
            }
            omega_compiler::compiler_intrinsic_diagnostic_label(&checked.typed, operator)
        };
        let mut used_intrinsics = std::collections::BTreeSet::new();

        for operator_use in checked.facts.operators.resolved_uses() {
            let Some(candidate) = checked.facts.operators.selected_candidate(operator_use) else {
                continue;
            };
            let Some(operator) = checked
                .typed
                .operators()
                .iter()
                .find(|operator| operator.symbol == candidate.operator_symbol)
            else {
                continue;
            };
            let Some(expected_intrinsic) = expected_intrinsic(operator) else {
                continue;
            };
            assert_ne!(
                operator_use.provider_plan_identity, 0,
                "{target} {expected_intrinsic} use must retain the selected ProviderPlan identity"
            );
            let plan = checked
                .selected_provider_plans()
                .plan_by_identity(operator_use.provider_plan_identity)
                .expect("operator evidence must resolve to one retained selected plan");
            assert_eq!(plan.target, target);
            assert_eq!(
                plan.schema.trait_name,
                psi_typed_trees::operator::boundary_operator_requirement_identity(
                    &checked.typed,
                    operator,
                )
            );
            let [row] = plan.rows.as_slice() else {
                panic!("exact operator plan must retain one realization row");
            };
            assert_eq!(row.method, "realize");
            assert!(
                matches!(
                    &row.binding,
                    omega_effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. }
                ),
                "{target} selected the wrong {expected_intrinsic} realization: {row:?}"
            );
            used_intrinsics.insert(expected_intrinsic);
        }

        assert_eq!(
            used_intrinsics,
            [
                "Float::add.f32".to_owned(),
                "Float::add.f64".to_owned(),
                "Float::divide.f32".to_owned(),
                "Float::divide.f64".to_owned(),
                "Float::equal.f32".to_owned(),
                "Float::equal.f64".to_owned(),
                "Float::greater.f32".to_owned(),
                "Float::greater.f64".to_owned(),
                "Float::greater_or_equal.f32".to_owned(),
                "Float::greater_or_equal.f64".to_owned(),
                "Float::less.f32".to_owned(),
                "Float::less.f64".to_owned(),
                "Float::less_or_equal.f32".to_owned(),
                "Float::less_or_equal.f64".to_owned(),
                "Float::multiply.f32".to_owned(),
                "Float::multiply.f64".to_owned(),
                "Float::not_equal.f32".to_owned(),
                "Float::not_equal.f64".to_owned(),
                "Float::subtract.f32".to_owned(),
                "Float::subtract.f64".to_owned(),
            ]
            .into_iter()
            .collect(),
            "every primitive float operation used by the canary must consume its target plan"
        );

        let mut selected_count = 0usize;
        for operator in checked.typed.operators() {
            let Some(expected_intrinsic) = expected_intrinsic(operator) else {
                continue;
            };
            selected_count += 1;
            let slot = psi_typed_trees::operator::boundary_operator_requirement_identity(
                &checked.typed,
                operator,
            );
            let plan = checked
                .selected_provider_plans()
                .plans()
                .iter()
                .find(|plan| plan.target == target && plan.schema.trait_name == slot)
                .unwrap_or_else(|| {
                    panic!("{target} must select exact plan slot {slot} for {expected_intrinsic}")
                });
            assert!(
                matches!(
                    plan.rows.as_slice(),
                    [row]
                        if row.method == "realize"
                            && matches!(
                                &row.binding,
                                omega_effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. }
                            )
                ),
                "{target} selected the wrong realization for {expected_intrinsic}: {plan:?}"
            );
        }
        assert_eq!(
            selected_count,
            if matches!(target, "linux_arm64" | "macos_arm64") {
                146
            } else {
                138
            },
            "all migrated primitive and target-valid named-operation slots must select"
        );
        let selected_fma_plans = checked
            .selected_provider_plans()
            .plans()
            .iter()
            .filter(|plan| {
                plan.target == target
                    && plan.rows.iter().any(|row| {
                        matches!(
                            &row.binding,
                            omega_effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. }
                        ) && optional_intrinsic_diagnostic_label(&checked, plan)
                            .is_some_and(|label| label.contains("::fused_multiply_add"))
                    })
            })
            .count();
        assert_eq!(
            selected_fma_plans,
            if matches!(target, "linux_arm64" | "macos_arm64") {
                8
            } else {
                0
            },
            "only baseline-FMADD AArch64 targets may select nearest or directed FMA slots"
        );
    }
}

#[test]
fn primitive_float_arithmetic_and_comparisons_execute_in_both_engines() {
    const DIFFERENTIAL_SUITE_ID: &str =
        "omega.float.hardware.macos_arm64.primitive-arithmetic-comparison.v1";
    const DIFFERENTIAL_COVERAGE: &[&str] = &[
        "binary32 finite add/subtract/multiply/divide",
        "binary64 finite add/subtract/multiply/divide",
        "binary32 equality and ordered comparisons",
        "binary64 equality and ordered comparisons",
    ];
    const EXPECTED_DIFFERENTIAL_RESULT_IDENTITY: u64 = 0x42ad_e03a_f099_ff9f;
    const PRIMITIVE_REQUIREMENTS: &[&str] = &[
        "Float::add",
        "Float::subtract",
        "Float::multiply",
        "Float::divide",
        "Float::equal",
        "Float::not_equal",
        "Float::less",
        "Float::less_or_equal",
        "Float::greater",
        "Float::greater_or_equal",
    ];

    let canary = pass_canary("operators/float_operator_identities");
    let main_path = canary.join("main.omg");
    let checked = omega_compiler::compile_to_checked(&main_path, None)
        .expect("primitive float arithmetic and comparisons should compile");
    let operator_path = |operator: &psi_typed_trees::operator::OperatorDefinition| {
        checked
            .typed
            .operator_path_members(operator.name)
            .iter()
            .map(|member| member.as_str())
            .collect::<Vec<_>>()
            .join("::")
    };
    let mut selected_intrinsics = std::collections::BTreeSet::new();
    let mut selected_plan_identities = Vec::new();
    for operator_use in checked.facts.operators.resolved_uses() {
        let Some(candidate) = checked.facts.operators.selected_candidate(operator_use) else {
            continue;
        };
        let Some(operator) = checked
            .typed
            .operators()
            .iter()
            .find(|operator| operator.symbol == candidate.operator_symbol)
        else {
            continue;
        };
        if !PRIMITIVE_REQUIREMENTS.contains(&operator_path(operator).as_str()) {
            continue;
        }
        let plan = checked
            .selected_provider_plans()
            .plan_by_identity(operator_use.provider_plan_identity)
            .expect("primitive float evidence must retain its selected plan");
        let [row] = plan.rows.as_slice() else {
            panic!("primitive float plan must retain one exact realization row");
        };
        let omega_effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. } = &row.binding
        else {
            panic!("primitive float plan must select a compiler intrinsic");
        };
        selected_intrinsics.insert(selected_intrinsic_diagnostic_label(&checked, plan));
        selected_plan_identities.push(plan.identity_fingerprint());
    }
    let expected_intrinsics = [
        "Float::add.f32",
        "Float::add.f64",
        "Float::divide.f32",
        "Float::divide.f64",
        "Float::equal.f32",
        "Float::equal.f64",
        "Float::greater.f32",
        "Float::greater.f64",
        "Float::greater_or_equal.f32",
        "Float::greater_or_equal.f64",
        "Float::less.f32",
        "Float::less.f64",
        "Float::less_or_equal.f32",
        "Float::less_or_equal.f64",
        "Float::multiply.f32",
        "Float::multiply.f64",
        "Float::not_equal.f32",
        "Float::not_equal.f64",
        "Float::subtract.f32",
        "Float::subtract.f64",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(selected_intrinsics, expected_intrinsics);
    selected_plan_identities.sort_unstable();
    selected_plan_identities.dedup();
    assert_eq!(
        selected_plan_identities.len(),
        20,
        "{DIFFERENTIAL_SUITE_ID} must bind one exact plan per operation and format"
    );

    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter must execute the complete primitive float matrix; error: {:?}",
        outcome.error
    );

    let build_dir = std::env::temp_dir().join(format!(
        "omega-primitive-float-arithmetic-comparison-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: main_path,
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("primitive float arithmetic and comparisons should compile natively");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("primitive float arithmetic and comparison canary should run");
    assert_eq!(output.status.code(), Some(70));
    let _ = fs::remove_dir_all(&build_dir);

    for target in ["linux_x64", "linux_arm64"] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-primitive-float-arithmetic-comparison-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source_dir = scratch.join("src");
        fs::create_dir_all(&source_dir).expect("primitive float cross-target source directory");
        fs::copy(canary.join("main.omg"), source_dir.join("main.omg"))
            .expect("copy primitive float canary");
        fs::write(
            source_dir.join("build.omg"),
            hosted_main_program_entry_build(target),
        )
        .expect("write primitive float target manifest");
        compile(CompileOptions {
            root_path: source_dir.join("main.omg"),
            build_dir: Some(scratch.join("out")),
            target_name: Some(target.to_owned()),
            write_output: true,
        })
        .unwrap_or_else(|diagnostics| {
            panic!("primitive float operations should compile for {target}: {diagnostics:#?}")
        });
        let _ = fs::remove_dir_all(&scratch);
    }

    let result_identity = retained_float_differential_result_identity(
        DIFFERENTIAL_SUITE_ID,
        "macos_arm64",
        DIFFERENTIAL_COVERAGE,
        &selected_intrinsics,
        &selected_plan_identities,
        &outcome,
        &output,
        &["linux_x64", "linux_arm64"],
    );
    assert_eq!(
        result_identity, EXPECTED_DIFFERENTIAL_RESULT_IDENTITY,
        "{DIFFERENTIAL_SUITE_ID} result changed ({result_identity:#018x}); validate the exact plans, edge corpus, interpreter/native results, and cross-target builds before refreshing the retained identity"
    );
}

#[test]
fn named_float_format_conversion_requirements_execute_in_both_engines() {
    const DIFFERENTIAL_SUITE_ID: &str = "omega.float.hardware.macos_arm64.format-conversion.v1";
    const DIFFERENTIAL_COVERAGE: &[&str] = &[
        "binary64-to-binary32 halfway tie-to-even",
        "binary64-to-binary32 just-above-halfway",
        "binary32-to-binary64 exact widening",
        "binary64 infinity to binary32 infinity",
    ];
    const EXPECTED_DIFFERENTIAL_RESULT_IDENTITY: u64 = 0xd1dd_0dcd_c054_6c30;

    let canary = pass_canary("float/runtime_named_format_conversion_exit");
    let main_path = canary.join("main.omg");
    let checked = omega_compiler::compile_to_checked(&main_path, None)
        .expect("public float-format conversion requirements should compile");

    let selected = checked
        .facts
        .operators
        .named_uses()
        .filter_map(|operator_use| {
            let plan = checked
                .selected_provider_plans()
                .plan_by_identity(operator_use.provider_plan_identity)?;
            let [row] = plan.rows.as_slice() else {
                return None;
            };
            let omega_effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. } =
                &row.binding
            else {
                return None;
            };
            let name = selected_intrinsic_diagnostic_label(&checked, plan);
            name.contains("::from_f").then_some((operator_use, name))
        })
        .collect::<Vec<_>>();
    assert_eq!(selected.len(), 5, "all five conversion calls retain a plan");
    let mut selected_intrinsics = std::collections::BTreeSet::new();
    let mut selected_plan_identities = Vec::new();
    for (operator_use, intrinsic) in selected {
        selected_intrinsics.insert(intrinsic.clone());
        selected_plan_identities.push(
            checked
                .selected_provider_plans()
                .plan_by_identity(operator_use.provider_plan_identity)
                .expect("format conversion evidence must retain its selected plan")
                .identity_fingerprint(),
        );
        let psi_typed_trees::expression::ExpressionNode::Cast(cast) = checked
            .typed
            .expression_table
            .expression(operator_use.expression)
        else {
            panic!("selected conversion `{intrinsic}` must rewrite to one typed cast");
        };
        let expected = if intrinsic == "F32::from_f64.f64" {
            psi_typed_trees::types::PrimitiveType::F32
        } else if intrinsic == "F64::from_f32.f32" {
            psi_typed_trees::types::PrimitiveType::F64
        } else {
            panic!("unexpected conversion intrinsic `{intrinsic}`");
        };
        assert_eq!(
            checked.typed.primitive_type_reference(cast.target_type),
            Some(expected)
        );
    }
    assert_eq!(
        selected_intrinsics,
        [
            "F32::from_f64.f64".to_owned(),
            "F64::from_f32.f32".to_owned(),
        ]
        .into_iter()
        .collect()
    );
    selected_plan_identities.sort_unstable();
    selected_plan_identities.dedup();
    assert_eq!(
        selected_plan_identities.len(),
        2,
        "{DIFFERENTIAL_SUITE_ID} must bind one exact plan per direction"
    );

    let classification_plan_evidence = checked
        .facts
        .operators
        .named_uses()
        .filter_map(|operator_use| {
            let plan = checked
                .selected_provider_plans()
                .plan_by_identity(operator_use.provider_plan_identity)?;
            let [row] = plan.rows.as_slice() else {
                return None;
            };
            let omega_effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. } =
                &row.binding
            else {
                return None;
            };
            let name = selected_intrinsic_diagnostic_label(&checked, plan);
            (name == "F32::is_infinite.f32")
                .then_some((operator_use.expression, operator_use.provider_plan_identity))
        })
        .collect::<Vec<_>>();
    let classification_plan_evidence =
        classification_plan_evidence
            .into_iter()
            .fold(Vec::new(), |mut unique, evidence| {
                if !unique.contains(&evidence) {
                    unique.push(evidence);
                }
                unique
            });
    assert_eq!(
        classification_plan_evidence.len(),
        2,
        "both repeated classification calls must retain checked plan evidence"
    );
    assert!(
        classification_plan_evidence[0].1 != 0
            && classification_plan_evidence[0].1 == classification_plan_evidence[1].1,
        "repeated classification calls must agree on one exact nonzero plan"
    );

    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter must execute nearest-even format conversion; error: {:?}",
        outcome.error
    );

    let build_dir = std::env::temp_dir().join(format!(
        "omega-named-format-conversion-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("public float-format conversions should compile from their authored native root");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("public float-format conversion canary should run");
    assert_eq!(output.status.code(), Some(70));
    let _ = fs::remove_dir_all(&build_dir);

    for target in ["linux_x64", "linux_arm64"] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-named-format-conversion-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        compile_rooted_canary_for_target(&canary, scratch.join("out"), target).unwrap_or_else(
            |diagnostics| {
                panic!("format conversions should compile for {target}: {diagnostics:#?}")
            },
        );
        let _ = fs::remove_dir_all(&scratch);
    }

    let result_identity = retained_float_differential_result_identity(
        DIFFERENTIAL_SUITE_ID,
        "macos_arm64",
        DIFFERENTIAL_COVERAGE,
        &selected_intrinsics,
        &selected_plan_identities,
        &outcome,
        &output,
        &["linux_x64", "linux_arm64"],
    );
    assert_eq!(
        result_identity, EXPECTED_DIFFERENTIAL_RESULT_IDENTITY,
        "{DIFFERENTIAL_SUITE_ID} result changed ({result_identity:#018x}); validate the exact plans, edge corpus, interpreter/native results, and cross-target builds before refreshing the retained identity"
    );
}

#[test]
fn named_integer_to_float_requirements_execute_in_both_engines() {
    const DIFFERENTIAL_SUITE_ID: &str = "omega.float.hardware.macos_arm64.integer-to-float.v1";
    const DIFFERENTIAL_COVERAGE: &[&str] = &[
        "narrow signed source extension",
        "narrow unsigned source extension",
        "signed binary32 precision-boundary tie",
        "signed binary64 precision-boundary tie",
        "maximum unsigned64 to binary32",
        "maximum unsigned64 to binary64",
    ];
    const EXPECTED_DIFFERENTIAL_RESULT_IDENTITY: u64 = 0x141a_7a9a_5d2a_1ceb;

    let canary = pass_canary("float/runtime_named_integer_to_float_conversion_exit");
    let main_path = canary.join("main.omg");
    let checked = omega_compiler::compile_to_checked(&main_path, None)
        .expect("public integer-to-float requirements should compile");

    let selected = checked
        .facts
        .operators
        .named_uses()
        .filter_map(|operator_use| {
            let plan = checked
                .selected_provider_plans()
                .plan_by_identity(operator_use.provider_plan_identity)?;
            let [row] = plan.rows.as_slice() else {
                return None;
            };
            let omega_effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. } =
                &row.binding
            else {
                return None;
            };
            let name = selected_intrinsic_diagnostic_label(&checked, plan);
            name.contains("::from_i")
                .then_some((plan.identity_fingerprint(), name.clone()))
                .or_else(|| {
                    name.contains("::from_u")
                        .then_some((plan.identity_fingerprint(), name.clone()))
                })
        })
        .collect::<Vec<_>>();
    let selected_intrinsics = selected
        .iter()
        .map(|(_, intrinsic)| intrinsic.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let mut selected_plan_identities = selected
        .iter()
        .map(|(identity, _)| *identity)
        .collect::<Vec<_>>();
    let expected_intrinsics = [
        "F32::from_i8.i8",
        "F32::from_i16.i16",
        "F32::from_i32.i32",
        "F32::from_i64.i64",
        "F32::from_u8.u8",
        "F32::from_u16.u16",
        "F32::from_u32.u32",
        "F32::from_u64.u64",
        "F64::from_i8.i8",
        "F64::from_i16.i16",
        "F64::from_i32.i32",
        "F64::from_i64.i64",
        "F64::from_u8.u8",
        "F64::from_u16.u16",
        "F64::from_u32.u32",
        "F64::from_u64.u64",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(selected_intrinsics, expected_intrinsics);
    selected_plan_identities.sort_unstable();
    selected_plan_identities.dedup();
    assert_eq!(
        selected_plan_identities.len(),
        16,
        "{DIFFERENTIAL_SUITE_ID} must bind one exact plan per source/destination slot"
    );

    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter must execute the complete integer-to-float matrix; error: {:?}",
        outcome.error
    );

    let build_dir = std::env::temp_dir().join(format!(
        "omega-named-integer-to-float-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "public integer-to-float requirements should compile from their authored native root",
    );
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("public integer-to-float canary should run");
    assert_eq!(output.status.code(), Some(70));
    let _ = fs::remove_dir_all(&build_dir);

    for target in ["linux_x64", "linux_arm64"] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-named-integer-to-float-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        compile_rooted_canary_for_target(&canary, scratch.join("out"), target).unwrap_or_else(
            |diagnostics| {
                panic!("integer-to-float conversions should compile for {target}: {diagnostics:#?}")
            },
        );
        let _ = fs::remove_dir_all(&scratch);
    }

    let result_identity = retained_float_differential_result_identity(
        DIFFERENTIAL_SUITE_ID,
        "macos_arm64",
        DIFFERENTIAL_COVERAGE,
        &selected_intrinsics,
        &selected_plan_identities,
        &outcome,
        &output,
        &["linux_x64", "linux_arm64"],
    );
    assert_eq!(
        result_identity, EXPECTED_DIFFERENTIAL_RESULT_IDENTITY,
        "{DIFFERENTIAL_SUITE_ID} result changed ({result_identity:#018x}); validate the exact plans, edge corpus, interpreter/native results, and cross-target builds before refreshing the retained identity"
    );
}

#[test]
fn named_float_to_integer_requirements_execute_in_both_engines() {
    const DIFFERENTIAL_SUITE_ID: &str = "omega.float.hardware.macos_arm64.float-to-integer.v1";
    const DIFFERENTIAL_COVERAGE: &[&str] = &[
        "binary32/binary64 to every signed width toward zero",
        "binary32/binary64 to every unsigned width toward zero",
        "in-range Trapping result dispatch",
        "signed upper-overflow saturation",
        "unsigned negative-input saturation",
        "NaN saturation to zero",
    ];
    const EXPECTED_DIFFERENTIAL_RESULT_IDENTITY: u64 = 0x9bed_09d4_a629_c573;

    let canary = pass_canary("float/runtime_named_float_to_integer_conversion_exit");
    let main_path = canary.join("main.omg");
    let checked = omega_compiler::compile_to_checked(&main_path, None)
        .expect("public float-to-integer requirements should compile");

    let selected = checked
        .facts
        .operators
        .named_uses()
        .filter_map(|operator_use| {
            let plan = checked
                .selected_provider_plans()
                .plan_by_identity(operator_use.provider_plan_identity)?;
            let [row] = plan.rows.as_slice() else {
                return None;
            };
            let omega_effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. } =
                &row.binding
            else {
                return None;
            };
            let name = selected_intrinsic_diagnostic_label(&checked, plan);
            [
                "I8::", "I16::", "I32::", "I64::", "U8::", "U16::", "U32::", "U64::",
            ]
            .iter()
            .any(|prefix| name.starts_with(prefix))
            .then_some((operator_use, name.clone()))
        })
        .collect::<Vec<_>>();
    let selected_intrinsics = selected
        .iter()
        .map(|(_, intrinsic)| intrinsic.clone())
        .collect::<std::collections::BTreeSet<_>>();
    let expected_intrinsics = [
        "I8::from_f32.f32.exact",
        "I8::from_f64.f64.exact",
        "I8::from_f64.f64.saturating",
        "I16::from_f32.f32.exact",
        "I16::from_f64.f64.exact",
        "I32::from_f32.f32.exact",
        "I32::from_f64.f64.exact",
        "I32::from_f64.f64.trapping",
        "I32::from_f64.f64.saturating",
        "I64::from_f32.f32.exact",
        "I64::from_f64.f64.exact",
        "U8::from_f32.f32.exact",
        "U8::from_f64.f64.exact",
        "U16::from_f32.f32.exact",
        "U16::from_f64.f64.exact",
        "U32::from_f32.f32.exact",
        "U32::from_f64.f64.exact",
        "U64::from_f32.f32.exact",
        "U64::from_f64.f64.exact",
        "U64::from_f64.f64.saturating",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(selected_intrinsics, expected_intrinsics);
    let mut selected_plan_identities = selected
        .iter()
        .map(|(operator_use, _)| {
            checked
                .selected_provider_plans()
                .plan_by_identity(operator_use.provider_plan_identity)
                .expect("float-to-integer evidence must retain its selected plan")
                .identity_fingerprint()
        })
        .collect::<Vec<_>>();
    selected_plan_identities.sort_unstable();
    selected_plan_identities.dedup();
    assert_eq!(
        selected_plan_identities.len(),
        20,
        "{DIFFERENTIAL_SUITE_ID} must bind one exact plan per source/destination/domain slot"
    );

    for (operator_use, intrinsic) in selected {
        let psi_typed_trees::expression::ExpressionNode::Cast(cast) = checked
            .typed
            .expression_table
            .expression(operator_use.expression)
        else {
            panic!("selected conversion `{intrinsic}` must rewrite to one typed cast");
        };
        let expected_domain = if intrinsic.ends_with(".trapping") {
            psi_numerics::arithmetic::ArithmeticDomain::Trapping
        } else if intrinsic.ends_with(".saturating") {
            psi_numerics::arithmetic::ArithmeticDomain::Saturating
        } else {
            psi_numerics::arithmetic::ArithmeticDomain::Exact
        };
        assert_eq!(
            cast.domain, expected_domain,
            "wrong rewrite for `{intrinsic}`"
        );
    }

    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "interpreter must execute the complete float-to-integer matrix; error: {:?}",
        outcome.error
    );

    let build_dir = std::env::temp_dir().join(format!(
        "omega-named-float-to-integer-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "public float-to-integer requirements should compile from their authored native root",
    );
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("public float-to-integer canary should run");
    assert_eq!(output.status.code(), Some(70));
    let _ = fs::remove_dir_all(&build_dir);

    for target in ["linux_x64", "linux_arm64"] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-named-float-to-integer-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        compile_rooted_canary_for_target(&canary, scratch.join("out"), target).unwrap_or_else(
            |diagnostics| {
                panic!("float-to-integer conversions should compile for {target}: {diagnostics:#?}")
            },
        );
        let _ = fs::remove_dir_all(&scratch);
    }

    let result_identity = retained_float_differential_result_identity(
        DIFFERENTIAL_SUITE_ID,
        "macos_arm64",
        DIFFERENTIAL_COVERAGE,
        &selected_intrinsics,
        &selected_plan_identities,
        &outcome,
        &output,
        &["linux_x64", "linux_arm64"],
    );
    assert_eq!(
        result_identity, EXPECTED_DIFFERENTIAL_RESULT_IDENTITY,
        "{DIFFERENTIAL_SUITE_ID} result changed ({result_identity:#018x}); validate the exact plans, edge corpus, interpreter/native results, and cross-target builds before refreshing the retained identity"
    );
}

#[test]
fn named_float_to_integer_rejections_are_explicit() {
    for (name, expected) in [
        (
            "float/named_float_to_integer_exact_unproven",
            "cannot prove unqualified `I32::from_f64` operand",
        ),
        (
            "float/named_float_to_integer_wrapping_rejected",
            "has no overload for result dispatch set `arithmetic:Wrapping`",
        ),
        (
            "float/named_float_to_integer_no_context_unproven",
            "cannot prove unqualified `I32::from_f64` operand",
        ),
        (
            "float/named_float_to_integer_implicit_discard_rejected",
            "discards its non-unit `i32` result",
        ),
        (
            "operators/named_operator_result_overload_duplicate_dispatch",
            "duplicate named requirement overload `Convert::value`",
        ),
    ] {
        let diagnostics = compile_canary_without_output(&fail_canary(name))
            .expect_err("invalid public float-to-integer call unexpectedly compiled");
        let rendered = diagnostics
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            rendered.contains(expected),
            "{name} should report `{expected}`, got:\n{rendered}"
        );
    }
}

#[test]
fn named_float_to_integer_trapping_requirements_trap_in_both_engines() {
    for name in [
        "float/runtime_named_float_to_integer_trapping_nan_traps",
        "float/runtime_named_float_to_integer_trapping_overflow_traps",
    ] {
        let canary = pass_canary(name);
        let main_path = canary.join("main.omg");
        let checked = omega_compiler::compile_to_checked(&main_path, None)
            .expect("named Trapping float-to-integer requirement should compile");
        let interpreted = interpret(&checked, &[]);
        assert!(
            interpreted.error.is_some(),
            "{name} reached its post-conversion sentinel in the interpreter"
        );

        let leaf = name.rsplit('/').next().unwrap_or("trap");
        let build_dir = std::env::temp_dir().join(format!(
            "omega-named-float-int-{leaf}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&build_dir);
        compile_rooted_canary_for_native_host(&canary, build_dir.clone())
            .expect("named Trapping float-to-integer requirement should compile natively");
        let output = Command::new(build_dir.join(executable_name()))
            .output()
            .expect("named Trapping float-to-integer canary should start");
        assert!(
            !output.status.success(),
            "{name} reached its post-conversion sentinel natively"
        );
        let _ = fs::remove_dir_all(&build_dir);
    }
}

#[test]
fn named_float_provider_calls_rewrite_to_selected_builtins() {
    #[derive(Debug, PartialEq, Eq)]
    struct FloatProviderContract {
        intrinsic: String,
        parameter_count: usize,
        parameter_type_identities: Vec<String>,
        has_result: bool,
        result_type_identity: Option<String>,
        service_reach: Vec<String>,
        synchronous_invocations: Vec<String>,
        may_suspend: bool,
        may_block: bool,
        terminates_guarantee: bool,
    }

    const DIFFERENTIAL_SUITE_ID: &str =
        "omega.float.hardware.macos_arm64.minimum-maximum-square-root.v1";
    const DIFFERENTIAL_COVERAGE: &[&str] = &[
        "binary32 NaN operand order",
        "binary64 NaN operand order",
        "binary32 minimum signed-zero choice",
        "binary64 maximum signed-zero choice",
        "binary32 exact square root",
        "binary64 exact square root",
    ];
    const EXPECTED_DIFFERENTIAL_RESULT_IDENTITY: u64 = 0x0b72_09a4_4518_814d;

    let canary = pass_canary("float/named_provider_min_max_sqrt_exit");
    let checked = omega_compiler::compile_to_checked(&canary.join("main.omg"), None)
        .expect("named float provider calls should compile to checked trees");
    let mut selected_intrinsics = std::collections::BTreeSet::new();
    let mut selected_plan_identities = Vec::new();
    let mut selected_contract_rows = std::collections::BTreeMap::new();
    for operator_use in checked.facts.operators.named_uses() {
        if operator_use.provider_plan_identity == 0 {
            continue;
        }
        let plan = checked
            .selected_provider_plans()
            .plan_by_identity(operator_use.provider_plan_identity)
            .expect("named operator evidence must resolve to its retained plan");
        let [row] = plan.rows.as_slice() else {
            panic!("named float plan must contain exactly one row");
        };
        let [method] = plan.schema.methods.as_slice() else {
            panic!("named float plan must contain exactly one service method");
        };
        assert_eq!(plan.schema.trait_name, method.requirement_identity);
        assert_eq!(method.requirement_owner, method.requirement_identity);
        assert_eq!(method.name, "realize");
        assert_eq!(row.method, method.name);
        assert_eq!(row.requirement_identity, method.requirement_identity);
        let omega_effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. } = &row.binding
        else {
            panic!("named float plan must select a compiler intrinsic");
        };
        let name = selected_intrinsic_diagnostic_label(&checked, plan);
        let contract = FloatProviderContract {
            intrinsic: name.clone(),
            parameter_count: method.parameter_count,
            parameter_type_identities: method.parameter_type_identities.clone(),
            has_result: method.has_result,
            result_type_identity: method.result_type_identity.clone(),
            service_reach: method.service_reach.clone(),
            synchronous_invocations: method.synchronous_invocations.clone(),
            may_suspend: method.may_suspend,
            may_block: method.may_block,
            terminates_guarantee: method.terminates_guarantee,
        };
        match selected_contract_rows.entry(method.requirement_identity.clone()) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(contract);
            }
            std::collections::btree_map::Entry::Occupied(entry) => {
                assert_eq!(entry.get(), &contract, "selected provider contract drifted");
            }
        }
        selected_intrinsics.insert(name.clone());
        selected_plan_identities.push(plan.identity_fingerprint());

        let psi_typed_trees::expression::ExpressionNode::Call(call) = checked
            .typed
            .expression_table
            .expression(operator_use.expression)
        else {
            panic!("named operator use must remain a call expression");
        };
        assert!(
            !call.receiver.is_valid(),
            "builtin dispatch removes F32/F64"
        );
        let expected_builtin = if name.contains("::minimum.") {
            "min"
        } else if name.contains("::maximum.") {
            "max"
        } else if name.contains("::square_root.") {
            "sqrt"
        } else {
            panic!("unexpected migrated named intrinsic `{name}`");
        };
        assert_eq!(call.target.as_str(), expected_builtin);
        assert_eq!(
            Some(call.target_symbol),
            checked
                .typed
                .symbols
                .builtin_function_symbol(match expected_builtin {
                    "min" => psi_symbols::BuiltinFunction::Min,
                    "max" => psi_symbols::BuiltinFunction::Max,
                    "sqrt" => psi_symbols::BuiltinFunction::Sqrt,
                    _ => unreachable!(),
                })
        );
    }

    assert_eq!(
        selected_intrinsics,
        [
            "F32::maximum.f32".to_owned(),
            "F32::minimum.f32".to_owned(),
            "F32::square_root.f32".to_owned(),
            "F64::maximum.f64".to_owned(),
            "F64::minimum.f64".to_owned(),
            "F64::square_root.f64".to_owned(),
        ]
        .into_iter()
        .collect()
    );
    let expected_contract =
        |intrinsic: &str, parameter_type_identities: &[&str], result_type_identity: &str| {
            FloatProviderContract {
                intrinsic: intrinsic.to_owned(),
                parameter_count: parameter_type_identities.len(),
                parameter_type_identities: parameter_type_identities
                    .iter()
                    .map(|identity| (*identity).to_owned())
                    .collect(),
                has_result: true,
                result_type_identity: Some(result_type_identity.to_owned()),
                service_reach: Vec::new(),
                synchronous_invocations: Vec::new(),
                may_suspend: false,
                may_block: false,
                terminates_guarantee: false,
            }
        };
    let expected_contract_rows: std::collections::BTreeMap<_, _> = [
        (
            "operator::F32::maximum(named(name(f32)),named(name(f32)))->named(name(f32))",
            expected_contract(
                "F32::maximum.f32",
                &["named(name(f32))", "named(name(f32))"],
                "named(name(f32))",
            ),
        ),
        (
            "operator::F32::minimum(named(name(f32)),named(name(f32)))->named(name(f32))",
            expected_contract(
                "F32::minimum.f32",
                &["named(name(f32))", "named(name(f32))"],
                "named(name(f32))",
            ),
        ),
        (
            "operator::F32::square_root(named(name(f32)))->named(name(f32))",
            expected_contract(
                "F32::square_root.f32",
                &["named(name(f32))"],
                "named(name(f32))",
            ),
        ),
        (
            "operator::F64::maximum(named(name(f64)),named(name(f64)))->named(name(f64))",
            expected_contract(
                "F64::maximum.f64",
                &["named(name(f64))", "named(name(f64))"],
                "named(name(f64))",
            ),
        ),
        (
            "operator::F64::minimum(named(name(f64)),named(name(f64)))->named(name(f64))",
            expected_contract(
                "F64::minimum.f64",
                &["named(name(f64))", "named(name(f64))"],
                "named(name(f64))",
            ),
        ),
        (
            "operator::F64::square_root(named(name(f64)))->named(name(f64))",
            expected_contract(
                "F64::square_root.f64",
                &["named(name(f64))"],
                "named(name(f64))",
            ),
        ),
    ]
    .into_iter()
    .map(|(requirement, contract)| (requirement.to_owned(), contract))
    .collect();
    assert_eq!(selected_contract_rows, expected_contract_rows);
    selected_plan_identities.sort_unstable();
    selected_plan_identities.dedup();
    assert_eq!(
        selected_plan_identities.len(),
        6,
        "{DIFFERENTIAL_SUITE_ID} must bind one exact plan per operation/format slot"
    );

    let outcome = interpret(&checked, &[]);
    assert_eq!(outcome.exit_code, 70, "rewritten builtins must execute");

    let build_dir =
        std::env::temp_dir().join(format!("omega-named-float-provider-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("named float provider calls should compile from their authored native root");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("named float provider canary should run");
    let _ = fs::remove_dir_all(&build_dir);
    assert_eq!(
        output.status.code(),
        Some(70),
        "selected named float builtins must execute natively; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    for target in ["linux_x64", "linux_arm64"] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-named-float-provider-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        compile_rooted_canary_for_target(&canary, scratch.join("out"), target).unwrap_or_else(
            |diagnostics| {
                panic!("named float provider calls should compile for {target}: {diagnostics:#?}")
            },
        );
        let _ = fs::remove_dir_all(&scratch);
    }

    let result_identity = retained_float_differential_result_identity(
        DIFFERENTIAL_SUITE_ID,
        "macos_arm64",
        DIFFERENTIAL_COVERAGE,
        &selected_intrinsics,
        &selected_plan_identities,
        &outcome,
        &output,
        &["linux_x64", "linux_arm64"],
    );
    assert_eq!(
        result_identity, EXPECTED_DIFFERENTIAL_RESULT_IDENTITY,
        "{DIFFERENTIAL_SUITE_ID} result changed ({result_identity:#018x}); validate the exact plans, edge corpus, interpreter/native results, and cross-target builds before refreshing the retained identity"
    );
}

#[test]
fn named_float_negate_and_is_nan_preserve_selected_roots_and_execute() {
    const DIFFERENTIAL_SUITE_ID: &str = "omega.float.hardware.macos_arm64.negate-is-nan.v1";
    const DIFFERENTIAL_COVERAGE: &[&str] = &[
        "binary32 signed-zero and infinity negation",
        "binary64 signed-zero and infinity negation",
        "binary32 NaN/infinity/finite predicate separation",
        "binary64 NaN/infinity/finite predicate separation",
        "selected-root unary evaluation shape",
    ];
    const EXPECTED_DIFFERENTIAL_RESULT_IDENTITY: u64 = 0x3c92_46b9_d29d_254c;

    let canary = pass_canary("float/named_provider_negate_is_nan_exit");
    let checked = omega_compiler::compile_to_checked(&canary.join("main.omg"), None)
        .expect("named negate/is_nan provider calls should compile to checked trees");

    let mut selected_intrinsics = std::collections::BTreeSet::new();
    let mut selected_plan_identities = Vec::new();
    for operator_use in checked.facts.operators.named_uses() {
        if operator_use.provider_plan_identity == 0 {
            continue;
        }
        let plan = checked
            .selected_provider_plans()
            .plan_by_identity(operator_use.provider_plan_identity)
            .expect("named operator evidence must resolve to its retained plan");
        let [row] = plan.rows.as_slice() else {
            panic!("named float plan must contain exactly one row");
        };
        let omega_effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. } = &row.binding
        else {
            panic!("named float plan must select a compiler intrinsic");
        };
        let name = selected_intrinsic_diagnostic_label(&checked, plan);
        selected_intrinsics.insert(name.clone());
        selected_plan_identities.push(plan.identity_fingerprint());

        if name.contains("::negate.") {
            let psi_typed_trees::expression::ExpressionNode::Binary(binary) = checked
                .typed
                .expression_table
                .expression(operator_use.expression)
            else {
                panic!("`{name}` must preserve its selected root as a primitive binary expression");
            };
            assert_eq!(
                binary.operator,
                psi_typed_trees::expression::BinaryOperator::Multiply
            );
            let psi_typed_trees::expression::ExpressionNode::Float(negative_one) =
                checked.typed.expression_table.expression(binary.right)
            else {
                panic!("`{name}` must multiply by a landed -1 literal");
            };
            assert_eq!(negative_one.text(), "-1.0");
            assert_eq!(
                negative_one.landing(),
                Some(if name.starts_with("F32::") {
                    psi_numerics::literals::FloatFormat::F32
                } else {
                    psi_numerics::literals::FloatFormat::F64
                })
            );
        } else if name.contains("::is_nan.") {
            let psi_typed_trees::expression::ExpressionNode::Call(call) = checked
                .typed
                .expression_table
                .expression(operator_use.expression)
            else {
                panic!("`{name}` must preserve its selected root as a unary builtin call");
            };
            assert_eq!(
                call.target_symbol,
                checked
                    .typed
                    .symbols
                    .builtin_function_symbol(psi_symbols::BuiltinFunction::FloatIsNan)
                    .expect("internal float is_nan builtin symbol")
            );
            assert_eq!(call.target.as_str(), "float#is_nan");
            assert!(!call.receiver.is_valid());
            assert_eq!(
                call.arguments.count(),
                1,
                "`{name}` must evaluate one argument"
            );
        } else {
            panic!("unexpected migrated named intrinsic `{name}`");
        }
    }

    assert_eq!(
        selected_intrinsics,
        [
            "F32::is_nan.f32".to_owned(),
            "F32::negate.f32".to_owned(),
            "F64::is_nan.f64".to_owned(),
            "F64::negate.f64".to_owned(),
        ]
        .into_iter()
        .collect()
    );
    selected_plan_identities.sort_unstable();
    selected_plan_identities.dedup();
    assert_eq!(
        selected_plan_identities.len(),
        4,
        "{DIFFERENTIAL_SUITE_ID} must bind one exact plan per operation/format slot"
    );

    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "rewritten negate/is_nan expressions must execute in the interpreter"
    );

    let build_dir = std::env::temp_dir().join(format!(
        "omega-named-float-negate-is-nan-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "named negate/is_nan provider calls should compile from their authored native root",
    );
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("named negate/is_nan provider canary should run");
    let _ = fs::remove_dir_all(&build_dir);
    assert_eq!(
        output.status.code(),
        Some(70),
        "selected named negate/is_nan expressions must execute natively; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    for target in ["linux_x64", "linux_arm64"] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-named-float-negate-is-nan-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        compile_rooted_canary_for_target(&canary, scratch.join("out"), target).unwrap_or_else(
            |diagnostics| {
            panic!(
                "named negate/is_nan provider calls should compile for {target}: {diagnostics:#?}"
            )
            },
        );
        let _ = fs::remove_dir_all(&scratch);
    }

    let result_identity = retained_float_differential_result_identity(
        DIFFERENTIAL_SUITE_ID,
        "macos_arm64",
        DIFFERENTIAL_COVERAGE,
        &selected_intrinsics,
        &selected_plan_identities,
        &outcome,
        &output,
        &["linux_x64", "linux_arm64"],
    );
    assert_eq!(
        result_identity, EXPECTED_DIFFERENTIAL_RESULT_IDENTITY,
        "{DIFFERENTIAL_SUITE_ID} result changed ({result_identity:#018x}); validate the exact plans, edge corpus, interpreter/native results, and cross-target builds before refreshing the retained identity"
    );
}

#[test]
fn named_float_classification_predicates_select_and_execute() {
    const DIFFERENTIAL_SUITE_ID: &str =
        "omega.float.hardware.macos_arm64.classification-predicates.v1";
    const DIFFERENTIAL_COVERAGE: &[&str] = &[
        "binary32/binary64 finite versus infinity",
        "binary32/binary64 infinity versus NaN",
        "binary32/binary64 normal versus subnormal",
        "binary32/binary64 subnormal versus zero",
        "exactly-once unary evaluation shape",
    ];
    const EXPECTED_DIFFERENTIAL_RESULT_IDENTITY: u64 = 0xa6bf_7c01_3cb0_fd6a;

    let canary = pass_canary("float/named_provider_classification_predicates_exit");
    let checked = omega_compiler::compile_to_checked(&canary.join("main.omg"), None)
        .expect("named float classification calls should compile to checked trees");

    let mut selected_intrinsics = std::collections::BTreeSet::new();
    let mut selected_plan_identities = Vec::new();
    for operator_use in checked.facts.operators.named_uses() {
        if operator_use.provider_plan_identity == 0 {
            continue;
        }
        let plan = checked
            .selected_provider_plans()
            .plan_by_identity(operator_use.provider_plan_identity)
            .expect("named classification evidence must retain its plan");
        let [row] = plan.rows.as_slice() else {
            panic!("named classification plan must contain one row");
        };
        let omega_effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. } = &row.binding
        else {
            panic!("named classification plan must select a compiler intrinsic");
        };
        let name = selected_intrinsic_diagnostic_label(&checked, plan);
        selected_intrinsics.insert(name.clone());
        selected_plan_identities.push(plan.identity_fingerprint());

        let (expected_builtin, expected_target) = if name.contains("::is_finite.") {
            (
                psi_symbols::BuiltinFunction::FloatIsFinite,
                "float#is_finite",
            )
        } else if name.contains("::is_infinite.") {
            (
                psi_symbols::BuiltinFunction::FloatIsInfinite,
                "float#is_infinite",
            )
        } else if name.contains("::is_normal.") {
            (
                psi_symbols::BuiltinFunction::FloatIsNormal,
                "float#is_normal",
            )
        } else if name.contains("::is_subnormal.") {
            (
                psi_symbols::BuiltinFunction::FloatIsSubnormal,
                "float#is_subnormal",
            )
        } else {
            panic!("unexpected classification intrinsic `{name}`");
        };
        let psi_typed_trees::expression::ExpressionNode::Call(call) = checked
            .typed
            .expression_table
            .expression(operator_use.expression)
        else {
            panic!("`{name}` must remain a unary builtin call");
        };
        assert_eq!(
            Some(call.target_symbol),
            checked
                .typed
                .symbols
                .builtin_function_symbol(expected_builtin)
        );
        assert_eq!(call.target.as_str(), expected_target);
        assert!(!call.receiver.is_valid());
        assert_eq!(call.arguments.count(), 1, "`{name}` evaluates one operand");
    }

    let expected_intrinsics = [
        "F32::is_finite.f32",
        "F32::is_infinite.f32",
        "F32::is_normal.f32",
        "F32::is_subnormal.f32",
        "F64::is_finite.f64",
        "F64::is_infinite.f64",
        "F64::is_normal.f64",
        "F64::is_subnormal.f64",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(selected_intrinsics, expected_intrinsics);
    selected_plan_identities.sort_unstable();
    selected_plan_identities.dedup();
    assert_eq!(
        selected_plan_identities.len(),
        8,
        "{DIFFERENTIAL_SUITE_ID} must bind one exact plan per predicate/format slot"
    );

    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "classification builtins must interpret"
    );

    let build_dir = std::env::temp_dir().join(format!(
        "omega-named-float-classification-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("named float classification calls should compile from their authored native root");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("named float classification canary should run");
    let _ = fs::remove_dir_all(&build_dir);
    assert_eq!(
        output.status.code(),
        Some(70),
        "classification predicates must execute natively; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    for target in ["linux_x64", "linux_arm64"] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-named-float-classification-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        compile_rooted_canary_for_target(&canary, scratch.join("out"), target).unwrap_or_else(
            |diagnostics| {
                panic!("classification calls should compile for {target}: {diagnostics:#?}")
            },
        );
        let _ = fs::remove_dir_all(&scratch);
    }

    let result_identity = retained_float_differential_result_identity(
        DIFFERENTIAL_SUITE_ID,
        "macos_arm64",
        DIFFERENTIAL_COVERAGE,
        &selected_intrinsics,
        &selected_plan_identities,
        &outcome,
        &output,
        &["linux_x64", "linux_arm64"],
    );
    assert_eq!(
        result_identity, EXPECTED_DIFFERENTIAL_RESULT_IDENTITY,
        "{DIFFERENTIAL_SUITE_ID} result changed ({result_identity:#018x}); validate the exact plans, edge corpus, interpreter/native results, and cross-target builds before refreshing the retained identity"
    );
}

#[test]
fn named_float_classify_preserves_enum_layout_and_executes() {
    const DIFFERENTIAL_SUITE_ID: &str = "omega.float.hardware.macos_arm64.classify-enum.v1";
    const DIFFERENTIAL_COVERAGE: &[&str] = &[
        "FloatClass eight-byte layout and source-order tags",
        "FloatClass sign payload at byte four",
        "binary32 all class tags and signed payloads",
        "binary64 all class tags and signed payloads",
        "exactly-once unary evaluation shape",
    ];
    const EXPECTED_DIFFERENTIAL_RESULT_IDENTITY: u64 = 0x9a27_9424_1f02_d5fa;

    let canary = pass_canary("float/named_provider_classify_exit");
    let checked = omega_compiler::compile_to_checked(&canary.join("main.omg"), None)
        .expect("named float classify calls should compile to checked trees");
    let layouts = omega_layout::build_layout_plan(&checked, omega_target::NativeTarget::host())
        .expect("FloatClass layout should build");
    let float_class = layouts
        .data_layouts
        .iter()
        .find(|(_, layout)| layout.name.as_str() == "FloatClass")
        .map(|(_, layout)| layout)
        .expect("FloatClass layout");
    assert_eq!(float_class.layout.size, 8, "packed intrinsic store width");
    assert_eq!(float_class.layout.alignment, 4, "tag alignment");
    let omega_layout::DataShape::Enum { variants, .. } = &float_class.shape else {
        panic!("FloatClass must remain an enum");
    };
    let variants = layouts.variants.span_or_empty(*variants);
    assert_eq!(
        variants
            .iter()
            .map(|variant| variant.name.as_str())
            .collect::<Vec<_>>(),
        ["NaN", "Infinity", "Normal", "Subnormal", "Zero"],
        "native tags are source-order ordinals"
    );
    for variant in &variants[1..] {
        let [negative] = layouts.fields.span_or_empty(variant.fields) else {
            panic!("{} must carry exactly one sign payload", variant.name);
        };
        assert_eq!(negative.name.as_str(), "negative");
        assert_eq!(negative.offset, 4, "sign payload follows the i32 tag");
        assert_eq!(negative.layout.size, 1, "sign payload is a bool");
    }

    let mut selected_intrinsics = std::collections::BTreeSet::new();
    let mut selected_plan_identities = Vec::new();
    for operator_use in checked.facts.operators.named_uses() {
        if operator_use.provider_plan_identity == 0 {
            continue;
        }
        let plan = checked
            .selected_provider_plans()
            .plan_by_identity(operator_use.provider_plan_identity)
            .expect("named classify evidence must retain its plan");
        let [row] = plan.rows.as_slice() else {
            panic!("named classify plan must contain one row");
        };
        let omega_effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. } = &row.binding
        else {
            panic!("named classify plan must select a compiler intrinsic");
        };
        let name = selected_intrinsic_diagnostic_label(&checked, plan);
        if !name.contains("::classify.") {
            continue;
        }
        selected_intrinsics.insert(name.clone());
        selected_plan_identities.push(plan.identity_fingerprint());
        let (expected_builtin, expected_target) = if name.starts_with("F32::") {
            (
                psi_symbols::BuiltinFunction::FloatClassifyF32,
                "float#classify_f32",
            )
        } else {
            (
                psi_symbols::BuiltinFunction::FloatClassifyF64,
                "float#classify_f64",
            )
        };
        let psi_typed_trees::expression::ExpressionNode::Call(call) = checked
            .typed
            .expression_table
            .expression(operator_use.expression)
        else {
            panic!("`{name}` must remain a unary builtin call");
        };
        assert_eq!(
            Some(call.target_symbol),
            checked
                .typed
                .symbols
                .builtin_function_symbol(expected_builtin)
        );
        assert_eq!(call.target.as_str(), expected_target);
        assert!(!call.receiver.is_valid());
        assert_eq!(call.arguments.count(), 1);
    }
    assert_eq!(
        selected_intrinsics,
        [
            "F32::classify.f32".to_owned(),
            "F64::classify.f64".to_owned(),
        ]
        .into_iter()
        .collect()
    );
    selected_plan_identities.sort_unstable();
    selected_plan_identities.dedup();
    assert_eq!(
        selected_plan_identities.len(),
        2,
        "{DIFFERENTIAL_SUITE_ID} must bind one exact plan per format slot"
    );

    let outcome = interpret(&checked, &[]);
    assert_eq!(outcome.exit_code, 70, "classify builtin must interpret");

    let build_dir =
        std::env::temp_dir().join(format!("omega-named-float-classify-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("named float classify calls should compile from their authored native root");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("named float classify canary should run");
    let _ = fs::remove_dir_all(&build_dir);
    assert_eq!(
        output.status.code(),
        Some(70),
        "classify must execute natively; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    for target in ["linux_x64", "linux_arm64"] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-named-float-classify-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        compile_rooted_canary_for_target(&canary, scratch.join("out"), target).unwrap_or_else(
            |diagnostics| panic!("classify calls should compile for {target}: {diagnostics:#?}"),
        );
        let _ = fs::remove_dir_all(&scratch);
    }

    let result_identity = retained_float_differential_result_identity(
        DIFFERENTIAL_SUITE_ID,
        "macos_arm64",
        DIFFERENTIAL_COVERAGE,
        &selected_intrinsics,
        &selected_plan_identities,
        &outcome,
        &output,
        &["linux_x64", "linux_arm64"],
    );
    assert_eq!(
        result_identity, EXPECTED_DIFFERENTIAL_RESULT_IDENTITY,
        "{DIFFERENTIAL_SUITE_ID} result changed ({result_identity:#018x}); validate the exact plans, edge corpus, interpreter/native results, and cross-target builds before refreshing the retained identity"
    );
}

#[test]
fn named_float_multiply_then_add_preserves_two_roundings_and_executes() {
    const DIFFERENTIAL_SUITE_ID: &str = "omega.float.hardware.macos_arm64.multiply-then-add.v1";
    const DIFFERENTIAL_COVERAGE: &[&str] = &[
        "binary32 cancellation edge",
        "binary64 cancellation edge",
        "two distinct roundings",
        "binary32 finite-overflow saturation",
    ];
    const EXPECTED_DIFFERENTIAL_RESULT_IDENTITY: u64 = 0x3469_73b6_84ba_8c5d;

    let canary = pass_canary("float/named_provider_multiply_then_add_exit");
    let checked = omega_compiler::compile_to_checked(&canary.join("main.omg"), None)
        .expect("named multiply-then-add provider calls should compile to checked trees");

    let main_machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("Main::main machine");
    let main_state = checked
        .typed
        .machine_states(main_machine)
        .first()
        .expect("Main::main entry state");
    let psi_typed_trees::statement::StatementNode::Assignment(result32_assignment) = &checked
        .typed
        .statement_table
        .statements(main_state.statement_nodes)[2]
    else {
        panic!("Main::main statement 2 must assign result32");
    };
    let result32_origin = psi_checked_trees::CheckedValueOrigin::StateStatement {
        machine_symbol: main_machine.symbol,
        state_symbol: main_state.symbol,
        statement_index: 2,
        role: psi_checked_trees::CheckedValueStatementRole::AssignmentValue,
    };
    let outer_add = checked
        .facts
        .operators
        .expression_use_in_origin(result32_assignment.value, result32_origin)
        .expect("the primitive add surrounding multiply-then-add must retain checked evidence");
    assert_eq!(
        outer_add.spelling,
        psi_language_core::operator_spelling::OperatorSpelling::Add
    );
    assert_ne!(outer_add.provider_plan_identity, 0);
    assert!(
        checked
            .selected_provider_plans()
            .plan_by_identity(outer_add.provider_plan_identity)
            .is_some()
    );
    let mut selected_intrinsics = std::collections::BTreeSet::new();
    let mut selected_plan_identities = Vec::new();
    for operator_use in checked.facts.operators.named_uses() {
        if operator_use.provider_plan_identity == 0 {
            continue;
        }
        let plan = checked
            .selected_provider_plans()
            .plan_by_identity(operator_use.provider_plan_identity)
            .expect("named operator evidence must resolve to its retained plan");
        let [row] = plan.rows.as_slice() else {
            panic!("named float plan must contain exactly one row");
        };
        let omega_effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. } = &row.binding
        else {
            panic!("named float plan must select a compiler intrinsic");
        };
        let name = selected_intrinsic_diagnostic_label(&checked, plan);
        selected_intrinsics.insert(name.clone());
        selected_plan_identities.push(plan.identity_fingerprint());

        let psi_typed_trees::expression::ExpressionNode::Call(call) = checked
            .typed
            .expression_table
            .expression(operator_use.expression)
        else {
            panic!("`{name}` must preserve its selected root as a compiler call");
        };
        assert_eq!(call.arguments.count(), 3);
        assert!(!call.receiver.is_valid());
        let expected_builtin = match name.as_str() {
            "F32::multiply_then_add.f32" => psi_symbols::BuiltinFunction::FloatMultiplyThenAddF32,
            "F64::multiply_then_add.f64" => psi_symbols::BuiltinFunction::FloatMultiplyThenAddF64,
            _ => panic!("unexpected multiply-then-add intrinsic `{name}`"),
        };
        assert_eq!(
            Some(call.target_symbol),
            checked
                .typed
                .symbols
                .builtin_function_symbol(expected_builtin)
        );
    }

    assert_eq!(
        selected_intrinsics,
        [
            "F32::multiply_then_add.f32".to_owned(),
            "F64::multiply_then_add.f64".to_owned(),
        ]
        .into_iter()
        .collect()
    );
    selected_plan_identities.sort_unstable();
    selected_plan_identities.dedup();
    assert_eq!(
        selected_plan_identities.len(),
        2,
        "{DIFFERENTIAL_SUITE_ID} must bind one exact plan per format slot"
    );

    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "selected multiply-then-add must keep its two-rounding semantics in the interpreter"
    );

    let build_dir = std::env::temp_dir().join(format!(
        "omega-named-float-multiply-then-add-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone()).expect(
        "named multiply-then-add provider calls should compile from their authored native root",
    );
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("named multiply-then-add provider canary should run");
    let _ = fs::remove_dir_all(&build_dir);
    assert_eq!(
        output.status.code(),
        Some(70),
        "selected multiply-then-add must execute natively; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    for target in ["linux_x64", "linux_arm64"] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-named-float-multiply-then-add-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        compile_rooted_canary_for_target(&canary, scratch.join("out"), target).unwrap_or_else(
            |diagnostics| {
            panic!(
                "named multiply-then-add provider calls should compile for {target}: {diagnostics:#?}"
            )
            },
        );
        let _ = fs::remove_dir_all(&scratch);
    }

    let result_identity = retained_float_differential_result_identity(
        DIFFERENTIAL_SUITE_ID,
        "macos_arm64",
        DIFFERENTIAL_COVERAGE,
        &selected_intrinsics,
        &selected_plan_identities,
        &outcome,
        &output,
        &["linux_x64", "linux_arm64"],
    );
    assert_eq!(
        result_identity, EXPECTED_DIFFERENTIAL_RESULT_IDENTITY,
        "{DIFFERENTIAL_SUITE_ID} result changed ({result_identity:#018x}); validate the exact plans, edge corpus, interpreter/native results, and cross-target builds before refreshing the retained identity"
    );
}

#[test]
fn named_float_fused_multiply_add_selects_aarch64_fmadd_and_executes() {
    const DIFFERENTIAL_SUITE_ID: &str = "omega.float.hardware.macos_arm64.nearest-fma.v1";
    const DIFFERENTIAL_COVERAGE: &[&str] = &[
        "binary32 cancellation edge",
        "binary64 cancellation edge",
        "single fused rounding",
    ];
    const EXPECTED_DIFFERENTIAL_RESULT_IDENTITY: u64 = 0xbb3f_d600_7ddf_03c0;

    let canary = pass_canary("float/named_provider_fused_multiply_add_exit");
    let checked = omega_compiler::compile_to_checked(&canary.join("main.omg"), None)
        .expect("named FMA provider calls should compile to checked trees on macOS AArch64");

    let mut selected_intrinsics = std::collections::BTreeSet::new();
    let mut selected_plan_identities = Vec::new();
    for operator_use in checked.facts.operators.named_uses() {
        if operator_use.provider_plan_identity == 0 {
            continue;
        }
        let plan = checked
            .selected_provider_plans()
            .plan_by_identity(operator_use.provider_plan_identity)
            .expect("named FMA evidence must resolve to its retained plan");
        let [row] = plan.rows.as_slice() else {
            panic!("named FMA plan must contain exactly one row");
        };
        let omega_effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. } = &row.binding
        else {
            panic!("named FMA plan must select a compiler intrinsic");
        };
        let name = selected_intrinsic_diagnostic_label(&checked, plan);
        if !name.contains("fused_multiply_add") {
            continue;
        }
        selected_intrinsics.insert(name.clone());
        selected_plan_identities.push(plan.identity_fingerprint());

        let psi_typed_trees::expression::ExpressionNode::Call(call) = checked
            .typed
            .expression_table
            .expression(operator_use.expression)
        else {
            panic!("`{name}` must preserve its selected root as a compiler call");
        };
        assert_eq!(call.arguments.count(), 3);
        assert!(!call.receiver.is_valid());
        let expected_builtin = match name.as_str() {
            "F32::fused_multiply_add.f32" => psi_symbols::BuiltinFunction::FloatFusedMultiplyAddF32,
            "F64::fused_multiply_add.f64" => psi_symbols::BuiltinFunction::FloatFusedMultiplyAddF64,
            _ => panic!("unexpected FMA intrinsic `{name}`"),
        };
        assert_eq!(
            Some(call.target_symbol),
            checked
                .typed
                .symbols
                .builtin_function_symbol(expected_builtin)
        );
    }

    assert_eq!(
        selected_intrinsics,
        [
            "F32::fused_multiply_add.f32".to_owned(),
            "F64::fused_multiply_add.f64".to_owned(),
        ]
        .into_iter()
        .collect()
    );
    selected_plan_identities.sort_unstable();
    selected_plan_identities.dedup();
    assert_eq!(
        selected_plan_identities.len(),
        2,
        "{DIFFERENTIAL_SUITE_ID} must bind one exact plan per format slot"
    );

    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "selected FMA must preserve its single-rounding semantics in the interpreter"
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-named-float-fma-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("named FMA provider calls should compile from their authored native root");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("named FMA provider canary should run");
    let _ = fs::remove_dir_all(&build_dir);
    assert_eq!(
        output.status.code(),
        Some(70),
        "selected FMA must execute natively; stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let scratch = std::env::temp_dir().join(format!(
        "omega-named-float-fma-linux-arm64-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_target(&canary, scratch.join("out"), "linux_arm64").unwrap_or_else(
        |diagnostics| {
            panic!("named FMA provider calls should compile for linux_arm64: {diagnostics:#?}")
        },
    );
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
        "{DIFFERENTIAL_SUITE_ID} result changed ({result_identity:#018x}); validate the exact plans, edge corpus, interpreter/native results, and cross-target builds before refreshing the retained identity"
    );
}

#[test]
fn named_float_directed_fused_multiply_add_selects_aarch64_fmadd_and_executes() {
    const DIFFERENTIAL_SUITE_ID: &str = "omega.float.hardware.macos_arm64.directed-fma.v1";
    const DIFFERENTIAL_COVERAGE: &[&str] = &[
        "binary32 half-ULP edge",
        "binary64 half-ULP edge",
        "toward zero",
        "toward positive",
        "toward negative",
        "single fused rounding",
        "floating-control restoration",
    ];
    const EXPECTED_DIFFERENTIAL_RESULT_IDENTITY: u64 = 0x4b6d_5c3b_9fb5_54a6;

    let canary = pass_canary("float/named_provider_directed_fused_multiply_add_exit");
    let checked = omega_compiler::compile_to_checked(&canary.join("main.omg"), None)
        .expect("directed-FMA provider calls should compile to checked trees on macOS AArch64");

    let mut selected_intrinsics = std::collections::BTreeSet::new();
    let mut selected_plan_identities = Vec::new();
    for operator_use in checked.facts.operators.named_uses() {
        let Some(plan) = checked
            .selected_provider_plans()
            .plan_by_identity(operator_use.provider_plan_identity)
        else {
            continue;
        };
        let [row] = plan.rows.as_slice() else {
            continue;
        };
        let omega_effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. } = &row.binding
        else {
            continue;
        };
        let name = selected_intrinsic_diagnostic_label(&checked, plan);
        if !name.contains("::fused_multiply_add_toward_") {
            continue;
        }
        selected_intrinsics.insert(name.clone());
        selected_plan_identities.push(plan.identity_fingerprint());

        let psi_typed_trees::expression::ExpressionNode::Call(call) = checked
            .typed
            .expression_table
            .expression(operator_use.expression)
        else {
            panic!("`{name}` must rewrite to an unnameable compiler call");
        };
        assert!(!call.receiver.is_valid());
        assert_eq!(call.arguments.count(), 3);
        assert!(
            call.target
                .as_str()
                .starts_with("float#fused_multiply_add_toward_")
        );
    }

    assert_eq!(
        selected_intrinsics,
        [
            "F32::fused_multiply_add_toward_negative.f32",
            "F32::fused_multiply_add_toward_positive.f32",
            "F32::fused_multiply_add_toward_zero.f32",
            "F64::fused_multiply_add_toward_negative.f64",
            "F64::fused_multiply_add_toward_positive.f64",
            "F64::fused_multiply_add_toward_zero.f64",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    );
    selected_plan_identities.sort_unstable();
    selected_plan_identities.dedup();
    assert_eq!(
        selected_plan_identities.len(),
        6,
        "{DIFFERENTIAL_SUITE_ID} must bind one exact plan per format/direction slot"
    );

    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "directed-FMA interpreter semantics must distinguish half-ULP edges"
    );

    let build_dir = std::env::temp_dir().join(format!("omega-directed-fma-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        .expect("directed-FMA providers should compile from their authored native root");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("directed-FMA provider canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "directed FMA must restore nearest-even before ordinary FMA; artifact: {}; stderr: {}",
        build_dir.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);

    let scratch = std::env::temp_dir().join(format!(
        "omega-directed-fma-linux-arm64-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&scratch);
    compile_rooted_canary_for_target(&canary, scratch.join("out"), "linux_arm64").unwrap_or_else(
        |diagnostics| {
            panic!("directed-FMA providers should compile for linux_arm64: {diagnostics:#?}")
        },
    );
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
        "{DIFFERENTIAL_SUITE_ID} result changed ({result_identity:#018x}); validate the exact plans, edge corpus, interpreter/native results, and cross-target builds before refreshing the retained identity"
    );
}

pub(super) fn retained_float_differential_result_identity(
    suite_id: &str,
    target: &str,
    coverage: &[&str],
    selected_intrinsics: &std::collections::BTreeSet<String>,
    selected_plan_identities: &[u64],
    outcome: &psi_checked_interpreter::InterpretOutcome,
    output: &std::process::Output,
    cross_targets: &[&str],
) -> u64 {
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

    let mut result_identity = 0xcbf29ce484222325_u64;
    retain(&mut result_identity, suite_id.as_bytes());
    retain(&mut result_identity, target.as_bytes());
    for category in coverage {
        retain(&mut result_identity, category.as_bytes());
    }
    for intrinsic in selected_intrinsics {
        retain(&mut result_identity, intrinsic.as_bytes());
    }
    for identity in selected_plan_identities {
        retain(&mut result_identity, &identity.to_le_bytes());
    }
    retain(&mut result_identity, &outcome.exit_code.to_le_bytes());
    retain(&mut result_identity, &outcome.stdout);
    retain(&mut result_identity, &outcome.stderr);
    retain(
        &mut result_identity,
        &output.status.code().unwrap_or_default().to_le_bytes(),
    );
    retain(&mut result_identity, &output.stdout);
    retain(&mut result_identity, &output.stderr);
    for target in cross_targets {
        retain(
            &mut result_identity,
            format!("{target}:cross-compile-passed").as_bytes(),
        );
    }
    result_identity
}

fn retained_float_policy_differential_result_identity(
    suite_id: &str,
    target: &str,
    coverage: &[&str],
    selected_evidence: &std::collections::BTreeSet<String>,
    selected_plan_identities: &[u64],
    observations: &[(
        &str,
        &psi_checked_interpreter::InterpretOutcome,
        &std::process::Output,
    )],
    cross_builds: &std::collections::BTreeSet<String>,
) -> u64 {
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

    let mut result_identity = 0xcbf29ce484222325_u64;
    retain(&mut result_identity, suite_id.as_bytes());
    retain(&mut result_identity, target.as_bytes());
    for category in coverage {
        retain(&mut result_identity, category.as_bytes());
    }
    for evidence in selected_evidence {
        retain(&mut result_identity, evidence.as_bytes());
    }
    for identity in selected_plan_identities {
        retain(&mut result_identity, &identity.to_le_bytes());
    }
    for (label, outcome, output) in observations {
        retain(&mut result_identity, label.as_bytes());
        retain(&mut result_identity, &outcome.exit_code.to_le_bytes());
        retain(&mut result_identity, &outcome.stdout);
        retain(&mut result_identity, &outcome.stderr);
        match &outcome.error {
            Some(error) => {
                retain(&mut result_identity, b"interpreter-error");
                retain(&mut result_identity, error.as_bytes());
            }
            None => retain(&mut result_identity, b"interpreter-success"),
        }
        retain(
            &mut result_identity,
            format!("{:?}", output.status).as_bytes(),
        );
        retain(&mut result_identity, &output.stdout);
        retain(&mut result_identity, &output.stderr);
    }
    for cross_build in cross_builds {
        retain(
            &mut result_identity,
            format!("{cross_build}:cross-compile-passed").as_bytes(),
        );
    }
    result_identity
}

#[test]
fn named_float_directed_add_selects_exact_plans_and_restores_control_state() {
    const DIFFERENTIAL_SUITE_ID: &str = "omega.float.hardware.macos_arm64.directed-add.v1";
    const DIFFERENTIAL_COVERAGE: &[&str] = &[
        "binary32 half-ULP tie",
        "binary64 half-ULP tie",
        "toward zero",
        "toward positive",
        "toward negative",
        "floating-control restoration",
    ];
    const EXPECTED_DIFFERENTIAL_RESULT_IDENTITY: u64 = 0x7e9b_cd52_c66c_6510;

    let canary = pass_canary("float/named_provider_directed_add_exit");
    let checked = omega_compiler::compile_to_checked(&canary.join("main.omg"), None)
        .expect("directed-add provider calls should compile to checked trees");

    let mut selected_intrinsics = std::collections::BTreeSet::new();
    let mut selected_plan_identities = Vec::new();
    for operator_use in checked.facts.operators.named_uses() {
        let Some(plan) = checked
            .selected_provider_plans()
            .plan_by_identity(operator_use.provider_plan_identity)
        else {
            continue;
        };
        let [row] = plan.rows.as_slice() else {
            continue;
        };
        let omega_effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. } = &row.binding
        else {
            continue;
        };
        let name = selected_intrinsic_diagnostic_label(&checked, plan);
        if !name.contains("::add_toward_") {
            continue;
        }
        selected_intrinsics.insert(name.clone());
        selected_plan_identities.push(plan.identity_fingerprint());
        let psi_typed_trees::expression::ExpressionNode::Call(call) = checked
            .typed
            .expression_table
            .expression(operator_use.expression)
        else {
            panic!("`{name}` must rewrite to an unnameable compiler call");
        };
        assert!(!call.receiver.is_valid());
        assert_eq!(call.arguments.count(), 2);
        assert!(call.target.as_str().starts_with("float#add_toward_"));
    }
    assert_eq!(
        selected_intrinsics,
        [
            "F32::add_toward_negative.f32",
            "F32::add_toward_positive.f32",
            "F32::add_toward_zero.f32",
            "F64::add_toward_negative.f64",
            "F64::add_toward_positive.f64",
            "F64::add_toward_zero.f64",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    );
    selected_plan_identities.sort_unstable();
    selected_plan_identities.dedup();
    assert_eq!(
        selected_plan_identities.len(),
        6,
        "{DIFFERENTIAL_SUITE_ID} must bind one exact plan per format/direction slot"
    );

    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "directed-add interpreter semantics must distinguish half-ULP edges"
    );

    let build_dir = std::env::temp_dir().join(format!("omega-directed-add-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("directed-add providers should compile natively");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("directed-add provider canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "directed add must restore nearest-even before ordinary arithmetic; artifact: {}; stderr: {}",
        build_dir.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);

    for target in ["linux_x64", "linux_arm64"] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-directed-add-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source_dir = scratch.join("src");
        fs::create_dir_all(&source_dir).expect("directed-add cross-target source directory");
        fs::copy(canary.join("main.omg"), source_dir.join("main.omg"))
            .expect("copy directed-add canary");
        fs::write(
            source_dir.join("build.omg"),
            hosted_main_program_entry_build(target),
        )
        .expect("write directed-add target manifest");
        compile(CompileOptions {
            root_path: source_dir.join("main.omg"),
            build_dir: Some(scratch.join("out")),
            target_name: Some(target.to_owned()),
            write_output: true,
        })
        .unwrap_or_else(|diagnostics| {
            panic!("directed-add providers should compile for {target}: {diagnostics:#?}")
        });
        let _ = fs::remove_dir_all(&scratch);
    }

    let result_identity = retained_float_differential_result_identity(
        DIFFERENTIAL_SUITE_ID,
        "macos_arm64",
        DIFFERENTIAL_COVERAGE,
        &selected_intrinsics,
        &selected_plan_identities,
        &outcome,
        &output,
        &["linux_x64", "linux_arm64"],
    );
    assert_eq!(
        result_identity, EXPECTED_DIFFERENTIAL_RESULT_IDENTITY,
        "{DIFFERENTIAL_SUITE_ID} result changed ({result_identity:#018x}); validate the exact plans, edge corpus, interpreter/native results, and cross-target builds before refreshing the retained identity"
    );
}

#[test]
fn named_float_directed_subtract_selects_exact_plans_and_restores_control_state() {
    const DIFFERENTIAL_SUITE_ID: &str = "omega.float.hardware.macos_arm64.directed-subtract.v1";
    const DIFFERENTIAL_COVERAGE: &[&str] = &[
        "binary32 midpoint",
        "binary64 midpoint",
        "toward zero",
        "toward positive",
        "toward negative",
        "floating-control restoration",
    ];
    const EXPECTED_DIFFERENTIAL_RESULT_IDENTITY: u64 = 0xb40d_f240_a7b2_6e47;

    let canary = pass_canary("float/named_provider_directed_subtract_exit");
    let checked = omega_compiler::compile_to_checked(&canary.join("main.omg"), None)
        .expect("directed-subtract provider calls should compile to checked trees");

    let mut selected_intrinsics = std::collections::BTreeSet::new();
    let mut selected_plan_identities = Vec::new();
    for operator_use in checked.facts.operators.named_uses() {
        let Some(plan) = checked
            .selected_provider_plans()
            .plan_by_identity(operator_use.provider_plan_identity)
        else {
            continue;
        };
        let [row] = plan.rows.as_slice() else {
            continue;
        };
        let omega_effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. } = &row.binding
        else {
            continue;
        };
        let name = selected_intrinsic_diagnostic_label(&checked, plan);
        if !name.contains("::subtract_toward_") {
            continue;
        }
        selected_intrinsics.insert(name.clone());
        selected_plan_identities.push(plan.identity_fingerprint());
        let psi_typed_trees::expression::ExpressionNode::Call(call) = checked
            .typed
            .expression_table
            .expression(operator_use.expression)
        else {
            panic!("`{name}` must rewrite to an unnameable compiler call");
        };
        assert!(!call.receiver.is_valid());
        assert_eq!(call.arguments.count(), 2);
        assert!(call.target.as_str().starts_with("float#subtract_toward_"));
    }
    assert_eq!(
        selected_intrinsics,
        [
            "F32::subtract_toward_negative.f32",
            "F32::subtract_toward_positive.f32",
            "F32::subtract_toward_zero.f32",
            "F64::subtract_toward_negative.f64",
            "F64::subtract_toward_positive.f64",
            "F64::subtract_toward_zero.f64",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    );
    selected_plan_identities.sort_unstable();
    selected_plan_identities.dedup();
    assert_eq!(
        selected_plan_identities.len(),
        6,
        "{DIFFERENTIAL_SUITE_ID} must bind one exact plan per format/direction slot"
    );

    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "directed-subtract interpreter semantics must distinguish midpoint edges"
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-directed-subtract-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("directed-subtract providers should compile natively");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("directed-subtract provider canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "directed subtract must restore nearest-even before ordinary arithmetic; artifact: {}; stderr: {}",
        build_dir.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);

    for target in ["linux_x64", "linux_arm64"] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-directed-subtract-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source_dir = scratch.join("src");
        fs::create_dir_all(&source_dir).expect("directed-subtract cross-target source directory");
        fs::copy(canary.join("main.omg"), source_dir.join("main.omg"))
            .expect("copy directed-subtract canary");
        fs::write(
            source_dir.join("build.omg"),
            hosted_main_program_entry_build(target),
        )
        .expect("write directed-subtract target manifest");
        compile(CompileOptions {
            root_path: source_dir.join("main.omg"),
            build_dir: Some(scratch.join("out")),
            target_name: Some(target.to_owned()),
            write_output: true,
        })
        .unwrap_or_else(|diagnostics| {
            panic!("directed-subtract providers should compile for {target}: {diagnostics:#?}")
        });
        let _ = fs::remove_dir_all(&scratch);
    }

    let result_identity = retained_float_differential_result_identity(
        DIFFERENTIAL_SUITE_ID,
        "macos_arm64",
        DIFFERENTIAL_COVERAGE,
        &selected_intrinsics,
        &selected_plan_identities,
        &outcome,
        &output,
        &["linux_x64", "linux_arm64"],
    );
    assert_eq!(
        result_identity, EXPECTED_DIFFERENTIAL_RESULT_IDENTITY,
        "{DIFFERENTIAL_SUITE_ID} result changed ({result_identity:#018x}); validate the exact plans, edge corpus, interpreter/native results, and cross-target builds before refreshing the retained identity"
    );
}

#[test]
fn named_float_directed_multiply_selects_exact_plans_and_restores_control_state() {
    const DIFFERENTIAL_SUITE_ID: &str = "omega.float.hardware.macos_arm64.directed-multiply.v1";
    const DIFFERENTIAL_COVERAGE: &[&str] = &[
        "binary32 exact-product edge",
        "binary64 exact-product edge",
        "toward zero",
        "toward positive",
        "toward negative",
        "floating-control restoration",
    ];
    const EXPECTED_DIFFERENTIAL_RESULT_IDENTITY: u64 = 0x4411_b314_20a5_c04b;

    let canary = pass_canary("float/named_provider_directed_multiply_exit");
    let checked = omega_compiler::compile_to_checked(&canary.join("main.omg"), None)
        .expect("directed-multiply provider calls should compile to checked trees");

    let mut selected_intrinsics = std::collections::BTreeSet::new();
    let mut selected_plan_identities = Vec::new();
    for operator_use in checked.facts.operators.named_uses() {
        let Some(plan) = checked
            .selected_provider_plans()
            .plan_by_identity(operator_use.provider_plan_identity)
        else {
            continue;
        };
        let [row] = plan.rows.as_slice() else {
            continue;
        };
        let omega_effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. } = &row.binding
        else {
            continue;
        };
        let name = selected_intrinsic_diagnostic_label(&checked, plan);
        if !name.contains("::multiply_toward_") {
            continue;
        }
        selected_intrinsics.insert(name.clone());
        selected_plan_identities.push(plan.identity_fingerprint());
        let psi_typed_trees::expression::ExpressionNode::Call(call) = checked
            .typed
            .expression_table
            .expression(operator_use.expression)
        else {
            panic!("`{name}` must rewrite to an unnameable compiler call");
        };
        assert!(!call.receiver.is_valid());
        assert_eq!(call.arguments.count(), 2);
        assert!(call.target.as_str().starts_with("float#multiply_toward_"));
    }
    assert_eq!(
        selected_intrinsics,
        [
            "F32::multiply_toward_negative.f32",
            "F32::multiply_toward_positive.f32",
            "F32::multiply_toward_zero.f32",
            "F64::multiply_toward_negative.f64",
            "F64::multiply_toward_positive.f64",
            "F64::multiply_toward_zero.f64",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    );
    selected_plan_identities.sort_unstable();
    selected_plan_identities.dedup();
    assert_eq!(
        selected_plan_identities.len(),
        6,
        "{DIFFERENTIAL_SUITE_ID} must bind one exact plan per format/direction slot"
    );

    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "directed-multiply interpreter semantics must distinguish exact-product edges"
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-directed-multiply-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("directed-multiply providers should compile natively");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("directed-multiply provider canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "directed multiply must restore nearest-even before ordinary arithmetic; artifact: {}; stderr: {}",
        build_dir.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);

    for target in ["linux_x64", "linux_arm64"] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-directed-multiply-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source_dir = scratch.join("src");
        fs::create_dir_all(&source_dir).expect("directed-multiply cross-target source directory");
        fs::copy(canary.join("main.omg"), source_dir.join("main.omg"))
            .expect("copy directed-multiply canary");
        fs::write(
            source_dir.join("build.omg"),
            hosted_main_program_entry_build(target),
        )
        .expect("write directed-multiply target manifest");
        compile(CompileOptions {
            root_path: source_dir.join("main.omg"),
            build_dir: Some(scratch.join("out")),
            target_name: Some(target.to_owned()),
            write_output: true,
        })
        .unwrap_or_else(|diagnostics| {
            panic!("directed-multiply providers should compile for {target}: {diagnostics:#?}")
        });
        let _ = fs::remove_dir_all(&scratch);
    }

    let result_identity = retained_float_differential_result_identity(
        DIFFERENTIAL_SUITE_ID,
        "macos_arm64",
        DIFFERENTIAL_COVERAGE,
        &selected_intrinsics,
        &selected_plan_identities,
        &outcome,
        &output,
        &["linux_x64", "linux_arm64"],
    );
    assert_eq!(
        result_identity, EXPECTED_DIFFERENTIAL_RESULT_IDENTITY,
        "{DIFFERENTIAL_SUITE_ID} result changed ({result_identity:#018x}); validate the exact plans, edge corpus, interpreter/native results, and cross-target builds before refreshing the retained identity"
    );
}

#[test]
fn named_float_directed_divide_selects_exact_plans_and_restores_control_state() {
    const DIFFERENTIAL_SUITE_ID: &str = "omega.float.hardware.macos_arm64.directed-divide.v1";
    const DIFFERENTIAL_COVERAGE: &[&str] = &[
        "binary32 exact-quotient edge",
        "binary64 exact-quotient edge",
        "toward zero",
        "toward positive",
        "toward negative",
        "floating-control restoration",
    ];
    const EXPECTED_DIFFERENTIAL_RESULT_IDENTITY: u64 = 0x5e1f_542f_ee21_0fd9;

    let canary = pass_canary("float/named_provider_directed_divide_exit");
    let checked = omega_compiler::compile_to_checked(&canary.join("main.omg"), None)
        .expect("directed-divide provider calls should compile to checked trees");

    let mut selected_intrinsics = std::collections::BTreeSet::new();
    let mut selected_plan_identities = Vec::new();
    for operator_use in checked.facts.operators.named_uses() {
        let Some(plan) = checked
            .selected_provider_plans()
            .plan_by_identity(operator_use.provider_plan_identity)
        else {
            continue;
        };
        let [row] = plan.rows.as_slice() else {
            continue;
        };
        let omega_effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. } = &row.binding
        else {
            continue;
        };
        let name = selected_intrinsic_diagnostic_label(&checked, plan);
        if !name.contains("::divide_toward_") {
            continue;
        }
        selected_intrinsics.insert(name.clone());
        selected_plan_identities.push(plan.identity_fingerprint());
        let psi_typed_trees::expression::ExpressionNode::Call(call) = checked
            .typed
            .expression_table
            .expression(operator_use.expression)
        else {
            panic!("`{name}` must rewrite to an unnameable compiler call");
        };
        assert!(!call.receiver.is_valid());
        assert_eq!(call.arguments.count(), 2);
        assert!(call.target.as_str().starts_with("float#divide_toward_"));
    }
    assert_eq!(
        selected_intrinsics,
        [
            "F32::divide_toward_negative.f32",
            "F32::divide_toward_positive.f32",
            "F32::divide_toward_zero.f32",
            "F64::divide_toward_negative.f64",
            "F64::divide_toward_positive.f64",
            "F64::divide_toward_zero.f64",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    );
    selected_plan_identities.sort_unstable();
    selected_plan_identities.dedup();
    assert_eq!(
        selected_plan_identities.len(),
        6,
        "{DIFFERENTIAL_SUITE_ID} must bind one exact plan per format/direction slot"
    );

    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "directed-divide interpreter semantics must distinguish exact-quotient edges"
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-directed-divide-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("directed-divide providers should compile natively");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("directed-divide provider canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "directed divide must restore nearest-even before ordinary arithmetic; artifact: {}; stderr: {}",
        build_dir.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);

    for target in ["linux_x64", "linux_arm64"] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-directed-divide-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source_dir = scratch.join("src");
        fs::create_dir_all(&source_dir).expect("directed-divide cross-target source directory");
        fs::copy(canary.join("main.omg"), source_dir.join("main.omg"))
            .expect("copy directed-divide canary");
        fs::write(
            source_dir.join("build.omg"),
            hosted_main_program_entry_build(target),
        )
        .expect("write directed-divide target manifest");
        compile(CompileOptions {
            root_path: source_dir.join("main.omg"),
            build_dir: Some(scratch.join("out")),
            target_name: Some(target.to_owned()),
            write_output: true,
        })
        .unwrap_or_else(|diagnostics| {
            panic!("directed-divide providers should compile for {target}: {diagnostics:#?}")
        });
        let _ = fs::remove_dir_all(&scratch);
    }

    let result_identity = retained_float_differential_result_identity(
        DIFFERENTIAL_SUITE_ID,
        "macos_arm64",
        DIFFERENTIAL_COVERAGE,
        &selected_intrinsics,
        &selected_plan_identities,
        &outcome,
        &output,
        &["linux_x64", "linux_arm64"],
    );
    assert_eq!(
        result_identity, EXPECTED_DIFFERENTIAL_RESULT_IDENTITY,
        "{DIFFERENTIAL_SUITE_ID} result changed ({result_identity:#018x}); validate the exact plans, edge corpus, interpreter/native results, and cross-target builds before refreshing the retained identity"
    );
}

#[test]
fn named_float_directed_square_root_selects_exact_plans_and_restores_control_state() {
    const DIFFERENTIAL_SUITE_ID: &str = "omega.float.hardware.macos_arm64.directed-square-root.v1";
    const DIFFERENTIAL_COVERAGE: &[&str] = &[
        "binary32 irrational-result edge",
        "binary64 irrational-result edge",
        "toward zero",
        "toward positive",
        "toward negative",
        "floating-control restoration",
    ];
    const EXPECTED_DIFFERENTIAL_RESULT_IDENTITY: u64 = 0x5bfe_5610_aa74_88bf;

    let canary = pass_canary("float/named_provider_directed_square_root_exit");
    let checked = omega_compiler::compile_to_checked(&canary.join("main.omg"), None)
        .expect("directed-square-root provider calls should compile to checked trees");

    let mut selected_intrinsics = std::collections::BTreeSet::new();
    let mut selected_plan_identities = Vec::new();
    for operator_use in checked.facts.operators.named_uses() {
        let Some(plan) = checked
            .selected_provider_plans()
            .plan_by_identity(operator_use.provider_plan_identity)
        else {
            continue;
        };
        let [row] = plan.rows.as_slice() else {
            continue;
        };
        let omega_effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. } = &row.binding
        else {
            continue;
        };
        let name = selected_intrinsic_diagnostic_label(&checked, plan);
        if !name.contains("::square_root_toward_") {
            continue;
        }
        selected_intrinsics.insert(name.clone());
        selected_plan_identities.push(plan.identity_fingerprint());
        let psi_typed_trees::expression::ExpressionNode::Call(call) = checked
            .typed
            .expression_table
            .expression(operator_use.expression)
        else {
            panic!("`{name}` must rewrite to an unnameable compiler call");
        };
        assert!(!call.receiver.is_valid());
        assert_eq!(call.arguments.count(), 1);
        assert!(call.target.as_str().starts_with("float#sqrt_toward_"));
    }
    assert_eq!(
        selected_intrinsics,
        [
            "F32::square_root_toward_negative.f32",
            "F32::square_root_toward_positive.f32",
            "F32::square_root_toward_zero.f32",
            "F64::square_root_toward_negative.f64",
            "F64::square_root_toward_positive.f64",
            "F64::square_root_toward_zero.f64",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    );
    selected_plan_identities.sort_unstable();
    selected_plan_identities.dedup();
    assert_eq!(
        selected_plan_identities.len(),
        6,
        "{DIFFERENTIAL_SUITE_ID} must bind one exact plan per format/direction slot"
    );

    let outcome = interpret(&checked, &[]);
    assert_eq!(
        outcome.exit_code, 70,
        "directed-square-root interpreter semantics must distinguish irrational-result edges"
    );

    let build_dir =
        std::env::temp_dir().join(format!("omega-directed-square-root-{}", std::process::id()));
    let _ = fs::remove_dir_all(&build_dir);
    compile(CompileOptions {
        root_path: canary.join("main.omg"),
        build_dir: Some(build_dir.clone()),
        target_name: None,
        write_output: true,
    })
    .expect("directed-square-root providers should compile natively");
    let output = Command::new(build_dir.join(executable_name()))
        .output()
        .expect("directed-square-root provider canary should run");
    assert_eq!(
        output.status.code(),
        Some(70),
        "directed square root must restore nearest-even before ordinary arithmetic; artifact: {}; stderr: {}",
        build_dir.display(),
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(&build_dir);

    for target in ["linux_x64", "linux_arm64"] {
        let scratch = std::env::temp_dir().join(format!(
            "omega-directed-square-root-{target}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&scratch);
        let source_dir = scratch.join("src");
        fs::create_dir_all(&source_dir)
            .expect("directed-square-root cross-target source directory");
        fs::copy(canary.join("main.omg"), source_dir.join("main.omg"))
            .expect("copy directed-square-root canary");
        fs::write(
            source_dir.join("build.omg"),
            hosted_main_program_entry_build(target),
        )
        .expect("write directed-square-root target manifest");
        compile(CompileOptions {
            root_path: source_dir.join("main.omg"),
            build_dir: Some(scratch.join("out")),
            target_name: Some(target.to_owned()),
            write_output: true,
        })
        .unwrap_or_else(|diagnostics| {
            panic!("directed-square-root providers should compile for {target}: {diagnostics:#?}")
        });
        let _ = fs::remove_dir_all(&scratch);
    }

    let result_identity = retained_float_differential_result_identity(
        DIFFERENTIAL_SUITE_ID,
        "macos_arm64",
        DIFFERENTIAL_COVERAGE,
        &selected_intrinsics,
        &selected_plan_identities,
        &outcome,
        &output,
        &["linux_x64", "linux_arm64"],
    );
    assert_eq!(
        result_identity, EXPECTED_DIFFERENTIAL_RESULT_IDENTITY,
        "{DIFFERENTIAL_SUITE_ID} result changed ({result_identity:#018x}); validate the exact plans, edge corpus, interpreter/native results, and cross-target builds before refreshing the retained identity"
    );
}

#[test]
fn float_policy_operator_uses_record_checked_result_adapters() {
    for (canary_name, expected) in [
        (
            "float/float_saturating_arithmetic_exit",
            psi_checked_trees::CheckedArithmeticPolicyAdapter::FloatSaturatingOverflowOnly {
                format: psi_numerics::float_semantics::FloatFormat::BINARY32,
            },
        ),
        (
            "float/float_trapping_overflow_traps",
            psi_checked_trees::CheckedArithmeticPolicyAdapter::FloatTrappingNonFinite {
                format: psi_numerics::float_semantics::FloatFormat::BINARY32,
            },
        ),
    ] {
        let canary = pass_canary(canary_name);
        let checked = omega_compiler::compile_to_checked(&canary.join("main.omg"), None)
            .unwrap_or_else(|diagnostics| {
                panic!("{canary_name} should compile to checked policy evidence: {diagnostics:?}")
            });
        assert!(
            checked
                .facts
                .operators
                .resolved_uses()
                .any(|operator_use| operator_use.policy_adapter == expected),
            "{canary_name} should retain `{expected:?}` beside its selected float operator"
        );
    }
}

#[test]
fn nested_attached_float_policy_operators_retain_selected_evidence() {
    let canary = pass_canary("arithmetic/float_saturating_overflow_exit");
    let checked = omega_compiler::compile_to_checked(&canary.join("main.omg"), None)
        .expect("nested attached-data float policy canary should check");
    let machine = checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("Main::main machine");
    let state_graph = omega_checked_trees_to_state_graph::build_state_graph(&checked)
        .expect("nested attached-data float facts should reach the state graph");
    let control_flow =
        omega_state_graph_to_control_flow::build_control_flow_plan_owned(state_graph.clone())
            .expect("nested attached-data float facts should reach control flow");

    for (state_name, statement_index) in [("negative", 0), ("nested", 0), ("nested32", 1)] {
        let state = checked
            .typed
            .machine_states(machine)
            .iter()
            .find(|state| state.name.as_str() == state_name)
            .unwrap_or_else(|| panic!("Main::main::{state_name} state"));
        let psi_typed_trees::statement::StatementNode::Assignment(assignment) = &checked
            .typed
            .statement_table
            .statements(state.statement_nodes)[statement_index]
        else {
            panic!("Main::main::{state_name} statement {statement_index} must be an assignment");
        };
        let origin = psi_checked_trees::CheckedValueOrigin::StateStatement {
            machine_symbol: machine.symbol,
            state_symbol: state.symbol,
            statement_index,
            role: psi_checked_trees::CheckedValueStatementRole::AssignmentValue,
        };
        let operator_use = checked
            .facts
            .operators
            .expression_use_in_origin(assignment.value, origin)
            .unwrap_or_else(|| {
                panic!(
                    "Main::main::{state_name} statement {statement_index} must retain its outer operator"
                )
            });
        assert!(matches!(
            operator_use.policy_adapter,
            psi_checked_trees::CheckedArithmeticPolicyAdapter::FloatSaturatingOverflowOnly { .. }
        ));
        assert_ne!(operator_use.provider_plan_identity, 0);
        assert!(
            checked
                .selected_provider_plans()
                .plan_by_identity(operator_use.provider_plan_identity)
                .is_some()
        );
        let state_key = state_graph
            .state_key_by_symbols(machine.symbol, state.symbol)
            .expect("checked state should reach the state graph");
        let state_node = state_graph
            .state_by_key(state_key)
            .expect("state graph should retain the checked state");
        let carried = state_graph
            .semantics
            .values
            .values
            .span_or_empty(state_node.values.values)
            .iter()
            .find(|value| {
                value.expression == assignment.value
                    && matches!(
                        value.origin,
                        omega_state_graph::StateValueOrigin::Statement {
                            statement_index: candidate,
                            ..
                        } if candidate == statement_index
                    )
            })
            .unwrap_or_else(|| {
                panic!(
                    "Main::main::{state_name} statement {statement_index} must reach the state-value spine"
                )
            });
        assert_eq!(
            carried.operator_provider_plan_identity,
            Some(operator_use.provider_plan_identity)
        );
        assert!(matches!(
            carried.arithmetic_policy_adapter,
            Some(
                psi_checked_trees::CheckedArithmeticPolicyAdapter::FloatSaturatingOverflowOnly { .. }
            )
        ));
        let control_state = control_flow
            .state_by_key(omega_control_flow::StateKey {
                machine: state_key.machine,
                state: state_key.state,
                segment_index: state_key.segment_index,
            })
            .expect("state graph key should reach control flow");
        let control_value = control_flow
            .semantics
            .values
            .values
            .span_or_empty(control_state.values.values)
            .iter()
            .find(|value| {
                value.expression == assignment.value
                    && matches!(
                        value.origin,
                        omega_control_flow::StateValueOrigin::Statement {
                            statement_index: candidate,
                            ..
                        } if candidate == statement_index
                    )
            })
            .unwrap_or_else(|| {
                panic!(
                    "Main::main::{state_name} statement {statement_index} must reach the control-flow value spine"
                )
            });
        assert_eq!(
            control_value.operator_provider_plan_identity,
            Some(operator_use.provider_plan_identity)
        );
        assert!(matches!(
            control_value.arithmetic_policy_adapter,
            Some(
                psi_checked_trees::CheckedArithmeticPolicyAdapter::FloatSaturatingOverflowOnly { .. }
            )
        ));
    }
}

#[test]
fn float_policy_adapters_retain_differential_results() {
    const DIFFERENTIAL_SUITE_ID: &str = "omega.float.hardware.macos_arm64.policy-adapters.v1";
    const DIFFERENTIAL_COVERAGE: &[&str] = &[
        "binary32/binary64 finite results under both adapters",
        "binary32/binary64 finite-overflow saturation",
        "nested saturation and repeated clamping",
        "division by zero remains unclamped under Saturating",
        "Trapping finite overflow",
        "Trapping division by zero and invalid operation",
        "Trapping propagated NaN and infinity",
    ];
    const EXPECTED_DIFFERENTIAL_RESULT_IDENTITY: u64 = 0x0115_beff_3918_3c7f;
    const CASES: &[(&str, Option<i32>, Option<&str>)] = &[
        ("float/runtime_policy_adapter_matrix_exit", Some(70), None),
        ("arithmetic/float_saturating_overflow_exit", Some(70), None),
        (
            "arithmetic/float_trapping_overflow_traps",
            None,
            Some("float overflow"),
        ),
        (
            "arithmetic/float_trapping_divzero_traps",
            None,
            Some("division by zero"),
        ),
        (
            "arithmetic/float_trapping_invalid_traps",
            None,
            Some("invalid float operation"),
        ),
        (
            "float/float_trapping_propagated_nan_traps",
            None,
            Some("non-finite NaN result"),
        ),
        (
            "float/float_trapping_propagated_infinity_traps",
            None,
            Some("non-finite infinity result"),
        ),
    ];

    let mut selected_evidence = std::collections::BTreeSet::new();
    let mut selected_plan_identities = Vec::new();
    let mut observations = Vec::new();
    let mut cross_builds = std::collections::BTreeSet::new();
    let case_filter = std::env::var("OMEGA_FLOAT_POLICY_CASE_FILTER").ok();
    let selected_cases = CASES
        .iter()
        .filter(|(case_name, _, _)| {
            case_filter
                .as_deref()
                .is_none_or(|filter| case_name.contains(filter))
        })
        .collect::<Vec<_>>();
    assert!(
        !selected_cases.is_empty(),
        "OMEGA_FLOAT_POLICY_CASE_FILTER selected no differential cases"
    );
    for (case_name, expected_exit, expected_error) in selected_cases.iter().copied() {
        let canary = pass_canary(case_name);
        let main_path = canary.join("main.omg");
        let checked =
            omega_compiler::compile_to_checked(&main_path, None).unwrap_or_else(|diagnostics| {
                panic!("{case_name} should compile to checked policy evidence: {diagnostics:#?}")
            });
        for operator_use in checked.facts.operators.resolved_uses() {
            let adapter = match operator_use.policy_adapter {
                psi_checked_trees::CheckedArithmeticPolicyAdapter::None => continue,
                psi_checked_trees::CheckedArithmeticPolicyAdapter::FloatSaturatingOverflowOnly {
                    format,
                } => {
                    if format == psi_numerics::float_semantics::FloatFormat::BINARY32 {
                        "saturating-overflow-only.binary32"
                    } else {
                        assert_eq!(
                            format,
                            psi_numerics::float_semantics::FloatFormat::BINARY64,
                            "policy evidence retained an unsupported float format"
                        );
                        "saturating-overflow-only.binary64"
                    }
                }
                psi_checked_trees::CheckedArithmeticPolicyAdapter::FloatTrappingNonFinite {
                    format,
                } => {
                    if format == psi_numerics::float_semantics::FloatFormat::BINARY32 {
                        "trapping-nonfinite.binary32"
                    } else {
                        assert_eq!(
                            format,
                            psi_numerics::float_semantics::FloatFormat::BINARY64,
                            "policy evidence retained an unsupported float format"
                        );
                        "trapping-nonfinite.binary64"
                    }
                }
            };
            let plan = checked
                .selected_provider_plans()
                .plan_by_identity(operator_use.provider_plan_identity)
                .expect("policy-adapted float evidence must retain its selected plan");
            let [row] = plan.rows.as_slice() else {
                panic!("policy-adapted float plan must retain one realization row");
            };
            let omega_effects::provider_plan::ProviderBinding::CompilerIntrinsic { .. } =
                &row.binding
            else {
                panic!("policy-adapted float plan must select a compiler intrinsic");
            };
            let name = selected_intrinsic_diagnostic_label(&checked, plan);
            selected_evidence.insert(format!("{name}|{adapter}"));
            selected_plan_identities.push(plan.identity_fingerprint());
        }

        let outcome = interpret(&checked, &[]);
        let build_dir = std::env::temp_dir().join(format!(
            "omega-float-policy-differential-{}-{}",
            case_name.replace('/', "-"),
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&build_dir);
        let native_compile = if matches!(
            *case_name,
            "arithmetic/float_trapping_overflow_traps"
                | "arithmetic/float_trapping_divzero_traps"
                | "arithmetic/float_trapping_invalid_traps"
                | "float/float_trapping_propagated_nan_traps"
                | "float/float_trapping_propagated_infinity_traps"
        ) {
            compile_rooted_canary_for_native_host(&canary, build_dir.clone())
        } else {
            compile(CompileOptions {
                root_path: main_path,
                build_dir: Some(build_dir.clone()),
                target_name: None,
                write_output: true,
            })
        };
        native_compile.unwrap_or_else(|diagnostics| {
            panic!("{case_name} should compile natively: {diagnostics:#?}")
        });
        let output = Command::new(build_dir.join(executable_name()))
            .output()
            .unwrap_or_else(|error| panic!("{case_name} should run natively: {error}"));
        let _ = fs::remove_dir_all(&build_dir);

        if let Some(expected_exit) = expected_exit {
            assert_eq!(
                outcome.exit_code, *expected_exit,
                "{case_name} interpreter result changed: {:?}",
                outcome.error
            );
            assert_eq!(
                output.status.code(),
                Some(*expected_exit),
                "{case_name} native result changed; stderr: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        } else {
            let error = outcome
                .error
                .as_deref()
                .unwrap_or_else(|| panic!("{case_name} interpreter should reject the result"));
            assert!(
                error.contains(expected_error.expect("trapping case error fragment")),
                "{case_name} interpreter error changed: {error}"
            );
            assert!(
                !output.status.success(),
                "{case_name} native execution reached its sailed-past sentinel"
            );
        }
        observations.push(((*case_name).to_owned(), outcome, output));

        for target in ["linux_x64", "linux_arm64"] {
            let scratch = std::env::temp_dir().join(format!(
                "omega-float-policy-differential-{}-{target}-{}",
                case_name.replace('/', "-"),
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&scratch);
            let cross_compile = if matches!(
                *case_name,
                "float/float_trapping_propagated_nan_traps"
                    | "float/float_trapping_propagated_infinity_traps"
            ) {
                compile_rooted_canary_for_target(&canary, scratch.join("out"), target)
            } else {
                let source_dir = scratch.join("src");
                fs::create_dir_all(&source_dir)
                    .expect("float-policy cross-target source directory");
                fs::copy(canary.join("main.omg"), source_dir.join("main.omg"))
                    .expect("copy float-policy canary");
                fs::write(
                    source_dir.join("build.omg"),
                    hosted_main_program_entry_build(target),
                )
                .expect("write float-policy target manifest");
                compile(CompileOptions {
                    root_path: source_dir.join("main.omg"),
                    build_dir: Some(scratch.join("out")),
                    target_name: Some(target.to_owned()),
                    write_output: true,
                })
            };
            cross_compile.unwrap_or_else(|diagnostics| {
                panic!("{case_name} should compile for {target}: {diagnostics:#?}")
            });
            let _ = fs::remove_dir_all(&scratch);
            cross_builds.insert(format!("{case_name}@{target}"));
        }
    }

    if case_filter.is_some() {
        assert_eq!(cross_builds.len(), selected_cases.len() * 2);
        return;
    }

    let expected_evidence = [
        "Float::add.f32|saturating-overflow-only.binary32",
        "Float::add.f32|trapping-nonfinite.binary32",
        "Float::add.f64|saturating-overflow-only.binary64",
        "Float::add.f64|trapping-nonfinite.binary64",
        "Float::divide.f32|saturating-overflow-only.binary32",
        "Float::divide.f32|trapping-nonfinite.binary32",
        "Float::divide.f64|saturating-overflow-only.binary64",
        "Float::divide.f64|trapping-nonfinite.binary64",
        "Float::multiply.f32|saturating-overflow-only.binary32",
        "Float::multiply.f32|trapping-nonfinite.binary32",
        "Float::multiply.f64|saturating-overflow-only.binary64",
        "Float::multiply.f64|trapping-nonfinite.binary64",
        "Float::subtract.f32|saturating-overflow-only.binary32",
        "Float::subtract.f32|trapping-nonfinite.binary32",
        "Float::subtract.f64|saturating-overflow-only.binary64",
        "Float::subtract.f64|trapping-nonfinite.binary64",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    assert_eq!(selected_evidence, expected_evidence);
    selected_plan_identities.sort_unstable();
    selected_plan_identities.dedup();
    assert_eq!(
        selected_plan_identities.len(),
        8,
        "{DIFFERENTIAL_SUITE_ID} must bind all four primitive plans in both formats"
    );
    assert_eq!(cross_builds.len(), CASES.len() * 2);

    let observation_refs = observations
        .iter()
        .map(|(label, outcome, output)| (label.as_str(), outcome, output))
        .collect::<Vec<_>>();
    let result_identity = retained_float_policy_differential_result_identity(
        DIFFERENTIAL_SUITE_ID,
        "macos_arm64",
        DIFFERENTIAL_COVERAGE,
        &selected_evidence,
        &selected_plan_identities,
        &observation_refs,
        &cross_builds,
    );
    assert_eq!(
        result_identity, EXPECTED_DIFFERENTIAL_RESULT_IDENTITY,
        "{DIFFERENTIAL_SUITE_ID} result changed ({result_identity:#018x}); validate the exact plans, adapter evidence, interpreter/native observations, and cross-target builds before refreshing the retained identity"
    );
}

// Without a declaration, mint, or signature selection, the domain meaning is
// inactive and the ordinary builtin operation stays selected.
// The evidence must say so explicitly (builtin fallback), not pretend the
// domain meaning won.
#[test]
fn domain_operator_selection_records_builtin_fallback_without_binding_selection() {
    let canary = pass_canary("domains/domain_operator_unproven_keeps_builtin_meaning");
    let checked = omega_compiler::compile_to_checked(&canary.join("main.omg"), None)
        .expect("unselected builtin fallback canary should compile to checked trees");

    let fallback_uses = checked
        .facts
        .operators
        .uses_with_status(psi_checked_trees::CheckedOperatorResolutionStatus::BuiltinFallback)
        .count();
    assert!(
        fallback_uses > 0,
        "expected the unselected `i32::Degrees` meaning to leave builtin `+` active \
         and record the use as a builtin fallback"
    );
    assert_eq!(
        checked
            .facts
            .operators
            .resolved_uses()
            .filter_map(|operator_use| checked.facts.operators.selected_candidate(operator_use))
            .filter(|candidate| candidate.is_domain_owned())
            .count(),
        0,
        "no domain-owned meaning may be selected without a declaration, mint, or signature selection"
    );
}

// Same-carrier domain theories coexist until an operand binding actually
// selects them. An unrelated or inactive declaration cannot inject a meaning
// into an existing expression.
#[test]
fn domain_operator_inactive_same_carrier_meanings_coexist() {
    let canary = pass_canary("domains/domain_operator_inactive_same_carrier_coexists");
    omega_compiler::compile_to_checked(&canary.join("main.omg"), None)
        .expect("inactive same-carrier domain meanings should coexist");
}

// When one binding statically selects both domains, both meanings participate
// in this use and the checked resolution must reject the ambiguity.
#[test]
fn domain_operator_competing_binding_meanings_fail_at_use_site() {
    let canary = fail_canary("domains/domain_operator_competing_spelling_meanings");
    let diagnostics = omega_compiler::compile_to_checked(&canary.join("main.omg"), None)
        .err()
        .expect("competing selected domain meanings should fail");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("ambiguous operator spelling `+`")
                && diagnostic.message.contains("static operand-domain tuple")
        }),
        "expected use-site operator ambiguity diagnostic, got: {diagnostics:?}"
    );
}
