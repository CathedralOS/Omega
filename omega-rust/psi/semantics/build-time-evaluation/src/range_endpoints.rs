//! COMPTIME evaluation of NAMED RANGE ENDPOINTS.
//!
//! `u64[0..=limit()]` puts a build-time-admissible, zero-argument machine call
//! in a declared integer range endpoint. The endpoint is a constant position
//! exactly as a fixed-array length is, so it takes the same route as
//! `const_lengths`: admit the callee through the shared
//! [`BuildTimeAdmissionPlan`], execute it with the checked interpreter under
//! the callee's declared integer return type, and substitute the resulting
//! decimal literal into the endpoint expression before checking.
//!
//! Folding here, in the typed trees, is what lets every later reader agree on
//! one value: declaration validation, structural generic inference in
//! `typed-trees-to-checked-trees/src/monomorphization/range_arguments.rs`,
//! proof, and layout all read an ordinary literal endpoint. The
//! context-free `i64` interval evaluator in `validation` never sees the call,
//! so it cannot become the identity of a named computation.
//!
//! A type qualifier is not a runtime receiver. Reuse the typed call's resolved
//! entry and receiver classification, then evaluate that exact machine symbol;
//! rebuilding a name could select an unrelated same-spelled machine. Calls with
//! runtime receivers or unresolved arguments remain outside this closed route.

use diagnostics::Diagnostic;
use numerics::literals::{IntegerLiteral, IntegerRadix};
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle};

use crate::BuildTimeAdmissionPlan;

struct PendingEndpoint {
    constrained_type: TypeReferenceHandle,
    expression: ExpressionHandle,
    machine: symbols::SymbolHandle,
    source_span: source::SourceSpan,
}

pub fn evaluate_const_range_endpoints_with_authority(
    typed: &mut TypedTrees,
    selection_authority: Option<std::sync::Arc<dyn crate::BuildTimeSelectionAuthority>>,
) -> Result<(), Vec<Diagnostic>> {
    let pending = pending_endpoints(typed);
    if pending.is_empty() {
        return Ok(());
    }

    let prepared = crate::PreparedBuildMachineProgram::prepare(typed)?;
    let execution = prepared.typed();
    let admission =
        BuildTimeAdmissionPlan::infer_with_selection_authority(execution, selection_authority);

    let mut diagnostics = Vec::new();
    let mut substitutions = Vec::new();
    for endpoint in &pending {
        let result = admission
            .evaluate_const_evaluable_machine_symbol_for_invocation(
                execution,
                endpoint.machine,
                Vec::new(),
                crate::BuildTimeInvocationCustody::Source(endpoint.source_span),
            )
            .and_then(|value| {
                let machine = execution
                    .machines()
                    .iter()
                    .find(|machine| machine.symbol == endpoint.machine)
                    .ok_or_else(|| "range endpoint lost its selected machine".to_owned())?;
                crate::const_lengths::decode_integer_result(execution, machine, value)
            });
        match result {
            Ok(value) => substitutions.push((endpoint.expression, value)),
            Err(reason) => diagnostics.push(Diagnostic::error(format!(
                "range endpoint of `{}`: const evaluation of `{}` failed: {reason}",
                typed
                    .type_reference_table
                    .display_name(endpoint.constrained_type),
                typed.symbols.display_path(endpoint.machine, "::"),
            ))),
        }
    }

    for (expression, value) in substitutions {
        let literal = IntegerLiteral::from_parts(
            value.is_negative(),
            IntegerRadix::Decimal,
            &value.abs().to_string(),
        )
        .map_err(|reason| {
            vec![Diagnostic::error(format!(
                "invalid evaluated range endpoint: {reason}"
            ))]
        })?;
        *typed.expression_table.expression_mut(expression) = ExpressionNode::Integer(literal);
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn pending_endpoints(typed: &TypedTrees) -> Vec<PendingEndpoint> {
    let mut pending = Vec::new();
    for (constrained_type, _, constraints) in typed
        .type_reference_table
        .constrained_type_reference_sites()
    {
        for constraint in typed.type_reference_table.constraints(constraints) {
            let TypeConstraintNode::Range {
                minimum, maximum, ..
            } = constraint
            else {
                continue;
            };
            for expression in [*minimum, *maximum] {
                let ExpressionNode::Call(call) = typed.expression_table.expression(expression)
                else {
                    continue;
                };
                if !typed
                    .expression_table
                    .expression_handles(call.arguments)
                    .is_empty()
                    || !call.machine_arguments.is_empty()
                    || !call.evidence_arguments.is_empty()
                    || call.static_machine_parameter.is_valid()
                    || call.static_requirement_dispatch.is_some()
                    || call.quotient_operation.is_some()
                    || call.private_layout_operation.is_some()
                {
                    continue;
                }
                let Some(machine) = typed.machines().iter().find(|machine| {
                    // No authored argument list does not mean the callee is
                    // closed. Folding must not erase an underdetermined generic
                    // application before ordinary call validation can reject it.
                    machine.type_parameters.is_empty()
                        && typed.machine_states(machine).first().is_some_and(|entry| {
                            typed.call_has_no_runtime_receiver(call, machine, entry)
                                && typed.state_parameters(entry).is_empty()
                        })
                }) else {
                    continue;
                };
                pending.push(PendingEndpoint {
                    constrained_type,
                    expression,
                    machine: machine.symbol,
                    source_span: typed.expression_table.source_span(expression),
                });
            }
        }
    }
    pending
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(source: &str) -> TypedTrees {
        let tokens = source_files_to_tokens::Lexer::new(source)
            .tokenize()
            .unwrap();
        let syntax =
            tokens_to_syntax_trees::parse_syntax_trees_with_id(source::SourceId(0), &tokens)
                .unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax).unwrap();
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap()
    }

    #[test]
    fn only_closed_calls_without_runtime_inputs_are_pending_endpoints() {
        for source in [
            "data Limits {} machine Limits::capacity(&self) -> u64 { 256 }
             machine bounded(limits: Limits, value: u64[0..=limits.capacity()]) {}",
            "data Limits {} machine Limits::capacity(value: u64) -> u64 { value }
             machine bounded(value: u64[0..=Limits::capacity(256)]) {}",
            "data Limits {} machine Limits::capacity<const N: u64>() -> u64 { 256 }
             machine bounded(value: u64[0..=Limits::capacity()]) {}",
        ] {
            let program = typed(source);
            assert!(pending_endpoints(&program).is_empty(), "{source}");
        }
    }

    #[test]
    fn substituted_target_cannot_borrow_another_type_qualifier() {
        let mut program = typed(
            "data Limits {} data Other {}
             machine Limits::capacity() -> u64 { 256 }
             machine Other::capacity() -> u64 { 512 }
             machine bounded(value: u64[0..=Limits::capacity()]) {}",
        );
        let pending = pending_endpoints(&program);
        assert_eq!(pending.len(), 1);
        let expression = pending[0].expression;
        let other = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Other::capacity")
            .expect("other attached machine");
        let other_entry = program.machine_states(other)[0].symbol;
        let ExpressionNode::Call(call) = program.expression_table.expression_mut(expression) else {
            panic!("authored endpoint call");
        };
        call.target_symbol = other_entry;
        assert!(
            pending_endpoints(&program).is_empty(),
            "resolved owner and qualifier must agree"
        );
    }
}
