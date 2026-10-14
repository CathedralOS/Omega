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
use crate::effects::storage_loans::TerminalBoundaryStorageBuffer;
use crate::effects::{TerminalEffect, TerminalEffectHandler, TerminalEffectRejection};
use crate::errors::TerminalInterpretError;
use crate::execution::TerminalExecution;
use crate::program::TerminalProgram;
use crate::results::TerminalExecutionStatus;

/// Identity only: creating a domain grants no exit authority. Each instance
/// owns a fresh simulated domain; no host or sibling process belongs to it.
pub(crate) struct SimulatedProcessDomain;

/// Explicit trusted host admission of domain-death crossing safety and exit
/// delegation to one exact instance and slot. This is an affine host premise,
/// not proof inferred from a declaration, reach, provider, or Binding roster.
pub struct TerminalProcessExitAdmission {
    program: terminal_psi::TerminalPsiIdentity,
    domain: std::sync::Arc<SimulatedProcessDomain>,
    demand: crate::capabilities::TerminalExternalCompletionDemand,
}

impl TerminalProcessExitAdmission {
    /// The host asserts that this instance's isolated simulated domain may
    /// abandon its in-domain custody without invalidating surviving owners'
    /// guarantees: all actual crossings, including subsequent ordinary host
    /// adapter invocations, admit death through isolation, retained custody,
    /// recovery, or reset. Merely selecting a provider or
    /// reading the artifact cannot justify this assertion. No external domain
    /// is ended, and no release or protocol receipt is created.
    ///
    /// Placed-view host loans have no domain-death admission carrier yet and
    /// are conservatively refused here. Ordinary in-domain loans are not.
    pub fn admit_isolated_simulated_domain(
        instance: &TerminalInstance<'_, '_>,
        boundary: BoundaryMachineId,
    ) -> Result<Self, TerminalBindingRejection> {
        if instance.closed {
            return Err(TerminalBindingRejection::InstanceClosed);
        }
        let execution = instance
            .execution
            .as_ref()
            .ok_or(TerminalBindingRejection::InstanceClosed)?;
        if !execution.placed_view_occurrences.is_empty()
            || execution
                .call_stack
                .iter()
                .any(|frame| !frame.placed_view_occurrences.is_empty())
        {
            return Err(TerminalBindingRejection::DomainDeathCrossingAdmissionMissing { boundary });
        }
        let demand = execution
            .external_demands
            .iter()
            .find(|demand| demand.boundary == boundary)
            .copied()
            .ok_or(TerminalBindingRejection::NotRequiredImport { boundary })?;
        match demand.kind {
            semantic_vocabulary::ExternalCompletionKind::ProcessExit => {}
        }
        Ok(Self {
            program: instance.program.terminal_psi(),
            domain: std::sync::Arc::clone(&instance.process_domain),
            demand,
        })
    }

    pub(crate) fn answers(
        &self,
        program: terminal_psi::TerminalPsiIdentity,
        domain: Option<&std::sync::Arc<SimulatedProcessDomain>>,
        demand: crate::capabilities::TerminalExternalCompletionDemand,
    ) -> bool {
        self.program == program
            && self.demand == demand
            && domain.is_some_and(|domain| std::sync::Arc::ptr_eq(domain, &self.domain))
    }
}

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
    ExternalCompletionRequiresDomainAdmission {
        boundary: BoundaryMachineId,
    },
    ExternalCompletionDomainMismatch {
        boundary: BoundaryMachineId,
    },
    DomainDeathCrossingAdmissionMissing {
        boundary: BoundaryMachineId,
    },
    /// The slot names no boundary machine this program declares — the
    /// occurrence simply does not exist.
    UnknownImport {
        boundary: BoundaryMachineId,
    },
    /// A declared catalog slot is not demanded by this entry/installation.
    NotRequiredImport {
        boundary: BoundaryMachineId,
    },
    /// The slot is realized by a checked provider candidate. An admitted
    /// provider installation supplies it; a host binding may not substitute.
    ProviderRealized {
        boundary: BoundaryMachineId,
    },
    /// The slot already holds an established binding. `Binding<R>` is affine:
    /// rebinding requires a fresh instance, not a second supply.
    DuplicateBinding {
        boundary: BoundaryMachineId,
    },
    /// The adapter's declared signature differs from the slot's exact
    /// contract — a wrong or incompatible host object, rejected before any
    /// guest effect can reach it.
    SignatureMismatch {
        boundary: BoundaryMachineId,
    },
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
    UnboundExternalCompletions {
        missing: Vec<crate::capabilities::TerminalExternalCompletionDemand>,
    },
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
/// The independent `'artifact` lifetime retains compiler-owned code across
/// pauses. Programs loaded from external bytes own their code instead.
pub struct TerminalInstance<'host, 'artifact> {
    program: TerminalProgram<'artifact>,
    process_domain: std::sync::Arc<SimulatedProcessDomain>,
    /// The interpreter state while this instance owns it. `close` releases it
    /// early; `None` after close.
    execution: Option<TerminalExecution<'artifact>>,
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

impl<'host, 'artifact> TerminalInstance<'host, 'artifact> {
    pub(crate) fn new(
        program: TerminalProgram<'artifact>,
        mut execution: TerminalExecution<'artifact>,
        required_imports: Vec<BoundaryMachineId>,
    ) -> Self {
        let process_domain = std::sync::Arc::new(SimulatedProcessDomain);
        execution.process_domain = Some(std::sync::Arc::clone(&process_domain));
        Self {
            program,
            process_domain,
            execution: Some(execution),
            required_imports,
            bindings: BTreeMap::new(),
            invoked: false,
            closed: false,
        }
    }

    /// The admitted program this instance runs — the program handle is
    /// retained by the instance for its whole life.
    pub fn program(&self) -> &TerminalProgram<'artifact> {
        &self.program
    }

    /// Exact demanded import under this instance's admitted supply choices.
    /// A retained checked alternative does not suppress an explicitly selected
    /// host crossing. Description alone still grants no adapter or authority.
    pub fn import(
        &self,
        slot: BoundaryMachineId,
    ) -> Option<&terminal_psi::BoundaryMachineDeclaration> {
        if self.closed || !self.required_imports.contains(&slot) {
            return None;
        }
        self.program.program.boundary_machines().get(&slot)
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
        if self
            .program
            .program
            .boundary_machines()
            .get(&slot)
            .is_some_and(|declaration| {
                matches!(
                    declaration.completion,
                    terminal_psi::BoundaryCompletion::External(_)
                )
            })
        {
            return Err(
                TerminalBindingRejection::ExternalCompletionRequiresDomainAdmission {
                    boundary: slot,
                },
            );
        }
        let Some(declaration) = self.import(slot) else {
            return if self.execution.as_ref().is_some_and(|execution| {
                matches!(
                    execution.boundary_supplies.get(&slot),
                    Some(crate::effects::AdmittedBoundarySupply::CheckedProvider(_))
                )
            }) {
                Err(TerminalBindingRejection::ProviderRealized { boundary: slot })
            } else if self.program.program.boundary_machines().contains_key(&slot) {
                Err(TerminalBindingRejection::NotRequiredImport { boundary: slot })
            } else {
                Err(TerminalBindingRejection::UnknownImport { boundary: slot })
            };
        };
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

    /// Consume an explicit trusted host delegation. The program, exact slot,
    /// completion kind, and fresh containing domain must all rejoin this
    /// instance before the binding roster changes. Provider candidates do not
    /// supply this authority and do not override a closed external demand.
    pub fn bind_external_completion(
        &mut self,
        boundary: BoundaryMachineId,
        admission: TerminalProcessExitAdmission,
    ) -> Result<(), TerminalBindingRejection> {
        if self.closed {
            return Err(TerminalBindingRejection::InstanceClosed);
        }
        let execution = self
            .execution
            .as_mut()
            .ok_or(TerminalBindingRejection::InstanceClosed)?;
        let demand = execution
            .external_demands
            .iter()
            .find(|demand| demand.boundary == boundary)
            .copied()
            .ok_or(TerminalBindingRejection::NotRequiredImport { boundary })?;
        if !admission.answers(
            self.program.terminal_psi(),
            Some(&self.process_domain),
            demand,
        ) {
            return Err(TerminalBindingRejection::ExternalCompletionDomainMismatch { boundary });
        }
        if execution.external_bindings.contains_key(&boundary) {
            return Err(TerminalBindingRejection::DuplicateBinding { boundary });
        }
        execution.external_bindings.insert(boundary, admission);
        Ok(())
    }

    /// Exact pending terminal demands, including every reachable dynamic row.
    pub fn unbound_external_completions(
        &self,
    ) -> impl Iterator<Item = crate::capabilities::TerminalExternalCompletionDemand> + '_ {
        self.execution.iter().flat_map(|execution| {
            execution
                .external_demands
                .iter()
                .copied()
                .filter(|demand| !execution.external_bindings.contains_key(&demand.boundary))
        })
    }

    pub fn external_completion(&self) -> Option<&crate::results::TerminalExternalCompletion> {
        self.execution
            .as_ref()
            .and_then(|execution| execution.external_completion.as_ref())
    }

    /// Actual host-established erased occurrence custody retained beside data
    /// storage. Domain exit creates no discharge receipt for these rows.
    pub fn established_erased_fields(
        &self,
    ) -> &[crate::structural_inputs::TerminalEstablishedErasedField] {
        self.execution.as_ref().map_or(&[], |execution| {
            execution.established_erased_fields.as_slice()
        })
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
        let missing = self.unbound_external_completions().collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(TerminalInstanceError::UnboundExternalCompletions { missing });
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

    pub fn observe_private_entry_ram(&self) -> Result<Vec<(u32, Vec<u8>)>, TerminalInstanceError> {
        self.execution
            .as_ref()
            .ok_or(TerminalInstanceError::Closed)?
            .observe_private_entry_ram()
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

impl<'a, 'host> ImportDispatch<'a, 'host> {
    /// The adapter bound at `boundary`, or the unbound-slot rejection every
    /// dispatch arm shares.
    fn adapter_at(
        &mut self,
        boundary: BoundaryMachineId,
    ) -> Result<&mut (dyn TerminalImportAdapter + 'host), TerminalEffectRejection> {
        Ok(&mut *self
            .bindings
            .get_mut(&boundary)
            .ok_or_else(|| {
                TerminalEffectRejection::new("boundary call reached an unbound import slot")
            })?
            .adapter)
    }
}

impl TerminalEffectHandler for ImportDispatch<'_, '_> {
    fn storage_arguments(&self, boundary: BoundaryMachineId) -> &[usize] {
        self.bindings
            .get(&boundary)
            .map_or(&[], |binding| binding.adapter.storage_arguments(boundary))
    }

    fn handle_effect_with_buffers(
        &mut self,
        effect: &TerminalEffect,
        byte_buffers: &mut [TerminalBoundaryByteBuffer],
        storage_buffers: &mut [TerminalBoundaryStorageBuffer],
    ) -> Result<TerminalEffectResult, TerminalEffectRejection> {
        match effect {
            TerminalEffect::BoundaryCall { boundary, .. } => self
                .adapter_at(*boundary)?
                .handle_effect_with_buffers(effect, byte_buffers, storage_buffers),
            TerminalEffect::PortWrite { .. } => Err(TerminalEffectRejection::new(
                "port output requires an admitted target-environment provider",
            )),
        }
    }

    fn handle_effect(&mut self, effect: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        match effect {
            TerminalEffect::BoundaryCall { boundary, .. } => {
                self.adapter_at(*boundary)?.handle_effect(effect)
            }
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
                .adapter_at(*boundary)?
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
            TerminalEffect::BoundaryCall { boundary, .. } => {
                self.adapter_at(*boundary)?.handle_effect_result(effect)
            }
            TerminalEffect::PortWrite { .. } => Err(TerminalEffectRejection::new(
                "port output requires an admitted target-environment provider",
            )),
        }
    }
}
