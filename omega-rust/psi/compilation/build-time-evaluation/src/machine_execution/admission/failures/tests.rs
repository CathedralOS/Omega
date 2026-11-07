use super::*;

fn typed(source: &str) -> typed_trees::TypedTrees {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("lex");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("parse");
    let mut sources = source::SourceMap::default();
    sources.add_with_metadata(
        "constant.omg".into(),
        source.into(),
        ".".into(),
        semantic_vocabulary::PackageKeyIdentity::from_digest([1; 32]),
        source::SourceOrigin::User,
    );
    let mut request = syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax);
    request.sources = Some(std::sync::Arc::new(sources));
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(request).expect("resolve");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).expect("type")
}

fn on_compilation_stack(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(test)
        .expect("start compilation test")
        .join()
        .expect("compilation test");
}

#[test]
fn source_admission_proves_guarded_actuals_before_execution() {
    on_compilation_stack(|| {
        let program = typed(
            r#"
            machine divide(numerator: u64, denominator: u64) -> u64
            crashes Trap denominator == 0u64 {
                transition denominator == 0u64 {
                    true -> bad()
                    false -> (numerator / denominator)
                }
                machine bad() -> u64 crashes Trap { crash Trap; }
            }
        "#,
        );
        let admission = BuildTimeAdmissionPlan::infer(&program, None);
        let value = admission
            .evaluate_machine(
                BuildTimeProgram::Typed(&program),
                "divide",
                vec![BuildTimeValue::Int(14), BuildTimeValue::Int(2)],
            )
            .expect("a nonzero concrete denominator removes the route");
        assert_eq!(value, BuildTimeValue::Int(7));
        let error = admission
            .evaluate_machine(
                BuildTimeProgram::Typed(&program),
                "divide",
                vec![BuildTimeValue::Int(14), BuildTimeValue::Int(0)],
            )
            .expect_err("a surviving route must refuse before interpretation");
        assert!(
            error.reason.contains("not build-time admissible"),
            "{error:?}"
        );
        assert!(error.reason.contains("unhandled [Trap]"), "{error:?}");
        assert!(error.reason.contains("before execution"), "{error:?}");
    });
}

#[test]
fn source_failure_summary_distinguishes_unknown_from_proved_empty() {
    on_compilation_stack(|| {
        let (program, preparation) = typed_trees_to_typed_trees::prepare_typed_trees(
            typed("machine answer() -> u64 { 7 }"),
            &typed_trees_to_typed_trees::PreparationRequest::settled(),
        )
        .expect("prepare");
        let mut checked = typed_trees_to_checked_trees::lower_typed_trees(
            program,
            preparation,
            &typed_trees_to_checked_trees::CheckingRequest::new(),
        )
        .expect("check");
        let owner = checked
            .typed
            .realized_machine_named("answer")
            .expect("answer")
            .symbol;
        assert!(checked.body_crash_summary(owner).is_some());
        concrete_invocation::require_empty_failure_summary(&checked, owner)
            .expect("ordinary checking proves the complete empty result");
        let plan = checked
            .facts
            .contract_plans
            .machines_mut()
            .iter_mut()
            .find(|plan| plan.machine == owner)
            .expect("exact plan");
        plan.crash = plan.crash.clone().with_checked_body_summary(None);
        assert!(checked.body_crash_summary(owner).is_none());
        let error = concrete_invocation::require_empty_failure_summary(&checked, owner)
            .expect_err("absence cannot grant no-failure evidence");
        assert!(
            error.contains("no complete checked failure summary"),
            "{error}"
        );
    });
}

// Keep the actual sealed declaration and its complete source custody. The test
// needs only the opaque target carrier, not the other layout library APIs.
fn typed_with_layout_target(source: &str) -> typed_trees::TypedTrees {
    let core = std::str::from_utf8(language_core::layout_storage::SOURCE).expect("core UTF-8");
    let mut sources = source::SourceMap::default();
    let core_id = sources
        .add_with_metadata(
            "/compiler/core/layout.omg".into(),
            core.into(),
            "/compiler/core".into(),
            None,
            source::SourceOrigin::Toolchain,
        )
        .source_id;
    let user_id = sources
        .add_with_metadata(
            "policy.omg".into(),
            source.into(),
            ".".into(),
            semantic_vocabulary::PackageKeyIdentity::from_digest([1; 32]),
            source::SourceOrigin::User,
        )
        .source_id;
    let tokens = source_files_to_tokens::Lexer::new(core)
        .tokenize()
        .expect("core lex");
    let mut syntax =
        tokens_to_syntax_trees::parse_syntax_trees_with_id(core_id, &tokens).expect("core parse");
    let roots = syntax
        .root_item_handles()
        .iter()
        .copied()
        .filter(|root| {
            matches!(syntax.root_item(*root), syntax_trees::item::Item::Data(data)
            if data.name.as_str(&syntax) == "LayoutTarget")
        })
        .collect::<Vec<_>>();
    assert_eq!(roots.len(), 1, "the actual sealed carrier is unique");
    syntax.roots.items = roots;
    syntax.roots.mathematical_definitions.clear();
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("policy lex");
    tokens_to_syntax_trees::parse(&mut syntax, user_id, &tokens).expect("policy parse");
    let mut request = syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax);
    request.sources = Some(std::sync::Arc::new(sources));
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(request).expect("policy resolve");
    symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("policy type")
}

#[test]
fn source_admission_checks_guarded_policy_with_real_target_argument() {
    on_compilation_stack(|| {
        let program = typed_with_layout_target(
            r#"
            machine plan(target: LayoutTarget, denominator: u64) -> u64
            crashes Trap denominator == 0u64 {
                transition denominator == 0u64 {
                    true -> bad()
                    false -> (7u64 / denominator)
                }
                machine bad() -> u64 crashes Trap { crash Trap; }
            }
        "#,
        );
        let target = layout_plans::LayoutTargetFacts {
            profile: "macos_arm64".into(),
            pointer_size: 8,
            pointer_alignment: 8,
            case_tag_size: 4,
            case_tag_alignment: 4,
            little_endian: true,
        };
        let admission = BuildTimeAdmissionPlan::infer(&program, None);
        let value = admission
            .evaluate_machine(
                BuildTimeProgram::Typed(&program),
                "plan",
                vec![
                    BuildTimeValue::LayoutTarget(target.clone()),
                    BuildTimeValue::Int(1),
                ],
            )
            .expect("a real opaque target does not prevent concrete guard discharge");
        assert_eq!(value, BuildTimeValue::Int(7));
        let error = admission
            .evaluate_machine(
                BuildTimeProgram::Typed(&program),
                "plan",
                vec![BuildTimeValue::LayoutTarget(target), BuildTimeValue::Int(0)],
            )
            .expect_err("the same target with a zero actual must refuse before execution");
        assert!(error.reason.contains("unhandled [Trap]"), "{error:?}");
        assert!(error.reason.contains("before execution"), "{error:?}");
    });
}

#[test]
fn source_admission_establishes_host_parameter_domains_before_execution() {
    on_compilation_stack(|| {
        let (program, preparation) = typed_trees_to_typed_trees::prepare_typed_trees(
            typed(
                r#"
            domain u64::NonZero requires self != 0;
            machine divide(denominator: u64 in NonZero) -> u64 { 7u64 / denominator }
        "#,
            ),
            &typed_trees_to_typed_trees::PreparationRequest::settled(),
        )
        .expect("prepare");
        let checked = typed_trees_to_checked_trees::lower_typed_trees(
            program,
            preparation,
            &typed_trees_to_checked_trees::CheckingRequest::new(),
        )
        .expect("check");
        let program = checked.typed.as_ref();
        let symbol = program
            .realized_machine_named("divide")
            .expect("divide")
            .symbol;
        concrete_invocation::require_empty_failure_summary(&checked, symbol)
            .expect("the body is crash-free under its declared parameter domain");
        let admission = BuildTimeAdmissionPlan::infer(program, None);
        let value = admission
            .evaluate_machine(
                BuildTimeProgram::Checked(&checked),
                "divide",
                vec![BuildTimeValue::Int(1)],
            )
            .expect("a host actual in the declared domain is established");
        assert_eq!(value, BuildTimeValue::Int(7));
        let value = admission
            .evaluate_machine_symbol_for_invocation_measured(
                BuildTimeProgram::Checked(&checked),
                symbol,
                vec![BuildTimeValue::Int(1)],
                BuildTimeInvocationCustody::Symbol(symbol),
            )
            .expect("the symbol entry establishes the same valid host domain")
            .into_parts()
            .0;
        assert_eq!(value, BuildTimeValue::Int(7));
        let error = admission
            .evaluate_machine(
                BuildTimeProgram::Checked(&checked),
                "divide",
                vec![BuildTimeValue::Int(0)],
            )
            .expect_err("a body summary cannot establish its host argument domain");
        assert!(
            error
                .reason
                .contains("constant invocation failed ordinary checking"),
            "{error:?}"
        );
        assert!(error.reason.contains("requires"), "{error:?}");
        assert!(
            error.reason.contains("not build-time admissible"),
            "{error:?}"
        );
        assert!(error.reason.contains("before execution"), "{error:?}");
        let error = admission
            .evaluate_machine_symbol_for_invocation_measured(
                BuildTimeProgram::Checked(&checked),
                symbol,
                vec![BuildTimeValue::Int(0)],
                BuildTimeInvocationCustody::Symbol(symbol),
            )
            .expect_err("the symbol entry must establish the same host domain");
        assert!(
            error.contains("constant invocation failed ordinary checking"),
            "{error}"
        );
        assert!(error.contains("requires"), "{error}");
        assert!(error.contains("before execution"), "{error}");
    });
}

#[test]
fn source_empty_failure_summary_does_not_discharge_authored_requires() {
    on_compilation_stack(|| {
        let (program, preparation) = typed_trees_to_typed_trees::prepare_typed_trees(
            typed("machine answer(value: u64) -> u64 requires value != 0 { 7 }"),
            &typed_trees_to_typed_trees::PreparationRequest::settled(),
        )
        .expect("prepare");
        let checked = typed_trees_to_checked_trees::lower_typed_trees(
            program,
            preparation,
            &typed_trees_to_checked_trees::CheckingRequest::new(),
        )
        .expect("check");
        let program = checked.typed.as_ref();
        let symbol = program
            .realized_machine_named("answer")
            .expect("answer")
            .symbol;
        concrete_invocation::require_empty_failure_summary(&checked, symbol)
            .expect("the constant body has an empty failure summary");
        let admission = BuildTimeAdmissionPlan::infer(program, None);
        let value = admission
            .evaluate_machine(
                BuildTimeProgram::Checked(&checked),
                "answer",
                vec![BuildTimeValue::Int(1)],
            )
            .expect("the exact actual establishes the authored entry requirement");
        assert_eq!(value, BuildTimeValue::Int(7));
        let error = admission
            .evaluate_machine(
                BuildTimeProgram::Checked(&checked),
                "answer",
                vec![BuildTimeValue::Int(0)],
            )
            .expect_err("a crash-free body cannot grant its invocation precondition");
        assert!(
            error
                .reason
                .contains("constant invocation failed ordinary checking"),
            "{error:?}"
        );
        assert!(error.reason.contains("requires"), "{error:?}");
        assert!(error.reason.contains("before execution"), "{error:?}");
    });
}
