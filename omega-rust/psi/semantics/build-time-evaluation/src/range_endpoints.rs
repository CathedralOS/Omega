//! COMPTIME evaluation of NAMED RANGE ENDPOINTS.
//!
//! `u64[0..=limit(256)]` puts a build-time-admissible machine call
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
//! Closed integer arguments share the type system's exact numeric evaluation,
//! with argument carrier and declaration-selection checks before interpreter
//! snapshots erase their authored types. No runtime flow bound supplies a value.

use diagnostics::Diagnostic;
use numerics::literals::{IntegerLiteral, IntegerRadix};
use typed_trees::TypedTrees;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::types::{TypeConstraintNode, TypeReferenceHandle};

use crate::BuildTimeAdmissionPlan;

mod arguments;

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
    let admission = BuildTimeAdmissionPlan::infer_with_selection_authority(
        execution,
        selection_authority.clone(),
    );

    let mut diagnostics = Vec::new();
    let mut substitutions = Vec::new();
    let mut warnings = Vec::new();
    for endpoint in &pending {
        let result = arguments::evaluate(
            execution,
            endpoint.expression,
            endpoint.machine,
            selection_authority.as_deref(),
        )
        .and_then(|(arguments, argument_warnings)| {
            warnings.extend(argument_warnings);
            admission.evaluate_const_evaluable_machine_symbol_for_invocation(
                execution,
                endpoint.machine,
                arguments,
                crate::BuildTimeInvocationCustody::Source(endpoint.source_span),
            )
        })
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
        for warning in warnings {
            eprintln!("{warning}");
        }
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
                if !call.machine_arguments.is_empty()
                    || !call.evidence_arguments.is_empty()
                    || call.static_machine_parameter.is_valid()
                    || call.static_requirement_dispatch.is_some()
                    || call.quotient_operation.is_some()
                    || call.private_layout_operation.is_some()
                {
                    continue;
                }
                let Some(machine) = typed.machines().iter().find(|machine| {
                    // Ordinary value arguments do not close generic binders.
                    // Folding must not erase an underdetermined generic
                    // application before ordinary call validation can reject it.
                    machine.type_parameters.is_empty()
                        && typed.machine_states(machine).first().is_some_and(|entry| {
                            typed.call_has_no_runtime_receiver(call, machine, entry)
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
    fn runtime_receivers_and_open_generic_callees_are_not_pending_endpoints() {
        for source in [
            "data Limits {} machine Limits::capacity(&self) -> u64 { 256 }
             machine bounded(limits: Limits, value: u64[0..=limits.capacity()]) {}",
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

    #[test]
    fn closed_integer_arguments_keep_carriers_and_exact_landings() {
        for (parameter, argument, expected) in [
            ("u64", "256", "256"),
            ("u64", "1 / 2 * 512", "256"),
            ("u64", "18446744073709551615u64", "18446744073709551615"),
            ("i64", "-9223372036854775808i64", "-9223372036854775808"),
            ("u8", "255", "255"),
            ("u64", "7u64 / 2 * 2", "6"),
        ] {
            let mut program = typed(&format!(
                "machine endpoint(value: {parameter}) -> {parameter} {{ value }}
                 machine bounded(value: {parameter}[0..=endpoint({argument})]) {{}}"
            ));
            let pending = pending_endpoints(&program);
            assert_eq!(pending.len(), 1);
            evaluate_const_range_endpoints_with_authority(&mut program, None)
                .unwrap_or_else(|errors| panic!("{argument}: {errors:?}"));
            let ExpressionNode::Integer(literal) =
                program.expression_table.expression(pending[0].expression)
            else {
                panic!("evaluated endpoint must be a literal");
            };
            assert_eq!(literal.value_bignum().unwrap().to_string(), expected);
        }
    }

    #[test]
    fn ignored_arguments_cannot_hide_invalid_or_unsupported_inputs() {
        for (parameter, arguments, diagnostic) in [
            ("u64", "true", "closed integer expression"),
            ("u8", "255u16", "range endpoint argument"),
            ("u8", "255u64", "landed range endpoint argument"),
            ("u8", "256", "cannot land exactly"),
            ("u64", "-1", "cannot land exactly"),
            ("u64", "1 / 2", "closed integer expression"),
            ("u64", "7u64 / 0", "closed integer expression"),
            (
                "u64",
                "18446744073709551615u64 + 1",
                "closed integer expression",
            ),
            ("u64[0..=8]", "9", "unconstrained exact builtin integer"),
            ("u64", "", "argument count"),
            ("u64", "1, 2", "argument count"),
            ("u64", "input", "closed integer expression"),
            ("u64", "nested()", "closed integer expression"),
        ] {
            let mut program = typed(&format!(
                "machine nested() -> u64 {{ 1 }}
                 machine endpoint(ignored: {parameter}) -> u64 {{ 256 }}
                 machine bounded(input: u64, value: u64[0..=endpoint({arguments})]) {{}}"
            ));
            let pending = pending_endpoints(&program);
            assert_eq!(pending.len(), 1);
            let errors = evaluate_const_range_endpoints_with_authority(&mut program, None)
                .expect_err("an ignored argument still needs source admission");
            assert!(
                errors
                    .iter()
                    .any(|error| error.message.contains(diagnostic)),
                "{parameter} <- {arguments}: {errors:?}"
            );
            assert!(
                matches!(
                    program.expression_table.expression(pending[0].expression),
                    ExpressionNode::Call(_)
                ),
                "rejection must not erase the authored call"
            );
        }
    }

    #[test]
    fn argument_landing_retains_nested_fractional_warnings() {
        for argument in ["1 / 2 * 512", "1u64 + (1 / 2 * 510)"] {
            let program = typed(&format!(
                "machine endpoint(value: u64) -> u64 {{ value }}
                 machine bounded(value: u64[0..=endpoint({argument})]) {{}}"
            ));
            let pending = pending_endpoints(&program);
            let (values, warnings) =
                arguments::evaluate(&program, pending[0].expression, pending[0].machine, None)
                    .expect("closed integer arguments land");
            assert_eq!(values, vec![crate::BuildTimeValue::Int(256)]);
            assert_eq!(warnings.len(), 1, "{argument}: {warnings:?}");
            assert!(warnings[0].source_span.is_some());
        }
    }

    #[test]
    fn folded_constant_arguments_still_require_their_own_selection_authority() {
        use semantic_vocabulary::PackageKeyIdentity;
        use std::{path::PathBuf, sync::Arc};
        struct Selection(bool);
        impl crate::BuildTimeSelectionAuthority for Selection {
            fn allows_declaration_selection(
                &self,
                _: PackageKeyIdentity,
                _: PackageKeyIdentity,
            ) -> bool {
                self.0
            }
            fn package_label(&self, _: PackageKeyIdentity) -> String {
                "argument-package".to_owned()
            }
        }
        let text = "const CAPACITY: u64 = 256;
                    machine endpoint(value: u64) -> u64 { value }
                    machine bounded(value: u64[0..=endpoint(CAPACITY)]) {}";
        let mut sources = source::SourceMap::default();
        let source_id = sources
            .add_with_metadata(
                PathBuf::from("main.omg"),
                text.to_owned(),
                PathBuf::from("."),
                Some(PackageKeyIdentity::from_digest([0x63; 32]).unwrap()),
                source::SourceOrigin::User,
            )
            .source_id;
        let tokens = source_files_to_tokens::Lexer::new(text).tokenize().unwrap();
        let syntax =
            tokens_to_syntax_trees::parse_syntax_trees_with_id(source_id, &tokens).unwrap();
        let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees_with_sources(
            &syntax,
            Arc::new(sources),
        )
        .unwrap();
        let program =
            symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
        let pending = pending_endpoints(&program);
        let endpoint = &pending[0];
        // Test argument admission directly so the callee's separate gate cannot
        // mask failure to consult the copied constant occurrence's authority.
        let (values, _) = arguments::evaluate(
            &program,
            endpoint.expression,
            endpoint.machine,
            Some(&Selection(true)),
        )
        .expect("admitted constant argument");
        assert_eq!(values, vec![crate::BuildTimeValue::Int(256)]);
        let error = arguments::evaluate(
            &program,
            endpoint.expression,
            endpoint.machine,
            Some(&Selection(false)),
        )
        .expect_err("the callee cannot grant selection authority to its arguments");
        assert!(
            error.contains("without direct dependency authority"),
            "{error}"
        );
    }
}
