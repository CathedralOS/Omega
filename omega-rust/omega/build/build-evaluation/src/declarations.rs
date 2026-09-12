//! Authored provider selections, boundary grants, and wire demands.

use crate::vocabulary::is_exact_toolchain_build_prelude_data;
use diagnostics::Diagnostic;
use provider_planning::{ProviderSelection, ProviderSelectionIdentity};
use symbols::SymbolKind;
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
pub(super) fn harvest_wire_compatibility_demands(
    typed: &TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> Result<Vec<WireCompatibilityDemand>, Vec<Diagnostic>> {
    let mut demands = Vec::new();
    let mut diagnostics = Vec::new();
    let mut record = |target: &str| {
        let Some(encoded) = target.strip_prefix("wire_compatibility#") else {
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

/// PRV4c: collect `builder.select_provider<Subject, ProviderType>();` from the
/// one authoritative build machine. `Subject` is either one exact boundary
/// trait, one exact explicit top-level boundary requirement, or one exact
/// package-qualified boundary-operator family. Merely spelling a declaration
/// elsewhere grants nothing; selection authority comes from this file-scoped
/// root.
pub fn harvest_provider_selections(
    typed: &TypedTrees,
    machine: &typed_trees::machine::Machine,
) -> Result<Vec<ProviderSelection>, Vec<Diagnostic>> {
    let mut selections: Vec<ProviderSelection> = Vec::new();
    let mut diagnostics = Vec::new();
    let mut record = |target: &str,
                      arguments: &[typed_trees::expression::StaticMachineArgument],
                      value_arguments: &[typed_trees::expression::ExpressionHandle],
                      source_span: source::SourceSpan| {
        if target != "select_provider" {
            return;
        }
        let [boundary_argument, provider_argument] = arguments else {
            diagnostics.push(Diagnostic::error(
                "provider selection must retain exactly two resolved type paths",
            ));
            return;
        };
        let project_identity = |argument: &typed_trees::expression::StaticMachineArgument| {
            let authored_path = argument
                .path
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::");
            ProviderSelectionIdentity {
                symbol: argument.symbol,
                package: typed.symbols.symbol_package_identity(argument.symbol),
                canonical_path: typed.symbols.display_path(argument.symbol, "::"),
                authored_path,
            }
        };
        let boundary_identity = project_identity(boundary_argument);
        let provider_type = project_identity(provider_argument);
        let composition_mode = match provider_selection_composition_mode(typed, value_arguments) {
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
            let Some(representative) =
                typed_trees::operator::declaration_by_symbol(typed, boundary_identity.symbol)
            else {
                diagnostics.push(Diagnostic::error(format!(
                    "provider selection subject `{}` has no exact retained operator declaration",
                    boundary_identity.authored_path
                )));
                return;
            };
            let canonical_path = typed
                .operator_path_members(representative.name)
                .iter()
                .map(|member| member.as_str())
                .collect::<Vec<_>>()
                .join("::");
            let family_package = typed.symbols.symbol_package_identity(representative.symbol);
            let mut coordinates = typed
                .operators()
                .iter()
                .chain(
                    typed
                        .domain_definitions()
                        .iter()
                        .flat_map(|domain| typed.domain_operators(domain)),
                )
                .filter(|operator| {
                    operator.is_boundary
                        && typed.symbols.symbol_package_identity(operator.symbol) == family_package
                        && typed
                            .operator_path_members(operator.name)
                            .iter()
                            .map(|member| member.as_str())
                            .collect::<Vec<_>>()
                            .join("::")
                            == canonical_path
                })
                .map(
                    |operator| provider_planning::ProviderOperatorFamilyCoordinate {
                        symbol: operator.symbol,
                        requirement_identity:
                            typed_trees::operator::boundary_operator_requirement_identity(
                                typed, operator,
                            ),
                        static_parameter_count: operator.lifetime_parameters.len()
                            + typed.operator_type_parameters(operator).len(),
                    },
                )
                .collect::<Vec<_>>();
            match provider_planning::ProviderOperatorFamilySelection::new(
                family_package,
                canonical_path,
                boundary_identity.authored_path,
                std::mem::take(&mut coordinates),
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
            selecting_machine: machine.symbol,
            source_span,
        });
    };
    for state in typed.machine_states(machine) {
        for statement in typed.statement_table.statements(state.statement_nodes) {
            match statement {
                typed_trees::statement::StatementNode::Expression(expression) => {
                    if let typed_trees::expression::ExpressionNode::Call(call) =
                        typed.expression_table.expression(*expression)
                    {
                        record(
                            call.target.as_str(),
                            &call.machine_arguments,
                            typed.expression_table.expression_handles(call.arguments),
                            typed.expression_table.source_span(*expression),
                        );
                    }
                }
                typed_trees::statement::StatementNode::Call(call) => {
                    record(
                        call.target.as_str(),
                        &call.machine_arguments,
                        typed.statement_table.expression_handles(call.arguments),
                        call.source_span,
                    );
                }
                _ => {}
            }
        }
    }
    if diagnostics.is_empty() {
        Ok(selections)
    } else {
        Err(diagnostics)
    }
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
        let typed_trees::expression::ExpressionNode::Name(path) =
            typed.expression_table.expression(*argument)
        else {
            return Err(Diagnostic::error(
                "provider selection composition mode must be the exact compiler-owned CompositionMode::Fused or CompositionMode::Independent case",
            ));
        };
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
                    if variant.symbol == path.symbol
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
        return match selected.name.as_str() {
            "Fused" => Ok(provider_planning::CompositionMode::Fused),
            "Independent" => Ok(provider_planning::CompositionMode::Independent),
            other => Err(Diagnostic::error(format!(
                "compiler-owned CompositionMode contains unsupported case `{other}`"
            ))),
        };
    };
    Ok(provider_planning::CompositionMode::Fused)
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
                    if let Some(path) = call.target.as_str().strip_prefix("accept_boundary#") {
                        record(path, authored_root_grant_statement_span(typed, call)?);
                    }
                    Vec::new()
                }
                _ => Vec::new(),
            };
            for handle in handles {
                if let typed_trees::expression::ExpressionNode::Call(call) =
                    typed.expression_table.expression(handle)
                    && let Some(path) = call.target.as_str().strip_prefix("accept_boundary#")
                {
                    record(path, authored_root_grant_expression_span(typed, handle)?);
                }
            }
        }
    }
    Ok(grants)
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
