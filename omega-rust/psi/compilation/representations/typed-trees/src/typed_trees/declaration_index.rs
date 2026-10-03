//! Symbol-to-declaration indexes owned by the program they describe.
//!
//! The checked stage and validation look declarations up by symbol often
//! enough that a per-query roster scan shows up in profiles. Those indexes
//! used to live in thread-local caches keyed on the program's address or
//! instance tag; the address could be recycled under a dropped program and
//! every entry had to re-derive its own freshness. The indexes now live
//! inside the program itself, guarded by the arena revision counters —
//! every mutation-capable call on an `Arena` bumps the revision, so a stale
//! index cannot outlive the table it describes regardless of which mutator
//! moved it.
//!
//! `TypedTrees` mutates declarations only through its methods
//! (`push_data_definition`, `push_domain_definition`, `push_machine` and
//! siblings), which is also the point where a revision moves. Keeping the
//! revision check inside each slot makes the indexes correct even where a
//! table is still reached through a `pub` field.

use crate::{data, machine};
use arena::{Handle, HandleSpan, OwnedIndex};
use symbols::SymbolHandle;

/// Handle-keyed maps take the integer hasher; none is iterated for output
/// order, and `Box<str>`/`usize`/tuple keys hash through the same mixing as
/// `symbols::SymbolKeyMap`.
type HashMap<K, V> = std::collections::HashMap<K, V, symbols::BuildSymbolHasher>;
type HashSet<T> = std::collections::HashSet<T, symbols::BuildSymbolHasher>;

/// A roster span's identity folded into a freshness key: `TypedTrees`
/// publishes `roots` and `tables` as `pub` fields, and a few callers
/// re-point a root span in place (retained-prefix builds, product pruning)
/// without touching the arena's revision. The span's start and count carry
/// exactly the visible roster, so they must join the key.
fn roster_marker<T>(span: HandleSpan<T>) -> u64 {
    ((span.start().arena_index() as u64) << 32) | span.count() as u64
}

/// Every `TypedTreeTables` field revision summed, plus every root span's
/// identity. `Arena` revisions only move forward, so the sum moves on any
/// mutation — appends, frees, and same-length rewrites alike — while the
/// span markers catch roots that were re-pointed in place without touching
/// an arena.
fn tables_fingerprint(tables: &crate::typed_trees::TypedTreeTables) -> u64 {
    tables
        .const_declarations
        .revision()
        .wrapping_add(tables.data_definitions.revision())
        .wrapping_add(tables.data_type_parameters.revision())
        .wrapping_add(tables.data_members.revision())
        .wrapping_add(tables.data_payload_fields.revision())
        .wrapping_add(tables.domain_definitions.revision())
        .wrapping_add(tables.proof_facts.revision())
        .wrapping_add(tables.propositions.revision())
        .wrapping_add(tables.proposition_binders.revision())
        .wrapping_add(tables.mathematical_definitions.revision())
        .wrapping_add(tables.mathematical_parameters.revision())
        .wrapping_add(tables.mathematical_types.revision())
        .wrapping_add(tables.domain_path_members.revision())
        .wrapping_add(tables.operator_path_members.revision())
        .wrapping_add(tables.machines.revision())
        .wrapping_add(tables.measures.revision())
        .wrapping_add(tables.measure_path_members.revision())
        .wrapping_add(tables.operators.revision())
        .wrapping_add(tables.machine_owned_data.revision())
        .wrapping_add(tables.machine_trait_conformances.revision())
        .wrapping_add(tables.machine_states.revision())
        .wrapping_add(tables.state_parameters.revision())
        .wrapping_add(tables.traits.revision())
        .wrapping_add(tables.conformances.revision())
        .wrapping_add(tables.trait_requirements.revision())
        .wrapping_add(tables.trait_machine_signatures.revision())
        .wrapping_add(tables.signature_invokes.revision())
        .wrapping_add(tables.signature_contracts.revision())
        .wrapping_add(tables.expression_table.revision())
        .wrapping_add(tables.statement_table.revision())
        .wrapping_add(tables.type_reference_table.revision())
        .wrapping_add(tables.wire_schemas.revision())
        .wrapping_add(tables.wire_members.revision())
}

fn roots_fingerprint(roots: &crate::typed_trees::TypedTreeRoots) -> u64 {
    roster_marker(roots.const_declarations)
        .wrapping_add(roster_marker(roots.data_definitions))
        .wrapping_add(roster_marker(roots.domain_definitions))
        .wrapping_add(roster_marker(roots.machines))
        .wrapping_add(roster_marker(roots.measures))
        .wrapping_add(roster_marker(roots.operators))
        .wrapping_add(roster_marker(roots.machine_token_bindings))
        .wrapping_add(roster_marker(roots.propositions))
        .wrapping_add(roster_marker(roots.mathematical_definitions))
        .wrapping_add(roster_marker(roots.traits))
        .wrapping_add(roster_marker(roots.conformances))
        .wrapping_add(roster_marker(roots.wire_schemas))
}

/// Machine-symbol lookup: the machine table is queried once per call
/// expression through `machine_by_symbol`; a roster scan per query walks
/// every machine. The map keeps the first position per symbol, matching the
/// roster `find` the index replaces.
#[derive(Clone, Default)]
struct MachinePositionIndex {
    by_symbol: HashMap<SymbolHandle, usize>,
}

/// State-symbol lookup: `(machine symbol, state symbol)` to the state's
/// position inside its machine, plus the flat `state symbol -> (machine
/// position, state position)` view some callers want. Built in one pass
/// over every machine's states.
#[derive(Clone, Default)]
struct StatePositionIndex {
    in_machine: HashMap<(SymbolHandle, SymbolHandle), usize>,
    location: HashMap<SymbolHandle, (usize, usize)>,
}

/// Data-definition roster: every position carrying a symbol, and every
/// position carrying a name. `by_symbol` keeps ALL matching positions (test
/// fixtures can forge duplicate symbols and write-frame analysis asks for
/// the whole set); `first_position` answers the common single-declaration
/// lookup.
#[derive(Clone, Default)]
pub struct DataDefinitionIndex {
    by_symbol: HashMap<SymbolHandle, Vec<u32>>,
    by_name: HashMap<Box<str>, Vec<u32>>,
    first_position: HashMap<SymbolHandle, u32>,
}

impl DataDefinitionIndex {
    /// Every position declaring `symbol`, in declaration order — absent,
    /// unique, and duplicate answers all read the same slice.
    pub fn positions_for_symbol(&self, symbol: SymbolHandle) -> &[u32] {
        self.by_symbol
            .get(&symbol)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
}

/// Domain-definition roster: the first position per symbol, matching the
/// roster `find` the index replaces. Shared by the in-crate proof readers
/// and the checked stage's field-domain walkers.
#[derive(Clone, Default)]
struct DomainPositionIndex {
    by_symbol: HashMap<SymbolHandle, usize>,
}

/// Data symbols owning an attached `::drop` machine, computed once per
/// machine-table revision.
#[derive(Clone, Default)]
struct AttachedDropIndex {
    hooks: HashSet<SymbolHandle>,
}

/// Call-target lookup for service-reach inference: the machines, machine
/// type parameters, traits, trait signatures, and parameter contracts the
/// boundary walkers keep resolving by symbol. Moved into the program from
/// the checking crate's thread-local so the index's lifetime is the
/// table's.
#[derive(Clone, Default)]
pub struct CallTargetIndex {
    /// Boundary-machine index by entry state symbol.
    pub boundary_machine_by_state: symbols::SymbolKeyMap<SymbolHandle, u32>,
    /// Machine-parameter symbol -> (machine index, parameter index).
    pub parameter_signature: symbols::SymbolKeyMap<SymbolHandle, (u32, u32)>,
    /// Trait symbol -> trait index.
    pub trait_by_symbol: symbols::SymbolKeyMap<SymbolHandle, u32>,
    /// Trait machine-signature symbol -> (trait index, signature index).
    pub trait_signature: symbols::SymbolKeyMap<SymbolHandle, (u32, u32)>,
    /// Machine-parameter symbol -> data type-parameter handle.
    pub parameter_contract: symbols::SymbolKeyMap<SymbolHandle, Handle<data::TypeParameter>>,
}

/// Write-frame isolation verdicts. The verdict keys are all program-owned
/// handle types, so the memo lives on the program: `isolation` answers
/// handle-plus-flag questions, `storage_match` handle pairs, `definitions`
/// per-definition positions (not symbols — forged duplicate symbols in
/// fixtures must keep independent verdicts), and `reference_free` /
/// `carry_write` handle-plus-binding-frame questions.
#[derive(Clone, Default)]
pub struct IsolationVerdicts {
    pub isolation: HashMap<(crate::types::TypeReferenceHandle, bool), bool>,
    pub storage_match: HashMap<
        (
            crate::types::TypeReferenceHandle,
            crate::types::TypeReferenceHandle,
        ),
        bool,
    >,
    /// Verdicts keyed on the definition's own address as `usize`, not its
    /// symbol — test fixtures can forge duplicate symbols, and two
    /// definitions sharing one must keep independent verdicts. The slot's
    /// revision guard bounds the address's lifetime: a rebuilt index drops
    /// every key the table mutation could have invalidated.
    pub definitions: HashMap<usize, bool>,
    pub reference_free: HashMap<
        (
            crate::types::TypeReferenceHandle,
            Vec<(SymbolHandle, crate::types::TypeReferenceHandle)>,
        ),
        bool,
    >,
    pub carry_write: HashMap<
        (
            crate::types::TypeReferenceHandle,
            Vec<(SymbolHandle, crate::types::TypeReferenceHandle)>,
        ),
        bool,
    >,
}

/// Every spelling a domain answers to, bucketed once per program. The
/// lowering stage fills it: a qualified-spelling match needs no exposure
/// check, while the relative spellings (declared, member-relative, leaf)
/// still require the caller's exposure rules.
#[derive(Clone, Default)]
pub struct DomainCandidateIndex {
    pub qualified: Vec<Box<str>>,
    pub by_qualified: HashMap<Box<str>, Vec<u32>>,
    pub by_relative: HashMap<Box<str>, Vec<u32>>,
}

/// The program-owned declaration indexes. One `Mutex` per index keeps
/// unrelated lookups from contending; each slot's revision guard is exactly
/// the sum of the arena revisions the index reads, so an index rebuilds
/// precisely when one of its source tables changed.
#[derive(Clone, Default)]
pub struct DeclarationIndexes {
    machines: OwnedIndex<MachinePositionIndex>,
    states: OwnedIndex<StatePositionIndex>,
    data_definitions: OwnedIndex<DataDefinitionIndex>,
    domains: OwnedIndex<DomainPositionIndex>,
    attached_drops: OwnedIndex<AttachedDropIndex>,
    call_targets: OwnedIndex<CallTargetIndex>,
    domain_candidates: OwnedIndex<DomainCandidateIndex>,
    isolation: OwnedIndex<IsolationVerdicts>,
}

// Derived content again: two programs with equal declarations are equal
// whether or not an index has been built yet.
impl PartialEq for DeclarationIndexes {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for DeclarationIndexes {}

impl std::fmt::Debug for DeclarationIndexes {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("DeclarationIndexes")
    }
}

impl super::TypedTrees {
    /// One freshness counter over the whole program: every table arena's
    /// revision, every root span's marker, and the lengths of the side Vecs
    /// and intern tables that have no revision of their own. Checking-phase
    /// memos that are not whole-table indexes (derived plans, per-query
    /// verdicts) key on `(identity, structural_fingerprint())` so a
    /// replacement program or an in-place mutation can never serve a stale
    /// answer.
    pub fn structural_fingerprint(&self) -> u64 {
        tables_fingerprint(&self.tables)
            .wrapping_add(roots_fingerprint(&self.roots))
            .wrapping_add(self.service_reaches.definitions().len() as u64)
            .wrapping_add(self.service_reach_rows.len() as u64)
            .wrapping_add(self.authored_service_reach_rows.len() as u64)
            .wrapping_add(self.semantic_domains.len() as u64)
            .wrapping_add(self.external_bindings.identities().len() as u64)
            .wrapping_add(self.plan_laid_layouts.len() as u64)
            .wrapping_add(self.placed_view_plans.len() as u64)
            .wrapping_add(self.wire_placements.revision())
            .wrapping_add(self.wire_encode_obligations.revision())
            .wrapping_add(self.wire_schema_plans.len() as u64)
            .wrapping_add(self.machine_specializations.len() as u64)
            .wrapping_add(self.boundary_calling_plans.len() as u64)
            .wrapping_add(self.boundary_completion_declarations.revision())
            .wrapping_add(self.fused_service_erasures.len() as u64)
            .wrapping_add(self.open_index_normalizations.len() as u64)
            .wrapping_add(self.evidence_forwardings.len() as u64)
            .wrapping_add(self.proof_output_calls.len() as u64)
            .wrapping_add(self.ranking_expression_custody.len() as u64)
            .wrapping_add(self.pending_const_range_endpoints.len() as u64)
    }

    /// Machine-table position of `symbol`, or `None` when no machine carries
    /// it. The index keeps the first position per symbol — the roster `find`
    /// it replaces had the same earliest-match rule.
    pub fn machine_position_by_symbol(&self, symbol: SymbolHandle) -> Option<usize> {
        if !symbol.is_valid() {
            return None;
        }
        self.declaration_indexes.machines.with(
            self.tables
                .machines
                .revision()
                .wrapping_add(roster_marker(self.roots.machines)),
            || {
                let machines = self.machines();
                let mut by_symbol =
                    HashMap::with_capacity_and_hasher(machines.len(), symbols::BuildSymbolHasher);
                for (position, machine) in machines.iter().enumerate() {
                    by_symbol.entry(machine.symbol).or_insert(position);
                }
                MachinePositionIndex { by_symbol }
            },
            |index| index.by_symbol.get(&symbol).copied(),
        )
    }

    /// The machine declaring `symbol`.
    pub fn machine_by_symbol(&self, symbol: SymbolHandle) -> Option<&machine::Machine> {
        self.machine_position_by_symbol(symbol)
            .map(|position| &self.machines()[position])
    }

    /// Position of `state` inside `machine`'s state span. Freshness covers
    /// both tables: a machine append or a state-table rewrite rebuilds.
    pub fn state_position_by_symbol(
        &self,
        machine: SymbolHandle,
        state: SymbolHandle,
    ) -> Option<usize> {
        if !machine.is_valid() || !state.is_valid() {
            return None;
        }
        self.declaration_indexes.states.with(
            self.tables
                .machines
                .revision()
                .wrapping_add(self.tables.machine_states.revision())
                .wrapping_add(roster_marker(self.roots.machines)),
            || self.build_state_positions(),
            |index| index.in_machine.get(&(machine, state)).copied(),
        )
    }

    /// `(machine position, state position)` of `state` regardless of which
    /// machine owns it.
    pub fn state_location_by_symbol(&self, state: SymbolHandle) -> Option<(usize, usize)> {
        if !state.is_valid() {
            return None;
        }
        self.declaration_indexes.states.with(
            self.tables
                .machines
                .revision()
                .wrapping_add(self.tables.machine_states.revision())
                .wrapping_add(roster_marker(self.roots.machines)),
            || self.build_state_positions(),
            |index| index.location.get(&state).copied(),
        )
    }

    fn build_state_positions(&self) -> StatePositionIndex {
        let mut index = StatePositionIndex::default();
        for (machine_index, machine) in self.machines().iter().enumerate() {
            for (state_index, state) in self.machine_states(machine).iter().enumerate() {
                // First match wins, like the roster `position` scans the
                // indexes replace: malformed duplicate rows cannot
                // supplement the first exact owner.
                index
                    .in_machine
                    .entry((machine.symbol, state.symbol))
                    .or_insert(state_index);
                index
                    .location
                    .entry(state.symbol)
                    .or_insert((machine_index, state_index));
            }
        }
        index
    }

    /// The whole data-definition index: every position per symbol, every
    /// position per declared name, and the first position per symbol.
    pub fn with_data_definition_index<R>(&self, run: impl FnOnce(&DataDefinitionIndex) -> R) -> R {
        self.declaration_indexes.data_definitions.with(
            self.tables
                .data_definitions
                .revision()
                .wrapping_add(roster_marker(self.roots.data_definitions)),
            || {
                let mut index = DataDefinitionIndex::default();
                for (position, definition) in self.data_definitions().iter().enumerate() {
                    let position = position as u32;
                    if definition.symbol.is_valid() {
                        index
                            .by_symbol
                            .entry(definition.symbol)
                            .or_default()
                            .push(position);
                        index
                            .first_position
                            .entry(definition.symbol)
                            .or_insert(position);
                    }
                    index
                        .by_name
                        .entry(definition.name.as_str().into())
                        .or_default()
                        .push(position);
                }
                index
            },
            run,
        )
    }

    /// First data-definition position carrying `symbol`, matching the roster
    /// `find` the thread-local index used to answer.
    pub fn data_definition_position_by_symbol(&self, symbol: SymbolHandle) -> Option<usize> {
        if !symbol.is_valid() {
            return None;
        }
        self.with_data_definition_index(|index| {
            index
                .first_position
                .get(&symbol)
                .map(|position| *position as usize)
        })
    }

    /// The first data declaration carrying `symbol`.
    pub fn data_definition_by_symbol(&self, symbol: SymbolHandle) -> Option<&data::DataDefinition> {
        self.data_definition_position_by_symbol(symbol)
            .map(|position| &self.data_definitions()[position])
    }

    /// The sole data declaration carrying `symbol`; absent, invalid and
    /// duplicate identities have no unique owner. Consult the complete indexed
    /// roster rather than the first-match query when uniqueness is evidence.
    pub fn unique_data_definition_by_symbol(
        &self,
        symbol: SymbolHandle,
    ) -> Option<&data::DataDefinition> {
        if !symbol.is_valid() {
            return None;
        }
        let position = self.with_data_definition_index(|index| {
            let [position] = index.positions_for_symbol(symbol) else {
                return None;
            };
            Some(*position as usize)
        })?;
        self.data_definitions().get(position)
    }

    /// All data-definition positions carrying `symbol`, in declaration
    /// order.
    pub fn data_definition_positions_for_symbol(&self, symbol: SymbolHandle) -> Vec<u32> {
        self.with_data_definition_index(|index| {
            index.by_symbol.get(&symbol).cloned().unwrap_or_default()
        })
    }

    /// All data-definition positions declared under `name`, in declaration
    /// order.
    pub fn data_definition_positions_for_name(&self, name: &str) -> Vec<u32> {
        self.with_data_definition_index(|index| {
            index.by_name.get(name).cloned().unwrap_or_default()
        })
    }

    /// First domain-definition position carrying `symbol`, matching the
    /// roster `find` the lookups it replaces used to run.
    pub fn domain_position_by_symbol(&self, symbol: SymbolHandle) -> Option<usize> {
        if !symbol.is_valid() {
            return None;
        }
        self.declaration_indexes.domains.with(
            self.tables
                .domain_definitions
                .revision()
                .wrapping_add(roster_marker(self.roots.domain_definitions)),
            || {
                let definitions = self.domain_definitions();
                let mut by_symbol = HashMap::with_capacity_and_hasher(
                    definitions.len(),
                    symbols::BuildSymbolHasher,
                );
                for (position, domain) in definitions.iter().enumerate() {
                    // `find` returns the first matching row; keep it.
                    by_symbol.entry(domain.symbol).or_insert(position);
                }
                DomainPositionIndex { by_symbol }
            },
            |index| index.by_symbol.get(&symbol).copied(),
        )
    }

    /// Whether a data declaration owns an attached `::drop` machine.
    pub fn attached_drop_machine_exists(&self, data_symbol: SymbolHandle) -> bool {
        if !data_symbol.is_valid() {
            return false;
        }
        self.declaration_indexes.attached_drops.with(
            self.tables
                .machines
                .revision()
                .wrapping_add(roster_marker(self.roots.machines)),
            || {
                let mut hooks = HashSet::default();
                for machine in self.machines() {
                    if machine.attached_data_symbol.is_valid()
                        && machine.name.as_str().ends_with("::drop")
                    {
                        hooks.insert(machine.attached_data_symbol);
                    }
                }
                AttachedDropIndex { hooks }
            },
            |index| index.hooks.contains(&data_symbol),
        )
    }

    /// The declaration indexes for call-target resolution, shared by the
    /// checking crate's effect inference. Freshness is every arena the
    /// builder reads; each revision moves only forward, so their sum moves
    /// exactly when one of the tables does.
    pub fn with_call_target_index<R>(&self, run: impl FnOnce(&CallTargetIndex) -> R) -> R {
        let revision = self
            .tables
            .machines
            .revision()
            .wrapping_add(self.tables.machine_states.revision())
            .wrapping_add(self.tables.data_type_parameters.revision())
            .wrapping_add(self.tables.traits.revision())
            .wrapping_add(self.tables.trait_machine_signatures.revision())
            .wrapping_add(roster_marker(self.roots.machines))
            .wrapping_add(roster_marker(self.roots.traits));
        self.declaration_indexes
            .call_targets
            .with(revision, || self.build_call_target_index(), run)
    }

    fn build_call_target_index(&self) -> CallTargetIndex {
        let mut index = CallTargetIndex::default();
        for (machine_index, machine) in self.machines().iter().enumerate() {
            if machine.supply_mode.is_boundary_declaration() {
                for state in self.machine_states(machine) {
                    index
                        .boundary_machine_by_state
                        .entry(state.symbol)
                        .or_insert(machine_index as u32);
                }
            }
            for (parameter_index, parameter) in
                self.machine_type_parameters(machine).iter().enumerate()
            {
                if matches!(parameter.kind, data::TypeParameterKind::Machine { .. }) {
                    index
                        .parameter_signature
                        .entry(parameter.symbol)
                        .or_insert((machine_index as u32, parameter_index as u32));
                }
            }
        }
        for (trait_index, trait_definition) in self.traits().iter().enumerate() {
            index
                .trait_by_symbol
                .entry(trait_definition.symbol)
                .or_insert(trait_index as u32);
            for (signature_index, signature) in self
                .trait_machine_signatures(trait_definition)
                .iter()
                .enumerate()
            {
                index
                    .trait_signature
                    .entry(signature.symbol)
                    .or_insert((trait_index as u32, signature_index as u32));
            }
        }
        for (handle, parameter) in self.data_type_parameters.iter() {
            if matches!(parameter.kind, data::TypeParameterKind::Machine { .. }) {
                index
                    .parameter_contract
                    .entry(parameter.symbol)
                    .or_insert(handle);
            }
        }
        index
    }

    /// The domain-candidate spelling index the lowering stage fills and
    /// consumes. `build` receives the index to fill; it is called whenever
    /// the domain table's revision moved since the stored index was built.
    /// The index's `qualified` length doubles as its fill watermark for
    /// callers that extend it themselves.
    pub fn with_domain_candidate_index<R>(
        &self,
        build: impl FnOnce(&mut DomainCandidateIndex),
        run: impl FnOnce(&DomainCandidateIndex) -> R,
    ) -> R {
        self.declaration_indexes.domain_candidates.with_mut(
            self.tables
                .domain_definitions
                .revision()
                .wrapping_add(roster_marker(self.roots.domain_definitions)),
            || {
                let mut index = DomainCandidateIndex::default();
                build(&mut index);
                index
            },
            |index| run(index),
        )
    }

    /// The write-frame isolation verdict memo, shared by the checking
    /// crate's write-frame inference. `revision` folds in whatever tables
    /// the caller's fingerprint requires; a changed revision rebuilds the
    /// memo.
    pub fn with_isolation_verdicts<R>(
        &self,
        revision: u64,
        run: impl FnOnce(&mut IsolationVerdicts) -> R,
    ) -> R {
        self.declaration_indexes
            .isolation
            .with_mut(revision, IsolationVerdicts::default, run)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TypedTrees;
    use crate::name::Identifier;

    fn data_definition(symbol_index: u32, name: &str) -> data::DataDefinition {
        data::DataDefinition {
            symbol: SymbolHandle::from_arena_index(symbol_index),
            name: Identifier::generated(name),
            ..Default::default()
        }
    }

    /// A cached miss must become a hit when the roster grows — the index is
    /// guarded by the arena's revision, not its address or length alone.
    #[test]
    fn data_definition_index_rebuilds_after_append() {
        let mut trees = TypedTrees::default();
        let symbol = SymbolHandle::from_arena_index(41);
        assert!(
            trees
                .data_definition_positions_for_symbol(symbol)
                .is_empty()
        );
        assert!(trees.data_definition_by_symbol(symbol).is_none());

        trees.push_data_definition(data_definition(41, "Late"));

        assert_eq!(trees.data_definition_positions_for_symbol(symbol).len(), 1);
        assert_eq!(trees.data_definition_positions_for_name("Late").len(), 1);
        assert_eq!(
            trees
                .data_definition_by_symbol(symbol)
                .map(|row| row.symbol),
            Some(symbol)
        );
    }

    /// A cached hit must be re-derived when a row is rewritten in place —
    /// `Arena::get_mut` bumps the revision without changing the length.
    #[test]
    fn data_definition_index_rebuilds_after_same_length_rewrite() {
        let mut trees = TypedTrees::default();
        let old_symbol = SymbolHandle::from_arena_index(41);
        let new_symbol = SymbolHandle::from_arena_index(77);
        trees.push_data_definition(data_definition(41, "Rewritten"));

        assert_eq!(
            trees.data_definition_positions_for_symbol(old_symbol).len(),
            1
        );

        let handle = trees.roots.data_definitions.start();
        trees.tables.data_definitions.get_mut(handle).symbol = new_symbol;

        assert!(
            trees
                .data_definition_positions_for_symbol(old_symbol)
                .is_empty()
        );
        assert_eq!(
            trees.data_definition_positions_for_symbol(new_symbol).len(),
            1
        );
        // The name index keys on a different property of the same row: it
        // must survive the rewrite rather than drop its hit.
        assert_eq!(
            trees.data_definition_positions_for_name("Rewritten").len(),
            1
        );
    }

    /// A dropped program's index cannot leak into a replacement built at
    /// the same address: the index lives inside the program.
    #[test]
    fn replacement_program_starts_with_an_empty_index() {
        let mut replacement = {
            let trees = TypedTrees::default();
            let symbol = SymbolHandle::from_arena_index(41);
            assert!(
                trees
                    .data_definition_positions_for_symbol(symbol)
                    .is_empty()
            );
            trees
        };
        // The replacement deliberately pushes a *different* roster before
        // asking the same question the dropped program memoized as a miss.
        replacement.push_data_definition(data_definition(9, "Other"));
        let symbol = SymbolHandle::from_arena_index(41);
        assert!(replacement.data_definition_by_symbol(symbol).is_none());
        assert_eq!(
            replacement
                .data_definition_positions_for_symbol(SymbolHandle::from_arena_index(9))
                .len(),
            1
        );
    }

    /// Duplicate symbols keep every definition position — the roster
    /// retains duplicates by design and the index must not collapse them.
    #[test]
    fn duplicate_symbols_keep_all_positions() {
        let mut trees = TypedTrees::default();
        let symbol = SymbolHandle::from_arena_index(41);
        trees.push_data_definition(data_definition(41, "First"));
        trees.push_data_definition(data_definition(41, "Second"));

        assert_eq!(trees.data_definition_positions_for_symbol(symbol).len(), 2);
        // `data_definition_by_symbol` keeps first-match semantics, matching
        // the roster `find` the index replaced.
        assert_eq!(
            trees
                .data_definition_by_symbol(symbol)
                .map(|row| row.name.as_str().to_owned()),
            Some("First".to_owned())
        );
    }

    #[test]
    fn unique_data_owner_tracks_duplicate_rewrite_and_roster_replacement() {
        let mut trees = TypedTrees::default();
        let symbol = SymbolHandle::from_arena_index(41);
        assert!(trees.unique_data_definition_by_symbol(symbol).is_none());
        trees.push_data_definition(data_definition(41, "First"));
        assert_eq!(
            trees
                .unique_data_definition_by_symbol(symbol)
                .unwrap()
                .name
                .as_str(),
            "First"
        );
        trees.push_data_definition(data_definition(41, "Second"));
        assert!(trees.unique_data_definition_by_symbol(symbol).is_none());

        // Rewriting one duplicate restores uniqueness without changing length.
        let first = trees.roots.data_definitions.start();
        trees.tables.data_definitions.get_mut(first).symbol = SymbolHandle::invalid();
        assert!(
            trees
                .unique_data_definition_by_symbol(SymbolHandle::invalid())
                .is_none()
        );
        assert_eq!(
            trees
                .unique_data_definition_by_symbol(symbol)
                .unwrap()
                .name
                .as_str(),
            "Second"
        );

        let replacement = trees
            .tables
            .data_definitions
            .insert_many([data_definition(41, "Replacement")]);
        // Prime the index after the arena mutation but before re-pointing its
        // root: the root's identity, not just the arena revision, must refresh it.
        assert_eq!(
            trees
                .unique_data_definition_by_symbol(symbol)
                .unwrap()
                .name
                .as_str(),
            "Second"
        );
        trees.roots.data_definitions = replacement;
        assert_eq!(
            trees
                .unique_data_definition_by_symbol(symbol)
                .unwrap()
                .name
                .as_str(),
            "Replacement"
        );
    }

    /// The isolation verdict memo rebuilds when the caller's revision key
    /// moves and accumulates verdicts within a revision.
    #[test]
    fn isolation_verdicts_follow_the_revision_key() {
        let trees = TypedTrees::default();
        let key = usize::MAX;
        trees.with_isolation_verdicts(1, |verdicts| {
            verdicts.definitions.insert(key, true);
        });
        trees.with_isolation_verdicts(1, |verdicts| {
            assert_eq!(verdicts.definitions.get(&key), Some(&true));
        });
        // A moved revision forfeits the memo — the caller's fingerprint
        // decides freshness, and a stale verdict cannot ride it.
        trees.with_isolation_verdicts(2, |verdicts| {
            assert!(verdicts.definitions.is_empty());
        });
    }

    /// The structural fingerprint moves on every mutation shape a
    /// checking-phase memo could be keyed against: appends, same-length
    /// rewrites, and in-place root re-points.
    #[test]
    fn structural_fingerprint_moves_on_append_rewrite_and_repoint() {
        let mut trees = TypedTrees::default();
        let initial = trees.structural_fingerprint();
        trees.push_data_definition(data_definition(41, "Late"));
        let after_append = trees.structural_fingerprint();
        assert_ne!(initial, after_append);

        let handle = trees.roots.data_definitions.start();
        trees.tables.data_definitions.get_mut(handle).name = Identifier::generated("Renamed");
        let after_rewrite = trees.structural_fingerprint();
        assert_ne!(after_append, after_rewrite);

        // Re-pointing the root span at a fresh roster without touching the
        // arena's mutation methods still moves the fingerprint.
        let fresh = trees
            .data_definitions
            .insert_many([data_definition(42, "Other")]);
        trees.roots.data_definitions = fresh;
        assert_ne!(after_rewrite, trees.structural_fingerprint());
    }
}
