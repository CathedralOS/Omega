//! Observe provider choices from an authenticated Terminal Build activation.
//!
//! Only artifact-bound source companions admit imports. Runtime values supply
//! the original Build identity and evaluated mode; static descriptions are
//! rejoined under their lexical authority after Product assembly.

use language_semantics::declaration_selection::{
    ExecutedProductProviderSelection, ProviderCompositionMode,
};
use psi_compiler::{TerminalBuildProviderCall, TerminalBuildProviderSelections};
use std::sync::Mutex;
use terminal_interpreter::{
    TerminalBoundaryByteBuffer, TerminalBoundaryStorageBuffer, TerminalEffect,
    TerminalEffectHandler, TerminalEffectRejection, TerminalEffectResult, TerminalImportAdapter,
    TerminalImportSignature, TerminalStorageContents, TerminalStructuralValue,
};

pub(super) struct TerminalBuildProviderHost<'code> {
    calls: &'code [TerminalBuildProviderCall],
    original: TerminalStructuralValue,
    executed: Mutex<Vec<ExecutedProductProviderSelection>>,
}

impl<'code> TerminalBuildProviderHost<'code> {
    pub fn new(
        artifact: terminal_codec::TerminalExecutionView<'_>,
        selections: &'code TerminalBuildProviderSelections,
        original: TerminalStructuralValue,
    ) -> Result<Self, String> {
        Ok(Self {
            calls: selections
                .calls_for(artifact)
                .ok_or("Build provider descriptions belong to another artifact")?,
            original,
            executed: Mutex::new(Vec::new()),
        })
    }
    pub fn adapter(&self, signature: TerminalImportSignature) -> ProviderAdapter<'_, 'code> {
        ProviderAdapter {
            host: self,
            signature,
        }
    }
    pub fn take_executed(&self) -> Result<Vec<ExecutedProductProviderSelection>, String> {
        self.executed
            .lock()
            .map(|mut rows| std::mem::take(&mut *rows))
            .map_err(|_| "Build provider request lock was poisoned".into())
    }
}

pub(super) struct ProviderAdapter<'host, 'code> {
    host: &'host TerminalBuildProviderHost<'code>,
    signature: TerminalImportSignature,
}
impl TerminalImportAdapter for ProviderAdapter<'_, '_> {
    fn implemented_signature(&self) -> TerminalImportSignature {
        self.signature.clone()
    }
}
impl TerminalEffectHandler for ProviderAdapter<'_, '_> {
    fn storage_arguments(&self, boundary: semantic_vocabulary::BoundaryMachineId) -> &[usize] {
        if self
            .host
            .calls
            .iter()
            .any(|call| call.boundary == boundary && !call.mode_cases.is_empty())
        {
            &[1]
        } else {
            &[]
        }
    }
    fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        Err(TerminalEffectRejection::new(
            "Build provider selection requires result-aware dispatch",
        ))
    }
    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        self.handle_effect_with_buffers(effect, &mut [], &mut [])
    }
    fn handle_effect_with_buffers(
        &mut self,
        effect: &TerminalEffect,
        bytes: &mut [TerminalBoundaryByteBuffer],
        storage: &mut [TerminalBoundaryStorageBuffer],
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        let refuse = TerminalEffectRejection::new;
        let TerminalEffect::BoundaryCall {
            operation,
            boundary,
            arguments,
            structural_arguments,
            ..
        } = effect
        else {
            return Err(refuse("not a Build provider selection effect"));
        };
        let call = self
            .host
            .calls
            .iter()
            .find(|call| call.operation == *operation && call.boundary == *boundary)
            .ok_or_else(|| refuse("Build provider selection has no source-issued description"))?;
        if !arguments.is_empty()
            || !bytes.is_empty()
            || structural_arguments.first() != Some(&self.host.original)
        {
            return Err(refuse(
                "provider selection requires the original compiler-issued Build activation",
            ));
        }
        let composition_mode = if call.mode_cases.is_empty() {
            if structural_arguments.len() != 1 || !storage.is_empty() {
                return Err(refuse("default provider selection has unexpected operands"));
            }
            ProviderCompositionMode::Fused
        } else {
            if structural_arguments.len() != 2 {
                return Err(refuse("provider mode selection lost its evaluated value"));
            }
            let [TerminalBoundaryStorageBuffer::Structural(buffer)] = storage else {
                return Err(refuse("provider mode has no captured semantic storage"));
            };
            if buffer.argument_index() != 1
                || buffer.access() != terminal_psi::StructuralAccess::Owned
            {
                return Err(refuse(
                    "provider mode storage has the wrong argument or access",
                ));
            }
            let [value] = buffer.values() else {
                return Err(refuse("provider mode has unexpected payload storage"));
            };
            let TerminalStorageContents::Case(case) = value.contents else {
                return Err(refuse("provider mode did not retain its evaluated case"));
            };
            if !value.path.is_empty() {
                return Err(refuse("provider mode case is not its whole value"));
            }
            call.mode_cases
                .iter()
                .find_map(|(candidate, mode)| (*candidate == case).then_some(*mode))
                .ok_or_else(|| refuse("provider mode case belongs to another declaration"))?
        };
        let mut rows = self
            .host
            .executed
            .lock()
            .map_err(|_| refuse("Build provider request lock was poisoned"))?;
        rows.try_reserve(1)
            .map_err(|_| refuse("cannot retain Build provider selection"))?;
        rows.push(ExecutedProductProviderSelection {
            source_span: call.source_span,
            selecting_machine: call.selecting_machine,
            subject: call.subject.clone(),
            provider: call.provider.clone(),
            composition_mode,
        });
        Ok(TerminalEffectResult::Unit)
    }
}
