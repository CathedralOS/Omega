//! Capture source ownership before executable Build bodies are erased.
use language_semantics::declaration_selection::{
    BuildSelectionRequest, NativeBindingFormation, ProductDeclarationRequest,
};
use std::sync::Arc;
pub(crate) fn capture(
    checked: &checked_trees::CheckedTrees,
    root: crate::TerminalMachineSelection<'_>,
    declared: &[Arc<ProductDeclarationRequest>],
) -> Result<Vec<NativeBindingFormation>, diagnostics::Diagnostic> {
    if !declared
        .iter()
        .any(|request| request.operation.is_native_intent())
    {
        return Ok(Vec::new());
    }
    let root = match root {
        crate::TerminalMachineSelection::Declarations => None,
        crate::TerminalMachineSelection::Symbol(symbol) => Some(symbol),
        crate::TerminalMachineSelection::Name(name) => {
            let mut matching = checked
                .machines()
                .iter()
                .filter(|machine| machine.name.as_str() == name);
            matching
                .next()
                .filter(|_| matching.next().is_none())
                .map(|machine| machine.symbol)
        }
    };
    let mut scopes: Vec<NativeBindingFormation> = Vec::new();
    for machine in checked.machines() {
        for state in checked.machine_states(machine) {
            for statement in checked.statement_table.statements(state.statement_nodes) {
                let mut expressions = Vec::new();
                validation::collect_statement_expression_trees(
                    checked,
                    statement,
                    &mut expressions,
                );
                for expression in expressions {
                    let typed_trees::expression::ExpressionNode::Call(call) =
                        checked.expression_table.expression(expression)
                    else {
                        continue;
                    };
                    let Some(BuildSelectionRequest::Declaration(request)) = checked
                        .expression_table
                        .build_selection_request(call.build_selection)
                    else {
                        continue;
                    };
                    if !request.operation.is_native_intent() {
                        continue;
                    }
                    let scope = typed_trees::product_operand::native_binding_formation(
                        &checked.typed,
                        machine.symbol,
                        root,
                        request.clone(),
                    )
                    .map_err(diagnostics::Diagnostic::error)?;
                    if let Some(prior) = scopes.iter().find(|prior| prior.request == scope.request)
                    {
                        if prior != &scope {
                            return Err(diagnostics::Diagnostic::error(
                                "native formation has ambiguous checked source ownership",
                            ));
                        }
                    } else {
                        scopes.push(scope);
                    }
                }
            }
        }
    }
    for request in declared
        .iter()
        .filter(|request| request.operation.is_native_intent())
    {
        if !scopes.iter().any(|scope| scope.request == *request) {
            return Err(diagnostics::Diagnostic::error(
                "native formation has no checked Build source owner",
            ));
        }
    }
    Ok(scopes)
}
