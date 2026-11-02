//! Admitted designated operands beside their exact emitted calls.
//!
//! Publication erases source joins. Capture them here, while the checked source
//! and optimized operations share custody; consumers never parse debug names or
//! reopen trees. Portable artifact bytes alone do not grant Build authority.

mod native_formations;
mod native_locator;
pub(super) use native_formations::capture as capture_native_formations;
pub use native_locator::{
    TerminalBuildNativeLocator, TerminalBuildValueCase, TerminalBuildValueField,
    TerminalBuildValueShape,
};

use crate::TerminalBuildQueryCall;
use checked_trees::{CheckedTrees, NominalMachineUseSite};
use diagnostics::Diagnostic;
use language_semantics::declaration_selection::product_operands::ProductStaticEnvironment;
use language_semantics::declaration_selection::{
    BuildSelectionRequest, NativeBindingFormation, ProductDeclarationRequest,
};
use semantic_vocabulary::{BoundaryMachineId, InvocationSite, OperationId};
use std::sync::Arc;
use terminal_codec::TerminalExecutionView;
use typed_trees::expression::ExpressionNode;
use typed_trees::product_operand::capture_static_environment;

/// Exact checked application and demanded static slots in this code owner.
/// This is source-issued companion context, not authority derived from bytes.
#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct TerminalBuildProviderRequirement {
    pub declaration: terminal_psi::NominalDeclarationIdentity,
    pub application: terminal_psi::TerminalIdentityIndex,
    pub boundaries: Vec<BoundaryMachineId>,
}

pub type TerminalBuildSelectionRequest = BuildSelectionRequest<TerminalBuildProviderRequirement>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, compiler_storage::Record)]
pub enum TerminalBuildBindingOperation {
    AwaitProvider,
    Prepare,
    Bind,
    BindNative,
    Select,
    SelectProvider,
    Exclude,
}

#[derive(Debug, PartialEq, Eq, compiler_storage::Record)]
pub struct TerminalBuildBindingCall {
    pub operation: OperationId,
    pub boundary: BoundaryMachineId,
    pub action: TerminalBuildBindingOperation,
    pub request: TerminalBuildSelectionRequest,
    pub selecting_machine: Option<source::SourceSpan>,
    pub static_environment: Option<Arc<ProductStaticEnvironment>>,
    pub native_locator: Option<TerminalBuildNativeLocator>,
}

/// Source-issued context bound to exact Terminal semantics and, for private
/// code, its live invocation owner. Loading bytes cannot recreate this context.
#[derive(Debug, PartialEq, Eq)]
pub struct TerminalBuildBindings {
    identity: terminal_codec::TerminalExecutionIdentity,
    calls: Arc<[TerminalBuildBindingCall]>,
    queries: Arc<[TerminalBuildQueryCall]>,
    declarations: Arc<[Arc<ProductDeclarationRequest>]>,
    native_formations: Arc<[NativeBindingFormation]>,
}

impl TerminalBuildBindings {
    pub fn queries_for(
        &self,
        artifact: TerminalExecutionView<'_>,
    ) -> Option<&Arc<[TerminalBuildQueryCall]>> {
        (self.identity == artifact.custody_identity()).then_some(&self.queries)
    }
    pub fn calls_for(
        &self,
        artifact: TerminalExecutionView<'_>,
    ) -> Option<&Arc<[TerminalBuildBindingCall]>> {
        (self.identity == artifact.custody_identity()).then_some(&self.calls)
    }

    pub fn declarations_for(
        &self,
        artifact: TerminalExecutionView<'_>,
    ) -> Option<&Arc<[Arc<ProductDeclarationRequest>]>> {
        (self.identity == artifact.custody_identity()).then_some(&self.declarations)
    }

    pub fn native_formations_for(
        &self,
        artifact: TerminalExecutionView<'_>,
    ) -> Option<&Arc<[NativeBindingFormation]>> {
        (self.identity == artifact.custody_identity()).then_some(&self.native_formations)
    }

    pub(super) fn new(
        artifact: TerminalExecutionView<'_>,
        calls: impl Into<Arc<[TerminalBuildBindingCall]>>,
        declarations: impl Into<Arc<[Arc<ProductDeclarationRequest>]>>,
        queries: impl Into<Arc<[TerminalBuildQueryCall]>>,
        native_formations: impl Into<Arc<[NativeBindingFormation]>>,
    ) -> Self {
        Self {
            identity: artifact.custody_identity(),
            calls: calls.into(),
            queries: queries.into(),
            declarations: declarations.into(),
            native_formations: native_formations.into(),
        }
    }
}

pub(super) fn capture(
    checked: &CheckedTrees,
    lowered: &lowered_psi::LoweredPsi,
) -> Result<Vec<TerminalBuildBindingCall>, Diagnostic> {
    let mut calls = Vec::new();
    let mut native_shapes = std::collections::BTreeMap::new();
    for occurrence in &lowered.source_call_occurrences {
        let Some(NominalMachineUseSite::Expression(expression)) = occurrence.source_site else {
            continue;
        };
        let ExpressionNode::Call(call) = checked.expression_table.expression(expression) else {
            continue;
        };
        if !call.build_selection.is_valid() {
            continue;
        }
        let coordinate = occurrence
            .origin
            .statement()
            .ok_or_else(|| Diagnostic::error("Build binding has no checked statement custody"))?;
        let request = checked
            .expression_table
            .build_selection_request(call.build_selection)
            .ok_or_else(|| {
                Diagnostic::error("Build call lost its designated operands before publication")
            })?;
        let InvocationSite::Operation(operation) = occurrence.terminal_site else {
            return Err(Diagnostic::error(
                "Build binding lost its ordinary result-producing call",
            ));
        };
        let action = match call.target.as_str() {
            "await_provider" => TerminalBuildBindingOperation::AwaitProvider,
            "prepare" => TerminalBuildBindingOperation::Prepare,
            "bind" | "bind_description" => TerminalBuildBindingOperation::Bind,
            "bind_native" => TerminalBuildBindingOperation::BindNative,
            "select" => TerminalBuildBindingOperation::Select,
            "select_provider" => TerminalBuildBindingOperation::SelectProvider,
            "exclude" => TerminalBuildBindingOperation::Exclude,
            _ => {
                return Err(Diagnostic::error(
                    "checked Build binding has no admitted operation",
                ));
            }
        };
        let boundary = emitted_boundary(lowered, operation)?;
        let selecting_machine = if request.requires_lexical_subject() {
            Some(
                checked
                    .machine_holding_state(coordinate.state)
                    .and_then(|machine| {
                        checked
                            .symbols
                            .symbol_source_span(checked.symbols.authored_origin(machine.symbol))
                    })
                    .ok_or_else(|| {
                        Diagnostic::error("Build selection lost its authored machine coordinate")
                    })?,
            )
        } else {
            None
        };
        let native_locator = if action == TerminalBuildBindingOperation::BindNative {
            Some(native_locator::capture(
                checked,
                lowered,
                coordinate.state,
                coordinate.statement_index,
                call,
                boundary,
                &mut native_shapes,
            )?)
        } else {
            None
        };
        let static_environment = if request
            .designated_operands()
            .iter()
            .any(|operand| operand.application.is_some())
        {
            checked
                .machine_holding_state(coordinate.state)
                .map(|machine| capture_static_environment(&checked.typed, machine.symbol))
                .transpose()
                .map_err(Diagnostic::error)?
                .flatten()
        } else {
            None
        };
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
                            && selected.source_state == coordinate.state
                    })
                    .ok_or_else(|| {
                        Diagnostic::error(
                            "Build readiness lost its emitted requirement application",
                        )
                    })?;
                Ok::<_, Diagnostic>(TerminalBuildProviderRequirement {
                    declaration: selected.declaration.clone(),
                    application: selected.application,
                    boundaries: selected.boundaries.clone(),
                })
            })?,
            selecting_machine,
            static_environment,
            native_locator,
        });
    }
    Ok(calls)
}

pub(super) fn emitted_boundary(
    lowered: &lowered_psi::LoweredPsi,
    operation: OperationId,
) -> Result<BoundaryMachineId, Diagnostic> {
    let emitted = lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| &machine.blocks)
        .flat_map(|block| &block.operations)
        .find(|emitted| emitted.id == operation)
        .ok_or_else(|| Diagnostic::error("Build operation lost its emitted call"))?;
    let terminal_psi::OperationKind::BoundaryCall { boundary, .. } = &emitted.kind else {
        return Err(Diagnostic::error(
            "Build operation no longer names a boundary call",
        ));
    };
    Ok(*boundary)
}

/// Formation sees every checked source request, including choices in helpers
/// and branches that optimization or execution never reaches. Copies of one
/// authored occurrence do not create another formation obligation.
pub(super) fn capture_declarations(checked: &CheckedTrees) -> Vec<Arc<ProductDeclarationRequest>> {
    let mut declarations = Vec::new();
    for (_, node) in checked.expression_table.iter_expressions() {
        let ExpressionNode::Call(call) = node else {
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
