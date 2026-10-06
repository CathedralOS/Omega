//! Guest instances: one reusable program's per-invocation runtime state.
//!
//! [`TerminalInstance`] is the owned counterpart of the admitted program —
//! its own scalar/structural values, custody, claims, suspension stack and
//! effect record live here while the verified code lives on the shared
//! program. Bindings are the instance's roster: `bind_import` joins one
//! demanded import slot to one checked host adapter, `invoke` runs the entry
//! under that roster, and `close` releases the interpreter state and ends the
//! binding lifetime.
//!
//! The mapping onto the embedding contract: a bound slot is the runtime
//! carrier `Binding<R>` describes — opaque (the host cannot read internals),
//! affine (one slot binds once; closing consumes the roster), and checked at
//! establishment against the declared contract rather than at first use.
//! Same-requirement slots bind different host objects because dispatch keys
//! on the slot's `BoundaryMachineId`, never on a trait or name.

use std::collections::BTreeMap;

use semantic_vocabulary::BoundaryMachineId;
use terminal_fuel::TerminalFuelMeter;

use crate::adapters::TerminalImportAdapter;
use crate::effects::byte_buffers::TerminalBoundaryByteBuffer;
use crate::effects::results::TerminalEffectResult;
use crate::effects::{TerminalEffect, TerminalEffectHandler, TerminalEffectRejection};
use crate::errors::TerminalInterpretError;
use crate::execution::TerminalExecution;
use crate::program::TerminalProgram;
use crate::results::TerminalExecutionStatus;

/// One established boundary-requirement binding — the `Binding<R>` carrier at
/// this surface: an exact import slot joined at establishment to the checked
/// host adapter that answers it. The instance roster owns it; it cannot
/// migrate, copy, or rebind into another instance.
struct TerminalImportBinding<'host> {
    /// The signature the adapter declared at establishment — the record of
    /// what this binding was checked against, readable through
    /// `bound_signature`.
    signature: crate::adapters::TerminalImportSignature,
    adapter: Box<dyn TerminalImportAdapter + 'host>,
}

/// Why a host adapter cannot establish an import slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalBindingRejection {
    /// The slot names no boundary machine this program declares — the
    /// occurrence simply does not exist.
    UnknownImport { boundary: BoundaryMachineId },
    /// A declared catalog slot is not demanded by this entry/installation.
    NotRequiredImport { boundary: BoundaryMachineId },
    /// The slot is realized by a checked provider candidate. An admitted
    /// provider installation supplies it; a host binding may not substitute.
    ProviderRealized { boundary: BoundaryMachineId },
    /// The slot already holds an established binding. `Binding<R>` is affine:
    /// rebinding requires a fresh instance, not a second supply.
    DuplicateBinding { boundary: BoundaryMachineId },
    /// The adapter's declared signature differs from the slot's exact
    /// contract — a wrong or incompatible host object, rejected before any
    /// guest effect can reach it.
    SignatureMismatch { boundary: BoundaryMachineId },
    /// The instance is closed; its binding roster is consumed.
    InstanceClosed,
}

/// Why an entry invocation could not run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalInstanceError {
    /// The instance is closed; no new entry is admitted.
    Closed,
    /// Demanded imports without an established binding. Every demanded import
    /// must be established before guest code runs — the roster is checked
    /// whole, before the first operation.
    UnboundImports { missing: Vec<BoundaryMachineId> },
    /// An execution-level failure from the interpreter.
    Execution(TerminalInterpretError),
}

/// Why an instance cannot close now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalInstanceCloseError {
    /// The entry activation is still live: it paused on fuel or a suspension
    /// boundary with custody outstanding. The caller may keep driving the
    /// instance or abandon it by dropping it; a live activation is not
    /// silently discarded by `close`.
    Busy,
}

/// One live instance of an admitted program: its runtime state, custody, and
/// host binding roster, sharing the program's immutable code.
///
/// Construction is `TerminalProgram::instantiate`. The `'host` lifetime ties
/// the borrowed host objects behind the bound adapters to this roster —
/// adapters cannot outlive the instance that bound them.
pub struct TerminalInstance<'host> {
    program: TerminalProgram,
    /// The interpreter state while this instance owns it. `close` releases it
    /// early; `None` after close.
    execution: Option<TerminalExecution>,
    /// Exact reachable host slots under this instance's provider installation.
    /// Retained unused declarations impose no binding requirement.
    required_imports: Vec<BoundaryMachineId>,
    /// Established import bindings by exact slot. Empty at construction;
    /// `invoke` refuses to start while a demanded import is unbound.
    bindings: BTreeMap<BoundaryMachineId, TerminalImportBinding<'host>>,
    /// Whether `invoke` has admitted the entry. Close of a paused activation
    /// rejects `Busy` only once the entry has actually started.
    invoked: bool,
    closed: bool,
}

impl<'host> TerminalInstance<'host> {
    pub(crate) fn new(
        program: TerminalProgram,
        execution: TerminalExecution,
        required_imports: Vec<BoundaryMachineId>,
    ) -> Self {
        Self {
            program,
            execution: Some(execution),
            required_imports,
            bindings: BTreeMap::new(),
            invoked: false,
            closed: false,
        }
    }

    /// The admitted program this instance runs — the program handle is
    /// retained by the instance for its whole life.
    pub fn program(&self) -> &TerminalProgram {
        &self.program
    }

    /// Establish one demanded import slot's checked host adapter.
    ///
    /// Checks run before the roster changes: the slot must be a boundary this
    /// program declares, not provider-realized, still unbound, and the
    /// adapter's declared signature must equal the slot's exact contract.
    /// A rejected bind changes nothing — the roster keeps its earlier state.
    pub fn bind_import(
        &mut self,
        slot: BoundaryMachineId,
        adapter: impl TerminalImportAdapter + 'host,
    ) -> Result<(), TerminalBindingRejection> {
        if self.closed {
            return Err(TerminalBindingRejection::InstanceClosed);
        }
        let Some(declaration) = self.program.import(slot) else {
            return if self.program.program.boundary_machines.contains_key(&slot) {
                // A declared boundary a provider candidate realizes — the
                // admitted installation answers it, never a host adapter.
                Err(TerminalBindingRejection::ProviderRealized { boundary: slot })
            } else {
                Err(TerminalBindingRejection::UnknownImport { boundary: slot })
            };
        };
        if !self.required_imports.contains(&slot) {
            return Err(TerminalBindingRejection::NotRequiredImport { boundary: slot });
        }
        if self.bindings.contains_key(&slot) {
            return Err(TerminalBindingRejection::DuplicateBinding { boundary: slot });
        }
        let signature = crate::adapters::TerminalImportSignature::from(declaration);
        if adapter.implemented_signature() != signature {
            return Err(TerminalBindingRejection::SignatureMismatch { boundary: slot });
        }
        self.bindings.insert(
            slot,
            TerminalImportBinding {
                signature,
                adapter: Box::new(adapter),
            },
        );
        Ok(())
    }

    /// Demanded imports still awaiting an established binding, in slot order.
    /// A closed instance's roster is consumed, not pending — it reports none.
    pub fn unbound_imports(&self) -> impl Iterator<Item = BoundaryMachineId> + '_ {
        let closed = self.closed;
        self.required_imports
            .iter()
            .copied()
            .filter(move |slot| !closed && !self.bindings.contains_key(slot))
    }

    /// The demanded imports this instance has already established.
    pub fn bound_imports(&self) -> impl Iterator<Item = BoundaryMachineId> + '_ {
        self.bindings.keys().copied()
    }

    /// The contract `slot`'s established binding was checked against —
    /// the retained import description this instance answered it with.
    /// Absent while the slot is unbound and after close consumes the roster.
    pub fn bound_signature(
        &self,
        slot: BoundaryMachineId,
    ) -> Option<&crate::adapters::TerminalImportSignature> {
        self.bindings.get(&slot).map(|binding| &binding.signature)
    }

    /// Drive the entry invocation until it completes, crashes, or pauses on
    /// the fuel meter.
    ///
    /// The whole demanded-import roster must be established first: a missing
    /// binding rejects before the first guest operation, so no host effect
    /// can run for an unbound slot. A fuel pause preserves custody — resume
    /// by calling `invoke` again with a replenished meter; already-committed
    /// effects are never replayed.
    pub fn invoke(
        &mut self,
        meter: &mut TerminalFuelMeter,
    ) -> Result<TerminalExecutionStatus, TerminalInstanceError> {
        if self.closed {
            return Err(TerminalInstanceError::Closed);
        }
        let missing = self.unbound_imports().collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(TerminalInstanceError::UnboundImports { missing });
        }
        let execution = self
            .execution
            .as_mut()
            .ok_or(TerminalInstanceError::Closed)?;
        self.invoked = true;
        execution
            .resume(
                meter,
                &mut ImportDispatch {
                    bindings: &mut self.bindings,
                },
            )
            .map_err(TerminalInstanceError::Execution)
    }

    /// The recorded effect receipt stream of this instance: empty before the
    /// first committed effect and after `close` releases execution state.
    pub fn effects(&self) -> &[TerminalEffect] {
        self.execution
            .as_ref()
            .map(|execution| execution.effects())
            .unwrap_or(&[])
    }

    /// The committed result once the entry invocation has produced one —
    /// the same value `invoke` returned, retained for post-run accounting.
    pub fn result(&self) -> Option<&crate::results::TerminalExecutionResult> {
        self.execution
            .as_ref()
            .and_then(|execution| execution.result())
    }

    /// Whether the entry invocation has reached a terminal outcome.
    pub fn is_finished(&self) -> bool {
        self.execution
            .as_ref()
            .is_none_or(|execution| execution.is_finished())
    }

    /// Whether this instance's roster and execution have been released.
    pub fn is_closed(&self) -> bool {
        self.closed
    }

    /// Close this instance: release the interpreter state and consume the
    /// binding roster.
    ///
    /// A live activation — invoked but not finished — rejects `Busy`: its
    /// custody is preserved for a later resume rather than silently dropped.
    /// Closing a never-invoked or finished instance succeeds and is
    /// idempotent. After close, `invoke` and `bind_import` reject, and the
    /// shared program remains for other instances.
    pub fn close(&mut self) -> Result<(), TerminalInstanceCloseError> {
        if self.closed {
            return Ok(());
        }
        if self.invoked
            && self
                .execution
                .as_ref()
                .is_some_and(|execution| !execution.is_finished())
        {
            return Err(TerminalInstanceCloseError::Busy);
        }
        self.execution = None;
        self.bindings.clear();
        self.closed = true;
        Ok(())
    }
}

/// Per-invocation dispatch: routes each boundary effect to the adapter bound
/// for its exact slot.
///
/// The dispatch exists only inside `invoke`, borrows the instance roster, and
/// resolves by `BoundaryMachineId` — the slot is the dispatch key, so two
/// slots carrying the same requirement reach different host objects and a
/// call to a slot no adapter bound rejects before any host code runs.
struct ImportDispatch<'a, 'host> {
    bindings: &'a mut BTreeMap<BoundaryMachineId, TerminalImportBinding<'host>>,
}

impl TerminalEffectHandler for ImportDispatch<'_, '_> {
    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        match effect {
            TerminalEffect::BoundaryCall { boundary, .. } => self
                .bindings
                .get_mut(boundary)
                .ok_or_else(|| {
                    TerminalEffectRejection::new("boundary call reached an unbound import slot")
                })?
                .adapter
                .handle_effect(effect),
            TerminalEffect::PortWrite { .. } => Err(TerminalEffectRejection::new(
                "port output requires an admitted target-environment provider",
            )),
        }
    }

    fn handle_effect_with_byte_buffers(
        &mut self,
        effect: &TerminalEffect,
        byte_buffers: &mut [TerminalBoundaryByteBuffer],
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        match effect {
            TerminalEffect::BoundaryCall { boundary, .. } => self
                .bindings
                .get_mut(boundary)
                .ok_or_else(|| {
                    TerminalEffectRejection::new("boundary call reached an unbound import slot")
                })?
                .adapter
                .handle_effect_with_byte_buffers(effect, byte_buffers),
            TerminalEffect::PortWrite { .. } => Err(TerminalEffectRejection::new(
                "port output requires an admitted target-environment provider",
            )),
        }
    }

    fn handle_effect_result(
        &mut self,
        effect: &TerminalEffect,
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        match effect {
            TerminalEffect::BoundaryCall { boundary, .. } => self
                .bindings
                .get_mut(boundary)
                .ok_or_else(|| {
                    TerminalEffectRejection::new("boundary call reached an unbound import slot")
                })?
                .adapter
                .handle_effect_result(effect),
            TerminalEffect::PortWrite { .. } => Err(TerminalEffectRejection::new(
                "port output requires an admitted target-environment provider",
            )),
        }
    }
}
