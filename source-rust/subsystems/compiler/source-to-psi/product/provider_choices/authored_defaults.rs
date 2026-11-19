//! Read provider default declarations while their exact typed source is available.
use crate::provider_choice_inputs::{ProviderSelection, TargetProviderDefaults};
use crate::source_state::provider_declarations::ProviderPlanningContext;
use diagnostics::Diagnostic;
use semantic_vocabulary::source_selections::CompositionMode;

/// "The selected target package supplies ordinary named default provider
/// implementations" (`wiki/spec/build/provider_selection.md`,
/// "Declaration and choice"). A package declares them in a machine named
/// `Type::provider_defaults`, whose body is a list of
/// `Type::select_provider::<Slot, Provider>(defaults)` calls. Each call
/// becomes one selection, read from the typed trees: this is source
/// policy, not code the Product runs. Machines of one target share a
/// row, and rows come out ordered by target.
pub fn retain_provider_defaults(
    context: ProviderPlanningContext<'_>,
) -> Result<Vec<TargetProviderDefaults>, Vec<Diagnostic>> {
    let mut rows: Vec<TargetProviderDefaults> = Vec::new();
    for machine in context.typed.machines().iter().filter(|machine| {
        machine.purpose == crate::source_state::machine::MachinePurpose::ProviderDefaults
            && !machine.target_family_interface
            && context.contains(machine.symbol)
            && (context.scope == source_provenance::DependencyScope::Product
                || machine.target.is_none())
    }) {
        let target = machine
            .authored_target
            .as_ref()
            .map(|name| name.as_str().to_owned());
        let selections = defaults::harvest(context, machine)?;
        match rows.iter_mut().find(|row| row.target == target) {
            Some(row) => {
                row.machines.push(machine.symbol);
                row.selections.extend(selections);
            }
            None => rows.push(TargetProviderDefaults {
                target,
                machines: vec![machine.symbol],
                selections,
            }),
        }
    }
    rows.sort_by(|left, right| left.target.cmp(&right.target));
    Ok(rows)
}

/// Reads the selections one `provider_defaults` machine declares.
///
/// Every statement must form a selection. Report an explicit composition
/// mode or an applied slot or provider at its authored occurrence.
mod defaults {
    use super::{CompositionMode, ProviderPlanningContext, ProviderSelection};
    use crate::product::provider_declarations::{form, identity};
    use crate::source_state::expression::{ExpressionNode, StaticMachineArgument};
    use crate::source_state::machine::Machine;
    use crate::source_state::product_operand::{ProductOperandKind, resolve_product_operand};
    use crate::source_state::statement::StatementNode;
    use crate::source_symbols::JoinedPath;
    use diagnostics::Diagnostic;

    /// One `select_provider` call as its statement holds it.
    struct Call<'typed> {
        target: &'typed str,
        resolved: bool,
        arguments: &'typed [StaticMachineArgument],
        values: usize,
        source_span: source_text::SourceSpan,
    }

    pub(super) fn harvest(
        context: ProviderPlanningContext<'_>,
        machine: &Machine,
    ) -> Result<Vec<ProviderSelection>, Vec<Diagnostic>> {
        let typed = context.typed;
        let name = machine.name.as_str();
        let refuse = |what: &str, source_span: source_text::SourceSpan| {
            vec![
                Diagnostic::error(format!(
                    "no product route yet: source provider defaults: `{name}` {what}"
                ))
                .with_source_span(source_span),
            ]
        };
        let mut selections: Vec<ProviderSelection> = Vec::new();
        for state in typed.machine_states(machine) {
            for statement in typed.statement_table.statements(state.statement_nodes) {
                let call = match statement {
                    StatementNode::Call(call) => Call {
                        target: call.target.as_str(),
                        resolved: call.target_symbol.is_valid(),
                        arguments: &call.machine_arguments,
                        values: typed
                            .statement_table
                            .expression_handles(call.arguments)
                            .len(),
                        source_span: call.source_span,
                    },
                    StatementNode::Expression(expression) => {
                        let source_span = typed.expression_table.source_span(*expression);
                        match typed.expression_table.expression(*expression) {
                            ExpressionNode::Call(call) => Call {
                                target: call.target.as_str(),
                                resolved: call.target_symbol.is_valid(),
                                arguments: &call.machine_arguments,
                                values: typed
                                    .expression_table
                                    .expression_handles(call.arguments)
                                    .len(),
                                source_span,
                            },
                            _ => {
                                return Err(refuse(
                                    "holds a statement that is not a `select_provider` call",
                                    source_span,
                                ));
                            }
                        }
                    }
                    _ => {
                        return Err(refuse(
                            "holds a statement that is not a `select_provider` call",
                            typed
                                .symbols
                                .symbol_source_span(machine.symbol)
                                .unwrap_or_default(),
                        ));
                    }
                };
                // Typing leaves the selection intrinsic unresolved; a
                // call that resolved to a machine is an ordinary call.
                if call.target != "select_provider" || call.resolved {
                    return Err(refuse(
                        "holds a statement that is not a `select_provider` call",
                        call.source_span,
                    ));
                }
                let [slot, provider] = call.arguments else {
                    return Err(refuse(
                        "selects a provider without exactly a slot and a provider",
                        call.source_span,
                    ));
                };
                // The one value argument is the defaults value itself. A
                // second one is an explicit `CompositionMode`.
                if call.values != 1 {
                    return Err(refuse(
                        "selects a provider with an explicit composition mode",
                        call.source_span,
                    ));
                }
                if slot.application.is_some() || provider.application.is_some() {
                    return Err(refuse(
                        "selects a slot or provider applied to static arguments",
                        call.source_span,
                    ));
                }
                let origin = symbol_resolved_trees_to_typed_trees::product_declarations::capture_build_selection_origin(
                    &typed.symbols,
                    machine.symbol,
                    machine.symbol,
                    call.source_span,
                    None,
                )
                .map_err(|reason| refuse(reason, call.source_span))?;
                let slot_symbol = resolve_product_operand(
                    typed,
                    slot,
                    call.source_span,
                    ProductOperandKind::BoundarySubject,
                )
                .ok_or_else(|| refuse(&format!(
                    "subject `{}` does not resolve to an exact boundary trait or requirement visible here",
                    slot.path.joined_path(),
                ), call.source_span))?;
                let provider_symbol = resolve_product_operand(
                    typed,
                    provider,
                    call.source_span,
                    ProductOperandKind::Provider,
                )
                .ok_or_else(|| {
                    refuse(
                        &format!(
                            "provider `{}` does not resolve to an exact declaration visible here",
                            provider.path.joined_path(),
                        ),
                        call.source_span,
                    )
                })?;
                let slot = identity(context, slot_symbol, &slot.path.joined_path());
                let provider = identity(context, provider_symbol, &provider.path.joined_path());
                let selection = form(
                    context,
                    slot,
                    provider,
                    origin,
                    None,
                    CompositionMode::Fused,
                )
                .map_err(|diagnostic| vec![diagnostic.with_source_span(call.source_span)])?;
                if let Some(existing) = selections
                    .iter()
                    .find(|existing| existing.subject.same_declaration_as(&selection.subject))
                    && existing.provider_type.symbol != selection.provider_type.symbol
                {
                    return Err(vec![
                        Diagnostic::error(format!(
                            "build selects two provider types for slot `{}`: `{}` and `{}`",
                            selection.subject.canonical_path(),
                            existing.provider_type.canonical_path,
                            selection.provider_type.canonical_path,
                        ))
                        .with_source_span(call.source_span),
                    ]);
                }
                selections.push(selection);
            }
        }
        Ok(selections)
    }
}
