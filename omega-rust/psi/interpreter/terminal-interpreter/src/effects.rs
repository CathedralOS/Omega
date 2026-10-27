//! Effects an execution raises to its handler, the handler trait, the
//! rejection it may answer with, and interpreter-local supply dispatch.
//!
//! `results.rs` binds a host result to the verified boundary result it
//! answers, and `byte_buffers.rs` stages the exact array and field loans a
//! boundary call lends the host and their separately committed replacement.
//! `storage_inputs.rs` captures primitive or complete active aggregate contents,
//! validates all replacements, then commits through the original backing.
//! Host buffers never name mutable interpreter storage; scalar restrictions
//! and endpoint queries remain owned by `scalar_loans.rs`.

pub(crate) mod boundary_predicates;
pub(crate) mod byte_buffers;
pub(crate) mod crash_outcome;
pub(crate) mod original_storage;
pub(crate) mod results;
pub(crate) mod scalar_loans;
pub(crate) mod storage_inputs;

use crate::effects::byte_buffers::TerminalBoundaryByteBuffer;
use crate::effects::results::TerminalEffectResult;
use crate::effects::storage_inputs::TerminalBoundaryStorageBuffer;
use crate::errors::TerminalInterpretError;
use crate::values::{TerminalScalarValue, TerminalStructuralValue};
use semantic_vocabulary::{BoundaryMachineId, MachineId, OperationId, ServiceId};
use std::collections::BTreeMap;
use terminal_psi::{BoundaryMachineResult, CompletionReceipt};

/// One externally observable terminal-Psi effect in semantic execution order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalEffect {
    /// A completed compiler-defined ordering event in the supplied private
    /// sequential model. This is an observation, never a host callback.
    MemoryFence {
        operation: OperationId,
        kind: terminal_psi::MemoryFenceKind,
    },
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
    /// Receipt of a real checked component invocation. Nested operation and
    /// boundary IDs stay under their producing artifact/instance namespace;
    /// flattening them into the receiver's ID table would change their meaning.
    ComponentCall {
        operation: OperationId,
        boundary: BoundaryMachineId,
        provider: terminal_psi::TerminalPsiIdentity,
        candidate: MachineId,
        instance_namespace: u64,
        effects: Vec<TerminalEffect>,
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
    /// Poll readiness without invoking the operation or changing lent storage.
    /// A false answer parks only a checked suspending call. The interpreter
    /// retains its prepared arguments and polls this same invocation on resume;
    /// `handle_effect_with_buffers` runs exactly once after readiness succeeds.
    /// Scheduling work here must be idempotent for this retained effect.
    fn poll_effect_readiness(
        &mut self,
        _effect: &TerminalEffect,
    ) -> Result<bool, TerminalEffectRejection> {
        Ok(true)
    }

    /// Whether borrowed storage must lend its actual original homes to this
    /// host binding. Ordinary semantic buffer adapters keep staged restoration.
    fn requires_original_storage(&self, _boundary: BoundaryMachineId) -> bool {
        false
    }
    /// Realize the already checked invocation's original allocations. The
    /// returned rows match requests and their scalar cells exactly. Preparing
    /// backing performs no provider call and supplies no claims or lifetimes.
    /// An actual original route must reject unsupported geometry before
    /// dispatching native code.
    fn prepare_original_storage(
        &mut self,
        _boundary: BoundaryMachineId,
        _requests: &[original_storage::TerminalOriginalStorageRequest],
    ) -> Result<original_storage::TerminalOriginalScalarBackings, TerminalEffectRejection> {
        Err(TerminalEffectRejection::new(
            "handler does not supply original storage homes",
        ))
    }

    /// Request captured semantic storage of these structural arguments before
    /// host dispatch. Owned arguments supply immutable payload snapshots;
    /// reading borrows retain their existing restoration protocol. This selects
    /// observations, never access or authority.
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
    pub(crate) x86_flags: Option<TerminalX86FlagsEnvironment>,
    pub(crate) memory_ordering: Option<crate::TerminalMemoryOrderingEnvironment>,
    pub(crate) entry: Option<terminal_psi::TargetMachineCallable>,
    pub(crate) terminal_psi: terminal_psi::TerminalPsiIdentity,
    pub(crate) supplies: BTreeMap<BoundaryMachineId, AdmittedBoundarySupply>,
    pub(crate) assumed_bindings: Vec<AdmittedAssumedBinding>,
    pub(crate) folded_constant_target: Option<String>,
    pub(crate) target_machines: Option<AdmittedTargetMachineSelections>,
    pub(crate) target_layouts: Option<std::sync::Arc<AdmittedTargetLayouts>>,
}

/// One receiving host's selected execution route, never catalog availability.
/// HostImport selects a crossing but establishes no host adapter or authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AdmittedBoundarySupply {
    CheckedProvider(MachineId),
    ComponentProvider {
        provider: terminal_psi::TerminalPsiIdentity,
        provider_boundary: BoundaryMachineId,
        candidate: MachineId,
    },
    /// The host schedules an independently checked component. This reserves
    /// a route, not a callable implementation or a host-adapter permission.
    DeferredComponent,
    HostImport,
}

/// Select host crossings alongside an existing sealed checked installation.
/// The receiving host owns this policy input; artifact possession cannot
/// choose imports on its behalf. Actual adapters remain instance-bound.
#[derive(Debug, Clone, Copy)]
pub struct TerminalRuntimeSupplyRequest<'installation> {
    /// Explicit modeled processor state. Absence supplies no PUSHFQ capability.
    pub x86_flags: Option<TerminalX86FlagsEnvironment>,
    /// Explicit scoped memory model; absence supplies no catalog fence semantics.
    pub memory_ordering: Option<crate::TerminalMemoryOrderingEnvironment>,
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
            x86_flags: None,
            memory_ordering: None,
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
            folded_constant_target: selected.folded_constant_target().map(str::to_owned),
            target_machines: None,
            target_layouts: None,
        }
    }
}

impl TerminalRuntimeSupplies {
    /// Add an explicitly selected cross-artifact route. This establishes no
    /// instance or adapter and cannot replace an existing local/host selection.
    pub fn with_component_provider(
        mut self,
        selected: &terminal_installation::AdmittedComponentProvider,
    ) -> Result<Self, TerminalInterpretError> {
        let boundary = selected.receiving_boundary();
        if selected.receiver() != self.terminal_psi || self.supplies.contains_key(&boundary) {
            return Err(TerminalInterpretError::RuntimeSupplySelectionRejected {
                boundary,
                reason: "component selection names another receiver or an already selected slot",
            });
        }
        self.supplies.insert(
            boundary,
            AdmittedBoundarySupply::ComponentProvider {
                provider: selected.provider(),
                provider_boundary: selected.provider_boundary(),
                candidate: selected.candidate(),
            },
        );
        Ok(self)
    }

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

/// Receiving-host selected x86 processor flags, independent of host authority.
/// The model supplies an architectural state, never an instruction callback.
/// Arithmetic flag restores merge the six condition bits into this image.
/// Reserved bits stay the bits captured from the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalX86FlagsEnvironment {
    rflags: u64,
}
impl TerminalX86FlagsEnvironment {
    pub fn new(rflags: u64) -> Result<Self, TerminalInterpretError> {
        // Reserved bits are fixed: bit 1 is set; bits 3, 5, 15 and 22..63
        // are clear. RF/VM may exist in the state but PUSHFQ clears their image.
        if rflags & 2 == 0 || rflags & !0x3f7fd7 != 0 {
            return Err(TerminalInterpretError::UnsupportedSemanticVariant(
                "invalid x86 flags environment: reserved RFLAGS bits",
            ));
        }
        Ok(Self { rflags })
    }
    pub(crate) fn snapshot(self) -> u64 {
        self.rflags & !((1 << 16) | (1 << 17))
    }

    /// Keep reserved bits from the host image. The saved place carries the
    /// arithmetic flags recorded by the producing instruction, not a complete
    /// RFLAGS image, so only CF, PF, AF, ZF, SF and OF move.
    pub(crate) fn restore_arithmetic_bits(&mut self, image: u64) {
        const ARITHMETIC: u64 = 0x8d5;
        self.rflags = (self.rflags & !ARITHMETIC) | (image & ARITHMETIC);
    }
}
