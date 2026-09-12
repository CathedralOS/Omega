//! Authored target root-slot bindings from the admitted build machine.
//!
//! This implementation statically projects direct declarations from the selected
//! companion build machine. The language permits evaluated helper/alias binding
//! through borrowed Build authority (build/declarations.md, Evaluated build work);
//! those routes require evaluated binding receipts and are not implemented here.
//! Reject them explicitly instead of treating an ignored declaration as success.

use diagnostics::Diagnostic;
use typed_trees::TypedTrees;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootBinding {
    pub slot: String,
    pub implementation: String,
}

pub(crate) fn validate_root_binding_owners(
    typed: &TypedTrees,
    build_source_id: Option<source::SourceId>,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    for machine in typed.machines() {
        if super::super::is_build_machine(typed, machine, build_source_id) {
            continue;
        }
        for state in typed.machine_states(machine) {
            for statement in typed.statement_table.statements(state.statement_nodes) {
                if let typed_trees::statement::StatementNode::RootBinding(binding) = statement {
                    diagnostics.push(Diagnostic::error("this root-binding implementation supports only declarations in the authoritative companion build machine; evaluated helper binding is not implemented")
                        .with_source_span(binding.source_span));
                }
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

/// Collect `builder.roots.bind(Target::Slot, Machine::entry);` declarations
/// from the one authoritative build machine. Slot membership and schema
/// checking belong to the selected target profile; this stage establishes the
/// closed, duplicate-free binding map and preserves the exact machine name.
pub(crate) fn harvest_root_bindings(
    typed: &TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> Result<Vec<RootBinding>, Vec<Diagnostic>> {
    let mut bindings: Vec<RootBinding> = Vec::new();
    let mut diagnostics = Vec::new();
    let mut record = |request: &typed_trees::statement::RootBinding| {
        let slot = product_path(&request.slot);
        let implementation = product_path(&request.implementation);
        if let Some(existing) = bindings.iter().find(|binding| binding.slot == slot) {
            diagnostics.push(Diagnostic::error(format!(
                "root slot `{slot}` is already bound to `{}`; it cannot also bind `{implementation}`",
                existing.implementation
            )));
            return;
        }
        bindings.push(RootBinding {
            slot,
            implementation,
        });
    };

    for state in typed.machine_states(machine) {
        for statement in typed.statement_table.statements(state.statement_nodes) {
            if let typed_trees::statement::StatementNode::RootBinding(binding) = statement {
                record(binding);
            }
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
