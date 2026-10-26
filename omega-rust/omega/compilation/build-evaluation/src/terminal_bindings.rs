//! Build binding effects over ordinary Terminal imports.
//!
//! Source production supplies designated-operand custody beside the artifact.
//! An activation authenticates its original receiver before evaluating a
//! delegated description; commit consumes the resulting private ticket. Neither
//! an integer nor a matching debug name grants selection authority. Records
//! remain inert until the product owner resolves and admits them.

use language_semantics::declaration_selection::{
    BuildOperation, BuildSelectionRequest, CompositionMode, ExecutedBuildSelection,
    ExecutedProductSelection, ExecutedRootBinding, ProductDeclarationRequest, RootBindingOperand,
    RootBindingSelection,
};
use psi_compiler::{
    TerminalBuildBindingCall, TerminalBuildBindingOperation, TerminalBuildBindings,
};
use std::sync::Mutex;
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalEffectResult,
    TerminalImportAdapter, TerminalImportSignature, TerminalScalarValue, TerminalStructuralValue,
};

/// One activation shared by its prepare/commit imports, never across builds.
/// The interpreter owns each adapter; the host retains this common state and
/// takes completed requests after dropping those adapters. A lock makes that
/// sharing explicit without granting either adapter mutable aliasing.
pub struct TerminalBuildBindingHost<'context> {
    calls: &'context [TerminalBuildBindingCall],
    declarations: &'context [std::sync::Arc<ProductDeclarationRequest>],
    original: TerminalStructuralValue,
    state: Mutex<BindingState>,
}

#[derive(Default)]
struct BindingState {
    pending: Vec<Option<BuildSelectionRequest>>,
    executed: Vec<ExecutedBuildSelection>,
}

impl BindingState {
    fn take_executed(&mut self) -> Result<Vec<ExecutedBuildSelection>, String> {
        if self.pending.iter().any(Option::is_some) {
            return Err("Build binding activation ended with an uncommitted request".to_owned());
        }
        Ok(std::mem::take(&mut self.executed))
    }
}

impl<'context> TerminalBuildBindingHost<'context> {
    pub fn new(
        artifact: terminal_codec::TerminalExecutionView<'_>,
        bindings: &'context TerminalBuildBindings,
        original: TerminalStructuralValue,
    ) -> Result<Self, String> {
        let calls = bindings
            .calls_for(artifact)
            .ok_or_else(|| "Build bindings belong to another Terminal artifact".to_owned())?;
        let declarations = bindings.declarations_for(artifact).ok_or_else(|| {
            "Build declaration formation belongs to another Terminal artifact".to_owned()
        })?;
        Ok(Self {
            calls,
            declarations,
            original,
            state: Mutex::new(BindingState::default()),
        })
    }

    pub(crate) fn declared(&self) -> &[std::sync::Arc<ProductDeclarationRequest>] {
        self.declarations
    }

    pub fn adapter(
        &self,
        signature: TerminalImportSignature,
    ) -> TerminalBuildBindingAdapter<'_, 'context> {
        TerminalBuildBindingAdapter {
            host: self,
            signature,
        }
    }

    pub fn into_executed(self) -> Result<Vec<ExecutedBuildSelection>, String> {
        self.state
            .into_inner()
            .map_err(|_| "Build binding activation lock was poisoned".to_owned())?
            .take_executed()
    }

    /// Retire committed selections after the sole activation has returned.
    /// The activation owner prevents a second execution with these adapters.
    pub(crate) fn take_executed(&self) -> Result<Vec<ExecutedBuildSelection>, String> {
        self.state
            .lock()
            .map_err(|_| "Build binding activation lock was poisoned".to_owned())?
            .take_executed()
    }
}

pub struct TerminalBuildBindingAdapter<'host, 'context> {
    host: &'host TerminalBuildBindingHost<'context>,
    signature: TerminalImportSignature,
}

impl TerminalImportAdapter for TerminalBuildBindingAdapter<'_, '_> {
    fn implemented_signature(&self) -> TerminalImportSignature {
        self.signature.clone()
    }
}

impl TerminalEffectHandler for TerminalBuildBindingAdapter<'_, '_> {
    fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        Err(TerminalEffectRejection::new(
            "Build bindings require result-aware dispatch",
        ))
    }

    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
        let refuse = TerminalEffectRejection::new;
        let TerminalEffect::BoundaryCall {
            operation,
            boundary,
            arguments,
            structural_arguments,
            ..
        } = effect
        else {
            return Err(refuse("not a Build binding effect"));
        };
        let call = self
            .host
            .calls
            .iter()
            .find(|call| call.operation == *operation && call.boundary == *boundary)
            .ok_or_else(|| refuse("Build binding call has no source-issued designated operands"))?;
        let mut state = self
            .host
            .state
            .lock()
            .map_err(|_| refuse("Build binding activation lock was poisoned"))?;
        match call.action {
            TerminalBuildBindingOperation::Prepare => {
                if !arguments.is_empty()
                    || structural_arguments.as_slice() != std::slice::from_ref(&self.host.original)
                {
                    return Err(refuse(
                        "root binding requires the original compiler-issued Build receiver",
                    ));
                }
                let ticket = u64::try_from(state.pending.len())
                    .map_err(|_| refuse("Build binding ticket storage is exhausted"))?;
                state
                    .pending
                    .try_reserve(1)
                    .map_err(|_| refuse("cannot retain Build binding request"))?;
                state.pending.push(Some(call.request.clone()));
                Ok(TerminalEffectResult::Scalar(TerminalScalarValue::Integer {
                    scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).map_err(|error| {
                        TerminalEffectRejection::new(format!(
                            "invalid Build ticket scalar type: {error:?}"
                        ))
                    })?,
                    value: IntegerValue::Unsigned(u128::from(ticket)),
                }))
            }
            TerminalBuildBindingOperation::Bind => {
                let [
                    TerminalScalarValue::Integer {
                        value: IntegerValue::Unsigned(ticket),
                        ..
                    },
                ] = arguments.as_slice()
                else {
                    return Err(refuse("root binding has no activation ticket"));
                };
                let ticket = usize::try_from(*ticket)
                    .map_err(|_| refuse("root binding ticket is outside its activation"))?;
                if state.pending.get(ticket).and_then(Option::as_ref) != Some(&call.request) {
                    return Err(refuse(
                        "root binding ticket was consumed or belongs to another request",
                    ));
                }
                let BuildSelectionRequest::Entry(binding) = &call.request else {
                    return Err(refuse(
                        "declaration selection requires its admitted commit operation",
                    ));
                };
                let RootBindingOperand::AuthoredPath(path) = &binding.implementation else {
                    return Err(refuse(
                        "Terminal Build descriptions require an issued product-query value",
                    ));
                };
                if !structural_arguments.is_empty() {
                    return Err(refuse(
                        "authored root binding has unexpected structural operands",
                    ));
                }
                state
                    .executed
                    .try_reserve(1)
                    .map_err(|_| refuse("cannot retain executed Build binding"))?;
                state.pending[ticket] = None;
                state
                    .executed
                    .push(ExecutedBuildSelection::Entry(ExecutedRootBinding {
                        source_span: binding.source_span,
                        slot: binding.slot.clone(),
                        implementation: RootBindingSelection::AuthoredPath(path.clone()),
                    }));
                Ok(TerminalEffectResult::Unit)
            }
            TerminalBuildBindingOperation::Select | TerminalBuildBindingOperation::SelectMode => {
                if !structural_arguments.is_empty() {
                    return Err(refuse(
                        "Product selection has unexpected structural operands",
                    ));
                }
                let (ticket, independent) = match (call.action, arguments.as_slice()) {
                    (
                        TerminalBuildBindingOperation::Select,
                        [
                            TerminalScalarValue::Integer {
                                value: IntegerValue::Unsigned(ticket),
                                ..
                            },
                        ],
                    ) => (*ticket, None),
                    (
                        TerminalBuildBindingOperation::SelectMode,
                        [
                            TerminalScalarValue::Integer {
                                value: IntegerValue::Unsigned(ticket),
                                ..
                            },
                            TerminalScalarValue::Boolean(independent),
                        ],
                    ) => (*ticket, Some(*independent)),
                    _ => {
                        return Err(refuse(
                            "Product selection has no exact ticket and checked composition value",
                        ));
                    }
                };
                let ticket = usize::try_from(ticket)
                    .map_err(|_| refuse("Product selection ticket is outside its activation"))?;
                if state.pending.get(ticket).and_then(Option::as_ref) != Some(&call.request) {
                    return Err(refuse(
                        "Product selection ticket was consumed or belongs to another request",
                    ));
                }
                let BuildSelectionRequest::Declaration(request) = &call.request else {
                    return Err(refuse(
                        "Product selection commit has no issued declaration request",
                    ));
                };
                let composition = match request.operation {
                    BuildOperation::ProviderSelection => Some(if independent.unwrap_or(false) {
                        CompositionMode::Independent
                    } else {
                        CompositionMode::Fused
                    }),
                    BuildOperation::RepresentationSelection if independent.is_none() => None,
                    _ => {
                        return Err(refuse(
                            "Product selection operation disagrees with its runtime operands",
                        ));
                    }
                };
                state
                    .executed
                    .try_reserve(1)
                    .map_err(|_| refuse("cannot retain executed Product selection"))?;
                state.pending[ticket] = None;
                state.executed.push(ExecutedBuildSelection::Declaration(
                    ExecutedProductSelection {
                        request: request.clone(),
                        composition,
                    },
                ));
                Ok(TerminalEffectResult::Unit)
            }
        }
    }
}
