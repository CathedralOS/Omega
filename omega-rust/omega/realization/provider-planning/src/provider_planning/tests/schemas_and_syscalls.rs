use super::{derive_provider_fixture, normalized_machine_identity, selection_plan};
use crate::ProviderPlanDerivation;
use crate::provider_planning::{
    ProviderBinding, ProviderPlanRow, TypedTrees, derive_satisfies_plans,
    exact_canonical_provider_schema, validate_provider_plan_candidates,
};

#[test]
fn table_field_leaf_requires_an_attached_layout_owner() {
    let mut plan = selection_plan("field-leaf", &["first"], &[]);
    plan.provider_type.clear();
    plan.rows.push(ProviderPlanRow {
        method: "first".to_owned(),
        requirement_identity: "Pair::first".to_owned(),
        requirement_lifetime_partition: Vec::new(),
        binding: ProviderBinding::VtableField {
            table: String::new(),
            field: "first".to_owned(),
        },
    });

    let diagnostics = validate_provider_plan_candidates(&TypedTrees::default(), &[plan]);

    assert_eq!(diagnostics.len(), 1);
    assert!(
        diagnostics[0]
            .message
            .contains("without an attached provider data type")
    );
}

#[test]
fn checked_adapter_requires_a_nominal_provider_type() {
    let mut plan = selection_plan("free-adapter", &["first"], &[]);
    plan.provider_type.clear();
    plan.rows.push(ProviderPlanRow {
        method: "first".to_owned(),
        requirement_identity: "Pair::first".to_owned(),
        requirement_lifetime_partition: Vec::new(),
        binding: ProviderBinding::CheckedAdapter {
            machine_identity: "first_adapter".to_owned(),
            machine_package_identity: None,
        },
    });

    let diagnostics = validate_provider_plan_candidates(&TypedTrees::default(), &[plan]);

    // Candidate-shape and typed-resolution validation remain cumulative:
    // the impossible free adapter has neither a nominal owner nor a typed
    // machine that could supply one.
    assert_eq!(diagnostics.len(), 2);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("has no nominal provider type"))
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("is absent from typed machines"))
    );
}

#[test]
fn checked_adapter_must_resolve_to_its_exact_checked_provider_conformance() {
    let source = r#"
        boundary trait Readable {
            machine read() -> i32;
        }

        boundary trait OtherBoundary {
            machine other() -> i32;
        }

        data Provider {}
        data OtherProvider {}

        machine Provider::read() -> i32 satisfies Readable::read { 1 }
        machine Provider::helper() -> i32 { 2 }
        machine OtherProvider::helper() -> i32 { 3 }
        machine Provider::external() -> i32
        satisfies OtherBoundary::other
        via ForeignBinding::CompilerIntrinsic;
    "#;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize adapter ownership fixture");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens)
        .expect("parse adapter ownership fixture");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve adapter ownership fixture");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type adapter ownership fixture");
    let plan = derive_satisfies_plans(&typed, ProviderPlanDerivation::unevaluated(None))
        .into_iter()
        .map(|derived| derived.plan)
        .collect::<Vec<_>>()
        .into_iter()
        .find(|plan| plan.schema.trait_name == "Readable")
        .expect("Readable provider plan");
    assert!(
        validate_provider_plan_candidates(&typed, std::slice::from_ref(&plan)).is_empty(),
        "the exact checked provider conformance remains valid"
    );

    let mut absent = plan.clone();
    absent.rows[0].binding = ProviderBinding::CheckedAdapter {
        machine_identity: "Provider::absent".to_owned(),
        machine_package_identity: None,
    };
    assert!(
        validate_provider_plan_candidates(&typed, &[absent])
            .iter()
            .any(|diagnostic| diagnostic.message.contains("is absent from typed machines"))
    );

    let mut wrong_provider = plan.clone();
    wrong_provider.rows[0].binding = ProviderBinding::CheckedAdapter {
        machine_identity: normalized_machine_identity(&typed, "OtherProvider::helper"),
        machine_package_identity: None,
    };
    assert!(
        validate_provider_plan_candidates(&typed, &[wrong_provider])
            .iter()
            .any(|diagnostic| diagnostic
                .message
                .contains("belongs to provider `OtherProvider`, not selected provider `Provider`"))
    );

    let mut external = plan.clone();
    external.rows[0].binding = ProviderBinding::CheckedAdapter {
        machine_identity: normalized_machine_identity(&typed, "Provider::external"),
        machine_package_identity: None,
    };
    assert!(
        validate_provider_plan_candidates(&typed, &[external])
            .iter()
            .any(|diagnostic| diagnostic
                .message
                .contains("does not name a checked body with an entry state"))
    );

    let mut unrelated = plan;
    unrelated.rows[0].binding = ProviderBinding::CheckedAdapter {
        machine_identity: normalized_machine_identity(&typed, "Provider::helper"),
        machine_package_identity: None,
    };
    assert!(
        validate_provider_plan_candidates(&typed, &[unrelated])
            .iter()
            .any(|diagnostic| diagnostic
                .message
                .contains("has no exact checked satisfies edge for requirement identity"))
    );
}

#[test]
fn checked_operator_adapter_must_resolve_to_its_exact_operator_conformance() {
    let source = r#"
        data CheckedMath {}
        boundary operator CheckedMath::offset_zero(value: i32) -> i32;

        data OtherMath {}
        boundary operator OtherMath::offset_zero(value: i32) -> i32;

        data CheckedMathProvider {}
        machine CheckedMathProvider::offset_zero_impl(input: i32) -> i32
        satisfies CheckedMath::offset_zero
        {
            transition { _ -> (input) }
        }
        machine CheckedMathProvider::decoy_impl(input: i32) -> i32
        satisfies OtherMath::offset_zero
        {
            transition { _ -> (input) }
        }
        machine CheckedMathProvider::wrong_signature(input: u64) -> u64
        satisfies CheckedMath::offset_zero
        {
            transition { _ -> (input) }
        }
    "#;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize checked operator adapter fixture");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens)
        .expect("parse checked operator adapter fixture");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve checked operator adapter fixture");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type checked operator adapter fixture");
    let operator = typed
        .operators()
        .iter()
        .find(|operator| {
            typed
                .operator_path_members(operator.name)
                .iter()
                .map(|member| member.as_str())
                .eq(["CheckedMath", "offset_zero"])
        })
        .expect("CheckedMath::offset_zero operator");
    let identity = typed_trees::operator::boundary_operator_requirement_identity(&typed, operator);
    let plan = derive_satisfies_plans(&typed, ProviderPlanDerivation::unevaluated(None))
        .into_iter()
        .map(|derived| derived.plan)
        .collect::<Vec<_>>()
        .into_iter()
        .find(|plan| plan.schema.trait_name == identity)
        .expect("CheckedMath::offset_zero provider plan");
    assert!(
        validate_provider_plan_candidates(&typed, std::slice::from_ref(&plan)).is_empty(),
        "the exact checked operator conformance remains valid"
    );

    for unrelated in [
        "CheckedMathProvider::decoy_impl",
        "CheckedMathProvider::wrong_signature",
    ] {
        let mut invalid = plan.clone();
        invalid.rows[0].binding = ProviderBinding::CheckedAdapter {
            machine_identity: normalized_machine_identity(&typed, unrelated),
            machine_package_identity: None,
        };
        assert!(
            validate_provider_plan_candidates(&typed, &[invalid])
                .iter()
                .any(|diagnostic| diagnostic
                    .message
                    .contains("has no exact checked satisfies edge for requirement identity")),
            "operator adapter `{unrelated}` must not satisfy the exact operator row"
        );
    }
}

#[test]
fn syscall_derivation_retains_exact_number_before_range_validation() {
    fn derive(number: i64) -> (TypedTrees, Vec<effects::provider_plan::ProviderPlan>) {
        let source = format!(
            r#"
                boundary trait Process {{
                    machine exit(code: i32);
                }}

                machine exit_leaf(code: i32)
                satisfies Process::exit
                via ForeignBinding::Syscall({number});
            "#
        );
        let tokens = source_files_to_tokens::Lexer::new(&source)
            .tokenize()
            .expect("tokenize syscall leaf");
        let syntax =
            tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse syscall leaf");
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
        )
        .expect("resolve syscall leaf");
        let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("type syscall leaf");
        let plans = derive_satisfies_plans(&typed, ProviderPlanDerivation::unevaluated(None))
            .into_iter()
            .map(|derived| derived.plan)
            .collect::<Vec<_>>();
        (typed, plans)
    }

    let maximum = i64::from(u32::MAX);
    let (typed, plans) = derive(maximum);
    let ProviderBinding::Syscall { number } = &plans[0].rows[0].binding else {
        panic!("source syscall leaf must retain a syscall binding");
    };
    assert_eq!(*number, maximum);
    assert!(validate_provider_plan_candidates(&typed, &plans).is_empty());

    let oversized = maximum + 1;
    let (typed, plans) = derive(oversized);
    let ProviderBinding::Syscall { number } = &plans[0].rows[0].binding else {
        panic!("source syscall leaf must retain a syscall binding");
    };
    assert_eq!(*number, oversized);
    assert_ne!(*number, 0, "oversized syscall must not normalize to zero");
    let diagnostics = validate_provider_plan_candidates(&typed, &plans);
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("target syscall plan requires a value in 0..=4294967295")
    }));
}

#[test]
fn checked_adapter_rejects_symbol_resolved_service_widening() {
    let source = r#"
        boundary trait Queryable {
            machine query();
        }

        boundary trait Readable {
            machine read(queryable: &mut Queryable);
        }

        data Provider {}

        machine Provider::read(queryable: &mut Queryable)
        satisfies Readable::read {
            queryable.query();
        }
    "#;
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse provider");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve provider");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type provider");
    let plans = derive_satisfies_plans(&typed, ProviderPlanDerivation::unevaluated(None))
        .into_iter()
        .map(|derived| derived.plan)
        .collect::<Vec<_>>();

    let diagnostics = validate_provider_plan_candidates(&typed, &plans);

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("boundary service(s) [Queryable]")
            && diagnostic
                .message
                .contains("declared service ceiling [Readable]")
    }));
}

#[test]
fn provider_candidate_requires_exact_canonical_typed_schema() {
    #[derive(Clone, Copy, Debug)]
    enum Drift {
        Method,
        RequirementOwner,
        RequirementIdentity,
        ParameterShape,
        EntryClaim,
        ResultShape,
        ResultClaim,
        ServiceReach,
        SynchronousInvocation,
        Suspension,
        Blocking,
        Termination,
        CallingPlan,
    }

    let source = r#"
        boundary trait Readable {
            machine read();
        }

        data Provider {}

        machine Provider::read()
        satisfies Readable::read {}
    "#;
    let (typed, plan) = derive_provider_fixture(source);
    assert!(validate_provider_plan_candidates(&typed, std::slice::from_ref(&plan)).is_empty());

    for drift in [
        Drift::Method,
        Drift::RequirementOwner,
        Drift::RequirementIdentity,
        Drift::ParameterShape,
        Drift::EntryClaim,
        Drift::ResultShape,
        Drift::ResultClaim,
        Drift::ServiceReach,
        Drift::SynchronousInvocation,
        Drift::Suspension,
        Drift::Blocking,
        Drift::Termination,
        Drift::CallingPlan,
    ] {
        let mut drifted = plan.clone();
        let method = &mut drifted.schema.methods[0];
        match drift {
            Drift::Method => {
                method.name = "other".to_owned();
                drifted.rows[0].method = method.name.clone();
            }
            Drift::RequirementOwner => method.requirement_owner = "Other".to_owned(),
            Drift::RequirementIdentity => {
                method.requirement_identity = "Other::read()".to_owned();
                drifted.rows[0].requirement_identity = method.requirement_identity.clone();
            }
            Drift::ParameterShape => {
                method.parameter_count = 1;
                method.parameter_type_identities = vec!["i32".to_owned()];
            }
            Drift::EntryClaim => {
                method.parameter_count = 1;
                method.parameter_type_identities = vec!["i32 in Accepted".to_owned()];
                method.entry_claims = vec![effects::provider_plan::ServiceEntryClaim {
                    parameter_index: 0,
                    carrier_identity: "named(name(Token))".to_owned(),
                    domain: "Accepted".to_owned(),
                    predicate_body: language_semantics::DomainPredicateBody::Bodyless,
                    effective_carry: language_semantics::CarryPolicy::STRICT,
                    authority_flow: effects::provider_plan::ServiceEntryAuthorityFlow::Accepts,
                }];
            }
            Drift::ResultShape => {
                method.has_result = true;
                method.result_type_identity = Some("i32".to_owned());
            }
            Drift::ResultClaim => {
                method.has_result = true;
                method.result_type_identity = Some("i32 in Returned".to_owned());
                method.result_claims = vec![effects::provider_plan::ServiceResultClaim {
                    domain: "Returned".to_owned(),
                    effective_carry: language_semantics::CarryPolicy::STRICT,
                }];
            }
            Drift::ServiceReach => {
                method.service_reach.push("Writable".to_owned());
                method.service_reach.sort_unstable();
            }
            Drift::SynchronousInvocation => {
                method.synchronous_invocations.push("Writable".to_owned())
            }
            Drift::Suspension => method.may_suspend = true,
            Drift::Blocking => method.may_block = true,
            Drift::Termination => method.terminates_guarantee = true,
            Drift::CallingPlan => {
                method.calling_plan_report_fingerprint = Some(1);
                method.calling_plan_commitment = Some(
                    typed_trees::typed_trees::BoundaryCallingPlanCommitment::from_digest([1; 32]),
                );
            }
        }

        let diagnostics = validate_provider_plan_candidates(&typed, &[drifted]);
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("does not equal its exact canonical typed schema")),
            "{drift:?} must fail canonical typed schema custody: {diagnostics:?}",
        );
    }
}

#[test]
fn forged_service_ceiling_cannot_launder_checked_adapter_reach() {
    let source = r#"
        boundary trait Queryable {
            machine query();
        }

        boundary trait Readable {
            machine read(queryable: &mut Queryable);
        }

        data Provider {}

        machine Provider::read(queryable: &mut Queryable)
        satisfies Readable::read {
            queryable.query();
        }
    "#;
    let (typed, mut plan) = derive_provider_fixture(source);
    plan.schema.methods[0]
        .service_reach
        .push("Queryable".to_owned());
    plan.schema.methods[0].service_reach.sort_unstable();
    plan.schema.methods[0].service_reach.dedup();

    let diagnostics = validate_provider_plan_candidates(&typed, &[plan]);
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("does not equal its exact canonical typed schema")
    }));
    assert!(!diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("boundary service(s) [Queryable] outside")
    }));
}

#[test]
fn canonical_schema_resolution_is_exact_and_unique() {
    let source = r#"
        boundary trait Readable {
            machine read();
        }

        boundary trait Unrelated {
            machine inspect();
        }

        data Provider {}

        machine Provider::read()
        satisfies Readable::read {}
    "#;
    let (typed, plan) = derive_provider_fixture(source);
    assert_eq!(
        exact_canonical_provider_schema(&typed, &plan).expect("exact schema"),
        plan.schema,
    );

    for identity in ["Missing", "pkg::Readable"] {
        let mut drifted = plan.clone();
        drifted.schema.trait_name = identity.to_owned();
        let diagnostic = exact_canonical_provider_schema(&typed, &drifted)
            .expect_err("unknown and qualified-leaf impostor schemas must reject");
        assert!(diagnostic.message.contains("resolves to 0 canonical typed"));
    }

    let mut duplicated = typed.clone();
    let duplicate = duplicated
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Readable")
        .expect("Readable trait")
        .clone();
    duplicated.push_trait_definition(duplicate);
    let diagnostic = exact_canonical_provider_schema(&duplicated, &plan)
        .expect_err("duplicate exact schema authority must reject");
    assert!(
        diagnostic
            .message
            .contains("resolves to 2 canonical typed boundary traits")
    );
}

#[test]
fn canonical_schema_accepts_exact_inherited_requirement() {
    let source = r#"
        boundary trait Parent {
            machine read();
        }

        boundary trait Child {
            requires Parent;
        }

        data Provider {}

        ProviderChild: Provider satisfies Child;

        machine Provider::read()
        satisfies Parent::read {}
    "#;
    let (typed, mut plan) = derive_provider_fixture(source);
    let child = typed
        .traits()
        .iter()
        .find(|definition| definition.name.as_str() == "Child")
        .expect("Child boundary schema");
    plan.schema = crate::service_schema::from_typed(&typed, child).expect("typed child schema");

    assert!(
        validate_provider_plan_candidates(&typed, &[plan]).is_empty(),
        "an exact child schema may retain its inherited parent requirement",
    );
}

#[test]
fn canonical_schema_rejects_duplicate_exact_carrier_arguments() {
    let source = r#"
        boundary trait Readable {
            machine read(&mut self);
        }

        data Provider {}

        ProviderReadable: Provider satisfies Readable;

        machine Provider::read(&mut self)
        satisfies Readable::read {}
    "#;
    let (mut typed, plan) = derive_provider_fixture(source);
    assert_eq!(typed.conformances().len(), 1);
    let duplicate = typed.conformances()[0].clone();
    typed.push_conformance(duplicate);

    let diagnostic = exact_canonical_provider_schema(&typed, &plan)
        .expect_err("duplicate carrier argument custody must reject");
    assert!(
        diagnostic
            .message
            .contains("resolves to 2 exact carrier argument rows")
    );
}

#[test]
fn canonical_schema_rejects_same_spelled_declarations_from_a_foreign_package() {
    // Boundary-trait and boundary-operator canonical identities are both
    // package-blind. A schema row carrying another package's identity must not
    // rejoin to the same-spelled declaration retained by this package.
    let foreign_package = semantic_vocabulary::PackageKeyIdentity::from_digest([0x99; 32])
        .expect("nonzero package identity");

    let trait_source = r#"
        boundary trait Readable {
            machine read();
        }

        data Provider {}

        machine Provider::read()
        satisfies Readable::read {}
    "#;
    let (typed, plan) = derive_provider_fixture(trait_source);
    let mut drifted = plan.clone();
    drifted.schema.trait_package_identity = Some(foreign_package);
    let diagnostic = exact_canonical_provider_schema(&typed, &drifted)
        .expect_err("a same-spelled trait schema from another package must reject");
    assert!(
        diagnostic.message.contains("resolves to 0 canonical typed"),
        "expected a zero-match canonical diagnostic, got {diagnostic:?}"
    );
    let diagnostics = validate_provider_plan_candidates(&typed, &[drifted]);
    assert!(
        !diagnostics.is_empty(),
        "candidate validation must not replay a foreign-package schema"
    );

    let operator_source = r#"
        data CheckedMath {}
        boundary operator CheckedMath::offset_zero(value: i32) -> i32;

        data CheckedMathProvider {}
        machine CheckedMathProvider::offset_zero_impl(input: i32) -> i32
        satisfies CheckedMath::offset_zero
        {
            transition { _ -> (input) }
        }
    "#;
    let (typed, plan) = derive_provider_fixture(operator_source);
    let mut drifted = plan.clone();
    drifted.schema.trait_package_identity = Some(foreign_package);
    let diagnostic = exact_canonical_provider_schema(&typed, &drifted)
        .expect_err("a same-spelled operator schema from another package must reject");
    assert!(
        diagnostic.message.contains("resolves to 0 canonical typed"),
        "expected a zero-match canonical diagnostic, got {diagnostic:?}"
    );
    let diagnostics = validate_provider_plan_candidates(&typed, &[drifted]);
    assert!(
        !diagnostics.is_empty(),
        "candidate validation must not replay a foreign-package operator schema"
    );
}
