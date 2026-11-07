//! Generated private entries must still match their original handler and
//! installation. A returning handler can retain only its own entry and table
//! anchor; its sealed execution also retains the original complete parent.
//! Final composition checks those exact original rows before publishing code.
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
    OriginOwners<'a>,
);

#[derive(Clone, Copy)]
struct OriginOwners<'a> {
    entry: &'a ObjectArtifact,
    plan: Option<&'a ObjectArtifact>,
    assumption: Option<&'a ObjectArtifact>,
    scope: Option<&'a ObjectArtifact>,
}

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
                    return Err(Error::PrivateEntryRecipeConflict {
                        identity: row.0,
                        detail: conflicting_recipes(prior, &row),
                    });
                }
                if prior.5.is_none() {
                    prior.5 = row.5;
                    prior.6.scope = row.6.scope;
                }
                if prior.4.is_none() {
                    prior.4 = row.4;
                    prior.6.assumption = row.6.assumption;
                }
                if prior.3.is_none() {
                    prior.3 = row.3;
                    prior.6.plan = row.6.plan;
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
        let (_, entry, installed, plan, assumption, parent_scope, _) = self.entries[index];
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
        entries.push((
            row.callback_function,
            row.entry(),
            false,
            None,
            None,
            None,
            OriginOwners {
                entry: body,
                plan: None,
                assumption: None,
                scope: None,
            },
        ));
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
        // A returning handler's own facts omit the parent's fatal handlers.
        // Use its retained complete parent only for the exact same admitted
        // callback. Other or nested installations keep their own source scope.
        let scope = if let Some(execution) = body.installed_entry_execution()
            && execution.entry().callback_function == row.callback_function
        {
            if execution.entry() != row
                || execution.scope().operation() != row.operation
                || row.entry().psi() != body.psi()
                || row.entry().target() != body.target()
                || row.entry().machine() != body.entry()
                || scope.is_none()
            {
                return Err(Error::Mismatch(
                    "private handler changed its original installation or admitted entry",
                ));
            }
            execution
                .require_current(installed)
                .map_err(Error::Mismatch)?;
            Some(execution.scope())
        } else {
            scope
        };
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
            OriginOwners {
                entry: body,
                plan: plan.map(|_| body),
                assumption: assumption.map(|_| body),
                scope: scope
                    .filter(|scope| scope.context_anchor().is_some())
                    .map(|_| body),
            },
        ));
    }
    Ok(())
}

/// Report only the actual fields which stopped the merge. Both source bodies
/// stay attached so equal local machine numbers do not hide their context.
fn conflicting_recipes(left: &EntryOrigin<'_>, right: &EntryOrigin<'_>) -> String {
    let mut differences = Vec::new();
    if left.1 != right.1 {
        differences.push(format!(
            "callback entry: left={:?}; right={:?}",
            left.1, right.1
        ));
    }
    if left.2 != right.2 {
        differences.push(format!(
            "installed role: left={}; right={}",
            left.2, right.2
        ));
    }
    if matches!((left.3, right.3), (Some(a), Some(b)) if a != b) {
        differences.push(format!(
            "installed plan: left={:?}; right={:?}",
            left.3, right.3
        ));
    }
    if matches!((left.4, right.4), (Some(a), Some(b)) if a != b) {
        differences.push(format!(
            "bootstrap assumption: left={:?}; right={:?}",
            left.4, right.4
        ));
    }
    if matches!((left.5, right.5), (Some(a), Some(b)) if a != b) {
        differences.push(format!(
            "parent scope: left={:?}; right={:?}",
            left.5, right.5
        ));
    }
    let owner = |body: &ObjectArtifact| {
        format!(
            "root={:?}, psi={:?}, target={:?}, profile={:?}, installed_execution={:?}",
            body.entry(),
            body.psi(),
            body.target(),
            body.x86_feature_profile(),
            body.installed_entry_execution().map(|execution| (
                execution.entry().callback_function,
                execution.scope().operation(),
            )),
        )
    };
    differences.push(format!(
        "merged optional owners: left plan={:?}, assumption={:?}, scope={:?}; right plan={:?}, assumption={:?}, scope={:?}",
        left.6.plan.map(owner), left.6.assumption.map(owner), left.6.scope.map(owner),
        right.6.plan.map(owner), right.6.assumption.map(owner), right.6.scope.map(owner),
    ));
    format!(
        "left source [{}]; right source [{}]; {}",
        owner(left.6.entry),
        owner(right.6.entry),
        differences.join("; "),
    )
}
