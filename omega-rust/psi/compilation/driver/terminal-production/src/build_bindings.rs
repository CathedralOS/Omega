//! Admitted designated operands beside their exact emitted calls.
//!
//! Publication erases source joins. Capture them here, while the checked source
//! and optimized operations share custody; consumers never parse debug names or
//! reopen trees. Portable artifact bytes alone do not grant Build authority.

use language_semantics::declaration_selection::{BuildSelectionRequest, ProductDeclarationRequest};
use semantic_vocabulary::OperationId;

/// Exact checked application and demanded static slots in this code owner.
/// This is source-issued companion context, not authority derived from bytes.
#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct TerminalBuildProviderRequirement {
    pub declaration: terminal_psi::NominalDeclarationIdentity,
    pub application: terminal_psi::TerminalIdentityIndex,
    pub boundaries: Vec<semantic_vocabulary::BoundaryMachineId>,
}

pub type TerminalBuildSelectionRequest = BuildSelectionRequest<TerminalBuildProviderRequirement>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub enum TerminalBuildBindingOperation {
    AwaitProvider,
    BindNative,
    BindTableFunction,
    BindVtableField,
    SameStackDemand,
    Prepare,
    Bind,
    Select,
    SelectProvider,
    Exclude,
}

#[derive(Debug, PartialEq, Eq, compiler_storage::Record)]
pub struct TerminalBuildBindingCall {
    pub operation: OperationId,
    pub boundary: semantic_vocabulary::BoundaryMachineId,
    pub action: TerminalBuildBindingOperation,
    pub request: TerminalBuildSelectionRequest,
    pub selecting_machine: Option<source::SourceSpan>,
    pub locator_widths: Option<[u64; 3]>,
    pub static_environment: Option<
        std::sync::Arc<
            language_semantics::declaration_selection::product_operands::ProductStaticEnvironment,
        >,
    >,
}

/// Source-issued context bound to exact Terminal semantics and, for private
/// code, its live invocation owner. Loading bytes cannot recreate this context.
#[derive(Debug, PartialEq, Eq)]
pub struct TerminalBuildBindings {
    identity: terminal_codec::TerminalExecutionIdentity,
    calls: std::sync::Arc<[TerminalBuildBindingCall]>,
    queries: std::sync::Arc<[crate::TerminalBuildQueryCall]>,
    declarations: std::sync::Arc<[std::sync::Arc<ProductDeclarationRequest>]>,
}

impl TerminalBuildBindings {
    pub fn queries_for(
        &self,
        artifact: terminal_codec::TerminalExecutionView<'_>,
    ) -> Option<&std::sync::Arc<[crate::TerminalBuildQueryCall]>> {
        (self.identity == artifact.custody_identity()).then_some(&self.queries)
    }
    pub fn calls_for(
        &self,
        artifact: terminal_codec::TerminalExecutionView<'_>,
    ) -> Option<&std::sync::Arc<[TerminalBuildBindingCall]>> {
        (self.identity == artifact.custody_identity()).then_some(&self.calls)
    }

    pub fn declarations_for(
        &self,
        artifact: terminal_codec::TerminalExecutionView<'_>,
    ) -> Option<&std::sync::Arc<[std::sync::Arc<ProductDeclarationRequest>]>> {
        (self.identity == artifact.custody_identity()).then_some(&self.declarations)
    }

    pub(super) fn new(
        artifact: terminal_codec::TerminalExecutionView<'_>,
        calls: impl Into<std::sync::Arc<[TerminalBuildBindingCall]>>,
        declarations: impl Into<std::sync::Arc<[std::sync::Arc<ProductDeclarationRequest>]>>,
        queries: impl Into<std::sync::Arc<[crate::TerminalBuildQueryCall]>>,
    ) -> Self {
        Self {
            identity: artifact.custody_identity(),
            calls: calls.into(),
            queries: queries.into(),
            declarations: declarations.into(),
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
        if !call.build_selection.is_valid() {
            continue;
        }
        let request = checked
            .expression_table
            .build_selection_request(call.build_selection)
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
            "await_provider" => TerminalBuildBindingOperation::AwaitProvider,
            "prepare" => TerminalBuildBindingOperation::Prepare,
            "bind_native" => TerminalBuildBindingOperation::BindNative,
            "bind_table_function" => TerminalBuildBindingOperation::BindTableFunction,
            "bind_vtable_field" => TerminalBuildBindingOperation::BindVtableField,
            "same_stack_demand" => TerminalBuildBindingOperation::SameStackDemand,
            "bind" | "bind_description" => TerminalBuildBindingOperation::Bind,
            "select" => TerminalBuildBindingOperation::Select,
            "select_provider" => TerminalBuildBindingOperation::SelectProvider,
            "exclude" => TerminalBuildBindingOperation::Exclude,
            _ => {
                return Err(diagnostics::Diagnostic::error(
                    "checked Build binding has no admitted operation",
                ));
            }
        };
        let boundary = emitted_boundary(lowered, operation)?;
        let selecting_machine = if request.requires_lexical_subject() {
            Some(
                checked
                    .machines()
                    .iter()
                    .find(|machine| {
                        checked
                            .machine_states(machine)
                            .iter()
                            .any(|state| state.symbol == occurrence.source_state)
                    })
                    .and_then(|machine| {
                        checked
                            .symbols
                            .symbol_source_span(checked.symbols.authored_origin(machine.symbol))
                    })
                    .ok_or_else(|| {
                        diagnostics::Diagnostic::error(
                            "Build selection lost its authored machine coordinate",
                        )
                    })?,
            )
        } else {
            None
        };
        let locator_widths = if action == TerminalBuildBindingOperation::BindNative {
            let arguments = checked.expression_table.expression_handles(call.arguments);
            let locator = arguments.get(2).ok_or_else(|| {
                diagnostics::Diagnostic::error("native binding lost its locator actual")
            })?;
            let reference = validation::expression_type_reference_in_state(
                checked,
                occurrence.source_state,
                occurrence.statement_index,
                *locator,
            )
            .ok_or_else(|| {
                diagnostics::Diagnostic::error("native binding lost its closed locator type")
            })?;
            Some(
                validation::compiler_operations::native_binding_widths(&checked.typed, reference)
                    .map_err(|errors| {
                    errors.into_iter().next().unwrap_or_else(|| {
                        diagnostics::Diagnostic::error("invalid native locator schema")
                    })
                })?,
            )
        } else {
            None
        };
        let static_environment = checked
            .machines()
            .iter()
            .find(|machine| {
                checked
                    .machine_states(machine)
                    .iter()
                    .any(|state| state.symbol == occurrence.source_state)
            })
            .map(|machine| {
                typed_trees::product_operand::capture_static_environment(
                    &checked.typed,
                    machine.symbol,
                )
            })
            .transpose()
            .map_err(diagnostics::Diagnostic::error)?
            .flatten();
        calls.push(TerminalBuildBindingCall {
            operation,
            boundary,
            action,
            request: request.clone().map_requirement(|_| {
                let selected = lowered
                    .build_provider_requirements
                    .iter()
                    .find(|selected| {
                        selected.selection == call.build_selection
                            && selected.source_state == occurrence.source_state
                    })
                    .ok_or_else(|| {
                        diagnostics::Diagnostic::error(
                            "Build readiness lost its emitted requirement application",
                        )
                    })?;
                Ok::<_, diagnostics::Diagnostic>(TerminalBuildProviderRequirement {
                    declaration: selected.declaration.clone(),
                    application: selected.application,
                    boundaries: selected.boundaries.clone(),
                })
            })?,
            selecting_machine,
            locator_widths,
            static_environment,
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

/// Formation sees every checked source request, including choices in helpers
/// and branches that optimization or execution never reaches. Copies of one
/// authored occurrence do not create another formation obligation.
pub(super) fn capture_declarations(
    checked: &checked_trees::CheckedTrees,
) -> Vec<std::sync::Arc<ProductDeclarationRequest>> {
    let mut declarations = Vec::new();
    for (_, node) in checked.expression_table.iter_expressions() {
        let typed_trees::expression::ExpressionNode::Call(call) = node else {
            continue;
        };
        let Some(BuildSelectionRequest::Declaration(request)) = checked
            .expression_table
            .build_selection_request(call.build_selection)
        else {
            continue;
        };
        if !declarations.contains(request) {
            declarations.push(request.clone());
        }
    }
    declarations
}
