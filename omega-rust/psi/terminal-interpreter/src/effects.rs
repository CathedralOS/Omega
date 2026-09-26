//! Effects an execution raises to its handler, the handler trait, the
//! rejection it may answer with, and provider installation admission.
//!
//! `results.rs` binds a host result to the verified boundary result it
//! answers, and `byte_buffers.rs` stages the exact array and field loans a
//! boundary call lends the host and their separately committed replacement.

pub(crate) mod byte_buffers;
pub(crate) mod results;

use crate::effects::byte_buffers::TerminalBoundaryByteBuffer;
use crate::effects::results::TerminalEffectResult;
use crate::values::{TerminalScalarValue, TerminalStructuralValue};
use semantic_vocabulary::{BoundaryMachineId, MachineId, OperationId, ServiceId};
use std::collections::BTreeMap;
use terminal_psi::{BoundaryMachineResult, CompletionReceipt};

/// One externally observable terminal-Psi effect in semantic execution order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalEffect {
    BoundaryCall {
        operation: OperationId,
        boundary: BoundaryMachineId,
        arguments: Vec<TerminalScalarValue>,
        structural_arguments: Vec<TerminalStructuralValue>,
        /// Exact byte payload aligned with `structural_arguments`. Whole
        /// immutable literals, forwarded views, and mutable inline fields
        /// contribute `Some`; other structural types contribute `None`.
        /// Mutable payloads record the pre-call snapshot, not replacement bytes.
        /// A byte view without executable
        /// contents rejects before invoking the handler.
        byte_sequence_arguments: Vec<Option<Vec<u8>>>,
        /// Required receipts on normal completion, not evidence that completion
        /// occurred. A valid crashing invocation remains an observable effect
        /// but commits none of these receipts.
        completion_receipts: Vec<CompletionReceipt>,
        result: BoundaryMachineResult,
    },
    PortWrite {
        operation: OperationId,
        service: ServiceId,
        port: u16,
        value: u8,
    },
}

/// Injected semantic effect sink used by the oracle and tests. Native provider
/// selection and hardware realization remain outside the Psi interpreter.
pub trait TerminalEffectHandler {
    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection>;

    /// Handle an effect with staged, bounded mutable byte arguments. The
    /// default rejects before performing an effect whose writeback it cannot
    /// supply; existing immutable effects retain their result handler.
    fn handle_effect_with_byte_buffers(
        &mut self,
        effect: &TerminalEffect,
        buffers: &mut [TerminalBoundaryByteBuffer],
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        if !buffers.is_empty() {
            return Err(TerminalEffectRejection::new(
                "handler does not support mutable boundary byte buffers",
            ));
        }
        self.handle_effect_result(effect)
    }

    /// Return the boundary's exact declared result. The default Unit handler
    /// rejects structural results before performing an effect it cannot finish.
    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        if matches!(
            effect,
            TerminalEffect::BoundaryCall {
                result: BoundaryMachineResult::Structural(_),
                ..
            }
        ) {
            return Err(TerminalEffectRejection::new(
                "handler does not supply structural boundary results",
            ));
        }
        self.handle_effect(effect)?;
        Ok(TerminalEffectResult::Unit)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalEffectRejection {
    pub reason: String,
}

/// Omega-owned policy input naming one exact verified terminal provider row.
/// Selection is intentionally separate from terminal-Psi semantic bytes.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProviderInstallationSelection {
    pub boundary: BoundaryMachineId,
    pub provider_identity: String,
    pub candidate: MachineId,
}

/// Validated provider installation bound to one exact terminal-Psi identity.
/// Private fields prevent callers from manufacturing a boundary-to-machine
/// redirect without replaying terminal decoding and verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedProviderInstallation {
    pub(crate) terminal_psi: terminal_psi::TerminalPsiIdentity,
    pub(crate) installed: BTreeMap<BoundaryMachineId, MachineId>,
}

impl AdmittedProviderInstallation {
    pub const fn terminal_psi(&self) -> terminal_psi::TerminalPsiIdentity {
        self.terminal_psi
    }
}

impl TerminalEffectRejection {
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

#[derive(Debug, Default)]
pub struct AcceptTerminalEffects;

impl TerminalEffectHandler for AcceptTerminalEffects {
    fn handle_effect(&mut self, _effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        Ok(())
    }
}
