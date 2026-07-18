//! PRV4 step (3) consumption: ADAPTER DISPATCH. A call through a field
//! whose declared type is a BOUNDARY trait rewrites to a direct call to the
//! unique checked adapter satisfying that requirement -- the same
//! pre-checking typed rewrite family as const lengths and MP4
//! specialization, and it runs in BOTH engine pipelines so the interpreter
//! and native builds dispatch identically (the differential contract).
//! Without a satisfying adapter the call keeps its host-lowering route
//! (the built-in tables / provides rows serve it exactly as before).
//! Receiver state never reaches an adapter: v1 adapters are FREE machines
//! and the boundary field is dispatch-only.

use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;

pub(crate) fn rewrite_adapter_calls(typed: &mut TypedTrees) -> Result<(), Vec<Diagnostic>> {
    // (trait leaf, method) -> (adapter leaf name, adapter symbol), FREE
    // machines with a body and a requirement-named satisfies edge (no via)
    // over a BOUNDARY trait. Two adapters for one requirement refuse.
    let mut adapters: Vec<(String, String, String, omega_core::symbols::SymbolHandle)> =
        Vec::new();
    let mut diagnostics = Vec::new();
    for machine in typed.machines() {
        if machine.attached_data.is_some() {
            continue; // v1: free adapters only (the field is dispatch-only)
        }
        if typed.machine_states(machine).is_empty() {
            continue; // bodyless = a via leaf, not an adapter
        }
        for conformance in typed.machine_trait_conformances(machine) {
            let Some(requirement) = conformance.requirement.as_ref() else {
                continue;
            };
            if conformance.via.is_some() {
                continue;
            }
            let trait_leaf = conformance.name.as_str().to_owned();
            let is_boundary = typed.traits().iter().any(|definition| {
                definition.is_boundary
                    && definition
                        .name
                        .as_str()
                        .rsplit("::")
                        .next()
                        .is_some_and(|leaf| leaf == trait_leaf)
            });
            if !is_boundary {
                continue;
            }
            let leaf_name = machine
                .name
                .as_str()
                .rsplit("::")
                .next()
                .unwrap_or(machine.name.as_str())
                .to_owned();
            if let Some((_, _, existing, _)) = adapters
                .iter()
                .find(|(t, m, _, _)| *t == trait_leaf && m == requirement.as_str())
            {
                diagnostics.push(Diagnostic::error(format!(
                    "requirement `{trait_leaf}::{}` has two checked adapters \
                     (`{existing}` and `{leaf_name}`) -- adapter dispatch is \
                     implicit only when unique",
                    requirement.as_str(),
                )));
                continue;
            }
            adapters.push((
                trait_leaf,
                requirement.as_str().to_owned(),
                leaf_name,
                machine.symbol,
            ));
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    if adapters.is_empty() {
        return Ok(());
    }

    // Field type resolution: (machine attached-data leaf, field name) ->
    // boundary-trait leaf. Built from every data definition up front.
    let mut field_traits: Vec<(String, String, String)> = Vec::new();
    for data in typed.data_definitions() {
        for member in typed.data_members(data) {
            let omega_typed_trees::data::DataMember::Field(field) = member else {
                continue;
            };
            let omega_typed_trees::types::TypeReferenceNode::Named { name, .. } =
                typed.type_reference_table.type_reference(field.type_reference)
            else {
                continue;
            };
            let type_name = name
                .as_str()
                .rsplit("::")
                .next()
                .unwrap_or(name.as_str())
                .to_owned();
            if adapters.iter().any(|(t, _, _, _)| *t == type_name) {
                field_traits.push((
                    data.name.as_str().to_owned(),
                    field.name.as_str().to_owned(),
                    type_name,
                ));
            }
        }
    }

    // The rewrite: statement calls `self.<field>.<method>(..)` and value
    // calls with a member receiver. Collected first (the borrow), applied
    // second.
    let mut statement_rewrites: Vec<(usize, (String, omega_core::symbols::SymbolHandle))> =
        Vec::new();
    let mut machine_data: Vec<(omega_core::symbols::SymbolHandle, Option<String>)> = Vec::new();
    for machine in typed.machines() {
        machine_data.push((
            machine.symbol,
            machine
                .attached_data
                .as_ref()
                .map(|name| name.as_str().to_owned()),
        ));
    }
    let _ = &machine_data;

    let trait_for_field = |attached: Option<&str>, field: &str| -> Option<String> {
        let attached = attached?;
        field_traits
            .iter()
            .find(|(data, name, _)| data == attached && name == field)
            .map(|(_, _, trait_leaf)| trait_leaf.clone())
    };

    // Statement calls.
    let machines: Vec<_> = typed
        .machines()
        .iter()
        .map(|machine| {
            (
                machine
                    .attached_data
                    .as_ref()
                    .map(|name| name.as_str().to_owned()),
                typed
                    .machine_states(machine)
                    .iter()
                    .map(|state| state.statement_nodes)
                    .collect::<Vec<_>>(),
            )
        })
        .collect();
    let _ = &statement_rewrites;
    for (attached, spans) in machines {
        for span in spans {
            let statements = typed.statement_table.statements(span).to_vec();
            for (index, statement) in statements.iter().enumerate() {
                let omega_typed_trees::statement::StatementNode::Call(call) = statement else {
                    continue;
                };
                // receiver path [self, field] or [field]
                let members = typed.statement_table.name_path_members(call.receiver);
                let field = match members {
                    [one] => one.as_str().to_owned(),
                    [head, one] if head.as_str() == "self" => one.as_str().to_owned(),
                    _ => continue,
                };
                let Some(trait_leaf) = trait_for_field(attached.as_deref(), &field) else {
                    continue;
                };
                let Some((_, _, adapter, symbol)) = adapters
                    .iter()
                    .find(|(t, m, _, _)| *t == trait_leaf && m == call.target.as_str())
                else {
                    continue;
                };
                let mut rewritten = call.clone();
                rewritten.receiver = omega_core::arena::HandleSpan::empty();
                rewritten.receiver_symbol = omega_core::symbols::SymbolHandle::invalid();
                rewritten.target =
                    omega_typed_trees::name::Identifier::generated(adapter.clone());
                rewritten.target_symbol = *symbol;
                typed.statement_table.statements_mut(span)[index] =
                    omega_typed_trees::statement::StatementNode::Call(rewritten);
            }
        }
    }

    // Value calls: walk every expression node; a Call with a Member
    // receiver (self.<field>) rewrites the same way.
    let handles: Vec<_> = typed
        .expression_table
        .expression_entries()
        .map(|(handle, _)| handle)
        .collect();
    for handle in handles {
        let omega_typed_trees::expression::ExpressionNode::Call(call) =
            typed.expression_table.expression(handle)
        else {
            continue;
        };
        if !call.receiver.is_valid() {
            continue;
        }
        // receiver: Member(self, field) or Name(field)
        let field = match typed.expression_table.expression(call.receiver) {
            omega_typed_trees::expression::ExpressionNode::Member(member) => {
                member.member.as_str().to_owned()
            }
            omega_typed_trees::expression::ExpressionNode::Name(path) => {
                match typed.expression_table.name_path_members(path.members) {
                    [one] => one.as_str().to_owned(),
                    [head, one] if head.as_str() == "self" => one.as_str().to_owned(),
                    _ => continue,
                }
            }
            _ => continue,
        };
        // v1: the field's owner is resolved by NAME across all data --
        // ambiguity (same field name, different data, adapter trait) is
        // acceptable here because the adapter row already names the trait.
        let Some((_, _, adapter, symbol)) = field_traits
            .iter()
            .find(|(_, name, _)| *name == field)
            .and_then(|(_, _, trait_leaf)| {
                adapters
                    .iter()
                    .find(|(t, m, _, _)| t == trait_leaf && m == call.target.as_str())
            })
            .map(|found| found.clone())
        else {
            continue;
        };
        let omega_typed_trees::expression::ExpressionNode::Call(call) =
            typed.expression_table.expression_mut(handle)
        else {
            continue;
        };
        call.receiver = omega_typed_trees::expression::ExpressionHandle::invalid();
        call.target = omega_typed_trees::name::Identifier::generated(adapter);
        call.target_symbol = symbol;
    }
    Ok(())
}
