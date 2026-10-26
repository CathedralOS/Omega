//! Admitted designated operands beside their exact emitted calls.
//!
//! Publication erases source joins. Capture them here, while the checked source
//! and optimized operations share custody; consumers never parse debug names or
//! reopen trees. Portable artifact bytes alone do not grant Build authority.

use language_semantics::declaration_selection::BuildProductRequest;
use semantic_vocabulary::OperationId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalBuildBindingOperation {
    Prepare,
    Bind,
    Select,
    SelectMode,
}

#[derive(Debug, PartialEq, Eq)]
pub struct TerminalBuildBindingCall {
    pub operation: OperationId,
    pub boundary: semantic_vocabulary::BoundaryMachineId,
    pub action: TerminalBuildBindingOperation,
    pub request: BuildProductRequest,
    pub author: source::SourceSpan,
    pub composition: Option<TerminalBuildCompositionCases>,
}

/// The checked Prelude sum joined to the exact emitted formal and case IDs.
/// Case spelling is used only at production, beside the nominal declaration;
/// host execution reads the interpreter's actual semantic discriminator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalBuildCompositionCases {
    pub structural_type: semantic_vocabulary::StructuralTypeId,
    pub fused: semantic_vocabulary::StructuralCaseId,
    pub independent: semantic_vocabulary::StructuralCaseId,
}

/// Source-issued context bound to exact Terminal semantics and, for private
/// code, its live invocation owner. Loading bytes cannot recreate this context.
#[derive(Debug, PartialEq, Eq)]
pub struct TerminalBuildBindings {
    identity: terminal_codec::TerminalExecutionIdentity,
    calls: Vec<TerminalBuildBindingCall>,
}

impl TerminalBuildBindings {
    pub fn calls_for(
        &self,
        artifact: terminal_codec::TerminalExecutionView<'_>,
    ) -> Option<&[TerminalBuildBindingCall]> {
        (self.identity == artifact.custody_identity()).then_some(&self.calls)
    }

    pub(super) fn new(
        artifact: terminal_codec::TerminalExecutionView<'_>,
        calls: Vec<TerminalBuildBindingCall>,
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
) -> Result<Vec<TerminalBuildBindingCall>, diagnostics::Diagnostic> {
    let mut calls = Vec::new();
    for occurrence in &lowered.source_call_occurrences {
        let Some(checked_trees::NominalMachineUseSite::Expression(expression)) =
            occurrence.source_site
        else {
            continue;
        };
        let typed_trees::expression::ExpressionNode::Call(call) =
            checked.expression_table.expression(expression)
        else {
            continue;
        };
        if !call.build_product_request.is_valid() {
            continue;
        }
        let request = checked
            .expression_table
            .build_product_request(call.build_product_request)
            .ok_or_else(|| {
                diagnostics::Diagnostic::error(
                    "Build call lost its designated operands before publication",
                )
            })?;
        let semantic_vocabulary::InvocationSite::Operation(operation) = occurrence.terminal_site
        else {
            return Err(diagnostics::Diagnostic::error(
                "Build binding lost its ordinary result-producing call",
            ));
        };
        let action = match call.target.as_str() {
            "prepare" => TerminalBuildBindingOperation::Prepare,
            "bind" | "bind_description" => TerminalBuildBindingOperation::Bind,
            "select" => TerminalBuildBindingOperation::Select,
            "select_mode" => TerminalBuildBindingOperation::SelectMode,
            _ => {
                return Err(diagnostics::Diagnostic::error(
                    "checked Build binding has no admitted operation",
                ));
            }
        };
        let boundary = emitted_boundary(lowered, operation)?;
        let author = checked
            .machine_holding_state(occurrence.source_state)
            .and_then(|machine| {
                checked
                    .symbols
                    .symbol_provenance_source_span(machine.symbol)
            })
            .ok_or_else(|| {
                diagnostics::Diagnostic::error("Build request lost its authored machine coordinate")
            })?;
        let composition = if action == TerminalBuildBindingOperation::SelectMode {
            Some(composition_cases(checked, lowered, boundary)?)
        } else {
            None
        };
        calls.push(TerminalBuildBindingCall {
            operation,
            boundary,
            action,
            request: request.clone(),
            author,
            composition,
        });
    }
    Ok(calls)
}

pub(super) fn emitted_boundary(
    lowered: &lowered_psi::LoweredPsi,
    operation: OperationId,
) -> Result<semantic_vocabulary::BoundaryMachineId, diagnostics::Diagnostic> {
    let emitted = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .find(|emitted| emitted.id == operation)
        .ok_or_else(|| diagnostics::Diagnostic::error("Build operation lost its emitted call"))?;
    let terminal_psi::OperationKind::BoundaryCall { boundary, .. } = &emitted.kind else {
        return Err(diagnostics::Diagnostic::error(
            "Build operation no longer names a boundary call",
        ));
    };
    Ok(*boundary)
}

fn composition_cases(
    checked: &checked_trees::CheckedTrees,
    lowered: &lowered_psi::LoweredPsi,
    boundary: semantic_vocabulary::BoundaryMachineId,
) -> Result<TerminalBuildCompositionCases, diagnostics::Diagnostic> {
    let refuse = || {
        diagnostics::Diagnostic::error("Build composition mode lost its exact checked Prelude sum")
    };
    let mut declarations = checked.data_definitions().iter().filter(|definition| {
        definition.name.as_str() == "CompositionMode"
            && checked.symbols.is_build_prelude_symbol(definition.symbol)
    });
    let declaration = declarations.next().ok_or_else(refuse)?;
    if declarations.next().is_some() {
        return Err(refuse());
    }
    let variants = checked
        .data_members(declaration)
        .iter()
        .filter_map(|member| {
            if let typed_trees::data::DataMember::Variant(variant) = member {
                Some(variant)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    if variants.len() != 2
        || variants
            .iter()
            .any(|variant| !checked.data_payload_fields(variant).is_empty())
    {
        return Err(refuse());
    }
    let fused = variants
        .iter()
        .find(|variant| variant.name.as_str() == "Fused")
        .ok_or_else(refuse)?
        .path_identity();
    let independent = variants
        .iter()
        .find(|variant| variant.name.as_str() == "Independent")
        .ok_or_else(refuse)?
        .path_identity();
    let module = &lowered.semantic_module;
    let parameters = &module
        .boundary_machines
        .iter()
        .find(|declaration| declaration.id == boundary)
        .ok_or_else(refuse)?
        .structural_parameters;
    let [parameter] = parameters.as_slice() else {
        return Err(refuse());
    };
    if parameter.access != terminal_psi::StructuralAccess::Owned {
        return Err(refuse());
    }
    let definition = module
        .structural_types
        .iter()
        .find(|definition| definition.id == parameter.structural_type)
        .ok_or_else(refuse)?;
    let terminal_psi::StructuralTypeShape::Sum { cases } = &definition.shape else {
        return Err(refuse());
    };
    if cases.len() != 2 || cases.iter().any(|case| !case.fields.is_empty()) {
        return Err(refuse());
    }
    let case = |identity: &str| {
        cases
            .iter()
            .find(|case| module.identities.text(case.identity) == Some(identity))
            .map(|case| case.id)
            .ok_or_else(refuse)
    };
    Ok(TerminalBuildCompositionCases {
        structural_type: parameter.structural_type,
        fused: case(&fused)?,
        independent: case(&independent)?,
    })
}
