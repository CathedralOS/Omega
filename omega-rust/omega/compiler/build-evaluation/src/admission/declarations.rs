//! Authored boundary grants, wire demands, and behavior
//! exclusions.

use crate::admission::behavior_exclusions::{
    AuthoredBehaviorExclusion, AuthoredBehaviorExclusionKind,
};
use build_time_evaluation::is_exact_toolchain_build_prelude_data;
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::BuildOperation;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;

pub use build_time_evaluation::wire_compatibility::WireCompatibilityDemand;

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
