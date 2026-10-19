//! Direct calls to receiver-free boundary requirements,
//! retained as the requirement-spelling twin of named boundary-operator uses.
//!
//! The typed call targets a requirement entry state or an exact boundary-trait
//! signature; the fact retains that declaration plus the result-policy adapter its
//! arguments select, exactly as a named `F32::*` operator use does, so each
//! target's selected plan joins either spelling by requirement identity and
//! the intrinsic execution bridge resolves both through one requirement view.

use super::uses::named_float_policy_adapter;
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
    // Trait requirements have their own exact signature declaration, rather
    // than a synthetic top-level Machine. Retain that declaration just as we
    // retain a top-level requirement; selection and execution admission still
    // belong to their ordinary downstream owners.
    let (requirement_symbol, return_type) =
        if let Some((owner, signature)) = program.trait_machine_signature(call.target_symbol) {
            // A namespace qualifier is represented as a receiver expression too.
            // Only this exact trait declaration is static here; an arbitrary value
            // or another namespace cannot be erased by selected execution.
            let static_receiver = !call.receiver.is_valid()
                || matches!(program.expression_table.expression(call.receiver),
                ExpressionNode::Name(path) if path.symbol == owner.symbol);
            if !owner.is_boundary
                || !static_receiver
                || !program.trait_type_parameters(owner).is_empty()
                || !signature.type_parameters.is_empty()
            {
                return None;
            }
            (signature.symbol, signature.return_type)
        } else {
            let requirement = directly_callable_requirement(program, call.target_symbol)?;
            let entry = program.machine_states(requirement).first()?;
            (requirement.symbol, entry.return_type)
        };
    let policy_adapter = match program.primitive_type_reference(return_type) {
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
        requirement_symbol,
        policy_adapter,
    })
}

/// The authored token-bearing machine selected by a synthesized call.
/// Specialization changes the executable entry while copies retain the same
/// authored `Operator` occurrence. Its retained registration identifies the
/// declaration all those copies selected, not any private executable instance.
pub fn token_bound_machine_call_target(
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
    let (machine, _) = crate::find_machine_head(program, target)?;
    machine.spelling.is_some().then_some(machine.symbol)
}
