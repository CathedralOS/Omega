//! Fixed-token checked-adapter dispatch.
//!
//! Spelled expressions retain their public operator fact while execution is
//! rewritten to the exact selected checked body. The rewrite consumes only the
//! operands already owned by the checked expression; it never re-resolves a
//! token from source text.

use super::{
    OperatorAdapterRewrite, OperatorAdapterSource, exact_operator_definition,
    resolve_checked_adapter_for_operator, resolve_exact_selected_plan,
};
use psi_checked_trees::{CheckedOperatorResolutionStatus, CheckedTrees};
use psi_diagnostics::Diagnostic;
use psi_language_core::operator_spelling::OperatorSpelling;
use psi_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

pub(super) fn resolve_selected_spelled_operator_adapter_call(
    checked: &CheckedTrees,
    selected_provider_plans: &[omega_effects::provider_plan::ProviderPlan],
    operator_use: &psi_checked_trees::CheckedOperatorUseFact,
) -> Result<Option<OperatorAdapterRewrite>, Diagnostic> {
    if operator_use.status != CheckedOperatorResolutionStatus::Resolved {
        return Err(Diagnostic::error(format!(
            "fixed-token operator use at expression {:?} is not uniquely resolved",
            operator_use.expression,
        )));
    }
    let plan = resolve_exact_selected_plan(
        selected_provider_plans,
        operator_use.provider_plan_report_fingerprint,
        operator_use.provider_plan_commitment,
        "fixed-token operator use",
    )?;
    let operator = exact_operator_definition(
        checked,
        operator_use.expression,
        operator_use.selected_operator_symbol,
    )?;
    if !operator.is_boundary {
        return Err(Diagnostic::error(format!(
            "selected fixed-token operator at expression {:?} does not name a boundary operator",
            operator_use.expression,
        )));
    }
    if operator.spelling != Some(operator_use.spelling) {
        return Err(Diagnostic::error(format!(
            "selected fixed-token operator at expression {:?} no longer owns exact spelling `{}`",
            operator_use.expression,
            operator_use.spelling.symbol(),
        )));
    }
    let Some(candidate) = checked.facts.operators.selected_candidate(operator_use) else {
        return Err(Diagnostic::error(format!(
            "fixed-token operator use at expression {:?} has no selected candidate fact",
            operator_use.expression,
        )));
    };
    if candidate.operator_symbol != operator.symbol || !candidate.is_boundary {
        return Err(Diagnostic::error(format!(
            "fixed-token operator use at expression {:?} does not rejoin its selected boundary candidate",
            operator_use.expression,
        )));
    }

    let operands = exact_spelled_operands(checked, operator_use)?;
    if checked.typed.operator_parameters(operator).len() != operands.len() {
        return Err(Diagnostic::error(format!(
            "selected fixed-token operator at expression {:?} has {} runtime parameter(s), but its checked expression retains {} operand(s)",
            operator_use.expression,
            checked.typed.operator_parameters(operator).len(),
            operands.len(),
        )));
    }
    let Some((machine, entry_symbol)) =
        resolve_checked_adapter_for_operator(checked, operator, plan, operator_use.expression)?
    else {
        return Ok(None);
    };

    Ok(Some(OperatorAdapterRewrite {
        expression: operator_use.expression,
        machine,
        entry_symbol,
        source: OperatorAdapterSource::Spelled(operands.into_boxed_slice()),
    }))
}

fn exact_spelled_operands(
    checked: &CheckedTrees,
    operator_use: &psi_checked_trees::CheckedOperatorUseFact,
) -> Result<Vec<ExpressionHandle>, Diagnostic> {
    let (spelling, operands) = match checked
        .typed
        .expression_table
        .expression(operator_use.expression)
    {
        ExpressionNode::Binary(binary) => {
            let Some(spelling) = binary_spelling(binary.operator) else {
                return Err(source_expression_drift(operator_use));
            };
            (spelling, vec![binary.left, binary.right])
        }
        ExpressionNode::Indexed(indexed) => {
            match checked.typed.expression_table.expression(indexed.index) {
                ExpressionNode::Range(_) => {
                    return Err(Diagnostic::error(format!(
                        "selected range operator at expression {:?} is not represented by fixed-token checked-adapter dispatch",
                        operator_use.expression,
                    )));
                }
                _ => (
                    OperatorSpelling::Index,
                    vec![indexed.collection, indexed.index],
                ),
            }
        }
        _ => return Err(source_expression_drift(operator_use)),
    };
    if spelling != operator_use.spelling || operands.iter().any(|operand| !operand.is_valid()) {
        return Err(source_expression_drift(operator_use));
    }
    Ok(operands)
}

fn source_expression_drift(operator_use: &psi_checked_trees::CheckedOperatorUseFact) -> Diagnostic {
    Diagnostic::error(format!(
        "fixed-token operator expression {:?} no longer has checked spelling `{}` and its exact operand shape",
        operator_use.expression,
        operator_use.spelling.symbol(),
    ))
}

fn binary_spelling(operator: BinaryOperator) -> Option<OperatorSpelling> {
    Some(match operator {
        BinaryOperator::Add => OperatorSpelling::Add,
        BinaryOperator::Subtract => OperatorSpelling::Subtract,
        BinaryOperator::Multiply => OperatorSpelling::Multiply,
        BinaryOperator::Divide => OperatorSpelling::Divide,
        BinaryOperator::Modulo => OperatorSpelling::Modulo,
        BinaryOperator::Equal => OperatorSpelling::Equal,
        BinaryOperator::NotEqual => OperatorSpelling::NotEqual,
        BinaryOperator::Less => OperatorSpelling::Less,
        BinaryOperator::LessOrEqual => OperatorSpelling::LessEqual,
        BinaryOperator::Greater => OperatorSpelling::Greater,
        BinaryOperator::GreaterOrEqual => OperatorSpelling::GreaterEqual,
        BinaryOperator::And
        | BinaryOperator::BitwiseAnd
        | BinaryOperator::BitwiseOr
        | BinaryOperator::BitwiseXor
        | BinaryOperator::Or
        | BinaryOperator::ShiftLeft
        | BinaryOperator::ShiftRight => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

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
        plan: omega_effects::provider_plan::ProviderPlan,
        use_handle: psi_arena::Handle<psi_checked_trees::CheckedOperatorUseFact>,
    }

    fn fixture() -> Fixture {
        let tokens = psi_source_files_to_tokens::Lexer::new(SOURCE)
            .tokenize()
            .expect("tokenize fixed-token dispatch fixture");
        let syntax = psi_tokens_to_syntax_trees::parse_syntax_trees(&tokens)
            .expect("parse fixed-token dispatch fixture");
        let resolved = psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
            .expect("resolve fixed-token dispatch fixture");
        let typed =
            psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
                .expect("type fixed-token dispatch fixture");
        let plans = omega_provider_planning::plans::derive_satisfies_plans(&typed, None);
        let [plan] = plans.as_slice() else {
            panic!("fixed-token dispatch fixture must derive one provider plan")
        };
        let mut checked = psi_typed_trees_to_checked_trees::lower_typed_trees(typed)
            .expect("check fixed-token dispatch fixture");
        let (use_handle, mut operator_use) = checked
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
        operator_use.provider_plan_report_fingerprint = plan.report_fingerprint();
        operator_use.provider_plan_commitment =
            psi_checked_trees::CheckedProviderPlanCommitment::from_digest(
                *plan.identity_digest().as_bytes(),
            );
        *checked.facts.operators.uses.get_mut(use_handle) = operator_use;
        Fixture {
            checked,
            plan: plan.clone(),
            use_handle,
        }
    }

    #[test]
    fn exact_fixed_token_use_rewrites_to_selected_checked_adapter() {
        let fixture = fixture();
        let selected = omega_effects::SelectedProviderPlanFacts::from_selection(
            std::slice::from_ref(&fixture.plan),
            std::slice::from_ref(&fixture.plan.name),
        )
        .expect("select fixed-token provider plan");
        let operator_use = *fixture.checked.facts.operators.uses.get(fixture.use_handle);
        let ExpressionNode::Binary(binary) = fixture
            .checked
            .typed
            .expression_table
            .expression(operator_use.expression)
        else {
            panic!("fixture use must begin as a binary expression")
        };
        let expected_arguments = [binary.left, binary.right];
        let original = Arc::new(fixture.checked);
        let mut settled = Arc::clone(&original);

        super::super::settle_selected_operator_adapter_dispatch(&mut settled, &selected)
            .expect("exact fixed-token adapter dispatches");

        assert!(!Arc::ptr_eq(&settled, &original));
        assert!(matches!(
            original
                .typed
                .expression_table
                .expression(operator_use.expression),
            ExpressionNode::Binary(_)
        ));
        let ExpressionNode::Call(call) = settled
            .typed
            .expression_table
            .expression(operator_use.expression)
        else {
            panic!("settled fixed-token expression must be an adapter call")
        };
        assert_eq!(call.target.as_str(), "CheckedMathProvider::same_impl");
        assert_eq!(
            settled
                .typed
                .expression_table
                .expression_handles(call.arguments),
            expected_arguments,
        );
        assert_eq!(
            settled.facts.operators.uses.get(fixture.use_handle),
            original.facts.operators.uses.get(fixture.use_handle),
            "execution redirection must preserve semantic operator evidence",
        );
    }

    #[test]
    fn fixed_token_drift_rejects_before_mutation() {
        for drift in ["commitment", "spelling", "status", "expression"] {
            let mut fixture = fixture();
            let selected = omega_effects::SelectedProviderPlanFacts::from_selection(
                std::slice::from_ref(&fixture.plan),
                std::slice::from_ref(&fixture.plan.name),
            )
            .expect("select fixed-token provider plan");
            let mut operator_use = *fixture.checked.facts.operators.uses.get(fixture.use_handle);
            match drift {
                "commitment" => {
                    operator_use.provider_plan_commitment =
                        psi_checked_trees::CheckedProviderPlanCommitment::from_digest([0xa5; 32]);
                }
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
            let before = fixture.checked.clone();
            let original = Arc::new(fixture.checked);
            let mut rejected = Arc::clone(&original);

            super::super::settle_selected_operator_adapter_dispatch(&mut rejected, &selected)
                .expect_err("fixed-token identity drift must reject");

            assert!(Arc::ptr_eq(&rejected, &original), "{drift}");
            assert_eq!(rejected.as_ref(), &before, "{drift}");
        }
    }
}
