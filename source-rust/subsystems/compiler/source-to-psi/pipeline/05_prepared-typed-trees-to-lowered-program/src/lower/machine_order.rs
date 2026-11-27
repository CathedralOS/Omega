//! Plan the machines of a module on the invocation's workers while keeping
//! exactly what planning and applying them one at a time, in order, yields.
//!
//! Storage and claim publication plan machine `k` against the module that
//! already holds the plans applied to the machines before it, so the loop
//! looks sequential. But applying a plan changes only the machine it was made
//! for, and most applications leave that machine as it was. A plan made in
//! advance against the starting module is the plan the loop would make as long
//! as no machine it read has changed by the time its turn comes.
//!
//! Both plans are affine ownership walks, and each walk replays the summaries
//! of the callees its machine reaches. Walks over one unchanged module share
//! one `ModuleWalks` round: the module's `ModuleAnalyses` bundle, whose
//! borrowed-storage rows, call fences, type permissions and callee summaries
//! every walk reads. The first application that changes a machine ends that
//! round, and later walks start another.

use semantic_vocabulary::MachineId;
use terminal_check_tables::program::{MachineSources, SourceOrigin};
use terminal_check_tables::{Construct, RefusalReason};
use terminal_psi::{TerminalMachine, TerminalModule};
use terminal_semantics::ModuleAnalyses;
use terminal_semantics::plain_affine::{self, MachineAffine, Site};

/// What every affine walk over one unchanged module shares. A round borrows
/// the module, so nothing can edit the module while the round exists. A pass
/// that edits the module drops its round first and builds a new one for the
/// module it leaves.
pub(crate) struct ModuleWalks<'m> {
    module: &'m TerminalModule,
    /// `None` when the module repeats a machine identity, which the bundle
    /// refuses. Each walk then builds its own closure, which refuses that
    /// module with the same error whenever the walk needs the closure, so
    /// there are no summaries to share.
    analyses: Option<ModuleAnalyses<'m>>,
}

impl<'m> ModuleWalks<'m> {
    pub(crate) fn new(module: &'m TerminalModule) -> Self {
        Self {
            module,
            analyses: ModuleAnalyses::new(module).ok(),
        }
    }

    pub(crate) fn module(&self) -> &'m TerminalModule {
        self.module
    }

    #[cfg(test)]
    pub(crate) fn analyses(&self) -> Option<&ModuleAnalyses<'m>> {
        self.analyses.as_ref()
    }

    /// The affine walks of `machine`. They read the machine's
    /// borrowed-storage closure and callee summaries from the round's bundle.
    pub(crate) fn affine(
        &self,
        machine: MachineId,
    ) -> Result<MachineAffine<'_>, plain_affine::Error> {
        match &self.analyses {
            Some(analyses) => analyses.affine(machine),
            None => MachineAffine::new(self.module, machine),
        }
    }

    /// The reference-end candidates of every machine. Finding them replays
    /// callees into the round's summaries, which the walks then reuse.
    pub(crate) fn reference_end_candidates(
        &self,
    ) -> Result<
        arena::IdTable<MachineId, terminal_semantics::reference_ends::Candidates>,
        terminal_semantics::reference_ends::Error,
    > {
        use terminal_semantics::reference_ends::{candidates, candidates_in};
        match &self.analyses {
            Some(analyses) => candidates_in(analyses),
            None => candidates(self.module),
        }
    }
}

/// Plan every machine of `module` and apply each plan to its machine in
/// machine order, with the result of the loop that plans machine `k` only
/// after applying the plans of the machines before it.
///
/// The machines are planned in advance on the invocation's workers against
/// the module as it stands, then applied in order. `reads` names the other
/// machines whose state, as `apply` can change it, a plan read; `None` when
/// that is unknown, which counts as every machine. When one of them has
/// changed before the plan's turn, the machine is planned again against the
/// module as it is then. `apply` may change only the machine it is given. The
/// first error `apply` returns ends the loop, as it would in order.
///
/// `plan` receives the `ModuleWalks` round of the module it plans against,
/// which every plan against that same unchanged module shares.
#[cfg(test)]
pub(crate) fn plan_then_apply<Plan: Send, Error>(
    module: &mut TerminalModule,
    plan: impl Fn(&ModuleWalks<'_>, &TerminalMachine) -> Plan + Sync,
    reads: impl Fn(&Plan) -> Option<&[MachineId]>,
    apply: impl FnMut(&mut TerminalMachine, Plan) -> Result<bool, Error>,
) -> Result<(), Error> {
    plan_then_apply_with(
        module,
        worker_budget::current_workers() > 1,
        plan,
        reads,
        apply,
    )
}

pub(crate) fn plan_then_apply_with<Plan: Send, Error>(
    module: &mut TerminalModule,
    parallel: bool,
    plan: impl Fn(&ModuleWalks<'_>, &TerminalMachine) -> Plan + Sync,
    reads: impl Fn(&Plan) -> Option<&[MachineId]>,
    mut apply: impl FnMut(&mut TerminalMachine, Plan) -> Result<bool, Error>,
) -> Result<(), Error> {
    let count = module.machines.len();
    let advance: Vec<Option<Plan>> = if parallel && count >= 2 {
        use rayon::prelude::*;
        let module = &*module;
        let walks = ModuleWalks::new(module);
        module
            .machines
            .par_iter()
            .with_max_len(1)
            .map(|machine| Some(plan(&walks, machine)))
            .collect()
    } else {
        (0..count).map(|_| None).collect()
    };
    plan_then_apply_from_advance(module, advance, plan, reads, |machine, _, planned| {
        apply(machine, planned)
    })
}

pub(crate) fn plan_then_apply_from_advance<Plan: Send, Error>(
    module: &mut TerminalModule,
    mut advance: Vec<Option<Plan>>,
    plan: impl Fn(&ModuleWalks<'_>, &TerminalMachine) -> Plan + Sync,
    reads: impl Fn(&Plan) -> Option<&[MachineId]>,
    mut apply: impl FnMut(
        &mut TerminalMachine,
        &[terminal_psi::StructuralTypeDeclaration],
        Plan,
    ) -> Result<bool, Error>,
) -> Result<(), Error> {
    let count = module.machines.len();
    // The machines an application changed so far, sorted.
    let mut changed: Vec<MachineId> = Vec::new();
    let mut ordinal = 0;
    while ordinal < count {
        // While no re-plan has needed a `ModuleWalks` round in this
        // epoch, `module` stays unborrowed so pre-planned `advance` hits apply
        // directly in place without cloning `TerminalMachine`.
        while ordinal < count {
            let machine = &module.machines[ordinal];
            let unchanged = |id: &MachineId| changed.binary_search(id).is_err();
            let usable = advance[ordinal].as_ref().is_some_and(|item| {
                changed.is_empty()
                    || unchanged(&machine.id)
                        && reads(item).is_some_and(|read| read.iter().all(unchanged))
            });
            if !usable {
                break;
            }
            let next = advance[ordinal]
                .take()
                .expect("usable advance entry is present");
            let id = machine.id;
            ordinal += 1;
            if apply(
                &mut module.machines[ordinal - 1],
                &module.types.structural_types,
                next,
            )? && let Err(position) = changed.binary_search(&id)
            {
                changed.insert(position, id);
            }
        }
        if ordinal >= count {
            break;
        }
        // Until an application changes a machine, the module stays as it is
        // here, so the walks planned again share one `ModuleWalks` round.
        let edit = {
            let module = &*module;
            let walks = ModuleWalks::new(module);
            loop {
                let Some(machine) = module.machines.get(ordinal) else {
                    break None;
                };
                // A plan finds its own machine by identity too, so that
                // identity counts among what it read.
                let unchanged = |id: &MachineId| changed.binary_search(id).is_err();
                let next = match advance[ordinal].take() {
                    Some(advance)
                        if changed.is_empty()
                            || unchanged(&machine.id)
                                && reads(&advance)
                                    .is_some_and(|read| read.iter().all(unchanged)) =>
                    {
                        advance
                    }
                    _ => plan(&walks, machine),
                };
                let mut applied = machine.clone();
                let mutated = apply(&mut applied, &module.types.structural_types, next)?;
                ordinal += 1;
                if mutated {
                    break Some(applied);
                }
            }
        };
        if let Some(applied) = edit {
            let id = applied.id;
            module.machines[ordinal - 1] = applied;
            if let Err(position) = changed.binary_search(&id) {
                changed.insert(position, id);
            }
        }
    }
    Ok(())
}

/// How every caller reports an ownership walk that refused: the construct the
/// error names, why the walk refused and the walk's own message. Storage
/// planning, claim transport and reference ends each add their own context to
/// `detail`, but the reason for each error kind is the same everywhere.
pub(crate) struct WalkRefusal {
    pub(crate) concerns: Construct,
    pub(crate) reason: RefusalReason,
    pub(crate) detail: &'static str,
}

impl WalkRefusal {
    pub(crate) fn new(error: &plain_affine::Error) -> Self {
        use plain_affine::Error;
        let concerns = match error.site() {
            Some(Site::Operation(at)) => Construct::Operation(at),
            Some(Site::Edge(at)) => Construct::Edge(at),
            None => Construct::Machine,
        };
        let (reason, detail) = match error {
            Error::Unavailable(unavailable) => {
                (RefusalReason::Unestablished, unavailable.message())
            }
            Error::Unconsumed { at_join, .. } => (
                RefusalReason::Violation,
                Error::unconsumed_message(*at_join),
            ),
            Error::MissingActiveCase { .. } => (
                RefusalReason::Unsupported,
                Error::missing_active_case_message(),
            ),
            Error::Unsupported { detail, .. } => (RefusalReason::Unsupported, *detail),
            Error::Malformed { detail, .. } => (RefusalReason::ForeignIdentity, *detail),
        };
        Self {
            concerns,
            reason,
            detail,
        }
    }

    /// Where the source wrote the operation or edge the error names.
    pub(crate) fn origin(&self, sources: Option<&MachineSources>) -> Option<SourceOrigin> {
        let sources = sources?;
        match self.concerns {
            Construct::Operation(at) => sources.operations.get(at).copied(),
            Construct::Edge(at) => sources.edges.get(at).copied(),
            _ => None,
        }
    }
}
