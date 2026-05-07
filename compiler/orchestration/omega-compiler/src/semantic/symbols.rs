use crate::diagnostics::Diagnostic;
use crate::ir::Program;
use crate::ir::data::DataDefinition;
use crate::ir::machine::Machine;
use crate::ir::platform::Platform;
use crate::ir::state::State;

#[derive(Debug)]
pub struct ProgramSymbols<'program> {
    data_definitions: Vec<DataDefinitionSymbol<'program>>,
    machines: Vec<MachineSymbol<'program>>,
    platforms: Vec<PlatformSymbol<'program>>,
}

#[derive(Debug)]
struct DataDefinitionSymbol<'program> {
    name: &'program str,
    definition: &'program DataDefinition,
}

#[derive(Debug)]
struct MachineSymbol<'program> {
    name: &'program str,
    machine: &'program Machine,
}

#[derive(Debug)]
struct PlatformSymbol<'program> {
    name: &'program str,
    platform: &'program Platform,
}

impl<'program> ProgramSymbols<'program> {
    pub fn build(program: &'program Program, diagnostics: &mut Vec<Diagnostic>) -> Self {
        let mut symbols = Self {
            data_definitions: Vec::new(),
            machines: Vec::new(),
            platforms: Vec::new(),
        };

        for data_definition in &program.data_definitions {
            if symbols
                .data_definition(data_definition.name.as_str())
                .is_some()
            {
                diagnostics.push(Diagnostic::error(format!(
                    "duplicate data `{}`",
                    data_definition.name
                )));
            }

            symbols.data_definitions.push(DataDefinitionSymbol {
                name: data_definition.name.as_str(),
                definition: data_definition,
            });
        }

        for machine in &program.machines {
            if symbols.machine(machine.name.as_str()).is_some() {
                diagnostics.push(Diagnostic::error(format!(
                    "duplicate machine `{}`",
                    machine.name
                )));
            }

            if symbols.data_definition(machine.name.as_str()).is_some() {
                diagnostics.push(Diagnostic::error(format!(
                    "`{}` is declared as both data and a machine",
                    machine.name
                )));
            }

            symbols.machines.push(MachineSymbol {
                name: machine.name.as_str(),
                machine,
            });
        }

        for platform in &program.platforms {
            if symbols.platform(platform.name.as_str()).is_some() {
                diagnostics.push(Diagnostic::error(format!(
                    "duplicate platform `{}`",
                    platform.name
                )));
            }

            if symbols.data_definition(platform.name.as_str()).is_some() {
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
            });
        }

        symbols
    }

    pub fn has_data_definition(&self, name: &str) -> bool {
        self.data_definition(name).is_some()
    }

    pub fn machine(&self, name: &str) -> Option<&'program Machine> {
        self.machines
            .iter()
            .find(|symbol| symbol.name == name)
            .map(|symbol| symbol.machine)
    }

    pub fn platform(&self, name: &str) -> Option<&'program Platform> {
        self.platforms
            .iter()
            .find(|symbol| symbol.name == name)
            .map(|symbol| symbol.platform)
    }

    pub fn is_callable_receiver_type(&self, name: &str) -> bool {
        self.machine(name).is_some() || self.platform(name).is_some()
    }

    fn data_definition(&self, name: &str) -> Option<&'program DataDefinition> {
        self.data_definitions
            .iter()
            .find(|symbol| symbol.name == name)
            .map(|symbol| symbol.definition)
    }
}

#[derive(Debug)]
pub struct MachineSymbols<'program> {
    contained_objects: Vec<ContainedObjectSymbol<'program>>,
    member_names: Vec<&'program str>,
    owned_data_names: Vec<&'program str>,
    states: Vec<StateSymbol<'program>>,
}

#[derive(Debug)]
struct ContainedObjectSymbol<'program> {
    name: &'program str,
    type_name: &'program str,
}

#[derive(Debug)]
struct StateSymbol<'program> {
    name: &'program str,
    state: &'program State,
}

impl<'program> MachineSymbols<'program> {
    pub fn build(machine: &'program Machine, diagnostics: &mut Vec<Diagnostic>) -> Self {
        let mut symbols = Self {
            contained_objects: Vec::new(),
            member_names: Vec::new(),
            owned_data_names: Vec::new(),
            states: Vec::new(),
        };

        for contained_object in &machine.contains {
            if symbols.has_member(contained_object.name.as_str()) {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` has duplicate member `{}`",
                    machine.name, contained_object.name
                )));
            }

            if symbols
                .contained_type(contained_object.name.as_str())
                .is_some()
            {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` has duplicate contained object `{}`",
                    machine.name, contained_object.name
                )));
            }

            symbols.member_names.push(contained_object.name.as_str());
            symbols.contained_objects.push(ContainedObjectSymbol {
                name: contained_object.name.as_str(),
                type_name: contained_object.type_name.as_str(),
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

            symbols.member_names.push(owned_data.name.as_str());
            symbols.owned_data_names.push(owned_data.name.as_str());
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

    pub fn contained_type(&self, name: &str) -> Option<&'program str> {
        self.contained_objects
            .iter()
            .find(|symbol| symbol.name == name)
            .map(|symbol| symbol.type_name)
    }

    pub fn has_state(&self, name: &str) -> bool {
        self.state(name).is_some()
    }

    pub fn has_member(&self, name: &str) -> bool {
        self.member_names.contains(&name)
    }

    pub fn has_owned_data(&self, name: &str) -> bool {
        self.owned_data_names.contains(&name)
    }
}
