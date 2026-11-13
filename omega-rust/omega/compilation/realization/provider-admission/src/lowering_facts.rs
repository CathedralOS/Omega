// Admitted facts read by target lowering, and sealed callback and interrupt
// cohorts.
//
// Terminal-to-abstract lowering (Omega 00) keeps these facts in its sealed
// verified input. Abstract optimization (Omega 01) hands them to target
// lowering (Omega 02) as `AdmittedLoweringFacts` so target lowering and the
// backends after it read them without the full input. Native callback arguments
// and installed interrupt entries stay sealed: each cohort is minted only by
// provider admission, bound to the Terminal program it was admitted for, and
// verified by its consumers.

use installation_evidence::InstalledAssumedBindingEvidence;
use semantic_vocabulary::{BoundaryMachineId, MachineId};
use terminal_installation::AdmittedTargetMachineSelections;
use terminal_psi::{TerminalModule, TerminalPsiIdentity};

use crate::{AdmittedInstalledInterruptEntry, AdmittedNativeCallbackArgument};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedNativeCallbackCohort {
    pub(crate) psi: TerminalPsiIdentity,
    pub(crate) callbacks: Vec<AdmittedNativeCallbackArgument>,
}

impl AdmittedNativeCallbackCohort {
    pub const fn empty(psi: TerminalPsiIdentity) -> Self {
        Self {
            psi,
            callbacks: Vec::new(),
        }
    }

    pub const fn psi(&self) -> TerminalPsiIdentity {
        self.psi
    }

    pub fn callbacks(&self) -> &[AdmittedNativeCallbackArgument] {
        &self.callbacks
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedInstalledInterruptCohort {
    pub(crate) psi: TerminalPsiIdentity,
    pub(crate) entries: Vec<AdmittedInstalledInterruptEntry>,
    pub(crate) scopes: Vec<crate::AdmittedInstalledInterruptScope>,
    pub(crate) helpers: Vec<crate::AdmittedInstalledInterruptHelper>,
}

impl AdmittedInstalledInterruptCohort {
    pub const fn empty(psi: TerminalPsiIdentity) -> Self {
        Self {
            psi,
            entries: Vec::new(),
            scopes: Vec::new(),
            helpers: Vec::new(),
        }
    }

    pub const fn psi(&self) -> TerminalPsiIdentity {
        self.psi
    }

    pub fn entries(&self) -> &[AdmittedInstalledInterruptEntry] {
        &self.entries
    }

    pub fn scopes(&self) -> &[crate::AdmittedInstalledInterruptScope] {
        &self.scopes
    }

    pub fn helpers(&self) -> &[crate::AdmittedInstalledInterruptHelper] {
        &self.helpers
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LoweringEntryCall {
    pub boundary: BoundaryMachineId,
    pub entry: MachineId,
    pub source_signature: [u8; 32],
}

impl LoweringEntryCall {
    pub const fn boundary(&self) -> BoundaryMachineId {
        self.boundary
    }

    pub const fn entry(&self) -> MachineId {
        self.entry
    }

    pub const fn source_signature(&self) -> [u8; 32] {
        self.source_signature
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedLoweringFacts {
    pub module: TerminalModule,
    pub program_entry: Option<LoweringEntryCall>,
    pub assumed_bindings: Vec<InstalledAssumedBindingEvidence>,
    pub target_machine_selections: Option<AdmittedTargetMachineSelections>,
    pub native_callbacks: AdmittedNativeCallbackCohort,
    pub installed_interrupts: AdmittedInstalledInterruptCohort,
}

impl AdmittedLoweringFacts {
    /// Retain private entry bindings at their real registrar operations. The call
    /// graph activates these rows only when execution reaches the registrar.
    pub fn callable_invocation_roots(
        &self,
        target: &target_operations::TargetOperationPlan,
    ) -> Result<terminal_semantics::callable_values::CallableInvocationRoots, String> {
        let module = self.module();
        let mut roots = terminal_semantics::callable_values::CallableInvocationRoots::new();
        for machine in &module.machines {
            for operation in machine.blocks.iter().flat_map(|block| &block.operations) {
                if !matches!(
                    operation.kind,
                    terminal_psi::OperationKind::BoundaryCall { .. }
                ) {
                    continue;
                }
                let mut entries =
                    terminal_semantics::callable_values::installed_interrupt_helper_roots(
                        module,
                        operation.id,
                    )?
                    .into_iter()
                    .collect::<std::collections::BTreeSet<_>>();
                entries.extend(
                    self.native_callbacks()
                        .callbacks()
                        .iter()
                        .filter(|callback| callback.terminal_operation == operation.id)
                        .map(|callback| callback.entry().machine()),
                );
                entries.extend(
                    self.installed_interrupts()
                        .entries()
                        .iter()
                        .filter(|entry| entry.operation == operation.id)
                        .map(|entry| entry.entry().machine()),
                );
                if !entries.is_empty() {
                    roots.insert((machine.id, operation.id), entries);
                }
            }
        }
        // Target lowering retains the admitted selected body for each boundary
        // invocation. Activate that body at this exact source operation, while its
        // boundary inputs remain external inputs rather than invented actual facts.
        for function in &target.functions {
            for operation in function.graph.operations() {
                let target_operations::TargetUnitOperation::Call {
                    origin,
                    psi_operation,
                    target: target_operations::NativeCallTarget::Machine(callee),
                    ..
                } = operation
                else {
                    continue;
                };
                let boundary = match origin {
                    target_operations::NativeCallOrigin::Authored => continue,
                    target_operations::NativeCallOrigin::ProgramEntry { boundary, .. }
                    | target_operations::NativeCallOrigin::TargetFamily { boundary, .. }
                    | target_operations::NativeCallOrigin::InstalledProvider { boundary, .. } => {
                        *boundary
                    }
                };
                let owner = module
                    .machine(function.machine)
                    .ok_or("selected native boundary caller is absent from the retained module")?;
                let mut sources = owner
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .filter(|source| source.id == *psi_operation);
                let (Some(source), None) = (sources.next(), sources.next()) else {
                    return Err(
                        "selected native boundary call has no exact source occurrence".into(),
                    );
                };
                if !matches!(source.kind, terminal_psi::OperationKind::BoundaryCall { boundary: found, .. } if found == boundary)
                    || module.machine(*callee).is_none()
                {
                    return Err(
                        "selected native boundary body does not rejoin its admitted source call"
                            .into(),
                    );
                }
                roots
                    .entry((function.machine, *psi_operation))
                    .or_default()
                    .insert(*callee);
            }
        }
        Ok(roots)
    }

    pub const fn module(&self) -> &TerminalModule {
        &self.module
    }

    pub const fn program_entry(&self) -> Option<&LoweringEntryCall> {
        self.program_entry.as_ref()
    }

    pub fn assumed_bindings(&self) -> &[InstalledAssumedBindingEvidence] {
        &self.assumed_bindings
    }

    pub const fn target_machine_selections(&self) -> Option<&AdmittedTargetMachineSelections> {
        self.target_machine_selections.as_ref()
    }

    pub const fn installed_interrupts(&self) -> &AdmittedInstalledInterruptCohort {
        &self.installed_interrupts
    }

    pub const fn native_callbacks(&self) -> &AdmittedNativeCallbackCohort {
        &self.native_callbacks
    }
}
