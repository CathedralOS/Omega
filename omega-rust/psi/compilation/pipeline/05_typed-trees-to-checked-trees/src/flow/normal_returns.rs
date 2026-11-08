//! Retain what a completed body proves about incoming mutable storage.
//!
//! Entry qualifications are requirements, not promises. A return row therefore
//! comes only from live facts at every normal exit, mapped to the same exact
//! invocation referent. Rows contain only source identities and predicates;
//! sweep-local places, facts and contexts never survive output replacement.
//! Callers consume the preceding completed table, not a callee being built.

use super::FlowBuildContext;
use facts::{Fact, FactPlan, PlaceRoot};
use symbols::SymbolHandle;
use typed_trees::types::TypeReferenceHandle;
use validation::field_domains::ByteSequencePredicate;

#[derive(Clone, Copy, PartialEq, Eq)]
struct ReturnedPredicate {
    parameter: SymbolHandle,
    reference_type: TypeReferenceHandle,
    predicate: ByteSequencePredicate,
}

type StorageIdentityKey = ((u32, u32), (u32, u32));

/// Opaque handles have no semantic order. Index preparation orders only their
/// complete identities, including generations, without dereferencing them.
fn storage_key(parameter: SymbolHandle, reference: TypeReferenceHandle) -> StorageIdentityKey {
    (
        (parameter.arena_index(), parameter.generation()),
        (reference.arena_index(), reference.generation()),
    )
}

impl ReturnedPredicate {
    fn key(&self) -> (StorageIdentityKey, ByteSequencePredicate) {
        (
            storage_key(self.parameter, self.reference_type),
            self.predicate,
        )
    }
}

/// Compact rows for actual producers, sorted by machine roster position.
#[derive(Default)]
pub(super) struct NormalReturnPredicates {
    rows: Vec<(usize, Vec<ReturnedPredicate>)>,
    /// Count each finite lattice cell once when extending convergence's bound.
    discovered: Vec<(usize, ReturnedPredicate)>,
}

impl NormalReturnPredicates {
    pub(super) fn clear(&mut self) {
        self.rows.clear();
        self.discovered.clear();
    }

    fn contains(&self, machine: usize, row: ReturnedPredicate) -> bool {
        self.rows
            .binary_search_by_key(&machine, |(position, _)| *position)
            .is_ok_and(|position| {
                self.rows[position]
                    .1
                    .binary_search_by_key(&row.key(), ReturnedPredicate::key)
                    .is_ok()
            })
    }

    /// All selected family bodies must prove the same typed formal's class.
    /// A different telescope or unresolved entry origin supplies no guarantee.
    pub(super) fn proves(
        &self,
        program: &dyn validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        root: PlaceRoot,
        predicate: ByteSequencePredicate,
    ) -> bool {
        let PlaceRoot::InvocationReferent {
            parameter,
            type_reference,
        } = root
        else {
            return false;
        };
        let Some(entry) = program.machine_states(machine).first() else {
            return false;
        };
        let Some(argument) = program.state_parameters(entry).iter().position(|formal| {
            formal.symbol == parameter && formal.type_reference == type_reference
        }) else {
            return false;
        };
        let Some(members) = family_members(program, machine.symbol) else {
            return false;
        };
        members.into_iter().all(|position| {
            let member = &program.machines()[position];
            member.body_is_present
                && program.machine_states(member).first().is_some_and(|entry| {
                    program
                        .state_parameters(entry)
                        .get(argument)
                        .is_some_and(|formal| {
                            formal.type_reference == type_reference
                                && self.contains(
                                    position,
                                    ReturnedPredicate {
                                        parameter: formal.symbol,
                                        reference_type: formal.type_reference,
                                        predicate,
                                    },
                                )
                        })
                })
        })
    }

    fn replace(&mut self, rows: Vec<(usize, Vec<ReturnedPredicate>)>) -> (Vec<usize>, usize) {
        let mut changed: Vec<_> = self.rows.iter().chain(&rows).map(|row| row.0).collect();
        changed.sort_unstable();
        changed.dedup();
        changed.retain(|position| {
            let old = self
                .rows
                .binary_search_by_key(position, |row| row.0)
                .ok()
                .map(|index| &self.rows[index].1);
            let new = rows
                .binary_search_by_key(position, |row| row.0)
                .ok()
                .map(|index| &rows[index].1);
            old != new
        });
        let mut discovered = self.discovered.clone();
        discovered
            .extend(rows.iter().flat_map(|(position, values)| {
                values.iter().map(move |value| (*position, *value))
            }));
        discovered.sort_unstable_by_key(|(position, row)| (*position, row.key()));
        discovered.dedup();
        let height = 2 * (discovered.len() - self.discovered.len());
        self.discovered = discovered;
        self.rows = rows;
        (changed, height)
    }
}

pub(super) fn family_members(
    program: &dyn validation::CheckingInput,
    symbol: SymbolHandle,
) -> Option<Vec<usize>> {
    let mut members = vec![program.machine_position_by_symbol(symbol)?];
    if let Some(family) = program.target_machine_families.iter().find(|family| {
        family.canonical == symbol || family.members.iter().any(|member| member.machine == symbol)
    }) {
        for member in std::iter::once(family.canonical)
            .chain(family.members.iter().map(|member| member.machine))
        {
            members.push(program.machine_position_by_symbol(member)?);
        }
    }
    members.sort_unstable();
    members.dedup();
    Some(members)
}

pub(super) fn has_exclusive_parameters(
    program: &dyn validation::CheckingInput,
    state: &typed_trees::state::State,
) -> bool {
    program.state_parameters(state).iter().any(|parameter| {
        validation::reference_shell(program, parameter.type_reference)
            .is_some_and(|shell| shell.access.is_exclusive())
    })
}

/// Reverse edges only schedule work; they convey no facts or access authority.
pub(super) fn dependents(
    program: &dyn validation::CheckingInput,
    borrow: &checked_trees::BorrowFacts,
) -> Vec<Vec<SymbolHandle>> {
    let mut rows = vec![Vec::new(); program.machines().len()];
    for (_, caller) in borrow.states.iter() {
        for call in borrow.calls.span_or_empty(caller.calls) {
            let Some((machine, state)) = validation::find_state(program, call.target_symbol) else {
                continue;
            };
            if !has_exclusive_parameters(program, state) {
                continue;
            }
            let Some(members) = family_members(program, machine.symbol) else {
                continue;
            };
            for member in members {
                rows[member].push(caller.state_symbol);
            }
        }
    }
    for row in &mut rows {
        row.sort_unstable_by_key(|symbol| (symbol.arena_index(), symbol.generation()));
        row.dedup();
    }
    rows
}

/// Publish only after a complete sweep. Every normal exit participates, so an
/// unknown origin or a missing proof removes that row instead of being ignored.
pub(super) fn refresh(
    program: &dyn validation::CheckingInput,
    semantic: &FactPlan,
    build: &mut FlowBuildContext<'_>,
    machines: &[usize],
) -> (Vec<usize>, usize) {
    let mut exits = vec![Vec::new(); machines.len()];
    for (_, exit) in build.control.exits.iter() {
        if let Some(position) = program.machine_position_by_symbol(exit.machine_symbol)
            && let Ok(index) = machines.binary_search(&position)
        {
            exits[index].push(exit.clone());
        }
    }
    let mut rows = Vec::new();
    for (&position, exits) in machines.iter().zip(exits) {
        let machine = &program.machines()[position];
        if !machine.body_is_present
            || exits.is_empty()
            || !program
                .machine_states(machine)
                .first()
                .is_some_and(|entry| has_exclusive_parameters(program, entry))
        {
            continue;
        }
        let mut common: Option<Vec<ReturnedPredicate>> = None;
        for exit in exits {
            let mut proved = Vec::new();
            if build
                .control
                .storage_snapshots
                .is_valid(exit.completed_storage)
                && let Some(state) = program.state_by_symbol(exit.state_symbol)
            {
                let contexts =
                    super::active_context_handles(&build.contexts, exit.entry_semantic_contexts);
                let handles = semantic.unique_context_facts(contexts);
                let mut facts: Vec<((SymbolHandle, TypeReferenceHandle), Fact)> = Vec::new();
                for handle in handles {
                    let fact = *semantic.facts.get(handle);
                    if !matches!(
                        fact.payload,
                        facts::FactPayload::AssignedValue { .. }
                            | facts::FactPayload::BytePredicate { .. }
                    ) && fact.payload.domain_membership().is_none()
                    {
                        continue;
                    }
                    let Some(place) = fact.place.as_place().and_then(|place| {
                        super::canonical_place_from_semantic_place(
                            program,
                            semantic,
                            semantic.places.get(place),
                        )
                    }) else {
                        continue;
                    };
                    let Some(origins) = super::entry_origins::project_invocation_place(
                        program,
                        build,
                        machine,
                        state,
                        exit.statement_index,
                        &place,
                        true,
                    ) else {
                        continue;
                    };
                    let [origin] = origins.as_slice() else {
                        continue;
                    };
                    let PlaceRoot::InvocationReferent {
                        parameter,
                        type_reference,
                    } = origin.root
                    else {
                        continue;
                    };
                    if !origin.segments.is_empty()
                        || !validation::reference_shell(program, type_reference)
                            .is_some_and(|shell| shell.access.is_exclusive())
                    {
                        continue;
                    }
                    facts.push(((parameter, type_reference), fact));
                }
                facts.sort_unstable_by_key(|((parameter, reference), _)| {
                    storage_key(*parameter, *reference)
                });
                for group in facts.chunk_by(|left, right| left.0 == right.0) {
                    let (parameter, reference_type) = group[0].0;
                    let facts: Vec<_> = group.iter().map(|(_, fact)| *fact).collect();
                    for predicate in ByteSequencePredicate::ALL {
                        if super::transfers::carrier_proves_predicate(program, &facts, predicate) {
                            proved.push(ReturnedPredicate {
                                parameter,
                                reference_type,
                                predicate,
                            });
                        }
                    }
                }
            }
            proved.sort_unstable_by_key(ReturnedPredicate::key);
            proved.dedup();
            match &mut common {
                Some(common) => common.retain(|row| {
                    proved
                        .binary_search_by_key(&row.key(), ReturnedPredicate::key)
                        .is_ok()
                }),
                None => common = Some(proved),
            }
        }
        if let Some(common) = common.filter(|rows| !rows.is_empty()) {
            rows.push((position, common));
        }
    }
    build.normal_return_predicates.replace(rows)
}
