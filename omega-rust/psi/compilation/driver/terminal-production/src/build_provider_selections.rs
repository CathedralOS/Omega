//! Executed provider descriptions beside exact ordinary Terminal imports.
//!
//! Source operands never enter portable code as authority. This companion is
//! issued from authenticated checked Build operations and bound to the emitted
//! artifact; only the original activation may turn its calls into receipts.

use diagnostics::Diagnostic;
use language_semantics::declaration_selection::ProviderCompositionMode;
use semantic_vocabulary::{BoundaryMachineId, OperationId, StructuralCaseId};

#[derive(Debug, PartialEq, Eq)]
pub struct TerminalBuildProviderCall {
    pub operation: OperationId,
    pub boundary: BoundaryMachineId,
    pub source_span: source::SourceSpan,
    pub selecting_machine: source::SourceSpan,
    pub mode_cases: Vec<(StructuralCaseId, ProviderCompositionMode)>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct TerminalBuildProviderSelections {
    identity: terminal_codec::TerminalExecutionIdentity,
    calls: Vec<TerminalBuildProviderCall>,
}

impl TerminalBuildProviderSelections {
    pub fn calls_for(
        &self,
        artifact: terminal_codec::TerminalExecutionView<'_>,
    ) -> Option<&[TerminalBuildProviderCall]> {
        (self.identity == artifact.custody_identity()).then_some(&self.calls)
    }
    pub(super) fn new(
        artifact: terminal_codec::TerminalExecutionView<'_>,
        calls: Vec<TerminalBuildProviderCall>,
    ) -> Self {
        Self {
            identity: artifact.custody_identity(),
            calls,
        }
    }
}

pub(super) fn capture(
    checked: &checked_trees::CheckedTrees,
    lowered: &lowered_psi::LoweredPsi,
) -> Result<Vec<TerminalBuildProviderCall>, Diagnostic> {
    let mut calls = Vec::new();
    for occurrence in &lowered.source_call_occurrences {
        let Some(checked_trees::NominalMachineUseSite::Expression(expression)) =
            occurrence.source_site
        else {
            continue;
        };
        let Some((definition, signature)) =
            validation::compiler_operations::provider_selection_protocol(checked, expression)
        else {
            continue;
        };
        let semantic_vocabulary::BoundaryInvocationSite::Operation(operation) =
            occurrence.terminal_site
        else {
            return Err(Diagnostic::error(
                "Build provider selection lost its ordinary emitted call",
            ));
        };
        let boundary = crate::build_bindings::emitted_boundary(lowered, operation)?;
        let declaration = lowered
            .semantic_module
            .boundary_machines
            .iter()
            .find(|declaration| declaration.id == boundary)
            .ok_or_else(|| {
                Diagnostic::error("Build provider selection lost its boundary declaration")
            })?;
        let machine = checked
            .machine_holding_state(occurrence.source_state)
            .ok_or_else(|| {
                Diagnostic::error("Build provider selection lost its authored machine")
            })?;
        let selecting_machine = checked
            .symbols
            .symbol_source_span(machine.symbol)
            .ok_or_else(|| {
                Diagnostic::error("Build provider selection has no authored machine coordinate")
            })?;
        let authored = checked.expression_table.authored_selection_occurrences(expression)
            .filter_map(|id| checked.authored_declaration_selections().get(id))
            .filter(|selection| selection.kind() == language_semantics::declaration_selection::AuthoredDeclarationSelectionKind::Call)
            .collect::<Vec<_>>();
        let [authored] = authored.as_slice() else {
            return Err(Diagnostic::error(
                "Build provider selection has no unique authored call occurrence",
            ));
        };
        // Rejoin the emitted mode schema through the exact toolchain protocol
        // signature. Numeric tag allocation and declaration order grant nothing.
        let mut mode_cases = Vec::new();
        if signature.name.as_str() == "select_provider_mode" {
            let parameters = checked.state_signature_parameters(signature);
            let mode = parameters
                .get(1)
                .ok_or_else(|| Diagnostic::error("Build provider mode has no formal"))?;
            let symbol = checked
                .type_reference_table
                .type_symbol(mode.type_reference);
            let mode = checked
                .data_definitions()
                .iter()
                .find(|data| data.symbol == symbol)
                .ok_or_else(|| {
                    Diagnostic::error("Build provider mode has no exact data declaration")
                })?;
            if mode.name.as_str() != "CompositionMode"
                || !language_semantics::build_requests::is_build_protocol(
                    &checked.symbols,
                    definition.symbol,
                    "BuildProviderSelectionOperations",
                )
            {
                return Err(Diagnostic::error(
                    "Build provider mode is not the compiler-owned protocol value",
                ));
            }
            let parameter = declaration.structural_parameters.get(1).ok_or_else(|| {
                Diagnostic::error("Build provider mode lost its structural argument")
            })?;
            let shape = lowered
                .semantic_module
                .structural_types
                .iter()
                .find(|data| data.id == parameter.structural_type)
                .ok_or_else(|| Diagnostic::error("Build provider mode lost its emitted schema"))?;
            let terminal_psi::StructuralTypeShape::Sum { cases } = &shape.shape else {
                return Err(Diagnostic::error(
                    "Build provider mode is not a closed case value",
                ));
            };
            for member in checked.data_members(mode) {
                let typed_trees::data::DataMember::Variant(variant) = member else {
                    return Err(Diagnostic::error(
                        "Build provider mode contains non-case data",
                    ));
                };
                let composition = match variant.name.as_str() {
                    "Fused" => ProviderCompositionMode::Fused,
                    "Independent" => ProviderCompositionMode::Independent,
                    _ => {
                        return Err(Diagnostic::error(
                            "Build provider mode contains an unknown case",
                        ));
                    }
                };
                let identity = checked_trees::CheckedMemberIdentity::of_member(
                    variant.identity,
                    variant.name.clone(),
                )
                .spelling();
                let matches = cases
                    .iter()
                    .filter(|case| {
                        lowered.semantic_module.identities.text(case.identity)
                            == Some(identity.as_str())
                            && case.fields.is_empty()
                    })
                    .collect::<Vec<_>>();
                let [case] = matches.as_slice() else {
                    return Err(Diagnostic::error(
                        "Build provider mode case lost its exact emitted identity",
                    ));
                };
                mode_cases.push((case.id, composition));
            }
            if mode_cases.len() != 2 || cases.len() != 2 {
                return Err(Diagnostic::error(
                    "Build provider mode schema is incomplete",
                ));
            }
        }
        calls.push(TerminalBuildProviderCall {
            operation,
            boundary,
            source_span: authored.source_span(),
            selecting_machine,
            mode_cases,
        });
    }
    Ok(calls)
}
