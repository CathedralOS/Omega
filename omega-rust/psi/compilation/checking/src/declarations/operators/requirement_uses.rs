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
/// A pending typed call may still name the exact machine declaration before
/// ordinary callable preparation publishes its entry state. Both coordinates
/// select the same declaration; neither spelling nor signature supplies a join.
/// Lexical resolution owns visibility; a private same-package requirement
/// uses the same selected-provider route as a public one.
fn directly_callable_requirement(
    program: &dyn crate::CheckingInput,
    target_symbol: symbols::SymbolHandle,
) -> Option<&typed_trees::machine::Machine> {
    if !target_symbol.is_valid() {
        return None;
    }
    program.machines().iter().find(|machine| {
        machine.supply_mode == language_semantics::MachineSupplyMode::TopLevelRequirement
            && !machine.body_is_present
            && program.machine_type_parameters(machine).is_empty()
            && matches!(
                program.machine_states(machine),
                [entry] if machine.symbol == target_symbol || entry.symbol == target_symbol
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
    // Every executable value edge keeps the enclosing statement origin.
    // A private partial child roster omitted requirements in record fields,
    // ranges and match patterns: lowering then had a call but no exact policy
    // selection. Share the complete executable traversal; this collector owns
    // only occurrence deduplication and the selected requirement's evidence.
    crate::expression_traversal::executable_children(program, node, |child| {
        collect(program, child, origin, seen, uses);
    });
}

fn named_requirement_use_fact(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    origin: CheckedValueOrigin,
    call: &TableCallExpression,
) -> Option<CheckedNamedRequirementUseFact> {
    if let Some(public) = program
        .typed_program()
        .selected_public_trait_requirement_signature(
            call.target_symbol,
            call.authored_machine_application,
        )
        && public.owner.is_boundary
        && typed_trees::operator::resolve_named_expression_call(program, call).is_none()
    {
        return Some(CheckedNamedRequirementUseFact {
            expression,
            origin,
            requirement_symbol: public.signature.symbol,
            policy_adapter: CheckedArithmeticPolicyAdapter::None,
        });
    }
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

/// The authored token-bearing declaration selected by a synthesized call,
/// including an exact public boundary requirement. Executable specialization
/// preserves its original registration; the private instance is not a new
/// authored operator occurrence.
pub fn token_bound_call_target(
    program: &dyn crate::CheckingInput,
    call: &TableCallExpression,
) -> Option<SymbolHandle> {
    if call.operational_acknowledgement.origin
        != language_semantics::CallOperationalAcknowledgementOrigin::CompilerSynthesized
    {
        return None;
    }
    let target = if call.authored_machine_application.is_valid() {
        program
            .expression_table
            .static_machine_application(call.authored_machine_application)
            .registration_operation
    } else {
        call.target_symbol
    };
    crate::find_machine_head(program, target)
        .filter(|(machine, _)| machine.spelling.is_some())
        .map(|(machine, _)| machine.symbol)
        .or_else(|| {
            let (owner, signature) = program.trait_machine_signature(call.target_symbol)?;
            if !owner.is_boundary || signature.spelling.is_none() {
                return None;
            }
            if let Some((_, row)) =
                program
                    .instantiated_requirement_signatures
                    .iter()
                    .find(|(_, row)| {
                        program.trait_machine_signatures.get(row.signature).symbol
                            == signature.symbol
                    })
            {
                (call.authored_machine_application.is_valid()
                    && typed_trees::static_applications::static_application_identities_equal(
                        program.typed_program(),
                        call.authored_machine_application,
                        program.typed_program(),
                        row.binding_application,
                    )
                    .is_ok_and(|same| same))
                .then_some(row.requirement_template)
            } else {
                Some(signature.symbol)
            }
        })
}
