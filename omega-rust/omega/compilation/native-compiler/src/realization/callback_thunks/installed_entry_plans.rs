//! Carry the executing installation's admitted entry recipe into each private
//! body. Reentry checks only that body; it does not admit sibling callbacks.

use backend_plan::NativeCallbackThunkSettlement;
use diagnostics::Diagnostic;
use runtime_abi::static_interrupts::{InstalledInterruptEntryPlan, InstalledInterruptFrame};
use std::sync::Arc;
use terminal_psi_to_abstract_operations::provider_admission::{
    AdmittedInstalledInterruptCohort, AdmittedInstalledInterruptEntry,
    AdmittedInstalledInterruptScope, realization_error,
};

#[derive(Clone, PartialEq, Eq)]
struct PreparedEntry {
    scope: Arc<AdmittedInstalledInterruptScope>,
    entry: AdmittedInstalledInterruptEntry,
    plan: InstalledInterruptEntryPlan,
}

pub(super) struct PreparedEntryPlans {
    entries: Vec<Option<PreparedEntry>>,
}

impl PreparedEntryPlans {
    pub(super) fn new(count: usize) -> Self {
        Self {
            entries: vec![None; count],
        }
    }

    pub(super) fn retain(
        &mut self,
        scope: &AdmittedInstalledInterruptScope,
        entries: &[&AdmittedInstalledInterruptEntry],
    ) -> Result<(), Vec<Diagnostic>> {
        let fail = |reason| realization_error("installed entry preparation", reason);
        let anchor = scope
            .context_anchor()
            .ok_or_else(|| fail("executing installation has no retained timer anchor"))?;
        let mut matching = entries
            .iter()
            .filter(|entry| entry.placement.application_index == anchor.application());
        let (Some(timer), None) = (matching.next(), matching.next()) else {
            return Err(fail("timer anchor has no unique admitted callback"));
        };
        if timer.callback_function != anchor.function()
            || timer.operation != scope.operation()
            || timer.placement.application_index != scope.acknowledgement_application()
        {
            return Err(fail(
                "timer anchor differs from its admitted installation entry",
            ));
        }
        let scope = Arc::new(scope.clone());
        for entry in entries {
            if entry.operation != scope.operation()
                || entry.entry().psi() != timer.entry().psi()
                || entry.entry().target() != timer.entry().target()
            {
                return Err(fail(
                    "installed entry changed its parent program, target or operation",
                ));
            }
            let plan = if entry.placement.application_index == scope.acknowledgement_application() {
                InstalledInterruptEntryPlan::new(InstalledInterruptFrame::NoErrorCode)
            } else {
                let mut matching = scope.fatal_entries().iter().filter(|fatal| {
                    fatal.application() == entry.placement.application_index
                        && fatal.machine() == entry.entry().machine()
                });
                let (Some(fatal), None) = (matching.next(), matching.next()) else {
                    return Err(fail("private entry has no exact admitted fatal handler"));
                };
                InstalledInterruptEntryPlan::fatal_context(
                    anchor.gate_byte_offset(),
                    fatal.gate_byte_offset(),
                    anchor.table_bytes(),
                )
                .ok_or_else(|| fail("fatal context has invalid retained gate geometry"))?
            };
            let retained = PreparedEntry {
                scope: Arc::clone(&scope),
                entry: (*entry).clone(),
                plan,
            };
            let slot = self
                .entries
                .get_mut(entry.placement_index)
                .ok_or_else(|| fail("installed placement exceeds the original callback roster"))?;
            if slot.as_ref().is_some_and(|prior| prior != &retained) {
                return Err(fail(
                    "one private placement has incompatible parent installation evidence",
                ));
            }
            *slot = Some(retained);
        }
        Ok(())
    }

    pub(super) fn for_body(
        &self,
        settlement: &NativeCallbackThunkSettlement<'_>,
        cohort: &AdmittedInstalledInterruptCohort,
    ) -> Result<
        Option<(
            InstalledInterruptEntryPlan,
            terminal_psi_to_abstract_operations::AdmittedInstalledEntryExecution,
        )>,
        Vec<Diagnostic>,
    > {
        let fail = |reason| realization_error("installed private entry", reason);
        let mut matching = cohort
            .entries()
            .iter()
            .filter(|entry| entry.placement_index == settlement.placement_index);
        let entry = matching.next();
        if matching.next().is_some() {
            return Err(fail(
                "private body has duplicate admitted placement identities",
            ));
        }
        let retained = self
            .entries
            .get(settlement.placement_index)
            .ok_or_else(|| fail("private placement exceeds the original callback roster"))?;
        match (entry, retained) {
            (None, None) => Ok(None),
            (Some(entry), Some(retained))
                if *entry == retained.entry
                    && entry.entry().psi() == settlement.artifact.view().identity()
                    && entry.entry().machine() == settlement.entry
                    && entry.operation == settlement.terminal_operation
                    && entry.callback_function == settlement.callback_function
                    && retained.scope.operation() == entry.operation =>
            {
                let execution = terminal_psi_to_abstract_operations::AdmittedInstalledEntryExecution::for_entry(
                    Arc::clone(&retained.scope),
                    entry,
                    cohort,
                ).map_err(fail)?;
                Ok(Some((retained.plan, execution)))
            }
            _ => Err(fail(
                "private entry changed its original installation, artifact or callback association",
            )),
        }
    }
}
