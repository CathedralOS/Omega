//! Float intrinsic dispatch tests.

use super::{
    Arc, ArithmeticDomain, BuiltinFunction, CheckedTrees, CompilerIntrinsicExecutionIdentity,
    FloatFormat, NamedFloatRealization, SelectedCompilerIntrinsicExecutionIdentity,
    StagedNamedFloatExecution, StagedNamedFloatRewrite,
    derive_selected_compiler_intrinsic_execution_identity,
    settle_selected_float_intrinsic_dispatch,
};
use crate::selected_dispatch::float_intrinsic::intrinsic_resolution::{
    SelectedIntrinsicUse, resolve_selected_float_intrinsic_call,
};
use crate::selected_dispatch::float_intrinsic::named_float_realizations::preflight_named_float_execution;
use effects::provider_plan::ProviderBinding;
use provider_planning::CompilerNumericType;
use provider_planning::ProviderPlanDerivation;
use typed_trees::expression::{BinaryOperator, ExpressionNode};
use typed_trees_to_checked_trees::{ExecutionSettlement, settle_checked_execution};

const SOURCE: &str = r#"
    data F32 {}
    boundary operator F32::minimum(left: f32, right: f32) -> f32;
    boundary operator F32::maximum(left: f32, right: f32) -> f32;
    boundary operator F32::negate(value: f32) -> f32;
    boundary operator F32::from_f64(value: f64) -> f32;
    data I32 {}
    boundary operator I32::from_f64(value: f64) -> i32 in Saturating;

    data FloatProvider {}
    machine FloatProvider::minimum(left: f32, right: f32) -> f32
    satisfies F32::minimum
    via Binding::CompilerIntrinsic;
    machine FloatProvider::maximum(left: f32, right: f32) -> f32
    satisfies F32::maximum
    via Binding::CompilerIntrinsic;
    machine FloatProvider::negate(value: f32) -> f32
    satisfies F32::negate
    via Binding::CompilerIntrinsic;
    machine FloatProvider::from_f64(value: f64) -> f32
    satisfies F32::from_f64
    via Binding::CompilerIntrinsic;
    machine FloatProvider::from_f64_saturating(value: f64) -> i32 in Saturating
    satisfies I32::from_f64
    via Binding::CompilerIntrinsic;

    machine run() -> f32 {
        transition { _ -> (F32::minimum(1.0f32, 2.0f32)) }
    }
"#;

struct Fixture {
    checked: CheckedTrees,
    minimum_plan: effects::provider_plan::ProviderPlan,
    maximum_plan: effects::provider_plan::ProviderPlan,
    negate_plan: effects::provider_plan::ProviderPlan,
    conversion_plan: effects::provider_plan::ProviderPlan,
    saturating_conversion_plan: effects::provider_plan::ProviderPlan,
    operator_use: checked_trees::CheckedNamedOperatorUseFact,
}

fn fixture() -> Fixture {
    let tokens = source_files_to_tokens::Lexer::new(SOURCE)
        .tokenize()
        .expect("tokenize named-float dispatch fixture");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens)
        .expect("parse named-float dispatch fixture");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve named-float dispatch fixture");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type named-float dispatch fixture");
    let plans = provider_planning::derive_satisfies_plans(
        &typed,
        ProviderPlanDerivation::unevaluated(None),
    )
    .into_iter()
    .map(|derived| derived.plan)
    .collect::<Vec<_>>();
    let minimum_plan = plans
        .iter()
        .find(|plan| plan.schema.trait_name.contains("F32::minimum"))
        .expect("F32::minimum provider plan")
        .clone();
    let maximum_plan = plans
        .iter()
        .find(|plan| plan.schema.trait_name.contains("F32::maximum"))
        .expect("F32::maximum provider plan")
        .clone();
    let negate_plan = plans
        .iter()
        .find(|plan| plan.schema.trait_name.contains("F32::negate"))
        .expect("F32::negate provider plan")
        .clone();
    let conversion_plan = plans
        .iter()
        .find(|plan| plan.schema.trait_name.contains("F32::from_f64"))
        .expect("F32::from_f64 provider plan")
        .clone();
    let saturating_conversion_plan = plans
        .iter()
        .find(|plan| plan.schema.trait_name.contains("I32::from_f64"))
        .expect("I32::from_f64 provider plan")
        .clone();
    let checked = typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .expect("check named-float dispatch fixture");
    let operator_use = checked
        .facts
        .operators
        .named_uses
        .iter()
        .map(|(_, operator_use)| *operator_use)
        .find(|operator_use| {
            checked
                .typed
                .operators()
                .iter()
                .find(|operator| operator.symbol == operator_use.selected_operator_symbol)
                .is_some_and(|operator| {
                    checked
                        .typed
                        .operator_path_members(operator.name)
                        .iter()
                        .map(|member| member.as_str())
                        .eq(["F32", "minimum"])
                })
        })
        .expect("checked F32::minimum use");
    Fixture {
        checked,
        minimum_plan,
        maximum_plan,
        negate_plan,
        conversion_plan,
        saturating_conversion_plan,
        operator_use,
    }
}

#[derive(Clone, Copy, Debug)]
enum Drift {
    None,
    UnknownPlan,
    MissingCommitment,
    WrongCommitment,
    DuplicatePlan,
    EmptyOverload,
    CrossOperatorPlan,
    MissingRow,
    DuplicateRow,
    ReadableRow,
    WrongIntrinsic,
    NonCallExpression,
    Arity,
    SourceOperator,
    NormalizedBuiltin,
    CheckedAdapter,
}

#[test]
fn exact_intrinsic_resolver_rejects_every_identity_drift() {
    let cases = [
        (Drift::None, None),
        (
            Drift::UnknownPlan,
            Some("unknown ProviderPlan report fingerprint"),
        ),
        (
            Drift::MissingCommitment,
            Some("without an exact commitment"),
        ),
        (
            Drift::WrongCommitment,
            Some("exact commitment that does not match"),
        ),
        (Drift::DuplicatePlan, Some("match 2 selected plans")),
        (Drift::EmptyOverload, Some("does not bind exact overload")),
        (
            Drift::CrossOperatorPlan,
            Some("does not bind exact overload"),
        ),
        (Drift::MissingRow, Some("exactly one realization row")),
        (Drift::DuplicateRow, Some("exactly one realization row")),
        (Drift::ReadableRow, Some("does not bind exact overload")),
        (
            Drift::WrongIntrinsic,
            Some("does not satisfy exact overload"),
        ),
        (Drift::NonCallExpression, Some("is not a call")),
        (Drift::Arity, Some("requires 2 runtime argument")),
        (
            Drift::SourceOperator,
            Some("no longer names its checked operator symbol"),
        ),
        (Drift::NormalizedBuiltin, None),
        (Drift::CheckedAdapter, None),
    ];

    for (drift, expected_error) in cases {
        let mut fixture = fixture();
        let mut plan = fixture.minimum_plan.clone();
        let mut plans = vec![plan.clone()];
        match drift {
            Drift::None => {}
            Drift::UnknownPlan => fixture.operator_use.provider_plan_report_fingerprint = u64::MAX,
            Drift::MissingCommitment => {}
            Drift::WrongCommitment => {}
            Drift::DuplicatePlan => plans.push(plan.clone()),
            Drift::EmptyOverload => {
                plan.schema.trait_name.clear();
                plan.schema.methods[0].requirement_owner.clear();
                plan.schema.methods[0].requirement_identity.clear();
                plan.rows[0].requirement_identity.clear();
                plans = vec![plan.clone()];
            }
            Drift::CrossOperatorPlan => {
                plan = fixture.maximum_plan.clone();
                plans = vec![plan.clone()];
            }
            Drift::MissingRow => {
                plan.rows.clear();
                plans = vec![plan.clone()];
            }
            Drift::DuplicateRow => {
                plan.rows.push(plan.rows[0].clone());
                plans = vec![plan.clone()];
            }
            Drift::ReadableRow => {
                plan.rows[0].method = "minimum".into();
                plans = vec![plan.clone()];
            }
            Drift::WrongIntrinsic => {
                plan.rows[0].binding = ProviderBinding::CompilerIntrinsic {
                    machine: "F32::maximum.f32".into(),
                };
                plans = vec![plan.clone()];
            }
            Drift::NonCallExpression => {
                let ExpressionNode::Call(call) = fixture
                    .checked
                    .typed
                    .expression_table
                    .expression(fixture.operator_use.expression)
                else {
                    panic!("fixture named-float expression is not a call");
                };
                fixture.operator_use.expression = fixture
                    .checked
                    .typed
                    .expression_table
                    .expression_handles(call.arguments)[0];
            }
            Drift::Arity => {
                let ExpressionNode::Call(mut call) = fixture
                    .checked
                    .typed
                    .expression_table
                    .expression(fixture.operator_use.expression)
                    .clone()
                else {
                    panic!("fixture named-float expression is not a call");
                };
                call.arguments = arena::HandleSpan::empty();
                *fixture
                    .checked
                    .typed
                    .expression_table
                    .expression_mut(fixture.operator_use.expression) = ExpressionNode::Call(call);
            }
            Drift::SourceOperator => {
                let maximum = fixture
                    .checked
                    .typed
                    .operators()
                    .iter()
                    .find(|operator| {
                        fixture
                            .checked
                            .typed
                            .operator_path_members(operator.name)
                            .iter()
                            .map(|member| member.as_str())
                            .eq(["F32", "maximum"])
                    })
                    .expect("F32::maximum operator");
                fixture.operator_use.selected_operator_symbol = maximum.symbol;
                plan = fixture.maximum_plan.clone();
                plans = vec![plan.clone()];
            }
            Drift::NormalizedBuiltin => {
                let symbol = fixture
                    .checked
                    .typed
                    .symbols
                    .builtin_function_symbol(BuiltinFunction::Min)
                    .expect("min builtin symbol");
                let ExpressionNode::Call(mut call) = fixture
                    .checked
                    .typed
                    .expression_table
                    .expression(fixture.operator_use.expression)
                    .clone()
                else {
                    panic!("fixture named-float expression is not a call");
                };
                call.receiver = typed_trees::expression::ExpressionHandle::invalid();
                call.target = typed_trees::name::Identifier::generated("min");
                call.target_symbol = symbol;
                *fixture
                    .checked
                    .typed
                    .expression_table
                    .expression_mut(fixture.operator_use.expression) = ExpressionNode::Call(call);
            }
            Drift::CheckedAdapter => {
                plan.rows[0].binding = ProviderBinding::CheckedAdapter {
                    machine_identity: "FloatProvider::minimum".into(),
                    machine_package_identity: None,
                };
                plans = vec![plan.clone()];
            }
        }
        if !matches!(drift, Drift::UnknownPlan) {
            fixture.operator_use.provider_plan_report_fingerprint = plan.report_fingerprint();
        }
        fixture.operator_use.provider_plan_commitment = if matches!(drift, Drift::MissingCommitment)
        {
            checked_trees::CheckedProviderPlanCommitment::default()
        } else {
            checked_trees::CheckedProviderPlanCommitment::from_digest(
                if matches!(drift, Drift::WrongCommitment) {
                    [0xa5; 32]
                } else {
                    *plan.identity_digest().as_bytes()
                },
            )
        };

        let result = resolve_selected_float_intrinsic_call(
            &fixture.checked,
            &plans,
            &SelectedIntrinsicUse::from(&fixture.operator_use),
        );
        match expected_error {
            Some(expected) => {
                let diagnostic = result.expect_err("drift must fail closed");
                assert!(
                    diagnostic.message.contains(expected),
                    "{drift:?}: expected `{expected}`, got `{}`",
                    diagnostic.message,
                );
            }
            None if matches!(drift, Drift::CheckedAdapter) => {
                assert_eq!(result.expect("checked adapter remains delegated"), None);
            }
            None => {
                let rewrite = result
                    .expect("exact intrinsic resolves")
                    .expect("compiler intrinsic stages a rewrite");
                assert_eq!(rewrite.expression, fixture.operator_use.expression);
                assert!(matches!(
                    rewrite.execution,
                    StagedNamedFloatExecution::Builtin {
                        function: BuiltinFunction::Min,
                        ..
                    }
                ));
            }
        }
    }
}

#[test]
fn review_identity_uses_exact_operator_and_builtin_root_slot() {
    let fixture = fixture();
    assert_eq!(
        derive_selected_compiler_intrinsic_execution_identity(
            &fixture.checked,
            &fixture.minimum_plan,
            fixture.operator_use.selected_operator_symbol,
        )
        .expect("exact selected minimum must rederive")
        .expect("minimum is a compiler intrinsic"),
        SelectedCompilerIntrinsicExecutionIdentity::Closed(
            CompilerIntrinsicExecutionIdentity::BuiltinFunction(BuiltinFunction::Min),
        ),
    );

    let mut spoofed = fixture.minimum_plan.clone();
    spoofed.rows[0].binding = ProviderBinding::CompilerIntrinsic {
        machine: "F32::maximum.f32".into(),
    };
    let diagnostic = derive_selected_compiler_intrinsic_execution_identity(
        &fixture.checked,
        &spoofed,
        fixture.operator_use.selected_operator_symbol,
    )
    .expect_err("a cross-operator realization string must not spoof builtin identity");
    assert!(
        diagnostic
            .message
            .contains("does not satisfy exact overload")
    );

    let mut textual_negation_spoof = fixture.negate_plan.clone();
    textual_negation_spoof.rows[0].binding = ProviderBinding::CompilerIntrinsic {
        machine: "NamedFloatNegation::f32".into(),
    };
    let negate_symbol = fixture
        .checked
        .typed
        .operators()
        .iter()
        .find(|operator| {
            fixture
                .checked
                .typed
                .operator_path_members(operator.name)
                .iter()
                .map(|member| member.as_str())
                .eq(["F32", "negate"])
        })
        .expect("F32::negate operator")
        .symbol;
    let diagnostic = derive_selected_compiler_intrinsic_execution_identity(
        &fixture.checked,
        &textual_negation_spoof,
        negate_symbol,
    )
    .expect_err("an authored lookalike name cannot mint compiler negation identity");
    assert!(
        diagnostic
            .message
            .contains("does not satisfy exact overload")
    );

    assert_eq!(
        derive_selected_compiler_intrinsic_execution_identity(
            &fixture.checked,
            &fixture.negate_plan,
            negate_symbol,
        )
        .expect("exact selected negation must rederive")
        .expect("negation is a compiler intrinsic"),
        SelectedCompilerIntrinsicExecutionIdentity::Closed(
            CompilerIntrinsicExecutionIdentity::NamedFloatNegation(FloatFormat::F32),
        ),
    );

    let conversion_symbol = fixture
        .checked
        .typed
        .operators()
        .iter()
        .find(|operator| {
            fixture
                .checked
                .typed
                .operator_path_members(operator.name)
                .iter()
                .map(|member| member.as_str())
                .eq(["F32", "from_f64"])
        })
        .expect("F32::from_f64 operator")
        .symbol;
    assert_eq!(
        derive_selected_compiler_intrinsic_execution_identity(
            &fixture.checked,
            &fixture.conversion_plan,
            conversion_symbol,
        )
        .expect("exact selected conversion must rederive")
        .expect("conversion is a compiler intrinsic"),
        SelectedCompilerIntrinsicExecutionIdentity::Closed(
            CompilerIntrinsicExecutionIdentity::NamedFloatConversion {
                source: CompilerNumericType::F64,
                target: CompilerNumericType::F32,
                domain: ArithmeticDomain::Exact,
            },
        ),
    );

    let saturating_conversion_symbol = fixture
        .checked
        .typed
        .operators()
        .iter()
        .find(|operator| {
            fixture
                .checked
                .typed
                .operator_path_members(operator.name)
                .iter()
                .map(|member| member.as_str())
                .eq(["I32", "from_f64"])
        })
        .expect("I32::from_f64 operator")
        .symbol;
    assert_eq!(
        derive_selected_compiler_intrinsic_execution_identity(
            &fixture.checked,
            &fixture.saturating_conversion_plan,
            saturating_conversion_symbol,
        )
        .expect("exact selected saturating conversion must rederive")
        .expect("saturating conversion is a compiler intrinsic"),
        SelectedCompilerIntrinsicExecutionIdentity::Closed(
            CompilerIntrinsicExecutionIdentity::NamedFloatConversion {
                source: CompilerNumericType::F64,
                target: CompilerNumericType::I32,
                domain: ArithmeticDomain::Saturating,
            },
        ),
    );
}

fn selected_fixture() -> (
    Fixture,
    effects::SelectedProviderPlanFacts,
    arena::Handle<checked_trees::CheckedNamedOperatorUseFact>,
    checked_trees::CheckedNamedOperatorUseFact,
) {
    let mut fixture = fixture();
    let selected = effects::SelectedProviderPlanFacts::from_selection(
        std::slice::from_ref(&fixture.minimum_plan),
        std::slice::from_ref(&fixture.minimum_plan.name),
    )
    .expect("select exact named-float plan");
    let (handle, mut retained) = fixture
        .checked
        .facts
        .operators
        .named_uses
        .iter()
        .map(|(handle, operator_use)| (handle, *operator_use))
        .find(|(_, operator_use)| operator_use.expression == fixture.operator_use.expression)
        .expect("fixture named-float use");
    retained.provider_plan_report_fingerprint = fixture.minimum_plan.report_fingerprint();
    retained.provider_plan_commitment = checked_trees::CheckedProviderPlanCommitment::from_digest(
        *fixture.minimum_plan.identity_digest().as_bytes(),
    );
    *fixture.checked.facts.operators.named_uses.get_mut(handle) = retained;
    (fixture, selected, handle, retained)
}

#[test]
fn shared_success_clones_only_after_complete_preflight() {
    let (fixture, selected, handle, retained) = selected_fixture();
    let original_contents = fixture.checked.clone();
    let original = Arc::new(fixture.checked);
    let settled = Arc::new(original.as_ref().clone());

    let settled = settle_selected_float_intrinsic_dispatch(settled, &selected)
        .expect("exact selected intrinsic rewrites");
    assert_eq!(
        original.as_ref(),
        &original_contents,
        "successful settlement must not mutate retained shared custody"
    );
    let ExpressionNode::Call(rewritten) = settled
        .typed
        .expression_table
        .expression(retained.expression)
    else {
        panic!("rewritten intrinsic is not a call");
    };
    assert_eq!(rewritten.target.as_str(), "min");
    assert_eq!(
        rewritten.target_symbol,
        settled
            .typed
            .symbols
            .builtin_function_symbol(BuiltinFunction::Min)
            .expect("min builtin symbol"),
    );
    assert_eq!(
        settled.facts.operators.named_uses.get(handle),
        &retained,
        "execution rewrite must not change exact checked evidence",
    );
}

#[test]
fn non_builtin_execution_forms_preflight_without_publication() {
    let fixture = fixture();
    let operator = fixture
        .checked
        .typed
        .operators()
        .iter()
        .find(|operator| operator.symbol == fixture.operator_use.selected_operator_symbol)
        .expect("selected F32::minimum operator");
    let requirement =
        provider_planning::IntrinsicRequirement::from_operator(&fixture.checked.typed, operator)
            .expect("the operator is an intrinsic requirement");
    assert_eq!(
        preflight_named_float_execution(
            &fixture.checked,
            &requirement,
            NamedFloatRealization::Negate(FloatFormat::F32),
        )
        .expect("primitive negate preflights"),
        StagedNamedFloatExecution::Negate(FloatFormat::F32),
    );
    assert_eq!(
        preflight_named_float_execution(
            &fixture.checked,
            &requirement,
            NamedFloatRealization::Convert(ArithmeticDomain::Exact),
        )
        .expect("exact cast preflights"),
        StagedNamedFloatExecution::Convert {
            domain: ArithmeticDomain::Exact,
            target_type: operator.return_type,
        },
    );
}

#[test]
fn negate_settlement_appends_exactly_one_landed_literal() {
    let (fixture, _, handle, retained) = selected_fixture();
    let expression_count = fixture.checked.typed.expression_table.expression_count();
    let rewrite = StagedNamedFloatRewrite {
        expression: retained.expression,
        origin: retained.origin,
        realization: NamedFloatRealization::Negate(FloatFormat::F32),
        execution: StagedNamedFloatExecution::Negate(FloatFormat::F32),
    };
    let settled = settle_checked_execution(
        fixture.checked,
        &ExecutionSettlement {
            float_intrinsics: &[rewrite.settled_intrinsic()],
            ..ExecutionSettlement::default()
        },
    )
    .expect("a negate intrinsic settles");
    assert_eq!(
        settled.typed.expression_table.expression_count(),
        expression_count + 1,
        "negate must append exactly one landed -1 literal"
    );
    let ExpressionNode::Binary(binary) = settled
        .typed
        .expression_table
        .expression(retained.expression)
    else {
        panic!("negate publication must replace the selected root with multiplication");
    };
    assert_eq!(binary.operator, BinaryOperator::Multiply);
    assert_eq!(
        binary.right.arena_index() as usize,
        expression_count + 1,
        "the landed -1 literal must retain exact append order"
    );
    let ExpressionNode::Float(negative_one) =
        settled.typed.expression_table.expression(binary.right)
    else {
        panic!("negate publication must append a float literal");
    };
    assert_eq!(negative_one.text(), "-1.0");
    assert_eq!(negative_one.landing(), Some(FloatFormat::F32));
    assert_eq!(
        settled.facts.operators.named_uses.get(handle),
        &retained,
        "arena publication must not change exact checked evidence"
    );
}

#[test]
fn one_invalid_intrinsic_use_rejects_the_complete_batch() {
    let (mut fixture, selected, _, retained) = selected_fixture();
    let mut invalid = retained;
    invalid.provider_plan_report_fingerprint = u64::MAX;
    fixture.checked.facts.operators.named_uses.append(invalid);
    let diagnostics =
        settle_selected_float_intrinsic_dispatch(Arc::new(fixture.checked), &selected)
            .expect_err("one invalid intrinsic use rejects the complete rewrite batch");
    assert!(
        diagnostics[0]
            .message
            .contains("unknown ProviderPlan report fingerprint")
    );
}

#[test]
fn empty_settlement_returns_the_program_unchanged() {
    let fixture = fixture();
    let original_contents = fixture.checked.clone();
    let settled = settle_selected_float_intrinsic_dispatch(
        Arc::new(fixture.checked),
        &effects::SelectedProviderPlanFacts::default(),
    )
    .expect("a program without selected float intrinsics is already settled");
    assert_eq!(settled.as_ref(), &original_contents);
}
