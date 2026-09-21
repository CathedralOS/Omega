//! Direct calls to public receiver-free top-level boundary requirements,
//! retained as the requirement-spelling twin of named boundary-operator uses.
//!
//! The typed call targets the requirement's entry state; the fact retains the
//! requirement machine itself plus the arithmetic-policy result adapter its
//! arguments select, exactly as a named `F32::*` operator use does, so
//! provider planning can stamp the selected plan and the intrinsic execution
//! bridge can resolve either spelling through one requirement view.

use arena::Arena;
use checked_trees::{
    CheckedArithmeticPolicyAdapter, CheckedNamedRequirementUseFact, CheckedValueFacts,
    CheckedValueOrigin,
};
use numerics::float_semantics::FloatFormat;
use std::collections::HashSet;
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use typed_trees::types::PrimitiveType;

/// A public, nongeneric, receiver-free top-level `boundary requirement`: the
/// shape a direct call may execute through its selected provider.
fn directly_callable_requirement(
    program: &TypedTrees,
    entry_symbol: symbols::SymbolHandle,
) -> Option<&typed_trees::machine::Machine> {
    if !entry_symbol.is_valid() {
        return None;
    }
    program.machines().iter().find(|machine| {
        machine.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
            && machine.is_public
            && !machine.body_is_present
            && machine.lifetime_parameters.is_empty()
            && program.machine_type_parameters(machine).is_empty()
            && matches!(
                program.machine_states(machine),
                [entry] if entry.symbol == entry_symbol
                    && !program.state_parameters(entry).iter().any(|parameter| parameter.is_self)
            )
    })
}

pub(crate) fn collect_named_requirement_uses(
    program: &TypedTrees,
    values: &CheckedValueFacts,
) -> Arena<CheckedNamedRequirementUseFact> {
    let mut uses = Arena::default();
    let mut seen = HashSet::new();
    for (_, value) in values.values.iter() {
        if matches!(value.origin, CheckedValueOrigin::NestedExpression { .. }) {
            continue;
        }
        collect(
            program,
            value.expression,
            value.origin,
            &mut seen,
            &mut uses,
        );
    }
    uses
}

fn collect(
    program: &TypedTrees,
    expression: ExpressionHandle,
    origin: CheckedValueOrigin,
    seen: &mut HashSet<(ExpressionHandle, CheckedValueOrigin)>,
    uses: &mut Arena<CheckedNamedRequirementUseFact>,
) {
    if !expression.is_valid() || !seen.insert((expression, origin)) {
        return;
    }
    let children = |handles: &[ExpressionHandle]| handles.to_vec();
    let nested = match program.expression_table.expression(expression) {
        ExpressionNode::Call(call) => {
            if let Some(fact) = named_requirement_use_fact(program, expression, origin, call) {
                uses.append(fact);
            }
            let mut nested = vec![call.receiver];
            nested.extend_from_slice(program.expression_table.expression_handles(call.arguments));
            nested
        }
        ExpressionNode::Binary(binary) => children(&[binary.left, binary.right]),
        ExpressionNode::Unary(unary) => children(&[unary.operand]),
        ExpressionNode::Member(member) => children(&[member.receiver]),
        ExpressionNode::Borrow(inner) => children(&[inner.target]),
        ExpressionNode::Atomic(atomic) => children(&[atomic.value]),
        ExpressionNode::Indexed(indexed) => children(&[indexed.collection, indexed.index]),
        ExpressionNode::Cast(cast) => children(&[cast.value]),
        ExpressionNode::ArrayLiteral(values) => {
            children(program.expression_table.expression_handles(*values))
        }
        ExpressionNode::Match(dispatch) => {
            let mut nested = vec![dispatch.subject];
            for arm in program.expression_table.match_arms(dispatch.arms) {
                nested.push(arm.value);
            }
            nested
        }
        _ => Vec::new(),
    };
    for child in nested {
        collect(program, child, origin, seen, uses);
    }
}

fn named_requirement_use_fact(
    program: &TypedTrees,
    expression: ExpressionHandle,
    origin: CheckedValueOrigin,
    call: &TableCallExpression,
) -> Option<CheckedNamedRequirementUseFact> {
    let requirement = directly_callable_requirement(program, call.target_symbol).or_else(|| {
        // `min`/`max`/`sqrt` shorthand for a requirement-spelled
        // `F32::minimum`-family slot retains the same use the direct call does.
        let (selected, _) =
            super::builtin_float_operator_selection(program, expression, origin, call)?;
        program.machines().iter().find(|machine| {
            machine.symbol == selected
                && machine.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
        })
    })?;
    let entry = program.machine_states(requirement).first()?;
    let policy_adapter = match program.primitive_type_reference(entry.return_type) {
        Some(PrimitiveType::F32) => {
            super::named_float_policy_adapter(program, call, origin, FloatFormat::BINARY32)
        }
        Some(PrimitiveType::F64) => {
            super::named_float_policy_adapter(program, call, origin, FloatFormat::BINARY64)
        }
        _ => CheckedArithmeticPolicyAdapter::None,
    };
    Some(CheckedNamedRequirementUseFact {
        expression,
        origin,
        requirement_symbol: requirement.symbol,
        policy_adapter,
        provider_plan_report_fingerprint: 0,
        provider_plan_commitment: Default::default(),
    })
}

#[cfg(test)]
mod tests {
    fn checked(source: &str) -> checked_trees::CheckedTrees {
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .expect("tokens");
        let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).expect("syntax");
        let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
            syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
        )
        .expect("symbols");
        let typed = symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
            .expect("types");
        crate::lower_typed_trees(typed, &crate::CheckingRequest::settled()).expect("check")
    }

    const SOURCE: &str = r#"
        pub data F32 {}
        pub boundary requirement F32::maximum(left: f32, right: f32) -> f32;
        machine run() -> f32 {
            let direct: f32 = F32::maximum(1.0f32, 2.0f32);
            let shorthand: f32 = max(direct, 3.0f32);
            transition { _ -> (shorthand) }
        }
    "#;

    /// `max(..)` over f32 operands is the shorthand for the visible
    /// `F32::maximum` slot; spelled as a top-level boundary requirement, the
    /// shorthand retains the same named requirement use the direct call does
    /// and no named operator use, and the drift resolver names the requirement.
    #[test]
    fn min_max_shorthand_resolves_to_a_requirement_spelled_slot() {
        let checked = checked(SOURCE);
        let requirement = checked
            .typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "F32::maximum")
            .expect("requirement");
        let mut uses = checked
            .facts
            .operators
            .named_requirement_uses()
            .map(|selected_use| {
                let call = match checked
                    .typed
                    .expression_table
                    .expression(selected_use.expression)
                {
                    typed_trees::expression::ExpressionNode::Call(call) => {
                        call.target.as_str().to_owned()
                    }
                    other => panic!("use is not a call: {other:?}"),
                };
                assert_eq!(selected_use.requirement_symbol, requirement.symbol);
                assert_eq!(
                    crate::resolve_checked_builtin_float_operator_requirement(
                        &checked.typed,
                        selected_use.expression,
                        selected_use.origin,
                    )
                    .is_some(),
                    call == "max",
                    "the drift resolver names the requirement only for the shorthand"
                );
                call
            })
            .collect::<Vec<_>>();
        uses.sort();
        assert_eq!(uses, ["max", "maximum"]);
        assert_eq!(checked.facts.operators.named_uses().count(), 0);
        assert_eq!(
            checked
                .facts
                .operators
                .boundary_applications
                .iter()
                .filter(|demand| demand.requirement_symbol == requirement.symbol)
                .count(),
            2,
            "both spellings of the call are requirement-keyed D29 demands"
        );
    }
}
