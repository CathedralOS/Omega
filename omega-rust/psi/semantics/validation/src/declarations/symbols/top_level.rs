use diagnostics::Diagnostic;
use std::collections::HashMap;
use std::sync::Mutex;
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::TypedTrees;
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
    data_definitions: Vec<DataDefinitionSymbol<'program>>,
    machines: Vec<MachineSymbol<'program>>,
    traits: Vec<TraitSymbol<'program>>,
    types: Vec<TypeSymbol<'program>>,
    /// Demand caches for the write-frame/caller-alias queries. Inserted lazily
    /// by readers; `build` leaves them empty.
    pub(crate) caller_sites: CallerSiteCaches,
}

#[derive(Debug)]
struct DataDefinitionSymbol<'program> {
    name: &'program str,
    symbol: SymbolHandle,
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
    pub fn build(program: &'program TypedTrees, diagnostics: &mut Vec<Diagnostic>) -> Self {
        let data_definition_count = program.data_definitions().len();
        let machine_count = program.machines().len();
        let trait_count = program.traits().len();
        let mut symbols = Self {
            data_definitions: Vec::with_capacity(data_definition_count),
            machines: Vec::with_capacity(machine_count),
            traits: Vec::with_capacity(trait_count),
            types: builtin_type_symbols(program),
            caller_sites: CallerSiteCaches::default(),
        };
        symbols.types.reserve(data_definition_count + trait_count);

        for data_definition in program.data_definitions() {
            let conflicts = symbols.data_definitions.iter().any(|previous| {
                previous.name == data_definition.name.as_str()
                    && !program
                        .symbols
                        .source_scopes_separate(previous.symbol, data_definition.symbol)
            });
            if conflicts {
                diagnostics.push(Diagnostic::error(format!(
                    "duplicate data `{}`",
                    data_definition.name
                )));
            }

            symbols.data_definitions.push(DataDefinitionSymbol {
                name: data_definition.name.as_str(),
                symbol: data_definition.symbol,
            });
            symbols.types.push(TypeSymbol {
                name: data_definition.name.as_str(),
                symbol: data_definition.symbol,
            });
        }

        for machine in program.machines() {
            let same_named = symbols
                .machines
                .iter()
                .filter(|symbol| {
                    symbol.name == machine.name.as_str()
                        && !program
                            .symbols
                            .source_scopes_separate(symbol.symbol, machine.symbol)
                })
                .collect::<Vec<_>>();
            let is_result_overload_family = !same_named.is_empty()
                && program
                    .normalized_machine_overload_identity(machine)
                    .is_some_and(|identity| {
                        same_named.iter().all(|previous| {
                            program
                                .normalized_machine_overload_identity(previous.machine)
                                .is_some_and(|previous_identity| {
                                    previous_identity.path() == identity.path()
                                        && previous_identity.parameters() == identity.parameters()
                                })
                        })
                    });
            if !same_named.is_empty() && !is_result_overload_family {
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
        }

        for trait_definition in program.traits() {
            if symbols.traits.iter().any(|previous| {
                previous.name == trait_definition.name.as_str()
                    && !program
                        .symbols
                        .source_scopes_separate(previous.symbol, trait_definition.symbol)
            }) {
                diagnostics.push(Diagnostic::error(format!(
                    "duplicate trait `{}`",
                    trait_definition.name
                )));
            }

            if symbols.data_definitions.iter().any(|previous| {
                previous.name == trait_definition.name.as_str()
                    && !program
                        .symbols
                        .source_scopes_separate(previous.symbol, trait_definition.symbol)
            }) {
                diagnostics.push(Diagnostic::error(format!(
                    "`{}` is declared as both data and a trait",
                    trait_definition.name
                )));
            }

            if symbols.machines.iter().any(|previous| {
                previous.name == trait_definition.name.as_str()
                    && !program
                        .symbols
                        .source_scopes_separate(previous.symbol, trait_definition.symbol)
            }) {
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
        program: &'program TypedTrees,
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
                    .iter()
                    .find(|state| state.name.as_str() == state_name)
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

fn builtin_type_symbols(program: &TypedTrees) -> Vec<TypeSymbol<'_>> {
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
