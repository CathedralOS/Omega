//! Result agreement for terminal machine calls in transition arms
//! ([state contracts](../../../../../../../wiki/spec/language/state_contracts.md#terminal-calls-in-transition-arms)).
//!
//! An arm whose resolved target is another machine invokes it, and the
//! callee's normal result completes the caller. Result compatibility is the
//! ordinary completion rule: every reachable terminal path must produce the
//! state's declared result, and a typed value keeps its carrier and
//! arithmetic policy across a store (`proof_contracts::arithmetic_domains::
//! check_narrowing_assignment`). A `u64` callee therefore cannot complete an
//! `i32` machine, nor a value callee a Unit machine; the caller converts
//! explicitly with a value arm (`-> (callee(..) as i32)`) instead.
//!
//! Only results whose shape this module classifies exactly are compared:
//! Unit, primitive carriers and concrete data types. Generic parameters,
//! traits, arrays and references keep their own type checks, so this rule
//! never reports a pairing it cannot name.

use diagnostics::Diagnostic;
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// The result one terminal call's callee completes with, in the shapes the
/// comparison below can decide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResultShape {
    Unit,
    Primitive(
        typed_trees::types::PrimitiveType,
        numerics::arithmetic::ArithmeticDomain,
    ),
    Data(symbols::SymbolHandle),
}

/// Report a terminal call whose callee's declared result differs from the
/// result of the state that tail-calls it.
pub(super) fn validate(
    program: &dyn crate::CheckingInput,
    caller_machine: &Machine,
    caller_state: &State,
    callee_machine: &Machine,
    callee_state: &State,
    target: typed_trees::statement::TransitionTargetHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if program.static_transition_type_context_is_unreachable(target) {
        return;
    }
    let expected_reference =
        program.statically_refined_transition_type_reference(target, caller_state.return_type);
    let (Some(expected), Some(actual)) = (
        shape(program, expected_reference),
        shape(program, callee_state.return_type),
    ) else {
        return;
    };
    if expected == actual {
        return;
    }
    let expected_spelling = spelling(program, expected_reference);
    // Only a primitive pair has a value-arm conversion to suggest.
    let remedy = match (expected, actual) {
        (ResultShape::Primitive(..), ResultShape::Primitive(..)) => format!(
            " -- return a converted value instead (`-> ({}(..) as {expected_spelling})`)",
            callee_machine.name
        ),
        _ => String::new(),
    };
    diagnostics.push(Diagnostic::error(format!(
        "machine `{}` state `{}` terminal call to `{}` completes with its `{}` result, but \
         the state returns `{expected_spelling}`; a terminal machine call's normal result \
         is its caller's result, so the two must match{remedy}",
        caller_machine.name,
        caller_state.name,
        callee_machine.name,
        spelling(program, callee_state.return_type),
    )));
}

fn shape(
    program: &dyn crate::CheckingInput,
    mut handle: TypeReferenceHandle,
) -> Option<ResultShape> {
    if !handle.is_valid() {
        return Some(ResultShape::Unit);
    }
    if let Some(primitive) = program.primitive_type_reference(handle) {
        return Some(ResultShape::Primitive(
            primitive,
            program.arithmetic_domain_for_type_reference(handle),
        ));
    }
    loop {
        match program.type_reference_table.type_reference(handle) {
            TypeReferenceNode::Unit => return Some(ResultShape::Unit),
            TypeReferenceNode::Constrained { base_type, .. } => handle = *base_type,
            TypeReferenceNode::Named { symbol, .. }
                if symbol.is_valid()
                    && crate::declarations::data_queries::data_definition_by_symbol(
                        program, *symbol,
                    )
                    .is_some() =>
            {
                return Some(ResultShape::Data(*symbol));
            }
            _ => return None,
        }
    }
}

fn spelling(program: &dyn crate::CheckingInput, handle: TypeReferenceHandle) -> String {
    if matches!(shape(program, handle), Some(ResultShape::Unit)) {
        "Unit".to_owned()
    } else {
        program.display_type_reference_with_constraints(handle)
    }
}
