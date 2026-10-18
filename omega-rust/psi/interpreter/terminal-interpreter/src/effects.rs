//! Effects an execution raises to its handler, the handler trait, the
//! rejection it may answer with, and interpreter-local supply dispatch.
//!
//! `results.rs` binds a host result to the verified boundary result it
//! answers, and `byte_buffers.rs` stages the exact array and field loans a
//! boundary call lends the host and their separately committed replacement.
//! `storage_loans.rs` captures primitive or complete active aggregate contents,
//! validates all replacements, then commits through the original backing.
//! Host buffers never name mutable interpreter storage; scalar restrictions
//! and endpoint queries remain owned by `scalar_loans.rs`.

pub(crate) mod byte_buffers;
pub(crate) mod crash_outcome;
pub(crate) mod results;
pub(crate) mod scalar_loans;
pub(crate) mod storage_loans;

use crate::effects::byte_buffers::TerminalBoundaryByteBuffer;
use crate::effects::results::TerminalEffectResult;
use crate::effects::storage_loans::TerminalBoundaryStorageBuffer;
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
    /// Request captured semantic storage of these structural arguments before
    /// host dispatch. This selects observations, never access or authority.
    fn storage_arguments(&self, _boundary: semantic_vocabulary::BoundaryMachineId) -> &[usize] {
        &[]
    }

    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection>;

    /// Handle primitive or aggregate storage loans beside byte buffers. The
    /// default refuses unsupported storage loans before invoking an old adapter.
    fn handle_effect_with_buffers(
        &mut self,
        effect: &TerminalEffect,
        byte_buffers: &mut [TerminalBoundaryByteBuffer],
        storage_buffers: &mut [TerminalBoundaryStorageBuffer],
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        if !storage_buffers.is_empty() {
            return Err(TerminalEffectRejection::new(
                "handler does not support boundary storage loans",
            ));
        }
        self.handle_effect_with_byte_buffers(effect, byte_buffers)
    }

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

pub use terminal_installation::{
    AdmittedAssumedBinding, AdmittedTargetLayouts, AdmittedTargetMachineSelections,
    AssumedBindingSelection, ProviderInstallationSelection, admit_target_layouts,
    admit_target_machine_selections,
};

/// Admitted supply selection bound to one exact Terminal-Psi identity.
/// Private fields preserve checked-provider custody; receiving hosts may add
/// normal import choices through `TerminalProgram::admit_runtime_supplies`.
/// Shared catalog admission supplies checked selections; this runtime carrier
/// adds the interpreter's dispatch choice. Neither selection route establishes
/// adapters or external-completion authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalRuntimeSupplies {
    pub(crate) entry: Option<terminal_psi::TargetMachineCallable>,
    pub(crate) terminal_psi: terminal_psi::TerminalPsiIdentity,
    pub(crate) supplies: BTreeMap<BoundaryMachineId, AdmittedBoundarySupply>,
    pub(crate) assumed_bindings: Vec<AdmittedAssumedBinding>,
    pub(crate) target_machines: Option<AdmittedTargetMachineSelections>,
    pub(crate) target_layouts: Option<std::sync::Arc<AdmittedTargetLayouts>>,
}

/// One receiving host's selected execution route, never catalog availability.
/// HostImport selects a crossing but establishes no host adapter or authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AdmittedBoundarySupply {
    CheckedProvider(MachineId),
    HostImport,
}

/// Select host crossings alongside an existing sealed checked installation.
/// The receiving host owns this policy input; artifact possession cannot
/// choose imports on its behalf. Actual adapters remain instance-bound.
#[derive(Debug, Clone, Copy)]
pub struct TerminalRuntimeSupplyRequest<'installation> {
    /// Requested retained callable; absence selects the artifact's default.
    /// This supplies no parameter backing or host/provider authority.
    pub entry: Option<terminal_psi::TargetMachineCallable>,
    pub installation: Option<&'installation terminal_installation::AdmittedProviderSelections>,
    pub host_imports: &'installation [BoundaryMachineId],
    /// Exact catalog selections, independently of provider or host authority.
    pub target_machines: Option<&'installation AdmittedTargetMachineSelections>,
    /// Exact geometry closure over the same immutable artifact and target.
    pub target_layouts: Option<&'installation AdmittedTargetLayouts>,
}

impl From<&terminal_installation::AdmittedProviderSelections> for TerminalRuntimeSupplies {
    fn from(selected: &terminal_installation::AdmittedProviderSelections) -> Self {
        Self {
            terminal_psi: selected.terminal_psi(),
            entry: None,
            supplies: selected
                .providers()
                .map(|(boundary, machine)| {
                    (boundary, AdmittedBoundarySupply::CheckedProvider(machine))
                })
                // These are runtime supplies under separately retained assumed
                // contracts, not new checked provider conformances. Admission
                // already rejects competing supplies and resolves the full path.
                .chain(selected.assumed_bindings().iter().map(|relationship| {
                    (
                        relationship.required,
                        AdmittedBoundarySupply::CheckedProvider(relationship.implementation),
                    )
                }))
                .collect(),
            assumed_bindings: selected.assumed_bindings().to_vec(),
            target_machines: None,
            target_layouts: None,
        }
    }
}

impl TerminalRuntimeSupplies {
    pub const fn terminal_psi(&self) -> terminal_psi::TerminalPsiIdentity {
        self.terminal_psi
    }

    /// The root-authorized assumed binding contracts admitted alongside the
    /// checked selections, in authored policy order. Each is conditional
    /// evidence: the artifact's guarantees hold under the recorded
    /// assumption, whose falsity invalidates them without producing any
    /// unconditional claim.
    pub fn assumed_bindings(&self) -> &[AdmittedAssumedBinding] {
        &self.assumed_bindings
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
