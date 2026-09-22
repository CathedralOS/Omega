//! Authored provider selections, boundary grants, wire demands, and behavior
//! exclusions.

use crate::admission::behavior_exclusions::{
    AuthoredBehaviorExclusion, AuthoredBehaviorExclusionKind,
};
use crate::admission::vocabulary::is_exact_toolchain_build_prelude_data;
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::BuildOperation;
use provider_planning::{ProviderSelection, ProviderSelectionIdentity};
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireCompatibilityDemand {
    pub edge: String,
    pub lineage: String,
    pub local_schema: String,
    pub peer_schema: String,
    pub require_readable: bool,
    pub require_writable: bool,
    pub require_unknown_preservation: bool,
    pub require_canonical: bool,
    pub require_complete_migration: bool,
}

/// Collect the edge-specific wire facts requested by the one
/// authoritative build machine. The parser has already validated the closed
/// fact vocabulary; this pass validates the marker encoding and duplicate
/// declarations before compatibility evaluation consumes it.
pub(crate) fn harvest_wire_compatibility_demands(
    typed: &TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> Result<Vec<WireCompatibilityDemand>, Vec<Diagnostic>> {
    let mut demands = Vec::new();
    let mut diagnostics = Vec::new();
    let mut record = |target: &str| {
        let Some(encoded) = BuildOperation::WireCompatibilityRequest.marker_operands(target) else {
            return;
        };
        let parts = encoded.split('#').collect::<Vec<_>>();
        if parts.len() < 5 {
            diagnostics.push(Diagnostic::error(format!(
                "malformed wire compatibility declaration `{target}`"
            )));
            return;
        }
        let mut demand = WireCompatibilityDemand {
            edge: parts[0].to_owned(),
            lineage: parts[1].to_owned(),
            local_schema: parts[2].to_owned(),
            peer_schema: parts[3].to_owned(),
            require_readable: false,
            require_writable: false,
            require_unknown_preservation: false,
            require_canonical: false,
            require_complete_migration: false,
        };
        for fact in &parts[4..] {
            match *fact {
                "Readable" => demand.require_readable = true,
                "Writable" => demand.require_writable = true,
                "PreserveUnknown" => demand.require_unknown_preservation = true,
                "Canonical" => demand.require_canonical = true,
                "CompleteMigration" => demand.require_complete_migration = true,
                other => diagnostics.push(Diagnostic::error(format!(
                    "malformed wire compatibility declaration `{target}`: unknown fact `{other}`"
                ))),
            }
        }
        if demands.iter().any(|existing: &WireCompatibilityDemand| {
            existing.edge == demand.edge
                && existing.lineage == demand.lineage
                && existing.local_schema == demand.local_schema
                && existing.peer_schema == demand.peer_schema
        }) {
            diagnostics.push(Diagnostic::error(format!(
                "wire compatibility demand for edge `{}`, lineage `{}`, local schema `{}`, \
                 and peer schema `{}` is declared twice",
                demand.edge, demand.lineage, demand.local_schema, demand.peer_schema
            )));
            return;
        }
        demands.push(demand);
    };

    for state in typed.machine_states(machine) {
        for statement in typed.statement_table.statements(state.statement_nodes) {
            match statement {
                typed_trees::statement::StatementNode::Expression(expression) => {
                    if let typed_trees::expression::ExpressionNode::Call(call) =
                        typed.expression_table.expression(*expression)
                    {
                        record(call.target.as_str());
                    }
                }
                typed_trees::statement::StatementNode::Call(call) => {
                    record(call.target.as_str());
                }
                _ => {}
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(demands)
    } else {
        Err(diagnostics)
    }
}

/// Static target-default declarations have their own admission path. Authored
/// Build overrides instead enter through executed call receipts below.
pub fn harvest_provider_selections(
    typed: &TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> Result<Vec<ProviderSelection>, Vec<Diagnostic>> {
    let mut requests = Vec::new();
    for state in typed.machine_states(machine) {
        for statement in typed.statement_table.statements(state.statement_nodes) {
            match statement {
                typed_trees::statement::StatementNode::Expression(expression) => {
                    if let typed_trees::expression::ExpressionNode::Call(call) =
                        typed.expression_table.expression(*expression)
                        && !call.target_symbol.is_valid()
                    {
                        requests.push(ProviderSelectionCall {
                            target: call.target.as_str(),
                            arguments: &call.machine_arguments,
                            value_arguments: typed
                                .expression_table
                                .expression_handles(call.arguments),
                            source_span: provider_selection_expression_source_span(
                                typed,
                                *expression,
                            ),
                            machine: machine.symbol,
                            composition_case: None,
                            product_operands: false,
                        });
                    }
                }
                typed_trees::statement::StatementNode::Call(call)
                    if !call.target_symbol.is_valid() =>
                {
                    requests.push(ProviderSelectionCall {
                        target: call.target.as_str(),
                        arguments: &call.machine_arguments,
                        value_arguments: typed.statement_table.expression_handles(call.arguments),
                        source_span: call.source_span,
                        machine: machine.symbol,
                        composition_case: None,
                        product_operands: false,
                    });
                }
                _ => {}
            }
        }
    }
    collect_provider_selections(typed, requests)
}

struct ProviderSelectionCall<'a> {
    target: &'a str,
    arguments: &'a [typed_trees::expression::StaticMachineArgument],
    value_arguments: &'a [typed_trees::expression::ExpressionHandle],
    source_span: source::SourceSpan,
    machine: SymbolHandle,
    composition_case: Option<SymbolHandle>,
    product_operands: bool,
}

/// Rejoin only calls evaluated on the original Build cell. An unreachable
/// selection does not become configuration merely because its text exists.
pub(crate) fn collect_executed_provider_selections(
    typed: &TypedTrees,
    executed: &[checked_interpreter::ExecutedProviderSelection],
) -> Result<Vec<ProviderSelection>, Vec<Diagnostic>> {
    let requests = executed
        .iter()
        .map(|row| {
            let request = match row.site {
                checked_interpreter::ExecutedProviderSelectionSite::Statement(handle) => {
                    match typed.statement_table.statement(handle) {
                        typed_trees::statement::StatementNode::Call(call)
                            if BuildOperation::from_call_target(call.target.as_str())
                                == Some(BuildOperation::ProviderSelection)
                                && !call.target_symbol.is_valid() =>
                        {
                            Some(ProviderSelectionCall {
                                target: call.target.as_str(),
                                arguments: &call.machine_arguments,
                                value_arguments: typed
                                    .statement_table
                                    .expression_handles(call.arguments),
                                source_span: call.source_span,
                                machine: row.machine,
                                composition_case: Some(row.composition_case),
                                product_operands: true,
                            })
                        }
                        _ => None,
                    }
                }
                checked_interpreter::ExecutedProviderSelectionSite::Expression(handle) => {
                    match typed.expression_table.expression(handle) {
                        typed_trees::expression::ExpressionNode::Call(call)
                            if BuildOperation::from_call_target(call.target.as_str())
                                == Some(BuildOperation::ProviderSelection)
                                && !call.target_symbol.is_valid() =>
                        {
                            Some(ProviderSelectionCall {
                                target: call.target.as_str(),
                                arguments: &call.machine_arguments,
                                value_arguments: typed
                                    .expression_table
                                    .expression_handles(call.arguments),
                                source_span: provider_selection_expression_source_span(
                                    typed, handle,
                                ),
                                machine: row.machine,
                                composition_case: Some(row.composition_case),
                                product_operands: true,
                            })
                        }
                        _ => None,
                    }
                }
            };
            request.ok_or_else(|| {
                vec![Diagnostic::error(
                    "executed provider selection did not rejoin its admitted call",
                )]
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    collect_provider_selections(typed, requests)
}

fn provider_selection_expression_source_span(
    typed: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
) -> source::SourceSpan {
    // The declaration occurrence names the call itself. An expression's
    // enclosing span can be synthetic after receiver/statement lowering.
    typed
        .expression_table
        .authored_selection_occurrences(expression)
        .filter_map(|occurrence| typed.authored_declaration_selections().get(occurrence))
        .find(|selection| {
            selection.kind()
                == language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::Call
        })
        .map_or_else(
            || typed.expression_table.source_span(expression),
            |selection| selection.source_span(),
        )
}

fn collect_provider_selections<'a>(
    typed: &TypedTrees,
    requests: impl IntoIterator<Item = ProviderSelectionCall<'a>>,
) -> Result<Vec<ProviderSelection>, Vec<Diagnostic>> {
    let mut selections: Vec<ProviderSelection> = Vec::new();
    let mut diagnostics = Vec::new();
    let mut record = |request: ProviderSelectionCall<'_>| {
        let ProviderSelectionCall {
            target,
            arguments,
            value_arguments,
            source_span,
            machine,
            composition_case,
            product_operands,
        } = request;
        if target != "select_provider" {
            return;
        }
        let [boundary_argument, provider_argument] = arguments else {
            diagnostics.push(Diagnostic::error(
                "provider selection must retain exactly two resolved type paths",
            ));
            return;
        };
        let project_identity = |argument: &typed_trees::expression::StaticMachineArgument,
                                is_provider: bool| {
            let authored_path = argument
                .path
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::");
            let symbol = if product_operands {
                typed_trees_to_checked_trees::typed_product_provider_selection_operand(
                    typed,
                    argument,
                    source_span,
                    !is_provider,
                )
                .unwrap_or_else(SymbolHandle::invalid)
            } else {
                argument.symbol
            };
            ProviderSelectionIdentity {
                symbol,
                package: typed.symbols.symbol_package_identity(symbol),
                canonical_path: typed.symbols.display_path(symbol, "::"),
                authored_path,
            }
        };
        let boundary_identity = project_identity(boundary_argument, false);
        let provider_type = project_identity(provider_argument, true);
        let composition_mode = match composition_case.map_or_else(
            || provider_selection_composition_mode(typed, value_arguments),
            |case| provider_composition_case(typed, case),
        ) {
            Ok(mode) => mode,
            Err(diagnostic) => {
                diagnostics.push(diagnostic);
                return;
            }
        };
        let subject = if boundary_identity.symbol.is_valid()
            && typed.symbols.get(boundary_identity.symbol).kind == SymbolKind::Trait
            && typed.traits().iter().any(|definition| {
                definition.symbol == boundary_identity.symbol && definition.is_boundary
            }) {
            provider_planning::ProviderSelectionSubject::BoundaryTrait(boundary_identity)
        } else if boundary_identity.symbol.is_valid()
            && typed.symbols.get(boundary_identity.symbol).kind == SymbolKind::Machine
            && typed.machines().iter().any(|requirement| {
                requirement.symbol == boundary_identity.symbol
                    && requirement.is_public
                    && requirement.supply_mode
                        == language_semantics::MachineSupplyMode::TopLevelRequirement
            })
        {
            provider_planning::ProviderSelectionSubject::BoundaryRequirement(boundary_identity)
        } else if boundary_identity.symbol.is_valid()
            && typed.symbols.get(boundary_identity.symbol).kind == SymbolKind::Operator
        {
            // One resolved overload symbol names its complete package-
            // qualified family; provider-planning derives the canonical
            // roster so harvest and later replay share one definition.
            match provider_planning::ProviderOperatorFamilySelection::derive(
                typed,
                boundary_identity.symbol,
                boundary_identity.authored_path,
            ) {
                Ok(family) => {
                    provider_planning::ProviderSelectionSubject::BoundaryOperatorFamily(family)
                }
                Err(reason) => {
                    diagnostics.push(Diagnostic::error(reason));
                    return;
                }
            }
        } else {
            diagnostics.push(Diagnostic::error(format!(
                "provider selection subject `{}` does not resolve to an exact boundary trait, top-level boundary requirement, or boundary-operator family",
                boundary_identity.authored_path
            )));
            return;
        };
        if !provider_type.symbol.is_valid()
            || typed.symbols.get(provider_type.symbol).kind != SymbolKind::Data
        {
            diagnostics.push(Diagnostic::error(format!(
                "provider selection type `{}` does not resolve to an exact data declaration",
                provider_type.authored_path
            )));
            return;
        }
        if let Some(existing) = selections
            .iter()
            .find(|selection| selection.subject.same_declaration_as(&subject))
        {
            if existing.provider_type.symbol != provider_type.symbol {
                diagnostics.push(Diagnostic::error(format!(
                    "build selects two provider types for slot `{}`: `{}` and `{}`",
                    subject.canonical_path(),
                    existing.provider_type.canonical_path,
                    provider_type.canonical_path,
                )));
                return;
            }
            if existing.composition_mode != composition_mode {
                diagnostics.push(Diagnostic::error(format!(
                    "build selects provider `{}` for slot `{}` with conflicting composition modes {:?} and {:?}",
                    provider_type.canonical_path,
                    subject.canonical_path(),
                    existing.composition_mode,
                    composition_mode,
                )));
                return;
            }
        }
        selections.push(ProviderSelection {
            subject,
            provider_type,
            composition_mode,
            selecting_machine: machine,
            source_span,
        });
    };
    for request in requests {
        record(request);
    }
    if diagnostics.is_empty() {
        Ok(selections)
    } else {
        Err(diagnostics)
    }
}

/// Recheck the declaration referenced by sealed executed-selection custody.
/// This validates its operands without rerunning Build or turning unexecuted
/// source calls into selections. Execution order and computed mode come from
/// the retained activation, not a second interpretation during package review.
pub fn validate_executed_provider_selection_declaration(
    typed: &TypedTrees,
    selection: &ProviderSelection,
) -> bool {
    let Some(machine) = typed
        .machines()
        .iter()
        .find(|machine| machine.symbol == selection.selecting_machine)
    else {
        return false;
    };
    let mut matched = false;
    let mut validate =
        |target: &str,
         target_symbol: SymbolHandle,
         arguments: &[typed_trees::expression::StaticMachineArgument],
         values: &[typed_trees::expression::ExpressionHandle]| {
            if target != "select_provider" || target_symbol.is_valid() || values.len() > 1 {
                return false;
            }
            let [slot, provider] = arguments else {
                return false;
            };
            let slot_symbol =
                typed_trees_to_checked_trees::typed_product_provider_selection_operand(
                    typed,
                    slot,
                    selection.source_span,
                    true,
                );
            let provider_symbol =
                typed_trees_to_checked_trees::typed_product_provider_selection_operand(
                    typed,
                    provider,
                    selection.source_span,
                    false,
                );
            let subject_symbol = match &selection.subject {
                provider_planning::ProviderSelectionSubject::BoundaryTrait(identity)
                | provider_planning::ProviderSelectionSubject::BoundaryRequirement(identity) => {
                    identity.symbol
                }
                provider_planning::ProviderSelectionSubject::BoundaryOperatorFamily(family) => {
                    // The representative may differ after declaration reordering;
                    // compare the complete derived family instead.
                    let Some(symbol) = slot_symbol else {
                        return false;
                    };
                    let path = slot
                        .path
                        .iter()
                        .map(|part| part.as_str())
                        .collect::<Vec<_>>()
                        .join("::");
                    if provider_planning::ProviderOperatorFamilySelection::derive(
                        typed, symbol, path,
                    )
                    .as_ref()
                        != Ok(family)
                    {
                        return false;
                    }
                    symbol
                }
            };
            if slot_symbol != Some(subject_symbol)
                || provider_symbol != Some(selection.provider_type.symbol)
            {
                return false;
            }
            if values.is_empty()
                && selection.composition_mode != provider_planning::CompositionMode::Fused
            {
                return false;
            }
            if let [value] = values
                && let Some(case) = exact_case_argument_symbol(typed, *value)
                && provider_composition_case(typed, case).ok() != Some(selection.composition_mode)
            {
                return false;
            }
            matched = true;
            true
        };
    for state in typed.machine_states(machine) {
        for statement in typed.statement_table.statements(state.statement_nodes) {
            if let typed_trees::statement::StatementNode::Call(call) = statement
                && call.source_span == selection.source_span
                && !validate(
                    call.target.as_str(),
                    call.target_symbol,
                    &call.machine_arguments,
                    typed.statement_table.expression_handles(call.arguments),
                )
            {
                return false;
            }
        }
    }
    for handle in typed_trees_to_checked_trees::typed_provider_selection_expressions(typed, machine)
    {
        let expression = typed.expression_table.expression(handle);
        if provider_selection_expression_source_span(typed, handle) == selection.source_span
            && let typed_trees::expression::ExpressionNode::Call(call) = expression
            && (!typed_trees_to_checked_trees::typed_build_provider_selection(typed, handle)
                || !validate(
                    call.target.as_str(),
                    call.target_symbol,
                    &call.machine_arguments,
                    typed.expression_table.expression_handles(call.arguments),
                ))
        {
            return false;
        }
    }
    matched
}

fn provider_selection_composition_mode(
    typed: &TypedTrees,
    arguments: &[typed_trees::expression::ExpressionHandle],
) -> Result<provider_planning::CompositionMode, Diagnostic> {
    let [] = arguments else {
        let [argument] = arguments else {
            return Err(Diagnostic::error(
                "provider selection must retain zero arguments for fused composition or one exact compiler-owned CompositionMode value",
            ));
        };
        let Some(case_symbol) = exact_case_argument_symbol(typed, *argument) else {
            return Err(Diagnostic::error(
                "provider selection composition mode must be the exact compiler-owned CompositionMode::Fused or CompositionMode::Independent case",
            ));
        };
        return provider_composition_case(typed, case_symbol);
    };
    Ok(provider_planning::CompositionMode::Fused)
}

fn provider_composition_case(
    typed: &TypedTrees,
    case_symbol: SymbolHandle,
) -> Result<provider_planning::CompositionMode, Diagnostic> {
    if !case_symbol.is_valid() {
        return Ok(provider_planning::CompositionMode::Fused);
    }
    let exact_modes = typed
        .data_definitions()
        .iter()
        .filter(|definition| {
            is_exact_toolchain_build_prelude_data(typed, definition.symbol, "CompositionMode")
        })
        .collect::<Vec<_>>();
    let [modes] = exact_modes.as_slice() else {
        return Err(Diagnostic::error(
            "explicit provider composition mode requires exactly one compiler-owned CompositionMode declaration",
        ));
    };
    let selected = typed
        .data_members(modes)
        .iter()
        .filter_map(|member| match member {
            typed_trees::data::DataMember::Variant(variant)
                if variant.symbol == case_symbol
                    && typed.symbols.get(variant.symbol).parent == modes.symbol =>
            {
                Some(variant)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [selected] = selected.as_slice() else {
        return Err(Diagnostic::error(
            "provider selection composition mode does not name an exact compiler-owned CompositionMode case",
        ));
    };
    if !typed.data_payload_fields(selected).is_empty() {
        return Err(Diagnostic::error(
            "provider selection composition mode case unexpectedly carries a payload",
        ));
    }
    match selected.name.as_str() {
        "Fused" => Ok(provider_planning::CompositionMode::Fused),
        "Independent" => Ok(provider_planning::CompositionMode::Independent),
        other => Err(Diagnostic::error(format!(
            "compiler-owned CompositionMode contains unsupported case `{other}`"
        ))),
    }
}

/// The static grant harvest: every `accept_boundary#<path>` marker call in
/// the build machine's states (the postfix carve's desugar of
/// `b.accept_boundary<path>();`). Order-preserving, deduplicated.
pub fn harvest_root_grants(
    typed: &TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> Result<Vec<trust_model::AuthoredRootGrant>, Diagnostic> {
    let mut grants = Vec::new();
    let mut record = |selector: &str, source_span: source::SourceSpan| {
        if !grants
            .iter()
            .any(|grant: &trust_model::AuthoredRootGrant| grant.selector == selector)
        {
            grants.push(trust_model::AuthoredRootGrant {
                selector: selector.to_owned(),
                selecting_machine: machine.symbol,
                source_span,
            });
        }
    };
    for state in typed.machine_states(machine) {
        for statement in typed.statement_table.statements(state.statement_nodes) {
            let handles: Vec<typed_trees::expression::ExpressionHandle> = match statement {
                typed_trees::statement::StatementNode::Expression(expression) => {
                    vec![*expression]
                }
                typed_trees::statement::StatementNode::Call(call) => {
                    // A statement-level call keeps the marker in its target.
                    if let Some(path) =
                        BuildOperation::BoundaryAcceptance.marker_operands(call.target.as_str())
                    {
                        record(path, authored_root_grant_statement_span(typed, call)?);
                    }
                    Vec::new()
                }
                _ => Vec::new(),
            };
            for handle in handles {
                if let typed_trees::expression::ExpressionNode::Call(call) =
                    typed.expression_table.expression(handle)
                    && let Some(path) =
                        BuildOperation::BoundaryAcceptance.marker_operands(call.target.as_str())
                {
                    record(path, authored_root_grant_expression_span(typed, handle)?);
                }
            }
        }
    }
    Ok(grants)
}

/// Behavior exclusions (wiki/spec/build/behavior_exclusions.md):
/// Build crash, physical-authority, and service exclusions are
/// product-admission requirements the granted build evaluation records as
/// each call actually executes against the activation's original `Build`.
/// A call present in the static call scope but never reached — an uncalled
/// helper, an untaken branch — selected nothing; authorized helpers select
/// only because the root `Build` value reaches their frame at run time.
///
/// This pass rejoins the executed coordinates and validates the retained
/// argument identities — an exact toolchain enum case or one exact
/// boundary trait — so replayable evidence, not a call-scope syntax pattern,
/// admits the exclusion.
pub fn harvest_behavior_exclusions(
    typed: &TypedTrees,
    executed: &[checked_interpreter::ExecutedBehaviorExclusion],
) -> Result<Vec<AuthoredBehaviorExclusion>, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut exclusions: Vec<AuthoredBehaviorExclusion> = Vec::new();
    for row in executed {
        // Rejoin the executed coordinate to its authored call for the
        // retained source span and the `exclude_service` type argument.
        struct ExecutedExclusionCall<'a> {
            source_span: source::SourceSpan,
            machine_arguments: &'a [typed_trees::expression::StaticMachineArgument],
            arguments: &'a [typed_trees::expression::ExpressionHandle],
        }
        let call = match row.site {
            checked_interpreter::ExecutedBehaviorExclusionSite::Statement(handle) => {
                match typed.statement_table.statement(handle) {
                    typed_trees::statement::StatementNode::Call(call) => {
                        Some(ExecutedExclusionCall {
                            source_span: call.source_span,
                            machine_arguments: &call.machine_arguments,
                            arguments: typed.statement_table.expression_handles(call.arguments),
                        })
                    }
                    _ => None,
                }
            }
            checked_interpreter::ExecutedBehaviorExclusionSite::Expression(handle) => {
                match typed.expression_table.expression(handle) {
                    typed_trees::expression::ExpressionNode::Call(call) => {
                        Some(ExecutedExclusionCall {
                            source_span: typed.expression_table.source_span(handle),
                            machine_arguments: &call.machine_arguments,
                            arguments: typed.expression_table.expression_handles(call.arguments),
                        })
                    }
                    _ => None,
                }
            }
        };
        let Some(call) = call else {
            diagnostics.push(Diagnostic::error(
                "executed behavior exclusion did not rejoin its authored call",
            ));
            continue;
        };
        let kind = match row.kind {
            checked_interpreter::ExecutedBehaviorExclusionKind::CrashCause { case_symbol } => {
                match evaluated_crash_cause(typed, case_symbol) {
                    Ok(cause) => AuthoredBehaviorExclusionKind::CrashCause { cause, case_symbol },
                    Err(diagnostic) => {
                        diagnostics.push(diagnostic.with_source_span(call.source_span));
                        continue;
                    }
                }
            }
            checked_interpreter::ExecutedBehaviorExclusionKind::Service => {
                match authored_service_exclusion(typed, call.machine_arguments, call.arguments) {
                    Ok(trait_symbol) => AuthoredBehaviorExclusionKind::Service { trait_symbol },
                    Err(diagnostic) => {
                        diagnostics.push(diagnostic.with_source_span(call.source_span));
                        continue;
                    }
                }
            }
            checked_interpreter::ExecutedBehaviorExclusionKind::PhysicalAuthorityClass {
                case_symbol,
            } => match evaluated_physical_authority_class(typed, case_symbol) {
                Ok(class) => {
                    AuthoredBehaviorExclusionKind::PhysicalAuthorityClass { class, case_symbol }
                }
                Err(diagnostic) => {
                    diagnostics.push(diagnostic.with_source_span(call.source_span));
                    continue;
                }
            },
        };
        let exclusion = AuthoredBehaviorExclusion {
            kind,
            selecting_machine: row.machine,
            source_span: call.source_span,
        };
        if !exclusions.iter().any(|existing| {
            existing.kind == exclusion.kind && existing.source_span == exclusion.source_span
        }) {
            exclusions.push(exclusion);
        }
    }
    if diagnostics.is_empty() {
        Ok(exclusions)
    } else {
        Err(diagnostics)
    }
}

/// The exact boundary trait an `exclude_service<Trait>()` marker names.
/// The parser retains exactly one plain type path; ordinary resolution
/// assigned its symbol, which must be a boundary trait declaration. Data,
/// ordinary traits, machines and unresolved paths reject: a service name is
/// an authorized declaration, never a label.
fn authored_service_exclusion(
    typed: &TypedTrees,
    machine_arguments: &[typed_trees::expression::StaticMachineArgument],
    arguments: &[typed_trees::expression::ExpressionHandle],
) -> Result<SymbolHandle, Diagnostic> {
    if !arguments.is_empty() {
        return Err(Diagnostic::error(
            "service exclusion takes no value arguments",
        ));
    }
    let [argument] = machine_arguments else {
        return Err(Diagnostic::error(
            "service exclusion must retain exactly one resolved boundary-trait type path",
        ));
    };
    let authored_path = argument
        .path
        .iter()
        .map(|member| member.as_str())
        .collect::<Vec<_>>()
        .join("::");
    let symbol = argument.symbol;
    if symbol.is_valid()
        && typed.symbols.get(symbol).kind == SymbolKind::Trait
        && typed
            .traits()
            .iter()
            .any(|definition| definition.symbol == symbol && definition.is_boundary)
    {
        Ok(symbol)
    } else {
        Err(Diagnostic::error(format!(
            "service exclusion `{authored_path}` does not resolve to an exact boundary trait declaration"
        )))
    }
}

/// The case symbol one build-declaration argument names, whichever typed
/// shape carries it. A payload-free case such as `CompositionMode::Independent`
/// or `CrashCause::Trap` is normalized to a constructor literal retaining the
/// exact case symbol; marker positions can instead retain the plain name path
/// or a member projection. Every shape must still resolve to an exact
/// toolchain case at the caller; any other expression is not a case.
fn exact_case_argument_symbol(
    typed: &TypedTrees,
    argument: typed_trees::expression::ExpressionHandle,
) -> Option<SymbolHandle> {
    match typed.expression_table.expression(argument) {
        typed_trees::expression::ExpressionNode::Name(path) => Some(path.symbol),
        typed_trees::expression::ExpressionNode::Member(member) => Some(member.member_symbol),
        typed_trees::expression::ExpressionNode::StructLiteral(literal)
            if literal.fields.is_empty() =>
        {
            Some(literal.case_symbol.unwrap_or_else(SymbolHandle::invalid))
        }
        _ => None,
    }
}

/// Map the `CrashCause` variant one EXECUTED `exclude_crash` selection
/// carried — the evaluated value's exact toolchain case symbol — to its
/// Terminal cause. The evaluator validated the case at record time; this
/// independently rechecks the identity so the retained evidence replays.
fn evaluated_crash_cause(
    typed: &TypedTrees,
    case_symbol: SymbolHandle,
) -> Result<terminal_psi::CrashCause, Diagnostic> {
    match evaluated_exclusion_case(typed, case_symbol, "CrashCause")? {
        "Trap" => Ok(terminal_psi::CrashCause::Trap),
        "Abort" => Ok(terminal_psi::CrashCause::Abort),
        other => Err(Diagnostic::error(format!(
            "compiler-owned CrashCause contains unsupported case `{other}`"
        ))),
    }
}

fn evaluated_physical_authority_class(
    typed: &TypedTrees,
    case_symbol: SymbolHandle,
) -> Result<effects::TerminalAuthorityClass, Diagnostic> {
    use effects::TerminalAuthorityClass;
    // This translates exact compiler-owned declarations, not program effects.
    // Mechanism classification remains exclusively in native realization.
    match evaluated_exclusion_case(typed, case_symbol, "PhysicalAuthorityClass")? {
        "FilesystemContentRead" => Ok(TerminalAuthorityClass::FilesystemContentRead),
        "FilesystemContentWrite" => Ok(TerminalAuthorityClass::FilesystemContentWrite),
        "FilesystemMetadataQuery" => Ok(TerminalAuthorityClass::FilesystemMetadataQuery),
        "DirectoryEnumeration" => Ok(TerminalAuthorityClass::DirectoryEnumeration),
        "FilesystemNamespaceMutation" => Ok(TerminalAuthorityClass::FilesystemNamespaceMutation),
        "FilesystemMetadataMutation" => Ok(TerminalAuthorityClass::FilesystemMetadataMutation),
        "ProcessOutput" => Ok(TerminalAuthorityClass::ProcessOutput),
        "ProcessTermination" => Ok(TerminalAuthorityClass::ProcessTermination),
        "MachineControl" => Ok(TerminalAuthorityClass::MachineControl),
        "PortIo" => Ok(TerminalAuthorityClass::PortIo),
        "InterruptControl" => Ok(TerminalAuthorityClass::InterruptControl),
        "InterruptEntry" => Ok(TerminalAuthorityClass::InterruptEntry),
        "RootMemoryAccess" => Ok(TerminalAuthorityClass::RootMemoryAccess),
        "ProcessInput" => Ok(TerminalAuthorityClass::ProcessInput),
        other => Err(Diagnostic::error(format!(
            "compiler-owned PhysicalAuthorityClass contains unsupported case `{other}`"
        ))),
    }
}

/// Recheck evaluated enum evidence independently of the interpreter. A
/// same-spelled user enum, sibling case, or payload-bearing case is not a
/// selection from the exact compiler-owned declaration.
fn evaluated_exclusion_case<'a>(
    typed: &'a TypedTrees,
    case_symbol: SymbolHandle,
    type_name: &str,
) -> Result<&'a str, Diagnostic> {
    let exact_types = typed
        .data_definitions()
        .iter()
        .filter(|definition| {
            is_exact_toolchain_build_prelude_data(typed, definition.symbol, type_name)
        })
        .collect::<Vec<_>>();
    let [declaration] = exact_types.as_slice() else {
        return Err(Diagnostic::error(format!(
            "behavior exclusion requires exactly one compiler-owned {type_name} declaration"
        )));
    };
    let selected = typed
        .data_members(declaration)
        .iter()
        .filter_map(|member| match member {
            typed_trees::data::DataMember::Variant(variant)
                if variant.symbol == case_symbol
                    && typed.symbols.get(variant.symbol).parent == declaration.symbol =>
            {
                Some(variant)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [selected] = selected.as_slice() else {
        return Err(Diagnostic::error(format!(
            "executed behavior exclusion does not name an exact compiler-owned {type_name} case"
        )));
    };
    if !typed.data_payload_fields(selected).is_empty() {
        return Err(Diagnostic::error(format!(
            "compiler-owned {type_name} case unexpectedly carries a payload"
        )));
    }
    Ok(selected.name.as_str())
}

fn authored_root_grant_statement_span(
    typed: &TypedTrees,
    call: &typed_trees::statement::TableCall,
) -> Result<source::SourceSpan, Diagnostic> {
    let Some(occurrence) = call.authored_call_selection else {
        return Err(Diagnostic::error(
            "build boundary grant has no authored selection occurrence",
        ));
    };
    authored_root_grant_selection_span(typed, occurrence)
}

fn authored_root_grant_expression_span(
    typed: &TypedTrees,
    expression: typed_trees::expression::ExpressionHandle,
) -> Result<source::SourceSpan, Diagnostic> {
    let occurrences = typed
        .expression_table
        .authored_selection_occurrences(expression)
        .filter_map(|occurrence| {
            typed
                .authored_declaration_selections()
                .get(occurrence)
                .filter(|selection| authored_root_grant_selection(**selection))
                .map(|_| occurrence)
        })
        .collect::<Vec<_>>();
    let [occurrence] = occurrences.as_slice() else {
        return Err(Diagnostic::error(format!(
            "build boundary grant expression has {} exact authored selection occurrences",
            occurrences.len(),
        )));
    };
    authored_root_grant_selection_span(typed, *occurrence)
}

fn authored_root_grant_selection_span(
    typed: &TypedTrees,
    occurrence: language_semantics::declaration_selection::AuthoredDeclarationSelectionOccurrenceId,
) -> Result<source::SourceSpan, Diagnostic> {
    let selection = typed
        .authored_declaration_selections()
        .get(occurrence)
        .filter(|selection| authored_root_grant_selection(**selection))
        .ok_or_else(|| {
            Diagnostic::error("build boundary grant has no exact authored selection evidence")
        })?;
    let span = selection.source_span();
    if span.span.start >= span.span.end {
        return Err(Diagnostic::error(
            "build boundary grant has an empty authored source span",
        ));
    }
    Ok(span)
}

fn authored_root_grant_selection(
    selection: language_semantics::declaration_selection::AuthoredDeclarationSelection,
) -> bool {
    use language_semantics::declaration_selection::{
        AuthoredDeclarationSelectionIntrinsic as Intrinsic,
        AuthoredDeclarationSelectionKind as Kind,
        AuthoredDeclarationSelectionLateBinding as LateBinding,
        AuthoredDeclarationSelectionTarget as Target,
    };
    selection.kind() == Kind::Call
        && matches!(
            selection.target(),
            Target::LateBound(LateBinding::CheckedCall)
                | Target::Intrinsic(Intrinsic::BuildBoundaryAcceptance)
        )
}
