use diagnostics::Diagnostic;
use std::sync::Mutex;
use symbols::{SymbolHandle, SymbolKeyMap as HashMap, SymbolKind};
use typed_trees::machine::Machine;
use typed_trees::state::State;
use typed_trees::trait_definition::TraitDefinition;

/// Memoized caller-prefix resolution for write-origin walks. The caller-side
/// queries repeat the same machine-level scans for every statement site:
/// whether a machine declares tracked origins at all, which (state, index)
/// a call/statement/expression site names, and the resulting prefix site.
/// All three are program-pure identities; they live here beside the other
/// program-level symbol indexes rather than rescanned per demand query.
#[derive(Debug, Default)]
pub(crate) struct CallerSiteCaches {
    /// machine symbol -> does any state declare tracked origins (incoming
    /// carriers or origin-declaring locals). The machine-level `.any` scan
    /// this replaces walks every statement of every state per call.
    pub tracked_origin_machines: Mutex<HashMap<SymbolHandle, bool>>,
    /// (machine symbol, site kind, site identity) -> (state symbol, index)
    /// for the unique statement a caller write site resolves to.
    pub site_statements: Mutex<HashMap<(SymbolHandle, u8, usize), Option<(SymbolHandle, usize)>>>,
    /// (machine symbol, site kind, site identity) -> the resolved prefix
    /// site outcome; None in the map encodes "no unique statement".
    pub prefix_site_entries: Mutex<HashMap<(SymbolHandle, u8, usize), Option<PrefixSiteEntry>>>,
    /// Complete, context-independent state effects shared by resolver and
    /// nested origin queries. Never holds contextual prefixes or failures.
    complete_state_summaries: Mutex<HashMap<SymbolHandle, Vec<String>>>,
}

impl CallerSiteCaches {
    /// Borrow the memo only while copying a completed row. Holding this lock
    /// across inference forced recursive queries to discard solved work into
    /// private tables. Active-state guards belong to callers, before lookup:
    /// callers may then push the target while deriving its complete summary.
    /// A poisoned memo is a miss; inference still computes the required effect.
    pub(crate) fn completed_state_writes(&self, state: SymbolHandle) -> Option<Vec<String>> {
        self.complete_state_summaries
            .lock()
            .ok()?
            .get(&state)
            .cloned()
    }

    /// Publish only after a successful Complete walk or a finite cycle's fixed
    /// point. No lock spans derivation, so nested/concurrent misses may repeat
    /// work but cannot observe partial evidence. The enclosing frozen-program
    /// identity bounds reuse; result-origin relations retain their own scopes.
    pub(crate) fn record_completed_state_writes(&self, state: SymbolHandle, writes: &[String]) {
        if let Ok(mut summaries) = self.complete_state_summaries.lock() {
            summaries.entry(state).or_insert_with(|| writes.to_vec());
        }
    }
}

/// The memoizable shape of `CallerPrefixSite`: `Tracked` retains the state's
/// symbol and statement index, and callers rebuild the borrowed `State` and
/// `StatementNode` from the program on a hit.
#[derive(Debug, Clone, Copy)]
pub(crate) enum PrefixSiteEntry {
    Untracked,
    Tracked { state: SymbolHandle, index: usize },
}

#[derive(Debug)]
pub struct TopLevelSymbols<'program> {
    machines: Vec<MachineSymbol<'program>>,
    traits: Vec<TraitSymbol<'program>>,
    types: Vec<TypeSymbol<'program>>,
    /// Demand caches for the write-frame/caller-alias queries. Inserted lazily
    /// by readers; every index built for one frozen program shares them.
    pub(crate) caller_sites: std::sync::Arc<CallerSiteCaches>,
}

#[derive(Debug)]
struct MachineSymbol<'program> {
    name: &'program str,
    machine: &'program Machine,
    symbol: SymbolHandle,
}

#[derive(Debug)]
struct TraitSymbol<'program> {
    name: &'program str,
    trait_definition: &'program TraitDefinition,
    symbol: SymbolHandle,
}

#[derive(Debug)]
struct TypeSymbol<'program> {
    name: &'program str,
    symbol: SymbolHandle,
}

impl<'program> TopLevelSymbols<'program> {
    pub fn build(
        program: &'program dyn crate::CheckingInput,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Self {
        let data_definition_count = program.data_definitions().len();
        let machine_count = program.machines().len();
        let trait_count = program.traits().len();
        let mut symbols = Self {
            machines: Vec::with_capacity(machine_count),
            traits: Vec::with_capacity(trait_count),
            types: builtin_type_symbols(program),
            caller_sites: crate::frozen_program::frozen_memo(
                program,
                |memos| &memos.caller_sites,
                CallerSiteCaches::default,
            ),
        };
        symbols.types.reserve(data_definition_count + trait_count);

        // Prior declarations bucketed by name: every conflict check below
        // used to scan the whole accumulated roster per new declaration.
        let mut data_by_name: symbols::SymbolKeyMap<&str, Vec<SymbolHandle>> =
            symbols::SymbolKeyMap::default();
        let mut machines_by_name: symbols::SymbolKeyMap<
            &str,
            Vec<&'program typed_trees::machine::Machine>,
        > = symbols::SymbolKeyMap::default();
        let mut traits_by_name: symbols::SymbolKeyMap<&str, Vec<SymbolHandle>> =
            symbols::SymbolKeyMap::default();

        for data_definition in program.data_definitions() {
            let conflicts =
                data_by_name
                    .get(data_definition.name.as_str())
                    .is_some_and(|previous| {
                        previous.iter().any(|previous| {
                            !program
                                .symbols
                                .source_scopes_separate(*previous, data_definition.symbol)
                        })
                    });
            if conflicts {
                diagnostics.push(Diagnostic::error(format!(
                    "duplicate data `{}`",
                    data_definition.name
                )));
            }

            symbols.types.push(TypeSymbol {
                name: data_definition.name.as_str(),
                symbol: data_definition.symbol,
            });
            data_by_name
                .entry(data_definition.name.as_str())
                .or_default()
                .push(data_definition.symbol);
        }

        for machine in program.machines() {
            let same_named = machines_by_name
                .get(machine.name.as_str())
                .map(|previous| {
                    previous
                        .iter()
                        .filter(|previous| {
                            let previous_origin = program.symbols.authored_origin(previous.symbol);
                            let current_origin = program.symbols.authored_origin(machine.symbol);
                            program.symbols.get(previous_origin).parent
                                == program.symbols.get(current_origin).parent
                                && !program
                                    .symbols
                                    .source_scopes_separate(previous.symbol, machine.symbol)
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let identity = program.machine_callable_identity(machine);
            let is_result_overload_family = !same_named.is_empty()
                && identity.is_valid()
                && same_named.iter().all(|previous| {
                    program.callable_identities_share_family(
                        program.machine_callable_identity(previous),
                        identity,
                    )
                });
            let is_boundary_token_overload_family = machine.spelling.is_some()
                && machine.supply_mode
                    == language_semantics::MachineSupplyMode::TopLevelRequirement
                && identity.is_valid()
                && same_named.iter().all(|previous| {
                    let previous_identity = program.machine_callable_identity(previous);
                    previous.spelling == machine.spelling
                        && previous.supply_mode
                            == language_semantics::MachineSupplyMode::TopLevelRequirement
                        && program.callable_identities_share_path(previous_identity, identity)
                        && !program.callable_identities_share_family(previous_identity, identity)
                });
            if !same_named.is_empty()
                && !is_result_overload_family
                && !is_boundary_token_overload_family
            {
                diagnostics.push(Diagnostic::error(format!(
                    "duplicate machine `{}`",
                    machine.name
                )));
            }

            symbols.machines.push(MachineSymbol {
                name: machine.name.as_str(),
                machine,
                symbol: machine.symbol,
            });
            machines_by_name
                .entry(machine.name.as_str())
                .or_default()
                .push(machine);
        }

        for trait_definition in program.traits() {
            if traits_by_name
                .get(trait_definition.name.as_str())
                .is_some_and(|previous| {
                    previous.iter().any(|previous| {
                        !program
                            .symbols
                            .source_scopes_separate(*previous, trait_definition.symbol)
                    })
                })
            {
                diagnostics.push(Diagnostic::error(format!(
                    "duplicate trait `{}`",
                    trait_definition.name
                )));
            }

            if data_by_name
                .get(trait_definition.name.as_str())
                .is_some_and(|previous| {
                    previous.iter().any(|previous| {
                        !program
                            .symbols
                            .source_scopes_separate(*previous, trait_definition.symbol)
                    })
                })
            {
                diagnostics.push(Diagnostic::error(format!(
                    "`{}` is declared as both data and a trait",
                    trait_definition.name
                )));
            }

            if machines_by_name
                .get(trait_definition.name.as_str())
                .is_some_and(|previous| {
                    previous.iter().any(|previous| {
                        !program
                            .symbols
                            .source_scopes_separate(previous.symbol, trait_definition.symbol)
                    })
                })
            {
                diagnostics.push(Diagnostic::error(format!(
                    "`{}` is declared as both a machine and a trait",
                    trait_definition.name
                )));
            }

            symbols.traits.push(TraitSymbol {
                name: trait_definition.name.as_str(),
                trait_definition,
                symbol: trait_definition.symbol,
            });
            symbols.types.push(TypeSymbol {
                name: trait_definition.name.as_str(),
                symbol: trait_definition.symbol,
            });
            traits_by_name
                .entry(trait_definition.name.as_str())
                .or_default()
                .push(trait_definition.symbol);
        }

        symbols
    }

    pub fn has_type(&self, name: &str) -> bool {
        self.type_symbol(name).is_valid()
            || self.machine_symbol(name).is_valid()
            || self.trait_symbol(name).is_valid()
    }

    pub fn has_type_symbol(&self, symbol: SymbolHandle) -> bool {
        symbol.is_valid()
            && (self
                .types
                .iter()
                .any(|candidate| candidate.symbol == symbol)
                || self
                    .machines
                    .iter()
                    .any(|candidate| candidate.symbol == symbol)
                || self
                    .traits
                    .iter()
                    .any(|candidate| candidate.symbol == symbol))
    }

    pub fn trait_definition_by_symbol(
        &self,
        symbol: SymbolHandle,
    ) -> Option<&'program TraitDefinition> {
        self.traits
            .iter()
            .find(|candidate| candidate.symbol == symbol)
            .map(|candidate| candidate.trait_definition)
    }

    fn type_symbol(&self, name: &str) -> SymbolHandle {
        self.types
            .iter()
            .find(|symbol| symbol.name == name)
            .map(|symbol| symbol.symbol)
            .unwrap_or_else(SymbolHandle::invalid)
    }

    pub fn machine(&self, name: &str) -> Option<&'program Machine> {
        self.machines
            .iter()
            .find(|symbol| symbol.name == name)
            .map(|symbol| symbol.machine)
    }

    pub fn attached_machine_state(
        &self,
        program: &'program dyn crate::CheckingInput,
        data_name: &str,
        state_name: &str,
    ) -> Option<(&'program Machine, &'program State)> {
        self.machines.iter().find_map(|symbol| {
            (symbol
                .machine
                .attached_data
                .as_ref()
                .is_some_and(|attached_data| attached_data.as_str() == data_name))
            .then(|| {
                program
                    .machine_states(symbol.machine)
                    .first()
                    .filter(|state| state.name.as_str() == state_name)
                    .map(|state| (symbol.machine, state))
            })?
        })
    }

    fn machine_symbol(&self, name: &str) -> SymbolHandle {
        self.machines
            .iter()
            .find(|symbol| symbol.name == name)
            .map(|symbol| symbol.symbol)
            .unwrap_or_else(SymbolHandle::invalid)
    }

    pub fn trait_definition(&self, name: &str) -> Option<&'program TraitDefinition> {
        self.traits
            .iter()
            .find(|symbol| symbol.name == name)
            .map(|symbol| symbol.trait_definition)
    }

    fn trait_symbol(&self, name: &str) -> SymbolHandle {
        self.traits
            .iter()
            .find(|symbol| symbol.name == name)
            .map(|symbol| symbol.symbol)
            .unwrap_or_else(SymbolHandle::invalid)
    }
}

fn builtin_type_symbols(program: &dyn crate::CheckingInput) -> Vec<TypeSymbol<'_>> {
    let Some(root_children) = program.symbols.child_handles(program.symbols.root()) else {
        return Vec::new();
    };
    let (min_count, _) = root_children.size_hint();
    let mut symbols = Vec::with_capacity(min_count);

    for symbol in root_children {
        let kind = program.symbols.get(symbol).kind;

        if kind != SymbolKind::BuiltinType {
            continue;
        }

        symbols.push(TypeSymbol {
            name: program.symbols.name(symbol),
            symbol,
        });
    }

    symbols
}
