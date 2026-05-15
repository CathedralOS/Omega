use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::{SymbolHandle, SymbolKind};
use omega_typed_trees::TypedTrees;
use omega_typed_trees::data::DataMember;
use omega_typed_trees::machine::Machine;
use omega_typed_trees::platform::Platform;
use omega_typed_trees::state::State;
use omega_typed_trees::types::TypeReference;

#[derive(Debug)]
pub struct ProgramSymbols<'program> {
    data_definitions: Vec<DataDefinitionSymbol<'program>>,
    machines: Vec<MachineSymbol<'program>>,
    platforms: Vec<PlatformSymbol<'program>>,
    types: Vec<TypeSymbol<'program>>,
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
struct PlatformSymbol<'program> {
    name: &'program str,
    platform: &'program Platform,
    symbol: SymbolHandle,
}

#[derive(Debug)]
struct TypeSymbol<'program> {
    name: &'program str,
    symbol: SymbolHandle,
}

impl<'program> ProgramSymbols<'program> {
    pub fn build(program: &'program TypedTrees, diagnostics: &mut Vec<Diagnostic>) -> Self {
        let mut symbols = Self {
            data_definitions: Vec::new(),
            machines: Vec::new(),
            platforms: Vec::new(),
            types: builtin_type_symbols(program),
        };

        for data_definition in program.data_definitions() {
            if symbols
                .data_definition_symbol(data_definition.name.as_str())
                .is_valid()
            {
                diagnostics.push(Diagnostic::error(format!(
                    "duplicate data `{}`",
                    data_definition.name
                )));
            }

            symbols.data_definitions.push(DataDefinitionSymbol {
                name: data_definition.name.as_str(),
                symbol: top_level_symbol(program, data_definition.name.as_str()),
            });
            symbols.types.push(TypeSymbol {
                name: data_definition.name.as_str(),
                symbol: top_level_symbol(program, data_definition.name.as_str()),
            });
        }

        for machine in program.machines() {
            if symbols.machine(machine.name.as_str()).is_some() {
                diagnostics.push(Diagnostic::error(format!(
                    "duplicate machine `{}`",
                    machine.name
                )));
            }

            symbols.machines.push(MachineSymbol {
                name: machine.name.as_str(),
                machine,
                symbol: top_level_symbol(program, machine.name.as_str()),
            });
        }

        for platform in program.platforms() {
            if symbols.platform(platform.name.as_str()).is_some() {
                diagnostics.push(Diagnostic::error(format!(
                    "duplicate platform `{}`",
                    platform.name
                )));
            }

            if symbols
                .data_definition_symbol(platform.name.as_str())
                .is_valid()
            {
                diagnostics.push(Diagnostic::error(format!(
                    "`{}` is declared as both data and a platform",
                    platform.name
                )));
            }

            if symbols.machine(platform.name.as_str()).is_some() {
                diagnostics.push(Diagnostic::error(format!(
                    "`{}` is declared as both a machine and a platform",
                    platform.name
                )));
            }

            symbols.platforms.push(PlatformSymbol {
                name: platform.name.as_str(),
                platform,
                symbol: top_level_symbol(program, platform.name.as_str()),
            });
        }

        symbols
    }

    pub fn has_type(&self, name: &str) -> bool {
        self.type_symbol(name).is_valid()
            || self.machine_symbol(name).is_valid()
            || self.platform_symbol(name).is_valid()
            || is_builtin_shape_type(name)
    }

    fn type_symbol(&self, name: &str) -> SymbolHandle {
        self.types
            .iter()
            .find(|symbol| symbol.name == name)
            .map(|symbol| symbol.symbol)
            .unwrap_or_else(SymbolHandle::invalid)
    }

    fn data_definition_symbol(&self, name: &str) -> SymbolHandle {
        self.data_definitions
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

    fn machine_symbol(&self, name: &str) -> SymbolHandle {
        self.machines
            .iter()
            .find(|symbol| symbol.name == name)
            .map(|symbol| symbol.symbol)
            .unwrap_or_else(SymbolHandle::invalid)
    }

    pub fn platform(&self, name: &str) -> Option<&'program Platform> {
        self.platforms
            .iter()
            .find(|symbol| symbol.name == name)
            .map(|symbol| symbol.platform)
    }

    fn platform_symbol(&self, name: &str) -> SymbolHandle {
        self.platforms
            .iter()
            .find(|symbol| symbol.name == name)
            .map(|symbol| symbol.symbol)
            .unwrap_or_else(SymbolHandle::invalid)
    }

    pub fn is_callable_receiver_type(&self, name: &str) -> bool {
        self.machine_symbol(name).is_valid() || self.platform_symbol(name).is_valid()
    }
}

fn is_builtin_shape_type(name: &str) -> bool {
    matches!(name, "IndexOf" | "Real" | "Uint")
}

#[derive(Debug)]
pub struct MachineSymbols<'program> {
    contained_objects: Vec<ContainedObjectSymbol<'program>>,
    member_symbols: Vec<MemberSymbol<'program>>,
    owned_data_symbols: Vec<MemberSymbol<'program>>,
    states: Vec<StateSymbol<'program>>,
}

#[derive(Debug)]
struct ContainedObjectSymbol<'program> {
    name: &'program str,
    type_name: &'program str,
    symbol: SymbolHandle,
}

#[derive(Debug)]
struct MemberSymbol<'program> {
    name: &'program str,
    symbol: SymbolHandle,
}

#[derive(Debug)]
struct StateSymbol<'program> {
    name: &'program str,
    state: &'program State,
    symbol: SymbolHandle,
}

impl<'program> MachineSymbols<'program> {
    pub fn build(
        program: &'program TypedTrees,
        machine: &'program Machine,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Self {
        let machine_symbol = top_level_symbol(program, machine.name.as_str());
        let mut symbols = Self {
            contained_objects: Vec::new(),
            member_symbols: Vec::new(),
            owned_data_symbols: Vec::new(),
            states: Vec::new(),
        };

        if let Some(data_definition) = program
            .data_definitions()
            .iter()
            .find(|definition| definition.name == machine.name)
        {
            for member in &data_definition.members {
                let DataMember::Field(field) = member else {
                    continue;
                };

                if symbols.has_member(field.name.as_str()) {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{}` has duplicate member `{}`",
                        machine.name, field.name
                    )));
                    continue;
                }

                let symbol = child_symbol(program, machine_symbol, field.name.as_str());
                symbols.member_symbols.push(MemberSymbol {
                    name: field.name.as_str(),
                    symbol,
                });
                symbols.owned_data_symbols.push(MemberSymbol {
                    name: field.name.as_str(),
                    symbol,
                });

                if let Some(type_name) = callable_receiver_type_name(&field.type_reference) {
                    symbols.contained_objects.push(ContainedObjectSymbol {
                        name: field.name.as_str(),
                        type_name,
                        symbol,
                    });
                }
            }
        }

        for contained_object in &machine.contains {
            if symbols.has_member(contained_object.name.as_str()) {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` has duplicate member `{}`",
                    machine.name, contained_object.name
                )));
            }

            if symbols
                .contained_symbol(contained_object.name.as_str())
                .is_valid()
            {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` has duplicate contained object `{}`",
                    machine.name, contained_object.name
                )));
            }

            let symbol = child_symbol(program, machine_symbol, contained_object.name.as_str());
            symbols.member_symbols.push(MemberSymbol {
                name: contained_object.name.as_str(),
                symbol,
            });
            symbols.contained_objects.push(ContainedObjectSymbol {
                name: contained_object.name.as_str(),
                type_name: contained_object.type_name.as_str(),
                symbol,
            });
        }

        for owned_data in &machine.owned_data {
            if symbols.has_member(owned_data.name.as_str()) {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` has duplicate member `{}`",
                    machine.name, owned_data.name
                )));
            }

            if symbols.has_owned_data(owned_data.name.as_str()) {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` has duplicate owned data `{}`",
                    machine.name, owned_data.name
                )));
            }

            let symbol = child_symbol(program, machine_symbol, owned_data.name.as_str());
            symbols.member_symbols.push(MemberSymbol {
                name: owned_data.name.as_str(),
                symbol,
            });
            symbols.owned_data_symbols.push(MemberSymbol {
                name: owned_data.name.as_str(),
                symbol,
            });
        }

        for state in &machine.states {
            if symbols.has_state(state.name.as_str()) {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` has duplicate state `{}`",
                    machine.name, state.name
                )));
            }

            symbols.states.push(StateSymbol {
                name: state.name.as_str(),
                state,
                symbol: child_symbol(program, machine_symbol, state.name.as_str()),
            });
        }

        symbols
    }

    pub fn state(&self, name: &str) -> Option<&'program State> {
        self.states
            .iter()
            .find(|symbol| symbol.name == name)
            .map(|symbol| symbol.state)
    }

    fn state_symbol(&self, name: &str) -> SymbolHandle {
        self.states
            .iter()
            .find(|symbol| symbol.name == name)
            .map(|symbol| symbol.symbol)
            .unwrap_or_else(SymbolHandle::invalid)
    }

    pub fn contained_type(&self, name: &str) -> Option<&'program str> {
        self.contained_objects
            .iter()
            .find(|symbol| symbol.name == name)
            .map(|symbol| symbol.type_name)
    }

    fn contained_symbol(&self, name: &str) -> SymbolHandle {
        self.contained_objects
            .iter()
            .find(|symbol| symbol.name == name)
            .map(|symbol| symbol.symbol)
            .unwrap_or_else(SymbolHandle::invalid)
    }

    pub fn has_state(&self, name: &str) -> bool {
        self.state_symbol(name).is_valid()
    }

    pub fn has_member(&self, name: &str) -> bool {
        self.member_symbol(name).is_valid()
    }

    pub fn has_owned_data(&self, name: &str) -> bool {
        self.owned_data_symbol(name).is_valid()
    }

    pub fn member_symbol(&self, name: &str) -> SymbolHandle {
        self.member_symbols
            .iter()
            .find(|symbol| symbol.name == name)
            .map(|symbol| symbol.symbol)
            .unwrap_or_else(SymbolHandle::invalid)
    }

    pub fn owned_data_symbol(&self, name: &str) -> SymbolHandle {
        self.owned_data_symbols
            .iter()
            .find(|symbol| symbol.name == name)
            .map(|symbol| symbol.symbol)
            .unwrap_or_else(SymbolHandle::invalid)
    }
}

fn callable_receiver_type_name(type_reference: &TypeReference) -> Option<&str> {
    match type_reference {
        TypeReference::Reference { referee, .. } => callable_receiver_type_name(referee),
        TypeReference::Constrained { base_type, .. } => callable_receiver_type_name(base_type),
        TypeReference::FixedArray { .. } | TypeReference::Slice { .. } => None,
        TypeReference::Generic { .. } => None,
        TypeReference::Named { name, .. } => Some(name.as_str()),
        TypeReference::Unit => None,
    }
}

fn top_level_symbol(program: &TypedTrees, name: &str) -> SymbolHandle {
    child_symbol(program, program.symbols.root(), name)
}

fn builtin_type_symbols(program: &TypedTrees) -> Vec<TypeSymbol<'_>> {
    let Some(root_children) = program.symbols.child_handles(program.symbols.root()) else {
        return Vec::new();
    };

    root_children
        .filter_map(|symbol| {
            let kind = program.symbols.get(symbol).kind;

            if kind != SymbolKind::BuiltinType {
                return None;
            }

            Some(TypeSymbol {
                name: program.symbols.name(symbol),
                symbol,
            })
        })
        .collect()
}

fn child_symbol(program: &TypedTrees, parent: SymbolHandle, name: &str) -> SymbolHandle {
    if !parent.is_valid() {
        return SymbolHandle::invalid();
    }

    let Some(children) = program.symbols.child_handles(parent) else {
        return SymbolHandle::invalid();
    };

    for child in children {
        if program.symbols.name(child) == name {
            return child;
        }
    }

    SymbolHandle::invalid()
}
