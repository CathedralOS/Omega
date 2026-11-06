//! Compare an emitted private entry with the recipe sealed before emission.
//! The original root and its private bodies retain their own admitted callback
//! rows. Public object metadata cannot replace those rows during this audit.
use crate::{
    FunctionFragmentObjectArtifactError as Error, ObjectArtifact, ObjectCompilerPrivateFunction,
};
use function_identity::MachineFunctionIdentity;
use provider_admission::AdmittedCallbackEntry;
use runtime_abi::static_interrupts::{InstalledInterruptEntryPlan, InstalledInterruptFrame};

type EntryOrigin<'a> = (
    MachineFunctionIdentity,
    &'a AdmittedCallbackEntry,
    bool,
    Option<InstalledInterruptEntryPlan>,
    Option<&'a installation_evidence::BootstrapEnvironmentAssumption>,
    Option<&'a provider_admission::AdmittedInstalledInterruptScope>,
);

pub(crate) struct EntryOrigins<'a> {
    entries: Vec<EntryOrigin<'a>>,
}
impl<'a> EntryOrigins<'a> {
    pub(crate) fn new(artifact: &'a ObjectArtifact) -> Result<Self, Error> {
        if artifact.installed_entry_execution().is_some() {
            return Err(Error::Mismatch(
                "an installed private execution domain cannot be an image root",
            ));
        }
        let mut entries = Vec::new();
        let source = artifact.composition_source.as_deref().unwrap_or(artifact);
        collect(source, &mut entries)?;
        for private in &artifact.private_functions {
            collect(&private.body, &mut entries)?;
        }
        entries.sort_unstable_by_key(|row| row.0.to_bytes());
        let mut merged: Vec<EntryOrigin<'a>> = Vec::with_capacity(entries.len());
        for row in entries {
            if let Some(prior) = merged.last_mut().filter(|prior| prior.0 == row.0) {
                if prior.1 != row.1
                    || prior.2 != row.2
                    || matches!((prior.3, row.3), (Some(left), Some(right)) if left != right)
                    || matches!((prior.4, row.4), (Some(left), Some(right)) if left != right)
                    || matches!((prior.5, row.5), (Some(left), Some(right)) if left != right)
                {
                    return Err(Error::Mismatch(
                        "private entry has conflicting original admission recipes",
                    ));
                }
                if prior.5.is_none() {
                    prior.5 = row.5;
                }
                if prior.4.is_none() {
                    prior.4 = row.4;
                }
                if prior.3.is_none() {
                    prior.3 = row.3;
                }
            } else {
                merged.push(row);
            }
        }
        let entries = merged;
        Ok(Self { entries })
    }
    pub(crate) fn verify(&self, private: &ObjectCompilerPrivateFunction) -> Result<(), Error> {
        let index = self
            .entries
            .binary_search_by_key(&private.identity.to_bytes(), |row| row.0.to_bytes())
            .map_err(|_| {
                Error::Mismatch("private entry has no original admitted callback recipe")
            })?;
        let (_, entry, installed, plan, assumption, parent_scope) = self.entries[index];
        let execution = private.body.installed_entry_execution();
        if installed != execution.is_some()
            || execution.is_some_and(|execution| {
                execution.entry().callback_function != private.identity
                    || execution.entry().entry() != entry
                    || Some(execution.scope()) != parent_scope
                    || execution.scope().bootstrap_assumption() != assumption
                    || execution.scope().operation() != execution.entry().operation
            })
        {
            return Err(Error::Mismatch(
                "private entry changed its retained privileged execution domain",
            ));
        }
        if installed != assumption.is_some() {
            return Err(Error::Mismatch(
                "installed entry lost its parent bootstrap assumption",
            ));
        }
        if private.identity.caller() != entry.psi()
            || private.source_psi != entry.psi()
            || private.body.psi() != entry.psi()
            || private.body.target() != entry.target()
            || private.body.entry() != entry.machine()
            || private.function.machine != entry.machine()
            || private.inbound != *entry.inbound()
            || private.inbound_ordinals != entry.inbound_ordinals()
            || private
                .entry_bridge
                .as_ref()
                .and_then(|bridge| bridge.installed_interrupt())
                != plan
            || installed != plan.is_some()
        {
            return Err(Error::Mismatch(
                "private entry metadata differs from its original admitted callback recipe",
            ));
        }
        Ok(())
    }
}
fn collect<'a>(body: &'a ObjectArtifact, entries: &mut Vec<EntryOrigin<'a>>) -> Result<(), Error> {
    let source = body.fragment_replay.as_ref().ok_or(Error::Mismatch(
        "private entry origins require retained admission input",
    ))?;
    let facts = source.0.lowering_facts();
    let ordinary = facts.native_callbacks();
    let installed = facts.installed_interrupts();
    if ordinary.psi() != body.psi() || installed.psi() != body.psi() {
        return Err(Error::Mismatch(
            "private entry cohort belongs to another program",
        ));
    }
    for row in ordinary.callbacks() {
        entries.push((row.callback_function, row.entry(), false, None, None, None));
    }
    for row in installed.entries() {
        let mut scopes = installed
            .scopes()
            .iter()
            .filter(|scope| scope.operation() == row.operation);
        let scope = scopes.next();
        if scopes.next().is_some() {
            return Err(Error::Mismatch(
                "private entry has duplicate installation scopes",
            ));
        }
        let assumption = scope.and_then(|scope| scope.bootstrap_assumption());
        let plan = if let Some(scope) = scope.filter(|scope| scope.context_anchor().is_some()) {
            let assumption = assumption.ok_or(Error::Mismatch(
                "installation lost its explicit bootstrap environment assumption",
            ))?;
            if assumption.psi != body.psi()
                || assumption.installation != scope.operation()
                || assumption.profile.native_target() != body.target()
            {
                return Err(Error::Mismatch(
                    "installation bootstrap assumption changed program, target or scope",
                ));
            }
            assumption.validate_description().map_err(Error::Mismatch)?;
            for condition in scope.installation_assumptions() {
                condition.validate_description().map_err(Error::Mismatch)?;
                if condition.psi() != body.psi()
                    || condition.installation() != scope.operation()
                    || condition.profile().native_target() != body.target()
                    || condition.entry() != assumption.entry
                {
                    return Err(Error::Mismatch(
                        "installation delivery condition changed its exact parent interval",
                    ));
                }
            }
            if !scope.fatal_entries().is_empty() && scope.interrupt_delivery_assumption().is_none()
            {
                return Err(Error::Mismatch(
                    "fatal private entry lost its installed delivery condition",
                ));
            }
            let anchor = scope.context_anchor().ok_or(Error::Mismatch(
                "installation lost its retained context anchor",
            ))?;
            if anchor.application() != scope.acknowledgement_application()
                || anchor.function().caller() != body.psi()
            {
                return Err(Error::Mismatch(
                    "private entry context anchor changed its program or designated callback",
                ));
            }
            if row.placement.application_index == anchor.application() {
                if row.callback_function != anchor.function() {
                    return Err(Error::Mismatch(
                        "timer entry changed its anchor provider identity",
                    ));
                }
                Some(InstalledInterruptEntryPlan::new(
                    InstalledInterruptFrame::NoErrorCode,
                ))
            } else {
                let mut matching = scope.fatal_entries().iter().filter(|fatal| {
                    fatal.application() == row.placement.application_index
                        && fatal.machine() == row.entry().machine()
                });
                let (Some(fatal), None) = (matching.next(), matching.next()) else {
                    return Err(Error::Mismatch(
                        "private entry is absent from the admitted fatal roster",
                    ));
                };
                Some(
                    InstalledInterruptEntryPlan::fatal_context(
                        anchor.gate_byte_offset(),
                        fatal.gate_byte_offset(),
                        anchor.table_bytes(),
                    )
                    .ok_or(Error::Mismatch(
                        "fatal entry changed its admitted gate geometry",
                    ))?,
                )
            }
        } else {
            // Private reentry retains its own callback only. Final composition
            // must also supply the executing parent's complete installation.
            None
        };
        entries.push((
            row.callback_function,
            row.entry(),
            true,
            plan,
            assumption,
            scope.filter(|scope| scope.context_anchor().is_some()),
        ));
    }
    Ok(())
}
