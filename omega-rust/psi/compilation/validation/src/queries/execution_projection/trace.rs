//! Optional diagnostics for declarations lost before a Build call is bound.
//!
//! This observes the original and projected owners without changing references,
//! choosing a callee or importing declarations into the checked program.

use typed_trees::TypedTrees;
use typed_trees::expression::ExpressionNode;

pub(super) fn unresolved_calls(original: &TypedTrees, projected: &TypedTrees) {
    if std::env::var_os("OMEGA_COMPILER_CHECK_SITES").is_none() {
        return;
    }
    let references = typed_trees::references::collect_program_references(projected);
    let original_references = typed_trees::references::collect_program_references(original);
    for expression in references.expressions {
        let ExpressionNode::Call(call) = projected.expression_table.expression(expression) else {
            continue;
        };
        if call.target_symbol.is_valid() {
            continue;
        }
        let Some(ExpressionNode::Name(receiver)) = projected
            .expression_table
            .expression_is_valid(call.receiver)
            .then(|| projected.expression_table.expression(call.receiver))
        else {
            continue;
        };
        let members = projected
            .expression_table
            .name_path_members(receiver.members);
        let path = members
            .iter()
            .map(|name| name.as_str())
            .chain([call.target.as_str()])
            .collect::<Vec<_>>()
            .join("::");
        let original_call = original
            .expression_table
            .expression_is_valid(expression)
            .then(|| original.expression_table.expression(expression));
        eprintln!(
            "Build projection unresolved call {expression:?}: path {path}, receiver {:?}, target {:?}, source binding {:?}, original expression {original_call:?}",
            receiver.symbol, call.target_symbol, call.authored_call_binding
        );
        for (label, owner) in [
            ("original", original),
            ("Product description", original.product_declarations()),
        ] {
            for machine in owner
                .machines()
                .iter()
                .filter(|machine| machine.name.as_str() == path.as_str())
            {
                let source = owner
                    .symbols
                    .symbol_provenance_source_span(machine.symbol)
                    .and_then(|span| owner.symbols.source_file(span));
                let source = source.map(|file| {
                    (
                        file.path.display().to_string(),
                        file.dependency_scope,
                        file.origin,
                    )
                });
                eprintln!(
                    "Build projection candidate in {label}: symbol {:?}, active {}, referenced {}, body {}, supply {:?}, source {source:?}",
                    machine.symbol,
                    projected.machine(machine.symbol).is_some(),
                    original_references.symbols.contains(&machine.symbol),
                    machine.body_is_present,
                    machine.supply_mode
                );
            }
        }
    }
}
