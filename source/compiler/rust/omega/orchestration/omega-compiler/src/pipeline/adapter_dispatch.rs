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

use psi_diagnostics::Diagnostic;
use psi_typed_trees::TypedTrees;

#[derive(Debug, Clone, PartialEq, Eq)]
struct AdapterRow {
    receiver_trait: psi_symbols::SymbolHandle,
    receiver_trait_name: String,
    requirement: String,
    requirement_identity: String,
    requirement_symbol: psi_symbols::SymbolHandle,
    adapter_target: String,
    symbol: psi_symbols::SymbolHandle,
    /// Self-forwarding shape: prepend the call's receiver as argument 0.
    forward_receiver: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BoundaryField {
    symbol: psi_symbols::SymbolHandle,
    trait_symbol: psi_symbols::SymbolHandle,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BoundaryFieldDeclaration {
    owner: psi_symbols::SymbolHandle,
    owner_name: String,
    field_name: String,
    field: BoundaryField,
}

#[derive(Debug, Clone)]
struct StatementRewrite {
    statements: psi_arena::HandleSpan<psi_typed_trees::statement::StatementNode>,
    index: usize,
    call: psi_typed_trees::statement::TableCall,
    receiver_members: Vec<psi_typed_trees::name::Identifier>,
    adapter: AdapterRow,
}

#[derive(Debug, Clone)]
struct ExpressionRewrite {
    expression: psi_typed_trees::expression::ExpressionHandle,
    call: psi_typed_trees::expression::TableCallExpression,
    adapter: AdapterRow,
}

pub(crate) fn rewrite_adapter_calls(
    typed: &mut TypedTrees,
    selected_plans: &omega_effects::SelectedProviderPlanFacts,
) -> Result<(), Vec<Diagnostic>> {
    let mut adapters = Vec::new();
    let mut diagnostics = Vec::new();
    for plan in selected_plans.plans() {
        for row in &plan.rows {
            match resolve_selected_adapter_row(typed, plan, row) {
                Ok(Some(adapter)) => {
                    if let Some(existing) = adapters.iter().find(|existing: &&AdapterRow| {
                        existing.receiver_trait == adapter.receiver_trait
                            && existing.requirement_symbol == adapter.requirement_symbol
                    }) {
                        diagnostics.push(Diagnostic::error(format!(
                            "selected boundary requirement `{}` has two checked adapters (`{}` and `{}`)",
                            adapter.requirement_identity,
                            existing.adapter_target,
                            adapter.adapter_target,
                        )));
                    } else {
                        adapters.push(adapter);
                    }
                }
                Ok(None) => {}
                Err(diagnostic) => diagnostics.push(diagnostic),
            }
        }
    }
    if adapters.is_empty() {
        return diagnostics.is_empty().then_some(()).ok_or(diagnostics);
    }

    // Exact typed field symbol -> exact boundary-trait symbol. Field spellings
    // may repeat across data owners and never participate in dispatch.
    let mut boundary_field_declarations = Vec::new();
    for data in typed.data_definitions() {
        for member in typed.data_members(data) {
            let psi_typed_trees::data::DataMember::Field(field) = member else {
                continue;
            };
            let psi_typed_trees::types::TypeReferenceNode::Named { symbol, .. } = typed
                .type_reference_table
                .type_reference(field.type_reference)
            else {
                continue;
            };
            if !adapters
                .iter()
                .any(|adapter| adapter.receiver_trait == *symbol)
            {
                continue;
            }
            if !field.symbol.is_valid() {
                diagnostics.push(Diagnostic::error(format!(
                    "boundary field `{}::{}` has no exact typed symbol for adapter dispatch",
                    data.name, field.name,
                )));
                continue;
            }
            if let Some(existing) = boundary_field_declarations
                .iter()
                .find(|existing: &&BoundaryFieldDeclaration| existing.field.symbol == field.symbol)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "boundary field symbol {:?} maps to both trait symbols {:?} and {:?}",
                    field.symbol, existing.field.trait_symbol, symbol,
                )));
                continue;
            }
            boundary_field_declarations.push(BoundaryFieldDeclaration {
                owner: data.symbol,
                owner_name: data.name.as_str().to_owned(),
                field_name: field.name.as_str().to_owned(),
                field: BoundaryField {
                    symbol: field.symbol,
                    trait_symbol: *symbol,
                },
            });
        }
    }

    // A direct `self.<field>` occurrence is stamped with the exact inherited
    // FIELD child of the attached machine, not the DATA declaration's field
    // symbol. Retain both coordinates: the declaration symbol serves nested
    // receiver leaves, while every exact machine-owned inherited symbol serves
    // direct statement and value receivers. Spelling is used only inside the
    // already-exact machine owner to resolve its unique child.
    let mut boundary_fields = boundary_field_declarations
        .iter()
        .map(|declaration| declaration.field)
        .collect::<Vec<_>>();
    for machine in typed.machines() {
        let Some(attached_data) = machine.attached_data.as_ref() else {
            continue;
        };
        let owners = typed
            .data_definitions()
            .iter()
            .filter(|data| data.name == *attached_data)
            .collect::<Vec<_>>();
        let [owner] = owners.as_slice() else {
            if boundary_field_declarations
                .iter()
                .any(|declaration| declaration.owner_name == attached_data.as_str())
            {
                diagnostics.push(Diagnostic::error(format!(
                    "adapter-dispatch machine `{}` resolves attached data `{attached_data}` to {} exact definitions",
                    machine.name,
                    owners.len(),
                )));
            }
            continue;
        };
        for declaration in boundary_field_declarations
            .iter()
            .filter(|declaration| declaration.owner == owner.symbol)
        {
            let symbols = machine
                .symbol
                .is_valid()
                .then(|| typed.symbols.child_handles(machine.symbol))
                .flatten()
                .into_iter()
                .flatten()
                .filter(|symbol| {
                    typed.symbols.get(*symbol).kind == psi_symbols::SymbolKind::Field
                        && typed.symbols.name(*symbol) == declaration.field_name
                })
                .collect::<Vec<_>>();
            let [symbol] = symbols.as_slice() else {
                diagnostics.push(Diagnostic::error(format!(
                    "adapter-dispatch machine `{}` resolves inherited boundary field `{}::{}` to {} exact FIELD children",
                    machine.name,
                    declaration.owner_name,
                    declaration.field_name,
                    symbols.len(),
                )));
                continue;
            };
            boundary_fields.push(BoundaryField {
                symbol: *symbol,
                trait_symbol: declaration.field.trait_symbol,
            });
        }
    }

    let machine_statement_spans = typed
        .machines()
        .iter()
        .flat_map(|machine| {
            typed
                .machine_states(machine)
                .iter()
                .map(|state| state.statement_nodes)
        })
        .collect::<Vec<_>>();
    let mut statement_rewrites = Vec::new();
    for span in machine_statement_spans {
        let statements = typed.statement_table.statements(span).to_vec();
        for (index, statement) in statements.iter().enumerate() {
            let psi_typed_trees::statement::StatementNode::Call(call) = statement else {
                continue;
            };
            // receiver path [self, field] or [field]
            let members = typed.statement_table.name_path_members(call.receiver);
            match members {
                [_] => {}
                [head, _] if head.as_str() == "self" => {}
                _ => continue,
            }
            let row = match resolve_adapter_call(
                &adapters,
                &boundary_fields,
                call.receiver_symbol,
                call.target_symbol,
                call.target.as_str(),
            ) {
                Ok(Some(row)) => row,
                Ok(None) => continue,
                Err(diagnostic) => {
                    diagnostics.push(diagnostic);
                    continue;
                }
            };
            let receiver_members: Vec<psi_typed_trees::name::Identifier> = typed
                .statement_table
                .name_path_members(call.receiver)
                .to_vec();
            statement_rewrites.push(StatementRewrite {
                statements: span,
                index,
                call: call.clone(),
                receiver_members,
                adapter: row.clone(),
            });
        }
    }

    // Value calls: walk every expression node; a Call with a Member
    // receiver (self.<field>) rewrites the same way. Forwarding prepends
    // the EXISTING receiver expression handle -- no synthesis needed.
    let handles = typed
        .expression_table
        .expression_entries()
        .map(|(handle, _)| handle)
        .collect::<Vec<_>>();
    let mut expression_rewrites = Vec::new();
    for handle in handles {
        let psi_typed_trees::expression::ExpressionNode::Call(call) =
            typed.expression_table.expression(handle)
        else {
            continue;
        };
        if !call.receiver.is_valid() {
            continue;
        }
        // receiver: Member(self, field) or Name(field)
        let receiver_symbol = match typed.expression_table.expression(call.receiver) {
            psi_typed_trees::expression::ExpressionNode::Member(member) => member.member_symbol,
            psi_typed_trees::expression::ExpressionNode::Name(path) => {
                match typed.expression_table.name_path_members(path.members) {
                    [_] => path.symbol,
                    [head, _] if head.as_str() == "self" => path.symbol,
                    _ => continue,
                }
            }
            _ => continue,
        };
        let row = match resolve_adapter_call(
            &adapters,
            &boundary_fields,
            receiver_symbol,
            call.target_symbol,
            call.target.as_str(),
        ) {
            Ok(Some(row)) => row,
            Ok(None) => continue,
            Err(diagnostic) => {
                diagnostics.push(diagnostic);
                continue;
            }
        };
        expression_rewrites.push(ExpressionRewrite {
            expression: handle,
            call: call.clone(),
            adapter: row.clone(),
        });
    }

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    for mut rewrite in statement_rewrites {
        if rewrite.adapter.forward_receiver {
            let receiver_expression = synthesize_place_expression(
                &mut typed.expression_table,
                &rewrite.receiver_members,
                rewrite.call.receiver_symbol,
            );
            let old_arguments = typed
                .statement_table
                .expression_handles(rewrite.call.arguments)
                .to_vec();
            rewrite.call.arguments = typed.statement_table.insert_expression_handles(
                std::iter::once(receiver_expression).chain(old_arguments),
            );
        }
        rewrite.call.receiver = psi_arena::HandleSpan::empty();
        rewrite.call.receiver_symbol = psi_symbols::SymbolHandle::invalid();
        rewrite.call.target =
            psi_typed_trees::name::Identifier::generated(rewrite.adapter.adapter_target);
        rewrite.call.target_symbol = rewrite.adapter.symbol;
        typed.statement_table.statements_mut(rewrite.statements)[rewrite.index] =
            psi_typed_trees::statement::StatementNode::Call(rewrite.call);
    }

    for mut rewrite in expression_rewrites {
        if rewrite.adapter.forward_receiver {
            let old_arguments = typed
                .expression_table
                .expression_handles(rewrite.call.arguments)
                .to_vec();
            rewrite.call.arguments = typed.expression_table.insert_expression_handles(
                std::iter::once(rewrite.call.receiver).chain(old_arguments),
            );
        }
        rewrite.call.receiver = psi_typed_trees::expression::ExpressionHandle::invalid();
        rewrite.call.target =
            psi_typed_trees::name::Identifier::generated(rewrite.adapter.adapter_target);
        rewrite.call.target_symbol = rewrite.adapter.symbol;
        *typed.expression_table.expression_mut(rewrite.expression) =
            psi_typed_trees::expression::ExpressionNode::Call(rewrite.call);
    }

    Ok(())
}

fn resolve_selected_adapter_row(
    typed: &TypedTrees,
    plan: &omega_effects::provider_plan::ProviderPlan,
    row: &omega_effects::provider_plan::ProviderPlanRow,
) -> Result<Option<AdapterRow>, Diagnostic> {
    use omega_effects::provider_plan::ProviderBinding;

    let ProviderBinding::CheckedAdapter {
        machine_identity, ..
    } = &row.binding
    else {
        return Ok(None);
    };
    if typed.operators().iter().any(|operator| {
        operator.is_boundary
            && psi_typed_trees::operator::boundary_operator_requirement_identity(typed, operator)
                == plan.schema.trait_name
    }) {
        return Ok(None);
    }
    if row.requirement_identity.is_empty() {
        return Err(Diagnostic::error(format!(
            "selected checked-adapter row `{}` in ProviderPlan `{}` has no exact overload identity",
            row.method, plan.name,
        )));
    }
    let methods = plan
        .schema
        .methods
        .iter()
        .filter(|method| plan.schema.row_binds_method(row, method))
        .collect::<Vec<_>>();
    let [method] = methods.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected checked-adapter row `{}` / `{}` in ProviderPlan `{}` binds {} exact schema methods",
            row.method,
            row.requirement_identity,
            plan.name,
            methods.len(),
        )));
    };

    let receiver_trait = exact_boundary_trait(typed, &plan.schema.trait_name, "selected schema")?;
    let requirement_owner =
        exact_boundary_trait(typed, &method.requirement_owner, "requirement owner")?;
    let signatures = typed
        .trait_machine_signatures(requirement_owner)
        .iter()
        .filter(|signature| {
            signature.name.as_str() == method.name
                && typed
                    .normalized_trait_requirement_overload_identity(requirement_owner, signature)
                    .identity()
                    == method.requirement_identity
        })
        .collect::<Vec<_>>();
    let [signature] = signatures.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected checked-adapter requirement `{}` resolves to {} exact typed signatures",
            method.requirement_identity,
            signatures.len(),
        )));
    };
    if !signature.symbol.is_valid() {
        return Err(Diagnostic::error(format!(
            "selected checked-adapter requirement `{}` has no exact typed symbol",
            method.requirement_identity,
        )));
    }

    if plan.provider_type.is_empty() {
        return Err(Diagnostic::error(format!(
            "selected checked-adapter ProviderPlan `{}` has no nominal provider type",
            plan.name,
        )));
    }
    let adapter = super::provider_plans::exact_checked_adapter(typed, plan, row)?;
    if adapter.attached_data.as_ref().map(|owner| owner.as_str())
        != Some(plan.provider_type.as_str())
    {
        return Err(Diagnostic::error(format!(
            "selected checked adapter `{machine_identity}` does not belong to nominal provider `{}`",
            plan.provider_type,
        )));
    }
    if !adapter.supply_mode.is_checked_body() {
        return Err(Diagnostic::error(format!(
            "selected checked adapter `{machine_identity}` is not a checked body",
        )));
    }
    let Some(entry) = typed.machine_states(adapter).first() else {
        return Err(Diagnostic::error(format!(
            "selected checked adapter `{machine_identity}` has no executable entry state",
        )));
    };
    if !entry.symbol.is_valid() {
        return Err(Diagnostic::error(format!(
            "selected checked adapter `{machine_identity}` has no exact entry-state symbol",
        )));
    }

    let conformances = typed
        .machine_trait_conformances(adapter)
        .iter()
        .filter(|conformance| {
            conformance.external_binding.is_none()
                && conformance.symbol == requirement_owner.symbol
                && conformance
                    .requirement
                    .as_ref()
                    .is_some_and(|requirement| requirement.as_str() == method.name)
                && exact_conformance_requirement_identity(
                    typed,
                    adapter,
                    requirement_owner,
                    method.name.as_str(),
                )
                .as_deref()
                    == Some(method.requirement_identity.as_str())
        })
        .count();
    if conformances != 1 {
        return Err(Diagnostic::error(format!(
            "selected checked adapter `{machine_identity}` binds exact overload `{}` through {conformances} checked conformances",
            method.requirement_identity,
        )));
    }

    // ServiceMethod arity excludes the language receiver. A concrete
    // provider's `self` is therefore part of its exact realization, not the
    // explicit boundary binding that adapter dispatch may forward.
    let actual_parameters = typed
        .state_parameters(entry)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    let forward_receiver = match exact_adapter_receiver_shape(
        typed,
        &actual_parameters,
        method.parameter_count,
        requirement_owner.symbol,
    ) {
        Some(forward_receiver) => forward_receiver,
        None => {
            let count = actual_parameters.len();
            return Err(Diagnostic::error(format!(
                "selected checked adapter `{machine_identity}` has {count} non-self entry parameters; exact overload `{}` requires {} or one leading `{}` receiver",
                method.requirement_identity, method.parameter_count, requirement_owner.name,
            )));
        }
    };

    Ok(Some(AdapterRow {
        receiver_trait: receiver_trait.symbol,
        receiver_trait_name: receiver_trait.name.as_str().to_owned(),
        requirement: method.name.clone(),
        requirement_identity: method.requirement_identity.clone(),
        requirement_symbol: signature.symbol,
        adapter_target: adapter.name.as_str().to_owned(),
        symbol: entry.symbol,
        forward_receiver,
    }))
}

fn exact_conformance_requirement_identity(
    typed: &TypedTrees,
    adapter: &psi_typed_trees::machine::Machine,
    owner: &psi_typed_trees::trait_definition::TraitDefinition,
    requirement: &str,
) -> Option<String> {
    let signatures = typed
        .trait_machine_signatures(owner)
        .iter()
        .filter(|signature| signature.name.as_str() == requirement)
        .collect::<Vec<_>>();
    let signature = match signatures.as_slice() {
        [signature] => *signature,
        many => {
            let implementation_dispatch = typed
                .machine_states(adapter)
                .first()
                .map(|entry| typed.normalized_result_dispatch_set(entry.return_type));
            let matches = many
                .iter()
                .copied()
                .filter(|signature| {
                    implementation_dispatch.as_ref().is_some_and(|dispatch| {
                        typed.normalized_result_dispatch_set(signature.return_type) == *dispatch
                    })
                })
                .collect::<Vec<_>>();
            let [signature] = matches.as_slice() else {
                return None;
            };
            *signature
        }
    };
    Some(
        typed
            .normalized_trait_requirement_overload_identity(owner, signature)
            .identity(),
    )
}

fn exact_boundary_trait<'typed>(
    typed: &'typed TypedTrees,
    name: &str,
    role: &str,
) -> Result<&'typed psi_typed_trees::trait_definition::TraitDefinition, Diagnostic> {
    if name.is_empty() {
        return Err(Diagnostic::error(format!(
            "selected checked-adapter {role} has no canonical identity",
        )));
    }
    let definitions = typed
        .traits()
        .iter()
        .filter(|definition| definition.is_boundary && definition.name.as_str() == name)
        .collect::<Vec<_>>();
    let [definition] = definitions.as_slice() else {
        return Err(Diagnostic::error(format!(
            "selected checked-adapter {role} `{name}` resolves to {} exact boundary traits",
            definitions.len(),
        )));
    };
    if !definition.symbol.is_valid() {
        return Err(Diagnostic::error(format!(
            "selected checked-adapter {role} `{name}` has no exact typed symbol",
        )));
    }
    Ok(*definition)
}

fn named_type_symbol(
    typed: &TypedTrees,
    type_reference: psi_typed_trees::types::TypeReferenceHandle,
) -> Option<psi_symbols::SymbolHandle> {
    let psi_typed_trees::types::TypeReferenceNode::Named { symbol, .. } =
        typed.type_reference_table.type_reference(type_reference)
    else {
        return None;
    };
    symbol.is_valid().then_some(*symbol)
}

fn exact_adapter_receiver_shape(
    typed: &TypedTrees,
    actual_parameters: &[&psi_typed_trees::signature::StateParameter],
    requirement_parameter_count: usize,
    requirement_owner: psi_symbols::SymbolHandle,
) -> Option<bool> {
    match actual_parameters.len() {
        count if count == requirement_parameter_count => Some(false),
        count
            if requirement_parameter_count.checked_add(1) == Some(count)
                && actual_parameters.first().is_some_and(|parameter| {
                    named_type_symbol(typed, parameter.type_reference) == Some(requirement_owner)
                }) =>
        {
            Some(true)
        }
        _ => None,
    }
}

fn resolve_adapter_call<'adapter>(
    adapters: &'adapter [AdapterRow],
    fields: &[BoundaryField],
    receiver_symbol: psi_symbols::SymbolHandle,
    target_symbol: psi_symbols::SymbolHandle,
    target_name: &str,
) -> Result<Option<&'adapter AdapterRow>, Diagnostic> {
    let field = fields.iter().find(|field| field.symbol == receiver_symbol);
    let Some(field) = field else {
        return Ok(None);
    };
    let matches = adapters
        .iter()
        .filter(|adapter| {
            adapter.receiver_trait == field.trait_symbol
                && adapter.requirement_symbol == target_symbol
        })
        .collect::<Vec<_>>();
    let adapter = match matches.as_slice() {
        [adapter] => *adapter,
        [] => {
            let readable = adapters
                .iter()
                .filter(|adapter| {
                    adapter.receiver_trait == field.trait_symbol
                        && adapter.requirement == target_name
                })
                .count();
            if readable == 0 {
                return Ok(None);
            }
            return Err(Diagnostic::error(format!(
                "boundary call `{}`::`{target_name}` matches {readable} selected checked-adapter rows by display name but not exact target symbol {:?}",
                adapters
                    .iter()
                    .find(|adapter| adapter.receiver_trait == field.trait_symbol)
                    .map(|adapter| adapter.receiver_trait_name.as_str())
                    .unwrap_or("<unknown>"),
                target_symbol,
            )));
        }
        many => {
            return Err(Diagnostic::error(format!(
                "boundary call target symbol {:?} matches {} selected checked-adapter rows",
                target_symbol,
                many.len(),
            )));
        }
    };
    if adapter.requirement != target_name {
        return Err(Diagnostic::error(format!(
            "boundary call target symbol {:?} names exact overload `{}`, but its readable method drifted to `{target_name}`",
            target_symbol, adapter.requirement_identity,
        )));
    }
    Ok(Some(adapter))
}

/// Build the argument expression for a forwarded receiver path: `[self, f]`
/// becomes `Member(Name([self]), f)` and `[f]` becomes `Name([f])` -- the
/// exact trees the parser produces for those argument spellings, so every
/// downstream pass sees a shape it already serves.
fn synthesize_place_expression(
    expressions: &mut psi_typed_trees::expression::ExpressionTable,
    members: &[psi_typed_trees::name::Identifier],
    receiver_symbol: psi_symbols::SymbolHandle,
) -> psi_typed_trees::expression::ExpressionHandle {
    use psi_typed_trees::expression::{ExpressionNode, TableMemberExpression, TableNamePath};
    match members {
        [head, field] => {
            let mut head_span = psi_arena::HandleSpan::empty();
            expressions.push_name_path_member(&mut head_span, head.clone());
            let head_expression = expressions.insert(ExpressionNode::Name(TableNamePath {
                members: head_span,
                member_symbols: psi_arena::HandleSpan::empty(),
                head_symbol: psi_symbols::SymbolHandle::invalid(),
                symbol: psi_symbols::SymbolHandle::invalid(),
            }));
            expressions.insert(ExpressionNode::Member(TableMemberExpression {
                receiver: head_expression,
                member_symbol: receiver_symbol,
                member: field.clone(),
                case_variant: None,
            }))
        }
        _ => {
            let mut span = psi_arena::HandleSpan::empty();
            for member in members {
                expressions.push_name_path_member(&mut span, member.clone());
            }
            expressions.insert(ExpressionNode::Name(TableNamePath {
                members: span,
                member_symbols: psi_arena::HandleSpan::empty(),
                head_symbol: receiver_symbol,
                symbol: receiver_symbol,
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_effects::provider_plan::{ProviderBinding, ProviderPlan};
    use psi_typed_trees::expression::ExpressionNode;

    const SOURCE: &str = r#"
        boundary trait Echo {
            machine echo(value: i32) -> i32;
            machine emit(value: i32);
        }
        boundary trait Other {
            machine echo(value: i32) -> i32;
            machine emit(value: i32);
        }
        boundary trait Stateful {
            machine touch(&mut self);
        }
        boundary trait Forward {
            machine send(value: i32);
        }

        data EchoProvider {}
        machine EchoProvider::echo_adapter(value: i32) -> i32 satisfies Echo::echo {
            transition { _ -> (value) }
        }
        machine EchoProvider::emit_adapter(value: i32) satisfies Echo::emit {}

        data OtherProvider {}
        machine OtherProvider::echo_adapter(value: i32) -> i32 satisfies Other::echo {
            transition { _ -> (value) }
        }
        machine OtherProvider::emit_adapter(value: i32) satisfies Other::emit {}

        data StatefulProvider {}
        machine StatefulProvider::touch(&mut self) satisfies Stateful::touch {}

        data ForwardProvider {}
        machine ForwardProvider::send_adapter(service: Forward, value: i32)
            satisfies Forward::send {}

        data EchoClient { service: Echo; }
        machine EchoClient::run(&mut self) -> i32 {
            self.service.emit(1);
            transition { _ -> (self.service.echo(35)) }
        }

        data OtherClient { service: Other; }
        machine OtherClient::run(&mut self) -> i32 {
            self.service.emit(2);
            transition { _ -> (self.service.echo(35)) }
        }
    "#;

    struct Fixture {
        typed: TypedTrees,
        plans: Vec<ProviderPlan>,
    }

    fn fixture() -> Fixture {
        let tokens = psi_source_files_to_tokens::Lexer::new(SOURCE)
            .tokenize()
            .expect("tokenize exact adapter-dispatch fixture");
        let syntax = psi_tokens_to_syntax_trees::parse_syntax_trees(&tokens)
            .expect("parse exact adapter-dispatch fixture");
        let resolved = psi_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
            .expect("resolve exact adapter-dispatch fixture");
        let typed =
            psi_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
                .expect("type exact adapter-dispatch fixture");
        let plans = crate::pipeline::provider_plans::derive_satisfies_plans(&typed, None);
        Fixture { typed, plans }
    }

    fn plan<'plans>(plans: &'plans [ProviderPlan], schema: &str) -> &'plans ProviderPlan {
        plans
            .iter()
            .find(|plan| plan.schema.trait_name == schema)
            .unwrap_or_else(|| panic!("missing `{schema}` provider plan"))
    }

    fn checked_row(plan: &ProviderPlan, method: &str) -> usize {
        plan.rows
            .iter()
            .position(|row| {
                row.method == method
                    && matches!(&row.binding, ProviderBinding::CheckedAdapter { .. })
            })
            .unwrap_or_else(|| panic!("missing checked `{method}` row"))
    }

    #[derive(Clone, Copy, Debug)]
    enum Drift {
        None,
        EmptyOverload,
        CrossOverload,
        AbsentSchema,
        AbsentSignature,
        AbsentMachine,
        DuplicateMachine,
        WrongOwner,
        NonCheckedMachine,
        WrongConformance,
        InvalidShape,
        NonAdapterBinding,
    }

    #[test]
    fn selected_rows_reject_every_exact_identity_drift() {
        let cases = [
            (Drift::None, None),
            (Drift::EmptyOverload, Some("no exact overload identity")),
            (Drift::CrossOverload, Some("binds 0 exact schema methods")),
            (
                Drift::AbsentSchema,
                Some("resolves to 0 exact boundary traits"),
            ),
            (
                Drift::AbsentSignature,
                Some("resolves to 0 exact typed signatures"),
            ),
            (Drift::AbsentMachine, Some("is absent from typed machines")),
            (
                Drift::DuplicateMachine,
                Some("resolves to 2 exact typed machines"),
            ),
            (
                Drift::WrongOwner,
                Some("does not belong to nominal provider"),
            ),
            (Drift::NonCheckedMachine, Some("is not a checked body")),
            (
                Drift::WrongConformance,
                Some("through 0 checked conformances"),
            ),
            (Drift::InvalidShape, Some("entry parameters")),
            (Drift::NonAdapterBinding, None),
        ];

        for (drift, expected_error) in cases {
            let mut fixture = fixture();
            let mut selected = plan(&fixture.plans, "Echo").clone();
            let row_index = checked_row(&selected, "echo");
            let method_index = selected
                .schema
                .methods
                .iter()
                .position(|method| {
                    method.requirement_identity == selected.rows[row_index].requirement_identity
                })
                .expect("exact echo schema method");
            let other = plan(&fixture.plans, "Other");
            let other_row = &other.rows[checked_row(other, "echo")];
            match drift {
                Drift::None => {}
                Drift::EmptyOverload => selected.rows[row_index].requirement_identity.clear(),
                Drift::CrossOverload => {
                    selected.rows[row_index].requirement_identity =
                        other_row.requirement_identity.clone();
                }
                Drift::AbsentSchema => selected.schema.trait_name = "Missing".into(),
                Drift::AbsentSignature => {
                    selected.schema.methods[method_index].requirement_owner = "Other".into();
                }
                Drift::AbsentMachine => {
                    selected.rows[row_index].binding = ProviderBinding::CheckedAdapter {
                        machine_identity: "EchoProvider::missing".into(),
                        machine_package_identity: None,
                    };
                }
                Drift::DuplicateMachine => {
                    let machine_identity = match &selected.rows[row_index].binding {
                        ProviderBinding::CheckedAdapter {
                            machine_identity, ..
                        } => machine_identity.clone(),
                        _ => unreachable!(),
                    };
                    let duplicate = fixture
                        .typed
                        .machine_by_normalized_overload_identity(&machine_identity)
                        .expect("selected adapter")
                        .clone();
                    fixture.typed.push_machine(duplicate);
                }
                Drift::WrongOwner => selected.provider_type = "OtherProvider".into(),
                Drift::NonCheckedMachine => {
                    fixture
                        .typed
                        .machines_mut()
                        .iter_mut()
                        .find(|machine| machine.name.as_str() == "EchoProvider::echo_adapter")
                        .expect("echo adapter")
                        .supply_mode = psi_language_semantics::MachineSupplyMode::Boundary;
                }
                Drift::WrongConformance => {
                    selected.provider_type = "OtherProvider".into();
                    selected.rows[row_index].binding = other_row.binding.clone();
                }
                Drift::InvalidShape => {
                    selected.schema.methods[method_index].parameter_count = usize::MAX;
                }
                Drift::NonAdapterBinding => {
                    selected.rows[row_index].binding = ProviderBinding::CompilerIntrinsic {
                        machine: "Echo::echo.i32".into(),
                    };
                }
            }

            let result =
                resolve_selected_adapter_row(&fixture.typed, &selected, &selected.rows[row_index]);
            match expected_error {
                Some(expected) => {
                    let diagnostic = result.expect_err("identity drift must fail closed");
                    assert!(
                        diagnostic.message.contains(expected),
                        "{drift:?}: expected `{expected}`, got `{}`",
                        diagnostic.message,
                    );
                }
                None if matches!(drift, Drift::NonAdapterBinding) => {
                    assert_eq!(result.expect("non-adapter rows remain delegated"), None);
                }
                None => {
                    let adapter = result
                        .expect("exact row resolves")
                        .expect("checked row yields adapter");
                    assert_eq!(adapter.receiver_trait_name, "Echo");
                    assert_eq!(adapter.adapter_target, "EchoProvider::echo_adapter");
                    assert!(!adapter.forward_receiver);
                }
            }
        }
    }

    #[test]
    fn concrete_provider_self_is_excluded_from_adapter_arity() {
        let fixture = fixture();
        let selected = plan(&fixture.plans, "Stateful");
        let row = &selected.rows[checked_row(selected, "touch")];

        let adapter = resolve_selected_adapter_row(&fixture.typed, selected, row)
            .expect("concrete provider self is an exact realization receiver")
            .expect("checked row yields adapter");
        assert_eq!(adapter.adapter_target, "StatefulProvider::touch");
        assert!(!adapter.forward_receiver);
    }

    #[test]
    fn exact_leading_non_self_requirement_receiver_is_forwarded() {
        let fixture = fixture();
        let selected = plan(&fixture.plans, "Forward");
        let row = &selected.rows[checked_row(selected, "send")];

        let adapter = resolve_selected_adapter_row(&fixture.typed, selected, row)
            .expect("exact leading boundary binding resolves")
            .expect("checked row yields adapter");
        assert_eq!(adapter.adapter_target, "ForwardProvider::send_adapter");
        assert!(adapter.forward_receiver);
    }

    #[test]
    fn wrong_nonleading_or_multiple_non_self_receivers_never_forward() {
        let fixture = fixture();
        let selected = plan(&fixture.plans, "Forward");
        let row = &selected.rows[checked_row(selected, "send")];
        let ProviderBinding::CheckedAdapter {
            machine_identity, ..
        } = &row.binding
        else {
            unreachable!()
        };
        let adapter = fixture
            .typed
            .machine_by_normalized_overload_identity(machine_identity)
            .expect("forwarding adapter");
        let entry = fixture
            .typed
            .machine_states(adapter)
            .first()
            .expect("forwarding entry");
        let parameters = fixture
            .typed
            .state_parameters(entry)
            .iter()
            .filter(|parameter| !parameter.is_self)
            .collect::<Vec<_>>();
        let owner = fixture
            .typed
            .traits()
            .iter()
            .find(|definition| definition.name.as_str() == "Forward")
            .expect("Forward boundary trait")
            .symbol;
        let cases = [
            (vec![parameters[1]], 0),
            (vec![parameters[1], parameters[0]], 1),
            (parameters, 0),
        ];

        for (actual, required) in cases {
            assert_eq!(
                exact_adapter_receiver_shape(&fixture.typed, &actual, required, owner),
                None,
            );
        }
    }

    fn symbol(index: u32) -> psi_symbols::SymbolHandle {
        psi_symbols::SymbolHandle::from_parts(index, 0)
    }

    fn adapter(
        receiver_trait: psi_symbols::SymbolHandle,
        requirement_symbol: psi_symbols::SymbolHandle,
        requirement: &str,
        target: &str,
    ) -> AdapterRow {
        AdapterRow {
            receiver_trait,
            receiver_trait_name: format!("Trait{receiver_trait:?}"),
            requirement: requirement.into(),
            requirement_identity: format!("exact::{target}"),
            requirement_symbol,
            adapter_target: target.into(),
            symbol: symbol(requirement_symbol.arena_index() + 100),
            forward_receiver: false,
        }
    }

    #[test]
    fn same_spelled_fields_and_readable_names_never_select_adapter_rows() {
        let first_trait = symbol(1);
        let second_trait = symbol(2);
        let first_field = symbol(3);
        let second_field = symbol(4);
        let first_requirement = symbol(5);
        let second_requirement = symbol(6);
        let adapters = vec![
            adapter(
                first_trait,
                first_requirement,
                "echo",
                "FirstProvider::echo",
            ),
            adapter(
                second_trait,
                second_requirement,
                "echo",
                "SecondProvider::echo",
            ),
        ];
        let fields = [
            (
                "service",
                BoundaryField {
                    symbol: first_field,
                    trait_symbol: first_trait,
                },
            ),
            (
                "service",
                BoundaryField {
                    symbol: second_field,
                    trait_symbol: second_trait,
                },
            ),
        ];
        assert_eq!(fields[0].0, fields[1].0, "fixture field names must collide");
        let exact_fields = fields.map(|(_, field)| field);

        let cases = [
            (
                first_field,
                first_requirement,
                "echo",
                Some("FirstProvider::echo"),
                None,
            ),
            (
                second_field,
                second_requirement,
                "echo",
                Some("SecondProvider::echo"),
                None,
            ),
            (
                first_field,
                second_requirement,
                "echo",
                None,
                Some("display name but not exact target symbol"),
            ),
            (
                first_field,
                first_requirement,
                "renamed",
                None,
                Some("readable method drifted"),
            ),
            (symbol(99), first_requirement, "echo", None, None),
        ];
        for (field, target, name, expected_adapter, expected_error) in cases {
            let result = resolve_adapter_call(&adapters, &exact_fields, field, target, name);
            match (expected_adapter, expected_error) {
                (Some(expected), None) => assert_eq!(
                    result
                        .expect("exact symbols resolve")
                        .expect("adapter selected")
                        .adapter_target,
                    expected,
                ),
                (None, Some(expected)) => assert!(
                    result
                        .expect_err("name-only drift must reject")
                        .message
                        .contains(expected),
                ),
                (None, None) => assert_eq!(result.expect("unrelated fields are ignored"), None),
                _ => unreachable!(),
            }
        }

        let duplicate = adapters[0].clone();
        assert!(
            resolve_adapter_call(
                &[adapters[0].clone(), duplicate],
                &exact_fields,
                first_field,
                first_requirement,
                "echo",
            )
            .expect_err("duplicate exact rows must reject")
            .message
            .contains("matches 2 selected checked-adapter rows")
        );
    }

    #[test]
    fn late_invalid_call_prevents_statement_and_value_rewrites() {
        let mut fixture = fixture();
        let selected_names = fixture
            .plans
            .iter()
            .map(|plan| plan.name.clone())
            .collect::<Vec<_>>();
        let selected = omega_effects::SelectedProviderPlanFacts::from_selection(
            &fixture.plans,
            &selected_names,
        )
        .expect("select exact fixture plans");

        let statement_before = fixture
            .typed
            .machines()
            .iter()
            .flat_map(|machine| fixture.typed.machine_states(machine))
            .flat_map(|state| {
                fixture
                    .typed
                    .statement_table
                    .statements(state.statement_nodes)
            })
            .find(|statement| matches!(statement, psi_typed_trees::statement::StatementNode::Call(call) if call.target.as_str() == "emit"))
            .expect("fixture emit statement")
            .clone();
        let calls = fixture
            .typed
            .expression_table
            .expression_entries()
            .filter_map(|(handle, expression)| {
                matches!(expression, ExpressionNode::Call(call) if call.target.as_str() == "echo")
                    .then_some(handle)
            })
            .collect::<Vec<_>>();
        assert_eq!(calls.len(), 2);
        let first_before = fixture.typed.expression_table.expression(calls[0]).clone();
        let ExpressionNode::Call(mut invalid) =
            fixture.typed.expression_table.expression(calls[1]).clone()
        else {
            unreachable!()
        };
        invalid.target_symbol = psi_symbols::SymbolHandle::invalid();
        *fixture.typed.expression_table.expression_mut(calls[1]) = ExpressionNode::Call(invalid);

        let diagnostics = rewrite_adapter_calls(&mut fixture.typed, &selected)
            .expect_err("one invalid late call rejects the complete batch");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains("not exact target symbol"))
        );
        assert_eq!(
            fixture.typed.expression_table.expression(calls[0]),
            &first_before,
            "a valid value call must remain untouched"
        );
        let statement_after = fixture
            .typed
            .machines()
            .iter()
            .flat_map(|machine| fixture.typed.machine_states(machine))
            .flat_map(|state| {
                fixture
                    .typed
                    .statement_table
                    .statements(state.statement_nodes)
            })
            .find(|statement| matches!(statement, psi_typed_trees::statement::StatementNode::Call(call) if call.target.as_str() == "emit"))
            .expect("fixture emit statement remains boundary call");
        assert_eq!(statement_after, &statement_before);
    }
}
