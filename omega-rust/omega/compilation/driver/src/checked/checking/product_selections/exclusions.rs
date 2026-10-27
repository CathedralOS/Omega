//! Join executed, source-issued Build exclusions to the final Product namespace.
//! Formation checks every service operand; only the normal-return execution
//! ledger selects restrictions. No source walk substitutes for execution.

use build_time_evaluation::is_exact_toolchain_build_prelude_data;
use diagnostics::Diagnostic;
use effects::{AuthoredBehaviorExclusion, AuthoredBehaviorExclusionKind};
use language_semantics::declaration_selection::{
    BuildOperation, ExclusionVocabulary, ExecutedEnumExclusion, ExecutedProductSelection,
    ProductDeclarationRequest,
};
use symbols::SymbolHandle;
use typed_trees::TypedTrees;

pub(super) fn resolve(
    typed: &TypedTrees,
    activation: Option<SymbolHandle>,
    declared: &[std::sync::Arc<ProductDeclarationRequest>],
    executed: &[ExecutedProductSelection],
    enums: &[ExecutedEnumExclusion],
) -> Result<Vec<AuthoredBehaviorExclusion>, Vec<Diagnostic>> {
    for request in declared
        .iter()
        .filter(|request| request.operation == BuildOperation::ServiceExclusion)
    {
        service(typed, request)
            .map_err(|error| vec![error.with_source_span(request.source_span)])?;
    }
    let mut exclusions = Vec::new();
    let mut retain = |kind, source_span, machine_span| -> Result<(), Diagnostic> {
        activation
            .filter(|symbol| typed.machine(*symbol).is_some())
            .ok_or_else(|| {
                Diagnostic::error("behavior exclusion has no authoritative Build activation")
            })?;
        let selecting_machine = typed
            .machines()
            .iter()
            .find(|machine| typed.symbols.symbol_source_span(machine.symbol) == Some(machine_span))
            .map(|machine| machine.symbol)
            .ok_or_else(|| {
                Diagnostic::error("behavior exclusion lost its authored selecting machine")
            })?;
        let row = AuthoredBehaviorExclusion {
            kind,
            source_span,
            selecting_machine,
        };
        if !exclusions.contains(&row) {
            exclusions.push(row);
        }
        Ok(())
    };
    for event in executed
        .iter()
        .filter(|event| event.request.operation == BuildOperation::ServiceExclusion)
    {
        let result = (|| {
            if !declared.contains(&event.request) || event.composition.is_some() {
                return Err(Diagnostic::error(
                    "executed service exclusion lost its source-issued declaration request",
                ));
            }
            retain(
                service(typed, &event.request)?,
                event.request.source_span,
                event.selecting_machine.ok_or_else(|| {
                    Diagnostic::error(
                        "executed service exclusion lost its authored machine coordinate",
                    )
                })?,
            )
        })();
        result.map_err(|error| vec![error.with_source_span(event.request.source_span)])?;
    }
    for event in enums {
        let result = (|| {
            let vocabulary = event.request.vocabulary;
            let case = usize::try_from(event.code)
                .ok()
                .and_then(|code| vocabulary.cases().get(code))
                .ok_or_else(|| {
                    Diagnostic::error("executed behavior exclusion has an invalid vocabulary code")
                })?;
            let declaration = typed
                .data_definitions()
                .iter()
                .find(|definition| {
                    is_exact_toolchain_build_prelude_data(
                        typed,
                        definition.symbol,
                        vocabulary.type_name(),
                    )
                })
                .ok_or_else(|| {
                    Diagnostic::error("behavior exclusion lost its exact toolchain enum")
                })?;
            let case_symbol = typed
                .data_members(declaration)
                .iter()
                .find_map(|member| match member {
                    typed_trees::data::DataMember::Variant(variant)
                        if variant.name.as_str() == *case =>
                    {
                        Some(variant.symbol)
                    }
                    _ => None,
                })
                .ok_or_else(|| {
                    Diagnostic::error("behavior exclusion lost its exact toolchain case")
                })?;
            let kind = match vocabulary {
                ExclusionVocabulary::CrashCause => AuthoredBehaviorExclusionKind::CrashCause {
                    cause: evaluated_crash_cause(typed, case_symbol)?,
                    case_symbol,
                },
                ExclusionVocabulary::PhysicalAuthorityClass => {
                    AuthoredBehaviorExclusionKind::PhysicalAuthorityClass {
                        class: evaluated_physical_authority_class(typed, case_symbol)?,
                        case_symbol,
                    }
                }
            };
            retain(kind, event.request.source_span, event.selecting_machine)
        })();
        result.map_err(|error| vec![error.with_source_span(event.request.source_span)])?;
    }
    Ok(exclusions)
}

fn service(
    typed: &TypedTrees,
    request: &ProductDeclarationRequest,
) -> Result<AuthoredBehaviorExclusionKind, Diagnostic> {
    let [operand] = request.operands.as_ref() else {
        return Err(Diagnostic::error(
            "service exclusion requires one exact Product boundary trait",
        ));
    };
    let symbol = typed_trees::product_operand::resolve_product_path(
        typed, &operand.path, operand.source_span,
        typed_trees::product_operand::ProductOperandKind::BoundarySubject,
    ).ok_or_else(|| Diagnostic::error("service exclusion does not resolve to an exact visible product boundary trait declaration"))?;
    let definition = typed
        .product_declarations()
        .trait_definition_by_symbol(symbol)
        .filter(|definition| definition.is_boundary)
        .ok_or_else(|| Diagnostic::error("service exclusion requires an exact boundary trait"))?;
    let identity = typed
        .normalized_hermetic_symbol_identity(definition.symbol)
        .map_err(|error| {
            Diagnostic::error(format!("service exclusion declaration identity: {error}"))
        })?;
    Ok(AuthoredBehaviorExclusionKind::Service {
        identity,
        name: definition.name.as_str().to_owned(),
    })
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
