//! Rejoin executed root-binding requests to the admitted build source.

use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::statement::{StatementHandle, StatementNode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootBinding {
    pub slot: String,
    pub implementation: String,
    /// Exact product machine selected under the occurrence's lexical package scope.
    pub implementation_symbol: symbols::SymbolHandle,
}

/// Admission is lexical per occurrence package: the `implementation` operand
/// resolves against the product machines declared in the package containing
/// the `roots.bind` statement, never the caller's. A borrowed root Build
/// carries operational authority, not the caller's product namespace, so a
/// foreign helper can bind its own package's entry while wrong-scope spellings
/// reject. Identity leaves here as an exact symbol so later admission never
/// reselects by spelling.
pub(crate) fn collect_root_bindings(
    typed: &TypedTrees,
    executed: &[StatementHandle],
) -> Result<Vec<RootBinding>, Vec<Diagnostic>> {
    let mut bindings: Vec<RootBinding> = Vec::with_capacity(executed.len());
    let mut diagnostics = Vec::new();
    for &statement in executed {
        let StatementNode::RootBinding(request) = typed.statement_table.statement(statement) else {
            return Err(vec![Diagnostic::error(
                "executed root-binding coordinate is not a declaration in the admitted program",
            )]);
        };
        let slot = product_path(&request.slot);
        let implementation = product_path(&request.implementation);
        if let Some(existing) = bindings.iter().find(|binding| binding.slot == slot) {
            diagnostics.push(Diagnostic::error(format!(
                "root slot `{slot}` is already bound to `{}`; it cannot also bind `{implementation}`", existing.implementation
            )).with_source_span(request.source_span));
            continue;
        }
        let candidates = typed
            .machines()
            .iter()
            .filter(|machine| {
                machine.name.as_str() == implementation
                    && typed
                        .symbols
                        .symbol_source_span(machine.symbol)
                        .is_some_and(|span| {
                            typed.symbols.same_source_package(span, request.source_span)
                        })
            })
            .collect::<Vec<_>>();
        match candidates.as_slice() {
            [machine] => bindings.push(RootBinding {
                slot,
                implementation,
                implementation_symbol: machine.symbol,
            }),
            [] => {
                let message = if typed
                    .machines()
                    .iter()
                    .any(|machine| machine.name.as_str() == implementation)
                {
                    format!(
                        "root binding names `{implementation}`, which is not a product declaration visible from this build source's package; borrowing Build does not grant the caller's product namespace"
                    )
                } else {
                    format!(
                        "root binding names unknown entry machine `{implementation}` in this build source's package"
                    )
                };
                diagnostics.push(Diagnostic::error(message).with_source_span(request.source_span));
            }
            _ => diagnostics.push(
                Diagnostic::error(format!(
                    "root binding `{implementation}` is ambiguous within its package"
                ))
                .with_source_span(request.source_span),
            ),
        }
    }
    if diagnostics.is_empty() {
        Ok(bindings)
    } else {
        Err(diagnostics)
    }
}

fn product_path(members: &[typed_trees::name::Identifier]) -> String {
    let mut path = String::new();
    for member in members {
        if !path.is_empty() {
            path.push_str("::");
        }
        path.push_str(member.as_str());
    }
    path
}
