use super::derive_checked_nongeneric_operator_application_realizations;
use checked_trees::{CheckedOperatorResolutionStatus, CheckedTrees};
use language_core::operator_spelling::OperatorSpelling;
use typed_trees::expression::ExpressionNode;

const SOURCE: &str = r#"
    data CheckedMath {}
    boundary operator == CheckedMath::same(left: i32, right: i32) -> bool;

    data CheckedMathProvider {}
    machine CheckedMathProvider::same_impl(left: i32, right: i32) -> bool
    satisfies CheckedMath::same
    {
        transition { _ -> true }
    }

    machine run(left: i32, right: i32) -> bool {
        transition { _ -> (left == right) }
    }
"#;

struct Fixture {
    checked: CheckedTrees,
    plan: effects::provider_plan::ProviderPlan,
    use_handle: arena::Handle<checked_trees::CheckedOperatorUseFact>,
}

fn fixture() -> Fixture {
    let tokens = source_files_to_tokens::Lexer::new(SOURCE)
        .tokenize()
        .expect("tokenize fixed-token dispatch fixture");
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens)
        .expect("parse fixed-token dispatch fixture");
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(syntax),
    )
    .expect("resolve fixed-token dispatch fixture");
    let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .expect("type fixed-token dispatch fixture");
    let plans = provider_planning::derive_satisfies_plans(
        provider_planning::ProviderPlanDerivation::unevaluated(
            provider_planning::ProviderPlanningContext::new(
                &typed,
                source::DependencyScope::Product,
            ),
            None,
        ),
    )
    .into_iter()
    .map(|derived| derived.plan)
    .collect::<Vec<_>>();
    let [plan] = plans.as_slice() else {
        panic!("fixed-token dispatch fixture must derive one provider plan")
    };
    let checked = typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .expect("check fixed-token dispatch fixture");
    let (use_handle, _) = checked
        .facts
        .operators
        .uses
        .iter()
        .map(|(handle, operator_use)| (handle, *operator_use))
        .find(|(_, operator_use)| {
            operator_use.spelling == OperatorSpelling::Equal
                && operator_use.status == CheckedOperatorResolutionStatus::Resolved
        })
        .expect("one resolved fixed-token boundary use");
    Fixture {
        checked,
        plan: plan.clone(),
        use_handle,
    }
}

#[test]
fn fixed_token_drift_rejects_realization_coverage() {
    for drift in ["spelling", "status", "expression"] {
        let mut fixture = fixture();
        let selected = effects::SelectedProviderPlanFacts::from_selection(
            std::slice::from_ref(&fixture.plan),
            std::slice::from_ref(&fixture.plan.name),
        )
        .expect("select fixed-token provider plan");
        assert_eq!(
            derive_checked_nongeneric_operator_application_realizations(
                &fixture.checked,
                &selected,
            )
            .expect("exact checked realization")
            .len(),
            1
        );
        let mut operator_use = *fixture.checked.facts.operators.uses.get(fixture.use_handle);
        match drift {
            "spelling" => operator_use.spelling = OperatorSpelling::Subtract,
            "status" => operator_use.status = CheckedOperatorResolutionStatus::Ambiguous,
            "expression" => {
                let ExpressionNode::Binary(binary) = fixture
                    .checked
                    .typed
                    .expression_table
                    .expression(operator_use.expression)
                else {
                    panic!("fixture use must be binary")
                };
                operator_use.expression = binary.left;
            }
            _ => unreachable!(),
        }
        *fixture
            .checked
            .facts
            .operators
            .uses
            .get_mut(fixture.use_handle) = operator_use;
        let diagnostics = derive_checked_nongeneric_operator_application_realizations(
            &fixture.checked,
            &selected,
        )
        .expect_err("fixed-token identity drift must reject");
        assert!(!diagnostics.is_empty(), "{drift}");
    }
}
