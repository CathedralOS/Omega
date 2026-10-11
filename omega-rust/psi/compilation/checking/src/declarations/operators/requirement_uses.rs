//! Direct calls to receiver-free top-level boundary requirements,
//! retained as the requirement-spelling twin of named boundary-operator uses.
//!
//! The typed call targets the requirement's entry state; the fact retains the
//! requirement machine itself plus the arithmetic-policy result adapter its
//! arguments select, exactly as a named `F32::*` operator use does, so each
//! target's selected plan joins either spelling by requirement identity and
//! the intrinsic execution bridge resolves both through one requirement view.

use super::uses::{builtin_float_operator_selection, named_float_policy_adapter};
use arena::Arena;
use checked_trees::{
    CheckedArithmeticPolicyAdapter, CheckedNamedRequirementUseFact, CheckedValueFacts,
    CheckedValueOrigin,
};
use numerics::float_semantics::FloatFormat;
use std::collections::HashSet;
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use typed_trees::types::PrimitiveType;

/// A receiver-free top-level `boundary requirement` with no static
/// generic binders: the shape a direct call may execute through its selected
/// provider. An erased lifetime telescope is admitted; it contributes no
/// static application arguments, so the retained use keys on the same
/// canonical-empty demand a nongeneric requirement produces.
/// Lexical resolution owns visibility; a private same-package requirement
/// uses the same selected-provider route as a public one.
fn directly_callable_requirement(
    program: &dyn crate::CheckingInput,
    entry_symbol: symbols::SymbolHandle,
) -> Option<&typed_trees::machine::Machine> {
    if !entry_symbol.is_valid() {
        return None;
    }
    program.machines().iter().find(|machine| {
        machine.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
            && !machine.body_is_present
            && program.machine_type_parameters(machine).is_empty()
            && matches!(
                program.machine_states(machine),
                [entry] if entry.symbol == entry_symbol
            )
    })
}

/// Discover named requirement uses once per enclosing executable origin.
pub fn collect_named_requirement_uses(
    program: &dyn crate::CheckingInput,
    values: &CheckedValueFacts,
) -> Arena<CheckedNamedRequirementUseFact> {
    let mut uses = Arena::default();
    let mut seen = HashSet::with_hasher(symbols::BuildSymbolHasher);
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
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    origin: CheckedValueOrigin,
    seen: &mut HashSet<(ExpressionHandle, CheckedValueOrigin), symbols::BuildSymbolHasher>,
    uses: &mut Arena<CheckedNamedRequirementUseFact>,
) {
    if !expression.is_valid() || !seen.insert((expression, origin)) {
        return;
    }
    let node = program.expression_table.expression(expression);
    if let ExpressionNode::Call(call) = node
        && let Some(fact) = named_requirement_use_fact(program, expression, origin, call)
    {
        uses.append(fact);
    }
    // Selection discovery shares the canonical value edges with other checks.
    // A local roster omitted structural initializers and match patterns, losing
    // requirement occurrences even though their operand values were checked.
    crate::expression_traversal::children(program, node, |child| {
        collect(program, child, origin, seen, uses);
    });
}

fn named_requirement_use_fact(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    origin: CheckedValueOrigin,
    call: &TableCallExpression,
) -> Option<CheckedNamedRequirementUseFact> {
    let requirement = directly_callable_requirement(program, call.target_symbol).or_else(|| {
        // `min`/`max`/`sqrt` shorthand for a requirement-spelled
        // `F32::minimum`-family slot retains the same use the direct call does.
        let (selected, _) = builtin_float_operator_selection(program, expression, origin, call)?;
        program.machines().iter().find(|machine| {
            machine.symbol == selected
                && machine.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
        })
    })?;
    let entry = program.machine_states(requirement).first()?;
    let policy_adapter = match program.primitive_type_reference(entry.return_type) {
        Some(PrimitiveType::F32) => {
            named_float_policy_adapter(program, call, origin, FloatFormat::BINARY32)
        }
        Some(PrimitiveType::F64) => {
            named_float_policy_adapter(program, call, origin, FloatFormat::BINARY64)
        }
        _ => CheckedArithmeticPolicyAdapter::None,
    };
    Some(CheckedNamedRequirementUseFact {
        expression,
        origin,
        requirement_symbol: requirement.symbol,
        policy_adapter,
    })
}

/// The token-bearing machine whose entry state a compiler-synthesized call
/// targets, if the call is one bound by `bind_selected_operator_calls`.
/// Finalization uses it to settle the retained `Operator` occurrence on the
/// rewritten node to the exact declaration.
pub fn token_bound_machine_call_target(
    program: &dyn crate::CheckingInput,
    call: &TableCallExpression,
) -> Option<SymbolHandle> {
    if call.operational_acknowledgement.origin
        != language_semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized
        || !call.target_symbol.is_valid()
    {
        return None;
    }
    program
        .machines()
        .iter()
        .filter(|machine| machine.spelling.is_some())
        .find(|machine| {
            program
                .machine_states(machine)
                .first()
                .is_some_and(|entry| entry.symbol == call.target_symbol)
        })
        .map(|machine| machine.symbol)
}
