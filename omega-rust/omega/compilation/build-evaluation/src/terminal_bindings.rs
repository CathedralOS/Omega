//! Build binding effects over ordinary Terminal imports.
//!
//! Source production supplies designated-operand custody beside the artifact.
//! An activation authenticates its original receiver before evaluating a
//! delegated description; commit consumes the resulting private ticket. Neither
//! an integer nor a matching debug name grants selection authority. Records
//! remain inert until the product owner resolves and admits them.

use language_semantics::declaration_selection::{
    BuildProductRequest, ExecutedBuildBehaviorExclusion, ExecutedBuildProductRequest,
    ExecutedProductDeclarationSelection, ExecutedRootBinding, ProviderCompositionMode,
    RootBindingOperand, RootBindingSelection,
};
use psi_compiler::{
    TerminalBuildBindingCall, TerminalBuildBindingOperation, TerminalBuildBindings,
};
use std::sync::Mutex;
use terminal_interpreter::{
    TerminalBoundaryByteBuffer, TerminalBoundaryStorageBuffer, TerminalEffect,
    TerminalEffectHandler, TerminalEffectRejection, TerminalEffectResult, TerminalImportAdapter,
    TerminalImportSignature, TerminalScalarValue, TerminalStorageContents, TerminalStructuralValue,
};

/// One activation shared by its prepare/commit imports, never across builds.
/// The interpreter owns each adapter; the host retains this common state and
/// takes completed requests after dropping those adapters. A lock makes that
/// sharing explicit without granting either adapter mutable aliasing.
pub struct TerminalBuildBindingHost<'context> {
    calls: &'context [TerminalBuildBindingCall],
    original: TerminalStructuralValue,
    state: Mutex<BindingState>,
}

#[derive(Default)]
struct BindingState {
    pending: Vec<Option<BuildProductRequest>>,
    executed: Vec<ExecutedBuildProductRequest>,
}

impl BindingState {
    fn take_executed(&mut self) -> Result<Vec<ExecutedBuildProductRequest>, String> {
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
        Ok(Self {
            calls,
            original,
            state: Mutex::new(BindingState::default()),
        })
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

    pub fn into_executed(self) -> Result<Vec<ExecutedBuildProductRequest>, String> {
        self.state
            .into_inner()
            .map_err(|_| "Build binding activation lock was poisoned".to_owned())?
            .take_executed()
    }

    /// Retire committed selections after the sole activation has returned.
    /// The activation owner prevents a second execution with these adapters.
    pub(crate) fn take_executed(&self) -> Result<Vec<ExecutedBuildProductRequest>, String> {
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

    fn storage_arguments(&self, boundary: semantic_vocabulary::BoundaryMachineId) -> &[usize] {
        if self.host.calls.iter().any(|call| {
            call.boundary == boundary && call.action == TerminalBuildBindingOperation::SelectMode
        }) {
            &[0]
        } else if self.host.calls.iter().any(|call| {
            call.boundary == boundary
                && call.action == TerminalBuildBindingOperation::ExcludeBehavior
        }) {
            &[1]
        } else {
            &[]
        }
    }

    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        self.dispatch(effect, &[])
    }

    fn handle_effect_with_buffers(
        &mut self,
        effect: &TerminalEffect,
        byte_buffers: &mut [TerminalBoundaryByteBuffer],
        storage_buffers: &mut [TerminalBoundaryStorageBuffer],
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        if !byte_buffers.is_empty() {
            return Err(TerminalEffectRejection::new(
                "Build product recording has unexpected byte operands",
            ));
        }
        self.dispatch(effect, storage_buffers)
    }
}

impl TerminalBuildBindingAdapter<'_, '_> {
    fn dispatch(
        &self,
        effect: &TerminalEffect,
        storage_buffers: &[TerminalBoundaryStorageBuffer],
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
            TerminalBuildBindingOperation::ExcludeBehavior => {
                let BuildProductRequest::BehaviorExclusion(request) = &call.request else {
                    return Err(refuse(
                        "Build exclusion has no authenticated source request",
                    ));
                };
                let [original, _] = structural_arguments.as_slice() else {
                    return Err(refuse(
                        "Build exclusion requires its original Build and evaluated case",
                    ));
                };
                if original != &self.host.original || !arguments.is_empty() {
                    return Err(refuse(
                        "Build exclusion requires the original compiler-issued Build receiver",
                    ));
                }
                let expected = call
                    .exclusion
                    .as_ref()
                    .ok_or_else(|| refuse("Build exclusion lost its checked case roster"))?;
                let [TerminalBoundaryStorageBuffer::Structural(buffer)] = storage_buffers else {
                    return Err(refuse(
                        "Build exclusion requires its actual semantic case contents",
                    ));
                };
                if buffer.argument_index() != 1
                    || buffer.access() != terminal_psi::StructuralAccess::Owned
                    || buffer.structural_type() != expected.structural_type
                {
                    return Err(refuse(
                        "Build exclusion observation differs from its checked formal",
                    ));
                }
                let [value] = buffer.values() else {
                    return Err(refuse(
                        "Build exclusion case has unexpected payload contents",
                    ));
                };
                if !value.path.is_empty() {
                    return Err(refuse("Build exclusion case has no root discriminator"));
                }
                let TerminalStorageContents::Case(case) = value.contents else {
                    return Err(refuse("Build exclusion value is not a semantic case"));
                };
                let mut cases = expected.cases.iter().filter(|(id, _)| *id == case);
                let (_, source_case) = cases.next().ok_or_else(|| {
                    refuse("Build exclusion value is outside its checked case roster")
                })?;
                if cases.next().is_some() {
                    return Err(refuse("Build exclusion has ambiguous case custody"));
                }
                state
                    .executed
                    .try_reserve(1)
                    .map_err(|_| refuse("cannot retain executed Build exclusion"))?;
                state
                    .executed
                    .push(ExecutedBuildProductRequest::BehaviorExclusion(
                        ExecutedBuildBehaviorExclusion {
                            request: request.clone(),
                            author: call.author,
                            case: *source_case,
                        },
                    ));
                Ok(TerminalEffectResult::Unit)
            }
            TerminalBuildBindingOperation::Prepare => {
                if !storage_buffers.is_empty()
                    || !arguments.is_empty()
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
            TerminalBuildBindingOperation::Bind
            | TerminalBuildBindingOperation::Select
            | TerminalBuildBindingOperation::SelectMode => {
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
                let executed = match (&call.request, call.action) {
                    (
                        BuildProductRequest::RootBinding(request),
                        TerminalBuildBindingOperation::Bind,
                    ) => {
                        let RootBindingOperand::AuthoredPath(path) = &request.implementation else {
                            return Err(refuse(
                                "Terminal Build descriptions require an issued product-query value",
                            ));
                        };
                        if !structural_arguments.is_empty() || !storage_buffers.is_empty() {
                            return Err(refuse(
                                "authored root binding has unexpected structural operands",
                            ));
                        }
                        ExecutedBuildProductRequest::RootBinding(ExecutedRootBinding {
                            source_span: request.source_span,
                            slot: request.slot.clone(),
                            implementation: RootBindingSelection::AuthoredPath(path.clone()),
                        })
                    }
                    (
                        BuildProductRequest::DeclarationSelection(request),
                        action @ (TerminalBuildBindingOperation::Select
                        | TerminalBuildBindingOperation::SelectMode),
                    ) => {
                        let composition = if action == TerminalBuildBindingOperation::SelectMode {
                            if structural_arguments.len() != 1 {
                                return Err(refuse(
                                    "Build provider selection has no evaluated composition mode",
                                ));
                            }
                            let expected = call.composition.ok_or_else(|| {
                                refuse(
                                    "Build provider selection lost its checked composition cases",
                                )
                            })?;
                            let [TerminalBoundaryStorageBuffer::Structural(buffer)] =
                                storage_buffers
                            else {
                                return Err(refuse(
                                    "Build composition mode requires its actual semantic contents",
                                ));
                            };
                            if buffer.argument_index() != 0
                                || buffer.access() != terminal_psi::StructuralAccess::Owned
                                || buffer.structural_type() != expected.structural_type
                            {
                                return Err(refuse(
                                    "Build composition observation differs from its checked formal",
                                ));
                            }
                            let [value] = buffer.values() else {
                                return Err(refuse(
                                    "Build composition mode has unexpected payload contents",
                                ));
                            };
                            if !value.path.is_empty() {
                                return Err(refuse(
                                    "Build composition mode has no root discriminator",
                                ));
                            }
                            match value.contents {
                                TerminalStorageContents::Case(case) if case == expected.fused => {
                                    ProviderCompositionMode::Fused
                                }
                                TerminalStorageContents::Case(case)
                                    if case == expected.independent =>
                                {
                                    ProviderCompositionMode::Independent
                                }
                                _ => {
                                    return Err(refuse(
                                        "Build composition mode is outside its checked case roster",
                                    ));
                                }
                            }
                        } else {
                            if !structural_arguments.is_empty() || !storage_buffers.is_empty() {
                                return Err(refuse(
                                    "Build selection has unexpected structural operands",
                                ));
                            }
                            ProviderCompositionMode::Fused
                        };
                        ExecutedBuildProductRequest::DeclarationSelection(
                            ExecutedProductDeclarationSelection {
                                request: request.clone(),
                                author: call.author,
                                composition,
                            },
                        )
                    }
                    _ => {
                        return Err(refuse(
                            "Build commit action differs from its source-issued request",
                        ));
                    }
                };
                state
                    .executed
                    .try_reserve(1)
                    .map_err(|_| refuse("cannot retain executed Build binding"))?;
                state.pending[ticket] = None;
                state.executed.push(executed);
                Ok(TerminalEffectResult::Unit)
            }
        }
    }
}
