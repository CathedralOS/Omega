//! Rejoin executed root-binding requests to the admitted build source.

use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::statement::StatementNode;

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
///
/// A delegated `ProductEntryRef` operand instead rejoins the description the
/// compiler issued at the `product.entry` query: selection authority and the
/// package check belonged to the query occurrence, so the bind consumes the
/// exact retained symbol and requires the authored slot to match the queried
/// slot. The description names a product declaration only -- its machine was
/// looked up, never executed.
pub(crate) fn collect_root_bindings(
    typed: &TypedTrees,
    executed: &[checked_interpreter::ExecutedRootBinding],
) -> Result<Vec<RootBinding>, Vec<Diagnostic>> {
    let mut bindings: Vec<RootBinding> = Vec::with_capacity(executed.len());
    let mut diagnostics = Vec::new();
    for request in executed {
        let StatementNode::RootBinding(binding) =
            typed.statement_table.statement(request.statement)
        else {
            return Err(vec![Diagnostic::error(
                "executed root-binding coordinate is not a declaration in the admitted program",
            )]);
        };
        let slot = product_path(&binding.slot);
        let (implementation, implementation_symbol) = if let Some(described) = &request.described {
            if slot != described.slot {
                diagnostics.push(
                    Diagnostic::error(format!(
                        "root binding slot `{slot}` does not match the product description's slot `{}`",
                        described.slot
                    ))
                    .with_source_span(binding.source_span),
                );
                continue;
            }
            if !typed
                .machines()
                .iter()
                .any(|machine| machine.symbol == described.machine_symbol)
            {
                return Err(vec![
                    Diagnostic::error(
                        "product description names a machine outside the admitted program",
                    )
                    .with_source_span(binding.source_span),
                ]);
            }
            (described.machine_name.clone(), described.machine_symbol)
        } else {
            let implementation = product_path(&binding.implementation);
            let candidates = typed
                .machines()
                .iter()
                .filter(|machine| {
                    machine.name.as_str() == implementation
                        && typed
                            .symbols
                            .symbol_source_span(machine.symbol)
                            .is_some_and(|span| {
                                typed
                                    .symbols
                                    .same_product_package_instance(binding.source_span, span)
                            })
                })
                .collect::<Vec<_>>();
            match candidates.as_slice() {
                [machine] => (implementation, machine.symbol),
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
                    diagnostics
                        .push(Diagnostic::error(message).with_source_span(binding.source_span));
                    continue;
                }
                _ => {
                    diagnostics.push(
                        Diagnostic::error(format!(
                            "root binding `{implementation}` is ambiguous within its package"
                        ))
                        .with_source_span(binding.source_span),
                    );
                    continue;
                }
            }
        };
        if let Some(existing) = bindings.iter().find(|binding| binding.slot == slot) {
            diagnostics.push(Diagnostic::error(format!(
                "root slot `{slot}` is already bound to `{}`; it cannot also bind `{implementation}`", existing.implementation
            )).with_source_span(binding.source_span));
            continue;
        }
        bindings.push(RootBinding {
            slot,
            implementation,
            implementation_symbol,
        });
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
