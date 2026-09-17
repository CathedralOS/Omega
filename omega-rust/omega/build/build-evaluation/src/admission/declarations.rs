//! Authored provider selections, boundary grants, wire demands, and behavior
//! exclusions.

use crate::admission::behavior_exclusions::{
    AuthoredBehaviorExclusion, AuthoredBehaviorExclusionKind,
};
use crate::admission::vocabulary::{
    is_exact_toolchain_build_prelude_data, toolchain_build_prelude_machine,
};
use diagnostics::Diagnostic;
use provider_planning::{ProviderSelection, ProviderSelectionIdentity};
use std::collections::{HashMap, HashSet};
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
        let Some(case_symbol) = exact_case_argument_symbol(typed, *argument) else {
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

/// Behavior exclusions (wiki/spec/build/behavior_exclusions.md):
/// `builder.exclude_crash(CrashCause::X)` is a product-admission requirement
/// harvested statically from the root build entry's checked static call
/// scope — the build machine itself plus every machine it can call
/// statically, so authorized helpers may assemble selections.
///
/// The scope boundary is conservative. A call to the toolchain exclusion
/// machine in a machine the build entry cannot reach, or in a nested
/// (non-statement) call position, is rejected rather than dropped: an
/// authored restriction the admission check could not see would silently
/// over-admit. A selection anywhere in the admitted scope applies
/// unconditionally — it is a declaration, not an executed effect, so
/// conditional placement does not narrow it.
pub fn harvest_behavior_exclusions(
    typed: &TypedTrees,
    operational_plan: &flow_effects::OperationalPlan,
    machine: &typed_trees::machine::Machine,
) -> Result<Vec<AuthoredBehaviorExclusion>, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let mut exclusions: Vec<AuthoredBehaviorExclusion> = Vec::new();
    // The crash vocabulary is a toolchain prelude machine; a program without
    // it cannot spell a crash exclusion. The service marker is a parser-level
    // build declaration (`exclude_service<Trait>()`) and is harvested by name
    // below regardless.
    let exclusion_machine = toolchain_build_prelude_machine(typed, "Build::exclude_crash");
    let mut exclusion_targets: HashSet<SymbolHandle> = exclusion_machine
        .and_then(|exclusion_machine| {
            typed
                .machines()
                .iter()
                .find(|candidate| candidate.symbol == exclusion_machine)
                .map(|candidate| {
                    typed
                        .machine_states(candidate)
                        .iter()
                        .map(|state| state.symbol)
                        .collect()
                })
        })
        .unwrap_or_default();
    exclusion_targets.extend(exclusion_machine);

    // The root build machine's checked static call scope. Dynamic calls
    // carry no target identity and cannot add or hide a selection here.
    let mut reachable = vec![machine.symbol];
    let mut pending = vec![machine.symbol];
    while let Some(symbol) = pending.pop() {
        let Some(machine_operational) = operational_plan
            .machines()
            .iter()
            .find(|candidate| candidate.symbol == symbol)
        else {
            continue;
        };
        for state in operational_plan
            .states
            .span_or_empty(machine_operational.states)
        {
            for call in operational_plan.calls.span_or_empty(state.calls) {
                if call.target_machine_symbol.is_valid()
                    && !reachable.contains(&call.target_machine_symbol)
                {
                    reachable.push(call.target_machine_symbol);
                    pending.push(call.target_machine_symbol);
                }
            }
        }
    }
    // A specialization template shares the authored span of its instances;
    // the executed copy is the reachable one, so a template is admitted
    // exactly when the build scope can reach at least one of its instances.
    let admissible = |symbol: SymbolHandle| {
        reachable.contains(&symbol)
            || typed.machine_specializations.iter().any(|specialization| {
                specialization.template == symbol && reachable.contains(&specialization.instance)
            })
    };

    let mut found_per_machine: HashMap<SymbolHandle, usize> = HashMap::new();
    let mut scope_errored: HashSet<SymbolHandle> = HashSet::new();
    let mut record = |typed: &TypedTrees,
                      candidate: &typed_trees::machine::Machine,
                      target: ExclusionCallTarget,
                      machine_arguments: &[typed_trees::expression::StaticMachineArgument],
                      arguments: &[typed_trees::expression::ExpressionHandle],
                      source_span: source::SourceSpan,
                      exclusions: &mut Vec<AuthoredBehaviorExclusion>,
                      diagnostics: &mut Vec<Diagnostic>| {
        let spelling = match target {
            ExclusionCallTarget::None => return,
            ExclusionCallTarget::CrashCause => "exclude_crash",
            ExclusionCallTarget::Service => "exclude_service",
        };
        if target == ExclusionCallTarget::CrashCause {
            *found_per_machine.entry(candidate.symbol).or_insert(0) += 1;
        }
        if !admissible(candidate.symbol) {
            if scope_errored.insert(candidate.symbol) {
                diagnostics.push(
                    Diagnostic::error(format!(
                        "behavior exclusion `{spelling}` in machine `{}` is outside the root build machine's checked call scope",
                        candidate.name.as_str(),
                    ))
                    .with_source_span(source_span),
                );
            }
            return;
        }
        let kind = match target {
            ExclusionCallTarget::CrashCause => match authored_crash_cause(typed, arguments) {
                Ok((cause, case_symbol)) => {
                    AuthoredBehaviorExclusionKind::CrashCause { cause, case_symbol }
                }
                Err(diagnostic) => {
                    diagnostics.push(diagnostic.with_source_span(source_span));
                    return;
                }
            },
            ExclusionCallTarget::Service => {
                match authored_service_exclusion(typed, machine_arguments, arguments) {
                    Ok(trait_symbol) => AuthoredBehaviorExclusionKind::Service { trait_symbol },
                    Err(diagnostic) => {
                        diagnostics.push(diagnostic.with_source_span(source_span));
                        return;
                    }
                }
            }
            ExclusionCallTarget::None => return,
        };
        let exclusion = AuthoredBehaviorExclusion {
            kind,
            selecting_machine: candidate.symbol,
            source_span,
        };
        if !exclusions.iter().any(|existing| {
            existing.kind == exclusion.kind && existing.source_span == exclusion.source_span
        }) {
            exclusions.push(exclusion);
        }
    };
    let classify = |target_symbol: SymbolHandle, target: &str| {
        if exclusion_targets.contains(&target_symbol) {
            ExclusionCallTarget::CrashCause
        } else if !target_symbol.is_valid() && target == "exclude_service" {
            ExclusionCallTarget::Service
        } else {
            ExclusionCallTarget::None
        }
    };

    for candidate in typed.machines() {
        for state in typed.machine_states(candidate) {
            for statement in typed.statement_table.statements(state.statement_nodes) {
                match statement {
                    typed_trees::statement::StatementNode::Call(call) => record(
                        typed,
                        candidate,
                        classify(call.target_symbol, call.target.as_str()),
                        &call.machine_arguments,
                        typed.statement_table.expression_handles(call.arguments),
                        call.source_span,
                        &mut exclusions,
                        &mut diagnostics,
                    ),
                    typed_trees::statement::StatementNode::Expression(expression) => {
                        if let typed_trees::expression::ExpressionNode::Call(call) =
                            typed.expression_table.expression(*expression)
                        {
                            record(
                                typed,
                                candidate,
                                classify(call.target_symbol, call.target.as_str()),
                                &call.machine_arguments,
                                typed.expression_table.expression_handles(call.arguments),
                                typed.expression_table.source_span(*expression),
                                &mut exclusions,
                                &mut diagnostics,
                            );
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // Coverage cross-check against the operational plan, which enumerates
    // every call site including nested call positions the statement scan
    // above deliberately does not retain. An admissible machine whose count
    // disagrees hid the selection where this harvest cannot see it.
    for machine_operational in operational_plan.machines() {
        let expected = operational_plan
            .states
            .span_or_empty(machine_operational.states)
            .iter()
            .flat_map(|state| operational_plan.calls.span_or_empty(state.calls).iter())
            .filter(|call| Some(call.target_machine_symbol) == exclusion_machine)
            .count();
        if expected == 0 {
            continue;
        }
        let found = found_per_machine
            .get(&machine_operational.symbol)
            .copied()
            .unwrap_or(0);
        let name = typed
            .machines()
            .iter()
            .find(|machine| machine.symbol == machine_operational.symbol)
            .map(|machine| machine.name.as_str().to_owned())
            .unwrap_or_else(|| "<unknown machine>".to_owned());
        if !admissible(machine_operational.symbol) {
            if scope_errored.insert(machine_operational.symbol) {
                diagnostics.push(Diagnostic::error(format!(
                    "behavior exclusion `exclude_crash` in machine `{name}` is outside the root build machine's checked call scope",
                )));
            }
        } else if found != expected {
            diagnostics.push(Diagnostic::error(format!(
                "behavior exclusion `exclude_crash` in machine `{name}` must be a direct statement; nested call positions are not implemented",
            )));
        }
    }

    if diagnostics.is_empty() {
        Ok(exclusions)
    } else {
        Err(diagnostics)
    }
}

/// Validate the single `exclude_crash` argument as an exact compiler-owned
/// `CrashCause` case, mirroring the `CompositionMode` case check.
/// Which exclusion spelling one scanned call is, if any.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExclusionCallTarget {
    None,
    CrashCause,
    Service,
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

fn authored_crash_cause(
    typed: &TypedTrees,
    arguments: &[typed_trees::expression::ExpressionHandle],
) -> Result<(terminal_psi::CrashCause, SymbolHandle), Diagnostic> {
    let [argument] = arguments else {
        return Err(Diagnostic::error(
            "behavior exclusion `exclude_crash` takes exactly one compiler-owned CrashCause case",
        ));
    };
    let Some(case_symbol) = exact_case_argument_symbol(typed, *argument) else {
        return Err(Diagnostic::error(
            "behavior exclusion `exclude_crash` argument must be the exact compiler-owned CrashCause case",
        ));
    };
    let exact_causes = typed
        .data_definitions()
        .iter()
        .filter(|definition| {
            is_exact_toolchain_build_prelude_data(typed, definition.symbol, "CrashCause")
        })
        .collect::<Vec<_>>();
    let [causes] = exact_causes.as_slice() else {
        return Err(Diagnostic::error(
            "behavior exclusion requires exactly one compiler-owned CrashCause declaration",
        ));
    };
    let selected = typed
        .data_members(causes)
        .iter()
        .filter_map(|member| match member {
            typed_trees::data::DataMember::Variant(variant)
                if variant.symbol == case_symbol
                    && typed.symbols.get(variant.symbol).parent == causes.symbol =>
            {
                Some(variant)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let [selected] = selected.as_slice() else {
        return Err(Diagnostic::error(
            "behavior exclusion `exclude_crash` does not name an exact compiler-owned CrashCause case",
        ));
    };
    if !typed.data_payload_fields(selected).is_empty() {
        return Err(Diagnostic::error(
            "compiler-owned CrashCause case unexpectedly carries a payload",
        ));
    }
    let cause = match selected.name.as_str() {
        "Trap" => terminal_psi::CrashCause::Trap,
        "Abort" => terminal_psi::CrashCause::Abort,
        other => {
            return Err(Diagnostic::error(format!(
                "compiler-owned CrashCause contains unsupported case `{other}`"
            )));
        }
    };
    Ok((cause, case_symbol))
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
