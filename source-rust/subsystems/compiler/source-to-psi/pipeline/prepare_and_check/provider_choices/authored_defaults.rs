//! Read provider default declarations while their exact typed source is available.
use crate::declaration_selection::BuildSelectionOrigin;
use crate::provider_choice_inputs::{
    ProviderSelection, ProviderSelectionIdentity, ProviderSelectionSubject, TargetProviderDefaults,
};
use crate::source_state::provider_declarations::ProviderPlanningContext;
use diagnostics::Diagnostic;
use semantic_vocabulary::source_selections::{CompositionMode, ProductTarget};

/// "The selected target package supplies ordinary named default provider
/// implementations" (`wiki/spec/build/provider_selection.md`,
/// "Declaration and choice"). A package declares them in a machine named
/// `Type::provider_defaults`, whose body is a list of
/// `Type::select_provider<Slot, Provider>(defaults)` calls. Each call
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
/// Only the forms the reader states are harvested. Anything else in such
/// a machine is refused by name rather than skipped: a statement that is
/// not a selection, an explicit composition mode, a slot or provider
/// applied to static arguments, and a boundary-operator family as the
/// slot. A skipped statement would be a default the Product silently
/// does without.
mod defaults {
    use super::{CompositionMode, ProviderPlanningContext, ProviderSelection, form, identity};
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

pub(crate) fn identity(
    context: ProviderPlanningContext<'_>,
    symbol: symbols::SymbolHandle,
    authored_path: &str,
) -> ProviderSelectionIdentity {
    let symbols = &context.typed.symbols;
    ProviderSelectionIdentity {
        symbol,
        application: Default::default(),
        applied: false,
        application_identity: None,
        package: symbols.symbol_package_identity(symbol),
        canonical_path: symbols.display_path(symbol, "::"),
        authored_path: authored_path.to_owned(),
    }
}

/// Classifies the slot and checks the supported static declaration form.
/// Defaults, explicit Build choices and retained dependency choices share
/// this validation; a default's namespace grants no provider value.
pub(crate) fn form(
    context: ProviderPlanningContext<'_>,
    slot: ProviderSelectionIdentity,
    provider: ProviderSelectionIdentity,
    origin: BuildSelectionOrigin,
    target: Option<ProductTarget>,
    composition_mode: CompositionMode,
) -> Result<ProviderSelection, Diagnostic> {
    use symbols::SymbolKind;
    let typed = context.typed;
    if !slot.symbol.is_valid() || !provider.symbol.is_valid() {
        return Err(Diagnostic::error(format!(
            "provider selection `{}` for `{}` does not resolve to declarations",
            provider.authored_path, slot.authored_path
        )));
    }
    if !context.contains(slot.symbol) || !context.contains(provider.symbol) {
        return Err(Diagnostic::error(format!(
            "provider selection subject or provider is outside the {:?} planning context",
            context.scope
        )));
    }
    let slot_kind = typed.symbols.get(slot.symbol).kind;
    // A slot or provider with parameters is selected only as one
    // closed application of them, which its identity then states.
    if slot.application_identity.is_none()
        && typed
            .trait_definition_by_symbol(slot.symbol)
            .is_some_and(|definition| !definition.type_parameters.is_empty())
    {
        return Err(Diagnostic::error(format!(
            "no product route yet: provider slot `{}` requires its complete static application",
            slot.authored_path,
        )));
    }
    let subject = if slot_kind == SymbolKind::Trait
        && typed
            .traits()
            .iter()
            .any(|definition| definition.symbol == slot.symbol && definition.is_boundary)
    {
        ProviderSelectionSubject::BoundaryTrait(slot)
    } else if slot_kind == SymbolKind::Machine
        && typed.machines().iter().any(|requirement| {
            requirement.symbol == slot.symbol
                && requirement.spelling.is_none()
                && requirement.supply_mode
                    == source_declarations::MachineSupplyMode::TopLevelRequirement
        })
    {
        ProviderSelectionSubject::BoundaryRequirement(slot)
    } else if slot_kind == SymbolKind::Operator
        || typed
            .machine_token_bindings()
            .iter()
            .any(|binding| binding.is_boundary && binding.symbol == slot.symbol)
    {
        return Err(Diagnostic::error(format!(
            "no product route yet: source provider defaults: slot `{}` is a boundary-operator family",
            slot.authored_path
        )));
    } else {
        return Err(Diagnostic::error(format!(
            "provider selection subject `{}` does not resolve to an exact boundary trait, top-level boundary requirement, or boundary-operator family",
            slot.authored_path
        )));
    };
    let implementation = typed
        .conformances()
        .iter()
        .find(|definition| definition.symbol == provider.symbol);
    if implementation.is_none() {
        return Err(Diagnostic::error(format!(
            "provider selection `{}` is not a static operation record from a named implementation; a data type is not a provider value",
            provider.authored_path
        )));
    }
    if provider.application_identity.is_none()
        && implementation.is_some_and(|implementation| !implementation.type_parameters.is_empty())
    {
        return Err(Diagnostic::error(format!(
            "no product route yet: provider `{}` requires its complete static application",
            provider.authored_path,
        )));
    }
    // A named implementation with lifetime parameters provides a
    // requirement its parent trait declares only when the parent's
    // lifetimes are spelled by the implementation's own.
    if let Some(implementation) =
        implementation.filter(|definition| definition.type_parameters.is_empty())
    {
        let root = typed.trait_definition_by_symbol(implementation.trait_symbol);
        let arguments = implementation.trait_lifetime_arguments.as_slice();
        let own = implementation.lifetime_parameters.len();
        let spelled = root.is_some_and(|root| {
            arguments.len() == root.lifetime_parameters.len()
                && arguments
                    .iter()
                    .all(|ordinal| usize::try_from(*ordinal).is_ok_and(|ordinal| ordinal < own))
        });
        for row in &implementation.rows {
            let inherited_without_lifetimes = typed
                .trait_definition_by_symbol(row.declaring_trait)
                .is_some_and(|declaring| declaring.lifetime_parameters.is_empty());
            let direct = root.is_some_and(|root| root.symbol == row.declaring_trait);
            if !spelled || !(direct || inherited_without_lifetimes) {
                return Err(Diagnostic::error(format!(
                    "named provider `{}` requires an unsupported inherited declaring-trait lifetime application for `{}`; the exact parent substitution is not retained",
                    provider.authored_path, row.requirement_name,
                )));
            }
        }
    }
    Ok(ProviderSelection {
        target,
        subject,
        provider_type: provider,
        composition_mode,
        origin,
    })
}
