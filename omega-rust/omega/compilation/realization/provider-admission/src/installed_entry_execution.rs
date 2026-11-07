//! Execution of one private body through its installing Scope.
//!
//! The installed entry runs at CPL0 only after the parent's CPU check and
//! publication. This condition belongs to the private object, not to every
//! occurrence of the same source machine. Final composition must retain and
//! check the same installing Scope and entry before exposing this body.

use std::sync::Arc;

use crate::{
    AdmittedInstalledInterruptCohort, AdmittedInstalledInterruptEntry,
    AdmittedInstalledInterruptScope,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdmittedInstalledEntryExecution {
    scope: Arc<AdmittedInstalledInterruptScope>,
    entry: AdmittedInstalledInterruptEntry,
}

impl AdmittedInstalledEntryExecution {
    pub fn for_entry(
        scope: Arc<AdmittedInstalledInterruptScope>,
        entry: &AdmittedInstalledInterruptEntry,
        current: &AdmittedInstalledInterruptCohort,
    ) -> Result<Self, &'static str> {
        Self::check_current(&scope, entry, current)?;
        Ok(Self {
            scope,
            entry: entry.clone(),
        })
    }

    /// A private handler can retain only part of its original installation.
    /// Check that the current records still agree with that exact parent before
    /// using the complete installation retained by this execution.
    pub fn require_current(
        &self,
        current: &AdmittedInstalledInterruptCohort,
    ) -> Result<(), &'static str> {
        Self::check_current(&self.scope, &self.entry, current)
    }

    fn check_current(
        scope: &AdmittedInstalledInterruptScope,
        entry: &AdmittedInstalledInterruptEntry,
        current: &AdmittedInstalledInterruptCohort,
    ) -> Result<(), &'static str> {
        if current.psi() != entry.entry().psi()
            || entry.operation != scope.operation()
            || entry.entry().target().architecture != target::Architecture::X86_64
            || (!scope.fatal_entries().is_empty()
                && scope.interrupt_delivery_assumption().is_none())
            || scope.bootstrap_assumption().is_none_or(|assumption| {
                assumption.psi != current.psi() || assumption.installation != scope.operation()
            })
        {
            return Err("installed execution changed its program, target or parent interval");
        }
        let mut entries = current
            .entries()
            .iter()
            .filter(|candidate| candidate.placement_index == entry.placement_index);
        if entries.next() != Some(entry) || entries.next().is_some() {
            return Err("installed execution has no exact unique current entry");
        }
        let mut scopes = current
            .scopes()
            .iter()
            .filter(|candidate| candidate.operation() == scope.operation());
        let Some(received) = scopes.next() else {
            return Err("installed execution lost its current installation");
        };
        if scopes.next().is_some()
            || received.service() != scope.service()
            || received.boundary() != scope.boundary()
            || received.arguments() != scope.arguments()
            || received.table_type() != scope.table_type()
            || received.context_type() != scope.context_type()
            || received.validator() != scope.validator()
            || received.work() != scope.work()
            || received.completion() != scope.completion()
            || received.acknowledgement_application() != scope.acknowledgement_application()
            || received
                .fatal_entries()
                .iter()
                .any(|entry| !scope.fatal_entries().contains(entry))
            || received
                .context_anchor()
                .is_some_and(|anchor| Some(anchor) != scope.context_anchor())
            || (!received.installation_assumptions().is_empty()
                && received.installation_assumptions() != scope.installation_assumptions())
        {
            return Err("installed execution substituted its parent installation");
        }
        Ok(())
    }

    pub fn scope(&self) -> &AdmittedInstalledInterruptScope {
        &self.scope
    }

    pub fn entry(&self) -> &AdmittedInstalledInterruptEntry {
        &self.entry
    }
}
