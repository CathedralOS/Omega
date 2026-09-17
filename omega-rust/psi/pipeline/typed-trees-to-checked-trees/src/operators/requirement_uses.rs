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
fn directly_callable_requirement<'program>(
    program: &'program TypedTrees,
    entry_symbol: symbols::SymbolHandle,
) -> Option<&'program typed_trees::machine::Machine> {
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
    let requirement = directly_callable_requirement(program, call.target_symbol)?;
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
