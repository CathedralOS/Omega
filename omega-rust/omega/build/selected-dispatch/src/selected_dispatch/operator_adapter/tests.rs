//! Operator adapter tests: terminal custody validation, adapter rewrites and resolution.

use super::{
    CheckedOperatorAuthoredUseKind, CheckedTrees, CheckedUnitEffectOperationPlan,
    CheckedValueOrigin, CheckedValueStatementRole, ExpressionNode, ProviderBinding,
    derive_checked_nongeneric_operator_application_realizations,
    resolve_selected_operator_adapter_call, validate_selected_operator_terminal_custody,
};
use crate::settle_selected_execution_dispatch;
use provider_planning::ProviderPlanDerivation;
use std::sync::Arc;

#[test]
fn selected_operator_crash_invocations_reject_terminal_custody() {
    for (operator_contract, caller_contract) in [
        ("crashes Trap", "crashes Trap"),
        ("crashes Abort", "crashes Abort"),
        ("crashes Trap false", ""),
    ] {
        let source = format!(
            "boundary operator == Comparison::equal(left: i32, right: i32) -> bool {operator_contract};
                 pub machine compare(left: i32, right: i32) -> bool {caller_contract} {{ left == right }}"
        );
        let tokens = source_files_to_tokens::Lexer::new(&source)
            .tokenize()
            .unwrap();
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
        )
        .unwrap();
        let typed =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        let checked = typed_trees_to_checked_trees::lower_typed_trees(
            typed,
            &typed_trees_to_checked_trees::CheckingRequest::settled(),
        )
        .expect("matched ceiling or false route must pass source checking");
        let diagnostics = validate_selected_operator_terminal_custody(
            &checked,
            &effects::SelectedProviderPlanFacts::default(),
        )
        .unwrap_err();
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic
                .message
                .contains("selected operator crash invocations have no Terminal replay support")),
            "{diagnostics:#?}"
        );
    }
}

const SOURCE: &str = r#"
        data CheckedMath {}
        boundary operator CheckedMath::offset_zero(value: i32) -> i32
        requires value == value
        ensures result == value + 0 && value == value;

        data OtherMath {}
        boundary operator OtherMath::offset_zero(value: i32) -> i32
        requires value == value
        ensures result == value + 0 && value == value;

        data CheckedMathProvider {}
        machine CheckedMathProvider::offset_zero_impl(input: i32) -> i32
        satisfies CheckedMath::offset_zero
        requires input == input
        ensures result == input + 0 && input == input
        {
            transition { _ -> (input + 0) }
        }
        machine CheckedMathProvider::decoy_impl(input: i32) -> i32
        satisfies OtherMath::offset_zero
        requires input == input
        ensures result == input + 0 && input == input
        {
            transition { _ -> (input + 0) }
        }

        data AlternateMathProvider {}
        machine AlternateMathProvider::offset_zero_impl(input: i32) -> i32
        satisfies CheckedMath::offset_zero
        requires input == input
        ensures result == input + 0 && input == input
        {
            transition { _ -> (input + 0) }
        }

        machine run() -> i32 {
            transition { _ -> (CheckedMath::offset_zero(70)) }
        }

        data Main {}
        machine Main::main(&mut self) {
            let result: i32 = CheckedMath::offset_zero(70);
        }
    "#;

struct Fixture {
    checked: CheckedTrees,
    checked_plan: effects::provider_plan::ProviderPlan,
    other_plan: effects::provider_plan::ProviderPlan,
    operator_use: checked_trees::CheckedNamedOperatorUseFact,
}

fn fixture_from_source(source: &str) -> Fixture {
    let tokens = source_files_to_tokens::Lexer::new(source)
        .tokenize()
        .expect("tokenize checked-operator dispatch fixture");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens)
        .expect("parse checked-operator dispatch fixture");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .expect("resolve checked-operator dispatch fixture");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type checked-operator dispatch fixture");
    let plans = provider_planning::derive_satisfies_plans(
        &typed,
        ProviderPlanDerivation::unevaluated(None),
    )
    .into_iter()
    .map(|derived| derived.plan)
    .collect::<Vec<_>>();
    let checked_plan = plans
        .iter()
        .find(|plan| {
            plan.schema.trait_name.contains("CheckedMath::offset_zero")
                && plan.provider_type == "CheckedMathProvider"
        })
        .expect("CheckedMath provider plan")
        .clone();
    let other_plan = plans
        .iter()
        .find(|plan| plan.schema.trait_name.contains("OtherMath::offset_zero"))
        .expect("OtherMath provider plan")
        .clone();
    let checked = typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .expect("check operator dispatch fixture");
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
                        .eq(["CheckedMath", "offset_zero"])
                })
        })
        .expect("checked named operator use");
    Fixture {
        checked,
        checked_plan,
        other_plan,
        operator_use,
    }
}

fn fixture() -> Fixture {
    fixture_from_source(SOURCE)
}

fn select_operator_use(
    fixture: &mut Fixture,
    matches_origin: impl Fn(CheckedValueOrigin) -> bool,
) -> (
    effects::SelectedProviderPlanFacts,
    checked_trees::CheckedNamedOperatorUseFact,
) {
    let (use_handle, mut operator_use) = fixture
        .checked
        .facts
        .operators
        .named_uses
        .iter()
        .map(|(handle, operator_use)| (handle, *operator_use))
        .find(|(_, operator_use)| matches_origin(operator_use.origin))
        .expect("selected operator use");
    operator_use.provider_plan_report_fingerprint = fixture.checked_plan.report_fingerprint();
    operator_use.provider_plan_commitment =
        checked_trees::CheckedProviderPlanCommitment::from_digest(
            *fixture.checked_plan.identity_digest().as_bytes(),
        );
    *fixture
        .checked
        .facts
        .operators
        .named_uses
        .get_mut(use_handle) = operator_use;
    let selected = effects::SelectedProviderPlanFacts::from_selection(
        std::slice::from_ref(&fixture.checked_plan),
        std::slice::from_ref(&fixture.checked_plan.name),
    )
    .expect("select exact checked-operator plan");
    (selected, operator_use)
}

fn settled_attached_unit_fixture() -> (
    Arc<CheckedTrees>,
    effects::SelectedProviderPlanFacts,
    checked_trees::CheckedNamedOperatorUseFact,
) {
    let mut fixture = fixture();
    let main = fixture
        .checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("Unit fixture machine");
    let main_symbol = main.symbol;
    let main_state = fixture.checked.typed.machine_states(main)[0].symbol;
    let (selected, operator_use) = select_operator_use(&mut fixture, |origin| {
        matches!(
            origin,
            CheckedValueOrigin::StateStatement {
                machine_symbol,
                state_symbol,
                statement_index: 0,
                role: CheckedValueStatementRole::LocalInitializer,
            } if machine_symbol == main_symbol && state_symbol == main_state
        )
    });
    let mut settled = Arc::new(fixture.checked);
    settle_selected_execution_dispatch(&mut settled, &selected)
        .expect("selected Unit operator application settles");
    (settled, selected, operator_use)
}

#[test]
fn derives_canonical_empty_nongeneric_checked_body_application() {
    let (settled, selected, operator_use) = settled_attached_unit_fixture();

    let rows = derive_checked_nongeneric_operator_application_realizations(&settled, &selected)
        .expect("exact selected application joins");
    let [row] = rows.as_slice() else {
        panic!("fixture must derive one attached-Unit application row")
    };
    assert_eq!(row.authored_use_kind, CheckedOperatorAuthoredUseKind::Named,);
    assert!(matches!(
        row.application_site,
        checked_trees::CheckedBoundaryOperatorApplicationUseSite::Expression {
            origin: CheckedValueOrigin::StateStatement {
                statement_index: 0,
                role: CheckedValueStatementRole::LocalInitializer,
                ..
            },
            ..
        }
    ));
    assert_eq!(
        row.requirement_operator,
        operator_use.selected_operator_symbol
    );
    assert!(
        row.requirement_overload_identity
            .contains("CheckedMath::offset_zero")
    );
    assert_eq!(
        row.provider_plan_commitment,
        operator_use.provider_plan_commitment,
    );
    assert!(!row.realization_contract_commitment.is_zero());
}

#[test]
fn application_realization_derivation_rejects_missing_or_substituted_joins() {
    let (settled, selected, _) = settled_attached_unit_fixture();

    let mut missing_application = settled.as_ref().clone();
    missing_application
        .facts
        .operators
        .boundary_applications
        .clear();
    let missing = derive_checked_nongeneric_operator_application_realizations(
        &missing_application,
        &selected,
    )
    .expect_err("selected call without its application demand must reject");
    assert_eq!(missing.len(), 1);
    assert!(
        missing[0]
            .message
            .contains("retained 0 exact application demands"),
        "unexpected diagnostic: {}",
        missing[0].message,
    );

    let mut substituted_contract = settled.as_ref().clone();
    let selected_call = substituted_contract
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .flat_map(|machine| &mut machine.operations)
        .find_map(|operation| match operation {
            CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall {
                realization_contract_commitment,
                ..
            } => Some(realization_contract_commitment),
            _ => None,
        })
        .expect("selected attached-Unit call");
    *selected_call = checked_trees::MachineContractCommitment::from_digest([0xa5; 32]);
    let substituted = derive_checked_nongeneric_operator_application_realizations(
        &substituted_contract,
        &selected,
    )
    .expect_err("substituted machine contract must reject");
    assert_eq!(substituted.len(), 1);
    assert!(
        substituted[0]
            .message
            .contains("does not rejoin its exact machine contract commitment"),
        "unexpected diagnostic: {}",
        substituted[0].message,
    );
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
    AbsentAdapter,
    DuplicateAdapter,
    WrongOwner,
    NonCheckedAdapter,
    WrongConformance,
    NonCallExpression,
    SourceOperator,
    Intrinsic,
}

#[test]
fn exact_resolver_rejects_every_identity_drift() {
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
        (Drift::AbsentAdapter, Some("is absent from typed machines")),
        (
            Drift::DuplicateAdapter,
            Some("resolves to 2 exact typed machines"),
        ),
        (
            Drift::WrongOwner,
            Some("does not belong to nominal provider"),
        ),
        (Drift::NonCheckedAdapter, Some("is not a checked body")),
        (
            Drift::WrongConformance,
            Some("through 0 checked conformances"),
        ),
        (Drift::NonCallExpression, Some("is not a named call")),
        (
            Drift::SourceOperator,
            Some("no longer names its checked operator symbol"),
        ),
        (Drift::Intrinsic, None),
    ];

    for (drift, expected_error) in cases {
        let mut fixture = fixture();
        let mut plan = fixture.checked_plan.clone();
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
                plan = fixture.other_plan.clone();
                plans = vec![plan.clone()];
            }
            Drift::AbsentAdapter => {
                plan.rows[0].binding = ProviderBinding::CheckedAdapter {
                    machine_identity: "CheckedMathProvider::absent".into(),
                    machine_package_identity: None,
                };
                plans = vec![plan.clone()];
            }
            Drift::DuplicateAdapter => {
                let machine_identity = match &plan.rows[0].binding {
                    ProviderBinding::CheckedAdapter {
                        machine_identity, ..
                    } => machine_identity.clone(),
                    binding => panic!("unexpected fixture binding {binding:?}"),
                };
                let duplicate = fixture
                    .checked
                    .typed
                    .machine_by_normalized_overload_identity(&machine_identity)
                    .expect("selected adapter")
                    .clone();
                fixture.checked.typed.push_machine(duplicate);
            }
            Drift::WrongOwner => {
                plan.provider_type = "OtherProvider".into();
                plans = vec![plan.clone()];
            }
            Drift::NonCheckedAdapter => {
                fixture
                    .checked
                    .typed
                    .machines_mut()
                    .iter_mut()
                    .find(|machine| machine.name.as_str().ends_with("offset_zero_impl"))
                    .expect("checked adapter")
                    .supply_mode = language_semantics::MachineSupplyMode::Boundary;
            }
            Drift::WrongConformance => {
                let decoy = fixture
                    .checked
                    .typed
                    .machines()
                    .iter()
                    .find(|machine| machine.name.as_str().ends_with("decoy_impl"))
                    .expect("decoy adapter");
                let machine_identity = fixture
                    .checked
                    .typed
                    .normalized_machine_overload_identity(decoy)
                    .expect("decoy adapter identity")
                    .identity();
                plan.rows[0].binding = ProviderBinding::CheckedAdapter {
                    machine_identity,
                    machine_package_identity: None,
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
                    panic!("fixture operator expression is not a call");
                };
                fixture.operator_use.expression = fixture
                    .checked
                    .typed
                    .expression_table
                    .expression_handles(call.arguments)[0];
            }
            Drift::SourceOperator => {
                let other = fixture
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
                            .eq(["OtherMath", "offset_zero"])
                    })
                    .expect("other operator");
                fixture.operator_use.selected_operator_symbol = other.symbol;
                plan = fixture.other_plan.clone();
                plans = vec![plan.clone()];
            }
            Drift::Intrinsic => {
                plan.rows[0].binding = ProviderBinding::CompilerIntrinsic {
                    machine: "CheckedMath::offset_zero".into(),
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

        let result =
            resolve_selected_operator_adapter_call(&fixture.checked, &plans, &fixture.operator_use);
        match expected_error {
            Some(expected) => {
                let diagnostic = result.expect_err("drift must fail closed");
                assert!(
                    diagnostic.message.contains(expected),
                    "{drift:?}: expected `{expected}`, got `{}`",
                    diagnostic.message,
                );
            }
            None if matches!(drift, Drift::Intrinsic) => {
                assert_eq!(result.expect("intrinsic remains delegated"), None);
            }
            None => {
                let rewrite = result
                    .expect("exact realization resolves")
                    .expect("checked adapter stages a rewrite");
                assert_eq!(rewrite.expression, fixture.operator_use.expression);
                assert_eq!(rewrite.machine, "CheckedMathProvider::offset_zero_impl");
            }
        }
    }
}

#[test]
fn shared_success_clones_only_after_complete_preflight() {
    let mut fixture = fixture();
    let selected = effects::SelectedProviderPlanFacts::from_selection(
        std::slice::from_ref(&fixture.checked_plan),
        std::slice::from_ref(&fixture.checked_plan.name),
    )
    .expect("select exact checked-operator plan");
    let (handle, mut retained) = fixture
        .checked
        .facts
        .operators
        .named_uses
        .iter()
        .map(|(handle, operator_use)| (handle, *operator_use))
        .find(|(_, operator_use)| operator_use.expression == fixture.operator_use.expression)
        .expect("fixture checked use");
    retained.provider_plan_report_fingerprint = fixture.checked_plan.report_fingerprint();
    retained.provider_plan_commitment = checked_trees::CheckedProviderPlanCommitment::from_digest(
        *fixture.checked_plan.identity_digest().as_bytes(),
    );
    *fixture.checked.facts.operators.named_uses.get_mut(handle) = retained;
    let original_contents = fixture.checked.clone();
    let original = Arc::new(fixture.checked);
    let mut settled = Arc::clone(&original);

    settle_selected_execution_dispatch(&mut settled, &selected)
        .expect("exact selected adapter rewrites");

    assert!(
        !Arc::ptr_eq(&settled, &original),
        "a shared successful settlement must publish through a fresh Arc"
    );
    assert_eq!(
        original.as_ref(),
        &original_contents,
        "successful settlement must not mutate retained shared custody"
    );
    let rewritten = settled
        .typed
        .expression_table
        .expression(retained.expression);
    let ExpressionNode::Call(rewritten) = rewritten else {
        panic!("rewritten expression is not a call");
    };
    let provider = settled
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "CheckedMathProvider::offset_zero_impl")
        .expect("exact checked adapter");
    assert_eq!(rewritten.target.as_str(), provider.name.as_str());
    assert_eq!(
        rewritten.target_symbol,
        settled.typed.machine_states(provider)[0].symbol,
    );
    assert_eq!(
        settled.facts.operators.named_uses.get(handle),
        &retained,
        "execution redirection must not rewrite retained semantic evidence",
    );
}

#[test]
fn selected_local_result_retains_exact_unit_realization_application() {
    let mut fixture = fixture();
    let main = fixture
        .checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("Unit fixture machine");
    let main_symbol = main.symbol;
    let main_state = fixture.checked.typed.machine_states(main)[0].symbol;
    let (selected, operator_use) = select_operator_use(&mut fixture, |origin| {
        matches!(
            origin,
            CheckedValueOrigin::StateStatement {
                machine_symbol,
                state_symbol,
                statement_index: 0,
                role: CheckedValueStatementRole::LocalInitializer,
            } if machine_symbol == main_symbol && state_symbol == main_state
        )
    });
    let mut settled = Arc::new(fixture.checked);

    settle_selected_execution_dispatch(&mut settled, &selected)
        .expect("selected Unit operator application settles");

    let unit_plan = settled
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| plan.machine == main_symbol && plan.state == main_state)
        .expect("selected Unit plan");
    assert!(unit_plan.operations.iter().any(|operation| {
        matches!(
            operation,
            CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall {
                requirement_operator,
                realization_machine,
                ..
            } if *requirement_operator == operator_use.selected_operator_symbol
                && settled
                    .typed
                    .machines()
                    .iter()
                    .find(|machine| machine.symbol == *realization_machine)
                    .is_some_and(|machine| {
                        machine.name.as_str() == "CheckedMathProvider::offset_zero_impl"
                    })
        )
    }));
    validate_selected_operator_terminal_custody(&settled, &selected)
        .expect("retained Unit call must rejoin its exact selected ProviderPlan");
    let missing = validate_selected_operator_terminal_custody(
        &settled,
        &effects::SelectedProviderPlanFacts::default(),
    )
    .expect_err("retained Unit call cannot outlive its selected ProviderPlan closure");
    assert_eq!(missing.len(), 1);
    assert!(
        missing[0]
            .message
            .contains("unknown ProviderPlan report fingerprint"),
        "unexpected diagnostic: {}",
        missing[0].message,
    );
}

#[test]
fn selected_integer_result_reaches_immediate_write_only_store() {
    let source = SOURCE.replace(
        "machine Main::main(&mut self) {\n            let result: i32 = CheckedMath::offset_zero(70);\n        }",
        "machine Main::main(destination: &write i32) {\n            let result: i32 = CheckedMath::offset_zero(70);\n            destination = result;\n        }",
    );
    let mut fixture = fixture_from_source(&source);
    let main = fixture
        .checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("Unit fixture machine");
    let main_symbol = main.symbol;
    let main_state = fixture.checked.typed.machine_states(main)[0].symbol;
    let (selected, _) = select_operator_use(&mut fixture, |origin| {
        matches!(
            origin,
            CheckedValueOrigin::StateStatement {
                machine_symbol,
                state_symbol,
                statement_index: 0,
                role: CheckedValueStatementRole::LocalInitializer,
            } if machine_symbol == main_symbol && state_symbol == main_state
        )
    });
    let mut settled = Arc::new(fixture.checked);

    settle_selected_execution_dispatch(&mut settled, &selected)
        .expect("selected result and immediate write-only store settle");

    let operations = &settled
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| plan.machine == main_symbol && plan.state == main_state)
        .expect("selected Unit plan")
        .operations;
    assert!(matches!(
        operations.as_slice(),
        [
            CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall { result, .. },
            CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
                statement_index: 1,
                path,
                destination: checked_trees::CheckedPrimitiveStoreDestination::Parameter { parameter_index: 0 },
                value: checked_trees::CheckedCallScalarArgument::Pure(checked_trees::CheckedScalarExpression::Local {
                    position: 0,
                    primitive_type: typed_trees::types::PrimitiveType::I32,
                }),
            },
            CheckedUnitEffectOperationPlan::Complete { statement_index: 2, .. },
        ] if path.is_empty() && result.statement_index == 0 && result.binding_ordinal == 0
    ));
    validate_selected_operator_terminal_custody(&settled, &selected)
        .expect("selected result store retains exact ProviderPlan custody");
}

#[test]
fn selected_local_result_retains_dependent_branch_free_scalar_local() {
    let source = SOURCE.replace(
        "let result: i32 = CheckedMath::offset_zero(70);",
        "let selected: i32 = CheckedMath::offset_zero(70);\n            let result: i32 = selected + 0i32;",
    );
    let mut fixture = fixture_from_source(&source);
    let main = fixture
        .checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("Unit fixture machine");
    let main_symbol = main.symbol;
    let main_state = fixture.checked.typed.machine_states(main)[0].symbol;
    let (selected, _) = select_operator_use(&mut fixture, |origin| {
        matches!(
            origin,
            CheckedValueOrigin::StateStatement {
                machine_symbol,
                state_symbol,
                statement_index: 0,
                role: CheckedValueStatementRole::LocalInitializer,
            } if machine_symbol == main_symbol && state_symbol == main_state
        )
    });
    let mut settled = Arc::new(fixture.checked);

    settle_selected_execution_dispatch(&mut settled, &selected)
        .expect("selected call and dependent scalar local settle");

    let operations = &settled
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter()
        .find(|plan| {
            settled
                .typed
                .machines()
                .iter()
                .find(|machine| machine.symbol == plan.machine)
                .is_some_and(|machine| machine.name.as_str() == "Main::main")
        })
        .expect("selected Unit plan")
        .operations;
    let [
        CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall {
            result: selected, ..
        },
        CheckedUnitEffectOperationPlan::EstablishScalarLocal { result, value },
        CheckedUnitEffectOperationPlan::Complete { .. },
    ] = operations.as_slice()
    else {
        panic!("selected Unit plan did not retain the exact scalar-local sequence")
    };
    assert_eq!(selected.binding_ordinal, 0);
    assert_eq!(result.binding_ordinal, 1);
    assert!(matches!(
        value,
        checked_trees::CheckedCallScalarArgument::Pure(checked_trees::CheckedScalarExpression::IntegerBinary {
            kind: checked_trees::CheckedIntegerBinaryKind::ExactAdd,
            left,
            right,
            ..
        }) if matches!(
            left.as_ref(),
            checked_trees::CheckedScalarExpression::Local { position: 0, .. }
        ) && matches!(
            right.as_ref(),
            checked_trees::CheckedScalarExpression::IntegerLiteral { .. }
        )
    ));
}

#[test]
fn nested_selected_unit_call_remains_fenced() {
    let source = SOURCE.replace(
        "let result: i32 = CheckedMath::offset_zero(70);",
        "let result: i32 = CheckedMath::offset_zero(70) + 0i32;",
    );
    let mut fixture = fixture_from_source(&source);
    let main = fixture
        .checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("Unit fixture machine");
    let main_symbol = main.symbol;
    let main_state = fixture.checked.typed.machine_states(main)[0].symbol;
    let (selected, _) = select_operator_use(&mut fixture, |origin| {
        matches!(
            origin,
            CheckedValueOrigin::StateStatement {
                machine_symbol,
                state_symbol,
                statement_index: 0,
                role: CheckedValueStatementRole::LocalInitializer,
            } if machine_symbol == main_symbol && state_symbol == main_state
        )
    });
    let mut settled = Arc::new(fixture.checked);

    let diagnostics = settle_selected_execution_dispatch(&mut settled, &selected)
        .expect_err("nested selected Unit call must remain fenced");
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("nested inside a Unit local initializer")
        }),
        "unexpected diagnostics: {diagnostics:#?}",
    );
}

#[test]
fn selected_unit_scalar_local_retains_short_circuit_value() {
    let source = SOURCE.replace(
        "let result: i32 = CheckedMath::offset_zero(70);",
        "let selected: i32 = CheckedMath::offset_zero(70);\n            let result: bool = selected == 70 && true;",
    );
    let mut fixture = fixture_from_source(&source);
    let main = fixture
        .checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("Unit fixture machine");
    let main_symbol = main.symbol;
    let main_state = fixture.checked.typed.machine_states(main)[0].symbol;
    let (selected, _) = select_operator_use(&mut fixture, |origin| {
        matches!(
            origin,
            CheckedValueOrigin::StateStatement {
                machine_symbol,
                state_symbol,
                statement_index: 0,
                role: CheckedValueStatementRole::LocalInitializer,
            } if machine_symbol == main_symbol && state_symbol == main_state
        )
    });
    let mut settled = Arc::new(fixture.checked);

    settle_selected_execution_dispatch(&mut settled, &selected)
        .expect("checked short-circuit scalar local remains represented");
    validate_selected_operator_terminal_custody(&settled, &selected)
        .expect("selected call retains exact ProviderPlan custody");
    let operations = &settled
        .facts
        .flow
        .terminal_unit_effects
        .for_machine(main_symbol)
        .expect("selected Unit plan")
        .operations;
    assert!(matches!(operations.as_slice(), [
        CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall { .. },
        CheckedUnitEffectOperationPlan::EstablishScalarLocal {
            result,
            value: checked_trees::CheckedCallScalarArgument::Pure(
                checked_trees::CheckedScalarExpression::Boolean(value)
            ),
        },
        CheckedUnitEffectOperationPlan::Complete { .. },
    ] if result.primitive_type == typed_trees::types::PrimitiveType::Bool
        && matches!(value.as_ref(), checked_trees::CheckedBooleanExpression::And { .. })));
}

#[test]
fn terminal_custody_rejects_another_conforming_realization() {
    let mut fixture = fixture();
    let main = fixture
        .checked
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::main")
        .expect("Unit fixture machine");
    let main_symbol = main.symbol;
    let main_state = fixture.checked.typed.machine_states(main)[0].symbol;
    let (use_handle, mut operator_use) = fixture
        .checked
        .facts
        .operators
        .named_uses
        .iter()
        .map(|(handle, operator_use)| (handle, *operator_use))
        .find(|(_, operator_use)| {
            matches!(
                operator_use.origin,
                CheckedValueOrigin::StateStatement {
                    machine_symbol,
                    state_symbol,
                    statement_index: 0,
                    role: CheckedValueStatementRole::LocalInitializer,
                } if machine_symbol == main_symbol && state_symbol == main_state
            )
        })
        .expect("selected operator local in Unit fixture");
    operator_use.provider_plan_report_fingerprint = fixture.checked_plan.report_fingerprint();
    operator_use.provider_plan_commitment =
        checked_trees::CheckedProviderPlanCommitment::from_digest(
            *fixture.checked_plan.identity_digest().as_bytes(),
        );
    *fixture
        .checked
        .facts
        .operators
        .named_uses
        .get_mut(use_handle) = operator_use;
    let selected = effects::SelectedProviderPlanFacts::from_selection(
        std::slice::from_ref(&fixture.checked_plan),
        std::slice::from_ref(&fixture.checked_plan.name),
    )
    .expect("select exact checked-operator plan");
    let mut settled = Arc::new(fixture.checked);
    settle_selected_execution_dispatch(&mut settled, &selected)
        .expect("selected Unit operator application settles");

    let alternate = settled
        .typed
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "AlternateMathProvider::offset_zero_impl")
        .expect("alternate conforming adapter");
    let alternate_machine = alternate.symbol;
    let alternate_state = settled.typed.machine_states(alternate)[0].symbol;
    let alternate_contract = settled
        .facts
        .contract_plans
        .for_machine(alternate_machine)
        .expect("alternate contract");
    let alternate_contract_report_fingerprint = alternate_contract.report_fingerprint;
    let alternate_contract_commitment = alternate_contract.commitment;
    let alternate_reach = settled
        .facts
        .flow
        .control
        .states
        .iter()
        .find(|(_, state)| {
            state.machine_symbol == alternate_machine && state.state_symbol == alternate_state
        })
        .map(|(_, state)| state.service_reach)
        .expect("alternate service reach");
    let settled_mut = Arc::make_mut(&mut settled);
    let selected_call = settled_mut
        .facts
        .flow
        .terminal_unit_effects
        .machines
        .iter_mut()
        .find(|plan| plan.machine == main_symbol && plan.state == main_state)
        .and_then(|plan| {
            plan.operations.iter_mut().find(|operation| {
                matches!(
                    operation,
                    CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall { .. }
                )
            })
        })
        .expect("selected Unit call");
    let CheckedUnitEffectOperationPlan::SelectedOperatorScalarCall {
        realization_machine,
        realization_state,
        realization_contract_report_fingerprint,
        realization_contract_commitment,
        service_reach,
        ..
    } = selected_call
    else {
        unreachable!("selected operation kind was filtered above")
    };
    *realization_machine = alternate_machine;
    *realization_state = alternate_state;
    *realization_contract_report_fingerprint = alternate_contract_report_fingerprint;
    *realization_contract_commitment = alternate_contract_commitment;
    *service_reach = alternate_reach;

    let diagnostics = validate_selected_operator_terminal_custody(&settled, &selected)
        .expect_err("another conforming realization must not replace the selected plan row");
    assert_eq!(diagnostics.len(), 1);
    assert!(
        diagnostics[0]
            .message
            .contains("but its exact ProviderPlan selects"),
        "unexpected diagnostic: {}",
        diagnostics[0].message,
    );
}

#[test]
fn shared_rejection_preserves_arc_identity_and_complete_contents() {
    let mut fixture = fixture();
    let selected = effects::SelectedProviderPlanFacts::from_selection(
        std::slice::from_ref(&fixture.checked_plan),
        std::slice::from_ref(&fixture.checked_plan.name),
    )
    .expect("select exact checked-operator plan");
    let handles = fixture
        .checked
        .facts
        .operators
        .named_uses
        .iter()
        .map(|(handle, operator_use)| (handle, *operator_use))
        .collect::<Vec<_>>();
    let (handle, mut valid) = handles
        .into_iter()
        .find(|(_, operator_use)| operator_use.expression == fixture.operator_use.expression)
        .expect("fixture checked use");
    valid.provider_plan_report_fingerprint = fixture.checked_plan.report_fingerprint();
    valid.provider_plan_commitment = checked_trees::CheckedProviderPlanCommitment::from_digest(
        *fixture.checked_plan.identity_digest().as_bytes(),
    );
    *fixture.checked.facts.operators.named_uses.get_mut(handle) = valid;
    let mut invalid = valid;
    invalid.provider_plan_report_fingerprint = u64::MAX;
    fixture.checked.facts.operators.named_uses.append(invalid);
    let before = fixture.checked.clone();
    let original = Arc::new(fixture.checked);
    let mut rejected = Arc::clone(&original);

    let diagnostics = settle_selected_execution_dispatch(&mut rejected, &selected)
        .expect_err("one invalid use rejects the complete rewrite batch");
    assert!(
        diagnostics[0]
            .message
            .contains("unknown ProviderPlan report fingerprint")
    );
    assert_eq!(
        rejected.as_ref(),
        &before,
        "a later failure must not publish an earlier staged rewrite",
    );
    assert!(
        Arc::ptr_eq(&rejected, &original),
        "rejection must preserve exact shared program custody"
    );
}

#[test]
fn empty_settlement_preserves_shared_arc_identity_and_contents() {
    let fixture = fixture();
    let original_contents = fixture.checked.clone();
    let original = Arc::new(fixture.checked);
    let mut settled = Arc::clone(&original);

    settle_selected_execution_dispatch(
        &mut settled,
        &effects::SelectedProviderPlanFacts::default(),
    )
    .expect("a program without selected operator adapters is already settled");

    assert!(Arc::ptr_eq(&settled, &original));
    assert_eq!(settled.as_ref(), &original_contents);
}
