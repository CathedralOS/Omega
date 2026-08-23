//! Psi pre-resolution bridge for zero-argument machine calls in const-generic
//! arguments (`Buffer<table_size()>`). Generic record instances must be
//! synthesized before the ordinary frontend runs, while the established
//! build-time evaluator needs typed trees. Build a sanitized probe program,
//! type it, reuse the same normalized build-time gate and interpreter entry as
//! fixed-array lengths, then substitute canonical decimal leaves into the
//! authoritative syntax tree before monomorphization.

use psi_diagnostics::Diagnostic;
use psi_numerics::literals::{IntegerLiteral, IntegerRadix};
use psi_syntax_trees::SyntaxTrees;
use psi_syntax_trees::expression::{ExpressionHandle, ExpressionNode};
use psi_syntax_trees::identifier::Identifier;
use psi_syntax_trees::types::TypeReferenceNode;
use std::collections::BTreeMap;

pub fn evaluate_const_generic_calls(
    mut syntax: SyntaxTrees,
) -> Result<SyntaxTrees, Vec<Diagnostic>> {
    let mut pending = Vec::new();
    let mut pending_type_references = Vec::new();
    for (type_reference, expression) in syntax.type_references.const_expression_nodes() {
        let before = pending.len();
        collect_call_leaves(&syntax, expression, &mut pending)?;
        if pending.len() > before {
            pending_type_references.push(type_reference);
        }
    }
    if pending.is_empty() {
        return Ok(syntax);
    }

    // The probe needs the same generic-template normalization as the real
    // program, but the call results are not known yet. A temporary zero leaf
    // for each owning const argument lets the frontend type/effect-check the
    // machine definitions themselves; no probe layout escapes this function.
    let mut probe = syntax.clone();
    for type_reference in &pending_type_references {
        probe.type_references.replace_type_reference(
            *type_reference,
            TypeReferenceNode::Named(Identifier::generated("0")),
        );
    }
    let probe = psi_generic_instances::normalize_pre_resolution(probe)?;
    let resolved = psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&probe)?;
    let typed = psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
        .map_err(|diagnostic| vec![diagnostic])?;
    let admission = crate::BuildTimeAdmissionPlan::infer(&typed);

    let mut values: BTreeMap<String, u64> = BTreeMap::new();
    for (_, machine_name) in &pending {
        if values.contains_key(machine_name) {
            continue;
        }
        let value = crate::evaluate_zero_argument_machine(
            &typed,
            &admission,
            machine_name,
            "generic argument",
        )
        .map_err(|reason| {
            vec![Diagnostic::error(format!(
                "const-generic evaluation of `{machine_name}()` failed: {reason}"
            ))]
        })?;
        let value = u64::try_from(value).map_err(|_| {
            vec![Diagnostic::error(format!(
                "const-generic evaluation of `{machine_name}()` returned {value}, but const data arguments must be non-negative"
            ))]
        })?;
        values.insert(machine_name.clone(), value);
    }

    for (expression, machine_name) in pending {
        let value = values[&machine_name];
        let literal =
            IntegerLiteral::from_parts(false, IntegerRadix::Decimal, value.to_string().as_str())
                .expect("a decimal u64 const-machine result is a valid integer literal");
        syntax
            .expressions
            .replace_expression(expression, ExpressionNode::Integer(literal));
    }
    Ok(syntax)
}

fn collect_call_leaves(
    syntax: &SyntaxTrees,
    expression: ExpressionHandle,
    pending: &mut Vec<(ExpressionHandle, String)>,
) -> Result<(), Vec<Diagnostic>> {
    match syntax.expressions.expression(expression) {
        ExpressionNode::Binary(binary) => {
            collect_call_leaves(syntax, binary.left, pending)?;
            collect_call_leaves(syntax, binary.right, pending)
        }
        ExpressionNode::Call(call) => {
            if !call.arguments.is_empty() || !call.machine_arguments.is_empty() {
                return Err(vec![Diagnostic::error(format!(
                    "const-generic call `{}` must take no value or machine arguments",
                    call.target.as_str()
                ))]);
            }
            let machine_name = call_machine_name(syntax, call)?;
            pending.push((expression, machine_name));
            Ok(())
        }
        _ => Ok(()),
    }
}

fn call_machine_name(
    syntax: &SyntaxTrees,
    call: &psi_syntax_trees::expression::TableCallExpression,
) -> Result<String, Vec<Diagnostic>> {
    if !call.receiver.is_valid() {
        return Ok(call.target.as_str().to_string());
    }
    let ExpressionNode::Name(path) = syntax.expressions.expression(call.receiver) else {
        return Err(vec![Diagnostic::error(
            "a const-generic machine call must use a free or type-scoped machine path",
        )]);
    };
    let mut name = syntax
        .expressions
        .identifier_path_members(*path)
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::");
    if !name.is_empty() {
        name.push_str("::");
    }
    name.push_str(call.target.as_str());
    Ok(name)
}
