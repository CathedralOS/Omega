//! Check every call: its write must not conflict with the saved reference.
//! If the arguments still depend on the caller's inputs, check its callers in
//! turn. A recursive call cycle cannot supply its own proof.
use crate::checks::borrows::loan_occurrences::LoanOccurrences;
use checked_trees::{BorrowAccessKind, CheckFacts};
use symbols::SymbolHandle;

use super::sources::{
    actual_for, actual_storage, collect_backings, input_backing, input_storage, state_flow,
};
use super::{
    Backing, IncomingBackingCalls, IncomingCallerRoster, Invocation, Requirement, Storage,
};

impl<'facts> IncomingBackingCalls<'facts> {
    pub(in crate::checks::borrows) fn new(
        program: &dyn validation::CheckingInput,
        facts: &'facts CheckFacts,
        external_entries: &[SymbolHandle],
    ) -> Self {
        let mut callers: Vec<Vec<IncomingCallerRoster<'facts>>> = Vec::new();
        for (_, state) in facts.flow.control.states.iter() {
            for call in facts.flow.control.calls.span_or_empty(state.calls) {
                let target = validation::call_target_entry_symbol(program, call.target_symbol);
                let position = target.arena_index() as usize;
                if callers.len() <= position {
                    callers.resize_with(position + 1, Vec::new);
                }
                // The dense index selects a bucket; the complete handle
                // selects its roster, preserving every generation and edge.
                let bucket = &mut callers[position];
                let roster = match bucket.iter().position(|roster| roster.target == target) {
                    Some(position) => position,
                    None => {
                        bucket.push(IncomingCallerRoster {
                            target,
                            invocations: Vec::new(),
                        });
                        bucket.len() - 1
                    }
                };
                bucket[roster].invocations.push(Invocation {
                    caller: state,
                    call,
                });
            }
        }
        let mut external_symbols = external_entries.to_vec();
        for association in validation::provider_application_associations(program).iter() {
            external_symbols.push(association.identity.provider);
        }
        for conformance in program.conformances() {
            for row in &conformance.rows {
                external_symbols.push(row.realization_machine);
                external_symbols.push(row.realization_state);
            }
        }
        for usage in &facts.nominal_machine_uses.uses {
            if usage.callback_entry.is_some() {
                external_symbols.push(usage.selected_machine);
                external_symbols.push(usage.selected_entry);
            }
        }
        let mut external_machines: Vec<Vec<SymbolHandle>> = Vec::new();
        for machine in external_symbols.into_iter().filter_map(|symbol| {
            program
                .machine_by_symbol(symbol)
                .map(|machine| machine.symbol)
                .or_else(|| {
                    validation::find_state(program, symbol).map(|(machine, _)| machine.symbol)
                })
        }) {
            let position = machine.arena_index() as usize;
            if external_machines.len() <= position {
                external_machines.resize_with(position + 1, Vec::new);
            }
            let bucket = &mut external_machines[position];
            if !bucket.contains(&machine) {
                bucket.push(machine);
            }
        }
        Self {
            callers,
            external_machines,
        }
    }

    pub(super) fn discharge(
        &self,
        program: &dyn validation::CheckingInput,
        facts: &CheckFacts,
        loan_occurrences: &LoanOccurrences<'_>,
        mut pending: Vec<Requirement>,
    ) -> Result<(), &'static str> {
        let mut pending = pending
            .drain(..)
            .map(|requirement| (requirement, Vec::new()))
            .collect::<Vec<_>>();
        let mut observed = Vec::new();
        let mut dependencies = Vec::new();
        let mut established = Vec::new();
        let mut lookup = None;
        while let Some((requirement, ancestry)) = pending.pop() {
            if observed.contains(&requirement) {
                continue;
            }
            // A recursive relation with a growing projection cannot be
            // represented by a finite input obligation. Do not truncate it.
            if ancestry.iter().any(|previous: &Requirement| {
                previous.state == requirement.state && previous != &requirement
            }) {
                return Err("recursive calls change the unresolved backing/storage relation");
            }
            observed.push(requirement.clone());
            let mut ancestry = ancestry;
            ancestry.push(requirement.clone());
            let state = state_flow(facts, requirement.state)?;
            if self
                .external_machines
                .get(state.machine_symbol.arena_index() as usize)
                .is_some_and(|bucket| bucket.contains(&state.machine_symbol))
            {
                return Err(
                    "external entry has no established incoming backing/storage compatibility",
                );
            }
            let roster = self
                .callers
                .get(requirement.state.arena_index() as usize)
                .and_then(|bucket| {
                    bucket
                        .iter()
                        .find(|roster| roster.target == requirement.state)
                })
                .filter(|roster| !roster.invocations.is_empty())
                .ok_or(
                    "incoming backing/storage compatibility reaches an entry without actual inputs",
                )?;
            for invocation in &roster.invocations {
                let backing_actual =
                    actual_for(program, facts, invocation, requirement.backing.slot.root)?;
                let source = validation::completed_storage::reference_sources_at_path(
                    program,
                    &facts.flow.control,
                    backing_actual,
                    &requirement.backing.slot.segments,
                    invocation.call.invocation_storage,
                );
                let mut backings = Vec::new();
                collect_backings(
                    program,
                    facts,
                    loan_occurrences,
                    invocation.caller,
                    source,
                    &mut backings,
                )?;
                if backings.is_empty() {
                    return Err("an invocation has no complete captured backing alternatives");
                }
                let storage_actual =
                    actual_for(program, facts, invocation, requirement.storage.root)?;
                let storage = actual_storage(
                    program,
                    facts,
                    loan_occurrences,
                    invocation.caller,
                    storage_actual,
                    &requirement.storage.segments,
                    invocation.call.invocation_storage,
                )?;
                if storage.is_empty() {
                    return Err("an invocation has no complete exclusive storage alternatives");
                }
                for backing in backings {
                    for destination in &storage {
                        match (&backing, destination) {
                            (Backing::Immutable, _) => {
                                if !established.contains(&requirement) {
                                    established.push(requirement.clone());
                                }
                            }
                            (
                                Backing::Place {
                                    place: source,
                                    projectable,
                                },
                                Storage::Place(destination),
                            ) => {
                                let mut source = source.clone();
                                if *projectable {
                                    source.segments = facts::project_captured_view_segments(
                                        &source.segments,
                                        &requirement.backing.suffix,
                                    )
                                    .ok_or(
                                        "the captured pointer cannot project its retained backing",
                                    )?;
                                }
                                let compatibility =
                                    super::super::overlap::captured_place_compatibility(
                                        program,
                                        &facts.borrow,
                                        &source,
                                        &BorrowAccessKind::Read,
                                        destination,
                                        &BorrowAccessKind::Mutable,
                                        &[],
                                        &mut lookup,
                                    );
                                if !compatibility.non_interfering {
                                    return Err(
                                        "an invocation's captured backing conflicts with its exclusive storage",
                                    );
                                }
                                if !established.contains(&requirement) {
                                    established.push(requirement.clone());
                                }
                            }
                            _ => {
                                let mut source = match &backing {
                                    Backing::Incoming(source) => source.clone(),
                                    Backing::Place { place: source, .. } => {
                                        input_backing(program, facts, invocation.caller, source)?
                                    }
                                    Backing::Immutable => {
                                        return Err(
                                            "immutable backing entered unresolved input propagation",
                                        );
                                    }
                                };
                                if !matches!(
                                    &backing,
                                    Backing::Place {
                                        projectable: false,
                                        ..
                                    }
                                ) {
                                    source.suffix.extend_from_slice(&requirement.backing.suffix);
                                }
                                let storage = match destination {
                                    Storage::Incoming(source) => source.clone(),
                                    Storage::Place(source) => {
                                        input_storage(program, facts, invocation.caller, source)?
                                    }
                                };
                                let next = Requirement {
                                    state: invocation.caller.state_symbol,
                                    backing: source,
                                    storage,
                                };
                                dependencies.push((requirement.clone(), next.clone()));
                                pending.push((next, ancestry.clone()));
                            }
                        }
                    }
                }
            }
        }
        // A closed call cycle cannot establish its own entry assumption.
        // Every propagated component must reach an actual proved relation.
        loop {
            let mut changed = false;
            for (parent, child) in &dependencies {
                if established.contains(child) && !established.contains(parent) {
                    established.push(parent.clone());
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        if observed
            .iter()
            .any(|requirement| !established.contains(requirement))
        {
            return Err(
                "a recursive incoming compatibility relation has no established actual boundary",
            );
        }
        Ok(())
    }
}
