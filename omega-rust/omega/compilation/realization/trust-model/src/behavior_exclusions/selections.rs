//! Rejoin executed Build exclusions to their exact authored declarations.

use build_time_evaluation::is_exact_toolchain_build_prelude_data;
use diagnostics::Diagnostic;
use effects::{AuthoredBehaviorExclusion, AuthoredBehaviorExclusionKind};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

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
        let source_span = match row.site {
            checked_interpreter::ExecutedBehaviorExclusionSite::Statement(handle) => {
                match typed.statement_table.statement(handle) {
                    typed_trees::statement::StatementNode::Call(call) => Some(call.source_span),
                    _ => None,
                }
            }
            checked_interpreter::ExecutedBehaviorExclusionSite::Expression(handle) => {
                match typed.expression_table.expression(handle) {
                    typed_trees::expression::ExpressionNode::Call(_) => {
                        Some(typed.expression_table.source_span(handle))
                    }
                    _ => None,
                }
            }
        };
        let Some(source_span) = source_span else {
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
                        diagnostics.push(diagnostic.with_source_span(source_span));
                        continue;
                    }
                }
            }
            checked_interpreter::ExecutedBehaviorExclusionKind::Service => {
                match authored_service_exclusion(typed, row.site, source_span) {
                    Ok((identity, name)) => {
                        AuthoredBehaviorExclusionKind::Service { identity, name }
                    }
                    Err(diagnostic) => {
                        diagnostics.push(diagnostic.with_source_span(source_span));
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
                    diagnostics.push(diagnostic.with_source_span(source_span));
                    continue;
                }
            },
        };
        let exclusion = AuthoredBehaviorExclusion {
            kind,
            selecting_machine: row.machine,
            source_span,
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

/// Retain the service identity selected by `Build::exclude_service<Trait>`:
/// the hermetic declaration identity Terminal service catalogs publish, plus
/// the declared name as its diagnostic label.
/// Resolve it in the executed Build's product-declaration environment, not its
/// own executable declarations: dependencies need not be executable Build code.
/// native realization must not reopen typed source to interpret this selection.
/// The call retains exactly one plain type path in its product namespace. Data,
/// ordinary traits, machines and unresolved paths reject: a service name is
/// an authorized declaration, never a label.
fn authored_service_exclusion(
    typed: &TypedTrees,
    site: checked_interpreter::ExecutedBehaviorExclusionSite,
    occurrence: source::SourceSpan,
) -> Result<(String, String), Diagnostic> {
    let checked_interpreter::ExecutedBehaviorExclusionSite::Expression(expression) = site else {
        return Err(Diagnostic::error(
            "service exclusion must retain its checked static Build call",
        ));
    };
    validation::compiler_operations::resolve_build_declaration_operand(typed, expression, 0, occurrence)
        .and_then(|symbol| {
            typed.product_declarations().trait_definition_by_symbol(symbol)
                .filter(|definition| definition.is_boundary)
                .and_then(|definition| {
                    let identity = typed.normalized_hermetic_symbol_identity(definition.symbol).ok()?;
                    Some((identity, definition.name.as_str().to_owned()))
                })
        })
        .ok_or_else(|| Diagnostic::error(
            "service exclusion does not resolve to an exact visible product boundary trait declaration",
        ))
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
