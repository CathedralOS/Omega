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

mod relationships;
mod requirement_applications;
use relationships::{DataMemberIndex, StateDeclarationIndex};
use requirement_applications::RequirementApplicationIndex;

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
        .wrapping_add(tables.data_application_definitions.revision())
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
        .wrapping_add(tables.applied_callable_signatures.revision())
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
/// every machine. Stable symbol rows preserve all declarations; the common
/// lookup takes the first, while ambiguity queries read the complete group.
#[derive(Clone, Default)]
struct MachinePositionIndex {
    rows: Vec<(SymbolHandle, u32)>,
}

/// Stored states in full-symbol order, retaining their machine and local
/// positions. Stable sorting preserves authored order, including duplicates;
/// first-match, entry-only and ambiguity readers use the same relationship.
#[derive(Clone, Default)]
struct StatePositionIndex {
    rows: Vec<(SymbolHandle, (u32, u32))>,
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

/// Exact type-parameter declaration handles, in symbol order. A flat arena
/// includes data, machine and trait parameters; call-target indexes cover only
/// machines and cannot answer this relationship. Last duplicate wins, matching
/// the declaration table consumed by custody queries before this index existed.
#[derive(Clone, Default)]
struct TypeParameterIndex {
    rows: Vec<(SymbolHandle, Handle<data::TypeParameter>)>,
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
/// handle-plus-flag questions, `storage_match` handle pairs, and `reference_free` /
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
/// unrelated lookups from contending. Machine/state topology keys retain the
/// arena revisions and full visible root span separately: shortening a roster
/// must not cancel an arena mutation or hide a root generation change.
#[derive(Clone, Default)]
pub struct DeclarationIndexes {
    machines: OwnedIndex<MachinePositionIndex, (u64, HandleSpan<machine::Machine>)>,
    states: OwnedIndex<StatePositionIndex, (u64, u64, HandleSpan<machine::Machine>)>,
    state_declarations: OwnedIndex<StateDeclarationIndex>,
    data_members: OwnedIndex<DataMemberIndex>,
    data_definitions: OwnedIndex<DataDefinitionIndex>,
    type_parameters: OwnedIndex<TypeParameterIndex>,
    domains: OwnedIndex<DomainPositionIndex>,
    attached_drops: OwnedIndex<AttachedDropIndex>,
    call_targets: OwnedIndex<CallTargetIndex>,
    requirement_applications: OwnedIndex<RequirementApplicationIndex>,
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
            .wrapping_add(self.placed_borrow_factories.len() as u64)
            .wrapping_add(self.wire_placements.revision())
            .wrapping_add(self.wire_encode_obligations.revision())
            .wrapping_add(self.wire_schema_plans.len() as u64)
            .wrapping_add(self.machine_specializations.len() as u64)
            .wrapping_add(self.boundary_completion_declarations.revision())
            .wrapping_add(self.evidence_forwardings.len() as u64)
            .wrapping_add(self.proof_output_calls.len() as u64)
            .wrapping_add(self.ranking_expression_custody.len() as u64)
            .wrapping_add(self.pending_const_range_endpoints.len() as u64)
            .wrapping_add(self.constant_recipes.revision())
            .wrapping_add(self.constant_recipe_closure.values.len() as u64)
            .wrapping_add(self.excluded_authored_selection_occurrences.fingerprint())
    }

    /// Machine-table position of `symbol`, or `None` when no machine carries
    /// it. The index keeps the first position per symbol — the roster `find`
    /// it replaces had the same earliest-match rule.
    pub fn machine_position_by_symbol(&self, symbol: SymbolHandle) -> Option<usize> {
        if !symbol.is_valid() {
            return None;
        }
        self.with_machine_position_index(|index| {
            relationships::first_symbol_row(&index.rows, symbol)
                .map(|(_, position)| *position as usize)
        })
    }

    /// Every machine carrying the exact symbol, in declaration order. Unlike
    /// the first-match query, ambiguity consumers need all duplicate rows.
    pub fn machine_positions_for_symbol(&self, symbol: SymbolHandle) -> Vec<u32> {
        self.with_machine_position_index(|index| {
            relationships::symbol_rows(&index.rows, symbol)
                .iter()
                .map(|(_, position)| *position)
                .collect()
        })
    }

    fn with_machine_position_index<R>(&self, run: impl FnOnce(&MachinePositionIndex) -> R) -> R {
        self.declaration_indexes.machines.with(
            (self.tables.machines.revision(), self.roots.machines),
            || {
                let mut rows = self
                    .machines()
                    .iter()
                    .enumerate()
                    .map(|(position, machine)| (machine.symbol, position as u32))
                    .collect::<Vec<_>>();
                relationships::sort_symbol_rows(&mut rows);
                MachinePositionIndex { rows }
            },
            run,
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
        self.with_state_position_index(|index| {
            relationships::symbol_rows(&index.rows, state)
                .iter()
                .find(|(_, (machine_position, _))| {
                    self.machines()[*machine_position as usize].symbol == machine
                })
                .map(|(_, (_, state_position))| *state_position as usize)
        })
    }

    fn with_state_position_index<R>(&self, run: impl FnOnce(&StatePositionIndex) -> R) -> R {
        self.declaration_indexes.states.with(
            (
                self.tables.machines.revision(),
                self.tables.machine_states.revision(),
                self.roots.machines,
            ),
            || self.build_state_positions(),
            run,
        )
    }

    /// All stored occurrences, in authored traversal order. Copy only matching
    /// coordinates so consumers can query declarations without retaining an
    /// index lock. Symbol-table parent edges are not storage membership.
    pub(crate) fn state_positions_for_symbol(&self, symbol: SymbolHandle) -> Vec<(u32, u32)> {
        self.with_state_position_index(|index| {
            relationships::symbol_rows(&index.rows, symbol)
                .iter()
                .map(|(_, position)| *position)
                .collect()
        })
    }

    /// First machine whose own symbol or stored state names `symbol`, matching
    /// a declaration-order roster search even for duplicate declarations.
    pub fn machine_by_own_or_state_symbol(
        &self,
        symbol: SymbolHandle,
    ) -> Option<&machine::Machine> {
        let direct = self.with_machine_position_index(|index| {
            relationships::first_symbol_row(&index.rows, symbol).map(|(_, position)| *position)
        });
        let containing = self.with_state_position_index(|index| {
            relationships::first_symbol_row(&index.rows, symbol)
                .map(|(_, (machine_position, _))| *machine_position)
        });
        let position = match (direct, containing) {
            (Some(direct), Some(containing)) => direct.min(containing),
            (direct, containing) => direct.or(containing)?,
        };
        Some(&self.machines()[position as usize])
    }

    /// First stored entry carrying this symbol, not a machine head or an
    /// internal state. Duplicate non-entry states do not hide a later entry.
    pub fn machine_by_entry_state_symbol(
        &self,
        symbol: SymbolHandle,
    ) -> Option<(&machine::Machine, &crate::state::State)> {
        let machine_position = self.with_state_position_index(|index| {
            relationships::symbol_rows(&index.rows, symbol)
                .iter()
                .find(|(_, (_, state_position))| *state_position == 0)
                .map(|(_, (machine_position, _))| *machine_position)
        })?;
        let machine = &self.machines()[machine_position as usize];
        Some((machine, &self.machine_states(machine)[0]))
    }

    fn build_state_positions(&self) -> StatePositionIndex {
        let mut index = StatePositionIndex::default();
        for (machine_position, machine) in self.machines().iter().enumerate() {
            for (state_position, state) in self.machine_states(machine).iter().enumerate() {
                index.rows.push((
                    state.symbol,
                    (machine_position as u32, state_position as u32),
                ));
            }
        }
        relationships::sort_symbol_rows(&mut index.rows);
        index
    }

    /// Resolve an exact parameter symbol without rebuilding a custody-owned
    /// copy of the declaration table. Generations stay part of the handle.
    pub fn data_type_parameter_by_symbol(
        &self,
        symbol: SymbolHandle,
    ) -> Option<&data::TypeParameter> {
        if !symbol.is_valid() {
            return None;
        }
        let handle = self.declaration_indexes.type_parameters.with(
            self.data_type_parameters.revision(),
            || {
                let mut rows: Vec<_> = self
                    .data_type_parameters
                    .iter()
                    .filter(|(_, parameter)| parameter.symbol.is_valid())
                    .map(|(handle, parameter)| (parameter.symbol, handle))
                    .collect();
                rows.sort_by_key(|(symbol, _)| (symbol.arena_index(), symbol.generation()));
                rows.dedup_by(|later, earlier| {
                    if later.0 == earlier.0 {
                        *earlier = *later;
                        true
                    } else {
                        false
                    }
                });
                TypeParameterIndex { rows }
            },
            |index| {
                index
                    .rows
                    .binary_search_by_key(
                        &(symbol.arena_index(), symbol.generation()),
                        |(symbol, _)| (symbol.arena_index(), symbol.generation()),
                    )
                    .ok()
                    .map(|position| index.rows[position].1)
            },
        )?;
        Some(self.data_type_parameters.get(handle))
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
