//! Rejoin executed root-binding requests to the admitted build source.

use diagnostics::Diagnostic;
use typed_trees::TypedTrees;
use typed_trees::statement::{StatementHandle, StatementNode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootBinding {
    pub slot: String,
    pub implementation: String,
}

/// Only executed occurrences contribute. Interpreter coordinates belong to this
/// exact prepared program; target-slot and implementation admission remain with
/// the selected product. Reborrowing Build never changes lexical visibility.
pub(crate) fn collect_root_bindings(
    typed: &TypedTrees,
    machine: &typed_trees::machine::Machine,
    executed: &[StatementHandle],
) -> Result<Vec<RootBinding>, Vec<Diagnostic>> {
    let root_source = typed
        .symbols
        .symbol_source_span(machine.symbol)
        .and_then(|span| typed.symbols.source_file(span));
    let mut bindings: Vec<RootBinding> = Vec::with_capacity(executed.len());
    let mut diagnostics = Vec::new();
    for &statement in executed {
        let StatementNode::RootBinding(request) = typed.statement_table.statement(statement) else {
            return Err(vec![Diagnostic::error(
                "executed root-binding coordinate is not a declaration in the admitted program",
            )]);
        };
        let source = typed.symbols.source_file(request.source_span);
        let same_owner = root_source.zip(source).is_some_and(|(root, source)| {
            root.origin == source.origin
                && match (root.package_identity, source.package_identity) {
                    (Some(root), Some(owner)) => root == owner,
                    (None, None) => {
                        !root.package_root.as_os_str().is_empty()
                            && root.package_root == source.package_root
                    }
                    _ => false,
                }
        });
        if !same_owner {
            diagnostics.push(Diagnostic::error("root binding from a foreign build helper requires lexical product-reference admission, which is not implemented; borrowing Build does not grant the caller's product namespace")
                .with_source_span(request.source_span));
            continue;
        }
        let slot = product_path(&request.slot);
        let implementation = product_path(&request.implementation);
        if let Some(existing) = bindings.iter().find(|binding| binding.slot == slot) {
            diagnostics.push(Diagnostic::error(format!(
                "root slot `{slot}` is already bound to `{}`; it cannot also bind `{implementation}`", existing.implementation
            )).with_source_span(request.source_span));
            continue;
        }
        bindings.push(RootBinding {
            slot,
            implementation,
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
