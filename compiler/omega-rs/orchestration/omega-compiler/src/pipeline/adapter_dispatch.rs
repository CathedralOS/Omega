//! PRV4 step (3) consumption: ADAPTER DISPATCH. A call through a field
//! whose declared type is a BOUNDARY trait rewrites to a direct call to the
//! unique checked adapter satisfying that requirement. The rewrite runs only
//! after semantic checking: the source call must first consume the boundary
//! requirement (and any admitted qualification receipt), while execution then
//! targets the selected checked adapter. It runs in BOTH engine pipelines so
//! the interpreter and native builds dispatch identically (the differential
//! contract).
//! Without a satisfying adapter the call keeps its host-lowering route
//! (the built-in tables or selected external leaves serve it).
//!
//! Adapters are static machines attached to a nominal provider type; receiver
//! state never reaches one. Selection chooses that type's whole conformance
//! closure. Two call shapes are admitted:
//! * EXACT: the adapter's entry signature matches the requirement -- the
//!   call rewrites to a bare call (the boundary field is dispatch-only).
//! * SELF-FORWARDING: the adapter takes the requirement's OWN trait as one
//!   extra LEADING parameter (`write_line_plus(console: Console, text)`
//!   satisfying `Console::write_line`) -- the call's receiver place is
//!   forwarded as the first argument, so the adapter body can reach the
//!   trait's remaining primitives through it. This is how a std surface
//!   method becomes proven Omega code over its own byte-level primitives.

use omega_core::diagnostics::Diagnostic;
use omega_typed_trees::TypedTrees;

struct AdapterRow {
    trait_leaf: String,
    requirement: String,
    requirement_identity: String,
    adapter_target: String,
    symbol: omega_core::symbols::SymbolHandle,
    /// Self-forwarding shape: prepend the call's receiver as argument 0.
    forward_receiver: bool,
}

pub(crate) fn rewrite_adapter_calls(
    typed: &mut TypedTrees,
    selected_plans: &omega_checked_trees::SelectedProviderPlanFacts,
) -> Result<(), Vec<Diagnostic>> {
    // (trait leaf, method) -> selected adapter row: machines with a body and a
    // requirement-named satisfies edge (no via) over a BOUNDARY trait. A
    // provider-type adapter is static (it receives no provider instance).
    let mut adapters: Vec<AdapterRow> = Vec::new();
    let mut diagnostics = Vec::new();
    for machine in typed.machines() {
        let Some(entry_state) = typed.machine_states(machine).first() else {
            continue; // bodyless = a via leaf, not an adapter
        };
        for conformance in typed.machine_trait_conformances(machine) {
            let Some(requirement) = conformance.requirement.as_ref() else {
                continue;
            };
            if conformance.via.is_some() {
                continue;
            }
            let Some(definition) = typed
                .traits()
                .iter()
                .find(|definition| definition.symbol == conformance.symbol)
                .filter(|definition| definition.is_boundary)
            else {
                continue;
            };
            let trait_leaf = definition
                .name
                .as_str()
                .rsplit("::")
                .next()
                .unwrap_or(definition.name.as_str())
                .to_owned();
            let selected_slot = selected_plans
                .plans()
                .iter()
                .find(|plan| plan.schema.trait_name == definition.name.as_str());
            let selected_row = selected_slot.and_then(|plan| {
                plan.rows.iter().find(|row| {
                    row.method == requirement.as_str()
                        && matches!(
                            &row.binding,
                            omega_effects::provider_plan::ProviderBinding::CheckedAdapter {
                                machine: selected_machine,
                            } if selected_machine == machine.name.as_str()
                        )
                })
            });
            // A retained whole-provider selection is authoritative: activate
            // only the exact checked-adapter rows copied into that immutable
            // plan. Unselected adapters never participate in dispatch.
            let Some(selected_row) = selected_row else {
                continue;
            };
            // Self-forwarding: entry takes the trait itself first, then the
            // requirement's parameters (the conformance validator admitted
            // the shape; this re-derivation only picks the rewrite form).
            let required_count = typed
                .trait_machine_signatures(definition)
                .iter()
                .find(|signature| {
                    signature.name == *requirement
                        && (selected_row.requirement_identity.is_empty()
                            || typed
                                .normalized_trait_requirement_overload_identity(
                                    definition, signature,
                                )
                                .identity()
                                == selected_row.requirement_identity)
                })
                .map(|signature| typed.state_signature_parameters(signature).len());
            let actual_parameters = typed.state_parameters(entry_state);
            let forward_receiver = required_count.is_some_and(|required| {
                actual_parameters.len() == required + 1
                    && actual_parameters.first().is_some_and(|parameter| {
                        parameter_type_leaf(typed, parameter) == Some(trait_leaf.clone())
                    })
            });
            // Preserve the complete selected machine identity. Free adapters
            // already have a one-segment name; nominal provider adapters need
            // their owner path (`Provider::method`) to remain executable after
            // the boundary receiver is removed.
            let leaf_name = machine.name.as_str().to_owned();
            if let Some(existing) = adapters.iter().find(|row| {
                row.trait_leaf == trait_leaf
                    && if selected_row.requirement_identity.is_empty() {
                        row.requirement == requirement.as_str()
                    } else {
                        row.requirement_identity == selected_row.requirement_identity
                    }
            }) {
                diagnostics.push(Diagnostic::error(format!(
                    "requirement `{trait_leaf}::{}` has two checked adapters \
                     (`{}` and `{leaf_name}`) -- adapter dispatch is \
                     implicit only when unique",
                    requirement.as_str(),
                    existing.adapter_target,
                )));
                continue;
            }
            adapters.push(AdapterRow {
                trait_leaf,
                requirement: requirement.as_str().to_owned(),
                requirement_identity: selected_row.requirement_identity.clone(),
                adapter_target: leaf_name,
                symbol: entry_state.symbol,
                forward_receiver,
            });
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
            let omega_typed_trees::types::TypeReferenceNode::Named { name, .. } = typed
                .type_reference_table
                .type_reference(field.type_reference)
            else {
                continue;
            };
            let type_name = name
                .as_str()
                .rsplit("::")
                .next()
                .unwrap_or(name.as_str())
                .to_owned();
            if adapters.iter().any(|row| row.trait_leaf == type_name) {
                field_traits.push((
                    data.name.as_str().to_owned(),
                    field.name.as_str().to_owned(),
                    type_name,
                ));
            }
        }
    }

    let trait_for_field = |attached: Option<&str>, field: &str| -> Option<String> {
        let attached = attached?;
        field_traits
            .iter()
            .find(|(data, name, _)| data == attached && name == field)
            .map(|(_, _, trait_leaf)| trait_leaf.clone())
    };

    // Statement calls: `self.<field>.<method>(..)` or `<place>.<method>(..)`
    // where the place's declared type is the boundary trait.
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
                let Some(row) = adapters.iter().find(|row| {
                    adapter_matches_call(
                        typed,
                        row,
                        &trait_leaf,
                        call.target.as_str(),
                        call.target_symbol,
                    )
                }) else {
                    continue;
                };
                let receiver_members: Vec<omega_typed_trees::name::Identifier> = typed
                    .statement_table
                    .name_path_members(call.receiver)
                    .to_vec();
                let mut rewritten = call.clone();
                if row.forward_receiver {
                    // The receiver place becomes argument 0 -- synthesized as
                    // the SAME expression tree the parser builds for
                    // `self.<field>` / `<field>` argument spellings.
                    let receiver_expression = synthesize_place_expression(
                        &mut typed.expression_table,
                        &receiver_members,
                        call.receiver_symbol,
                    );
                    let old_arguments = typed
                        .statement_table
                        .expression_handles(call.arguments)
                        .to_vec();
                    rewritten.arguments = typed.statement_table.insert_expression_handles(
                        std::iter::once(receiver_expression).chain(old_arguments),
                    );
                }
                rewritten.receiver = omega_core::arena::HandleSpan::empty();
                rewritten.receiver_symbol = omega_core::symbols::SymbolHandle::invalid();
                rewritten.target =
                    omega_typed_trees::name::Identifier::generated(row.adapter_target.clone());
                rewritten.target_symbol = row.symbol;
                typed.statement_table.statements_mut(span)[index] =
                    omega_typed_trees::statement::StatementNode::Call(rewritten);
            }
        }
    }

    // Value calls: walk every expression node; a Call with a Member
    // receiver (self.<field>) rewrites the same way. Forwarding prepends
    // the EXISTING receiver expression handle -- no synthesis needed.
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
        let Some((row_symbol, row_leaf, row_forward)) = field_traits
            .iter()
            .find(|(_, name, _)| *name == field)
            .and_then(|(_, _, trait_leaf)| {
                adapters.iter().find(|row| {
                    adapter_matches_call(
                        typed,
                        row,
                        trait_leaf,
                        call.target.as_str(),
                        call.target_symbol,
                    )
                })
            })
            .map(|row| (row.symbol, row.adapter_target.clone(), row.forward_receiver))
        else {
            continue;
        };
        let receiver = call.receiver;
        let old_arguments = typed
            .expression_table
            .expression_handles(call.arguments)
            .to_vec();
        let forwarded_arguments = row_forward.then(|| {
            typed
                .expression_table
                .insert_expression_handles(std::iter::once(receiver).chain(old_arguments))
        });
        let omega_typed_trees::expression::ExpressionNode::Call(call) =
            typed.expression_table.expression_mut(handle)
        else {
            continue;
        };
        call.receiver = omega_typed_trees::expression::ExpressionHandle::invalid();
        call.target = omega_typed_trees::name::Identifier::generated(row_leaf);
        call.target_symbol = row_symbol;
        if let Some(arguments) = forwarded_arguments {
            call.arguments = arguments;
        }
    }
    Ok(())
}

fn adapter_matches_call(
    typed: &TypedTrees,
    row: &AdapterRow,
    trait_leaf: &str,
    target_name: &str,
    target_symbol: omega_core::symbols::SymbolHandle,
) -> bool {
    if row.trait_leaf != trait_leaf || row.requirement != target_name {
        return false;
    }
    if row.requirement_identity.is_empty() {
        return true;
    }
    typed.traits().iter().any(|definition| {
        typed
            .trait_machine_signatures(definition)
            .iter()
            .any(|signature| {
                signature.symbol == target_symbol
                    && typed
                        .normalized_trait_requirement_overload_identity(definition, signature)
                        .identity()
                        == row.requirement_identity
            })
    })
}

/// Leaf name of a state parameter's declared type, when it is a plain named
/// reference (`console: Console` -> `Console`).
fn parameter_type_leaf(
    typed: &TypedTrees,
    parameter: &omega_typed_trees::signature::StateParameter,
) -> Option<String> {
    let omega_typed_trees::types::TypeReferenceNode::Named { name, .. } = typed
        .type_reference_table
        .type_reference(parameter.type_reference)
    else {
        return None;
    };
    Some(
        name.as_str()
            .rsplit("::")
            .next()
            .unwrap_or(name.as_str())
            .to_owned(),
    )
}

/// Build the argument expression for a forwarded receiver path: `[self, f]`
/// becomes `Member(Name([self]), f)` and `[f]` becomes `Name([f])` -- the
/// exact trees the parser produces for those argument spellings, so every
/// downstream pass sees a shape it already serves.
fn synthesize_place_expression(
    expressions: &mut omega_typed_trees::expression::ExpressionTable,
    members: &[omega_typed_trees::name::Identifier],
    receiver_symbol: omega_core::symbols::SymbolHandle,
) -> omega_typed_trees::expression::ExpressionHandle {
    use omega_typed_trees::expression::{ExpressionNode, TableMemberExpression, TableNamePath};
    match members {
        [head, field] => {
            let mut head_span = omega_core::arena::HandleSpan::empty();
            expressions.push_name_path_member(&mut head_span, head.clone());
            let head_expression = expressions.insert(ExpressionNode::Name(TableNamePath {
                members: head_span,
                member_symbols: omega_core::arena::HandleSpan::empty(),
                head_symbol: omega_core::symbols::SymbolHandle::invalid(),
                symbol: omega_core::symbols::SymbolHandle::invalid(),
            }));
            expressions.insert(ExpressionNode::Member(TableMemberExpression {
                receiver: head_expression,
                member_symbol: receiver_symbol,
                member: field.clone(),
                case_variant: None,
            }))
        }
        _ => {
            let mut span = omega_core::arena::HandleSpan::empty();
            for member in members {
                expressions.push_name_path_member(&mut span, member.clone());
            }
            expressions.insert(ExpressionNode::Name(TableNamePath {
                members: span,
                member_symbols: omega_core::arena::HandleSpan::empty(),
                head_symbol: receiver_symbol,
                symbol: receiver_symbol,
            }))
        }
    }
}
