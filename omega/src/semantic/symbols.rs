use std::collections::{HashMap, HashSet};

use crate::diagnostics::Diagnostic;
use crate::ir::Program;
use crate::ir::data::DataDefinition;
use crate::ir::machine::{CommandDefinition, Machine};
use crate::ir::platform::Platform;

#[derive(Debug)]
pub struct ProgramSymbols<'program> {
    data_definitions: HashMap<&'program str, &'program DataDefinition>,
    machines: HashMap<&'program str, &'program Machine>,
    platforms: HashMap<&'program str, &'program Platform>,
}

impl<'program> ProgramSymbols<'program> {
    pub fn build(program: &'program Program, diagnostics: &mut Vec<Diagnostic>) -> Self {
        let mut symbols = Self {
            data_definitions: HashMap::new(),
            machines: HashMap::new(),
            platforms: HashMap::new(),
        };

        for data_definition in &program.data_definitions {
            if symbols
                .data_definitions
                .insert(data_definition.name.as_str(), data_definition)
                .is_some()
            {
                diagnostics.push(Diagnostic::error(format!(
                    "duplicate data `{}`",
                    data_definition.name
                )));
            }
        }

        for machine in &program.machines {
            if symbols
                .machines
                .insert(machine.name.as_str(), machine)
                .is_some()
            {
                diagnostics.push(Diagnostic::error(format!(
                    "duplicate machine `{}`",
                    machine.name
                )));
            }

            if symbols.data_definitions.contains_key(machine.name.as_str()) {
                diagnostics.push(Diagnostic::error(format!(
                    "`{}` is declared as both data and a machine",
                    machine.name
                )));
            }
        }

        for platform in &program.platforms {
            if symbols
                .platforms
                .insert(platform.name.as_str(), platform)
                .is_some()
            {
                diagnostics.push(Diagnostic::error(format!(
                    "duplicate platform `{}`",
                    platform.name
                )));
            }

            if symbols
                .data_definitions
                .contains_key(platform.name.as_str())
            {
                diagnostics.push(Diagnostic::error(format!(
                    "`{}` is declared as both data and a platform",
                    platform.name
                )));
            }

            if symbols.machines.contains_key(platform.name.as_str()) {
                diagnostics.push(Diagnostic::error(format!(
                    "`{}` is declared as both a machine and a platform",
                    platform.name
                )));
            }
        }

        symbols
    }

    pub fn has_type(&self, name: &str) -> bool {
        self.data_definitions.contains_key(name)
            || self.machines.contains_key(name)
            || self.platforms.contains_key(name)
    }

    pub fn has_data_definition(&self, name: &str) -> bool {
        self.data_definitions.contains_key(name)
    }

    pub fn machine(&self, name: &str) -> Option<&'program Machine> {
        self.machines.get(name).copied()
    }

    pub fn platform(&self, name: &str) -> Option<&'program Platform> {
        self.platforms.get(name).copied()
    }

    pub fn is_command_receiver_type(&self, name: &str) -> bool {
        self.machines.contains_key(name) || self.platforms.contains_key(name)
    }
}

#[derive(Debug)]
pub struct MachineSymbols<'program> {
    commands: HashMap<&'program str, &'program CommandDefinition>,
    contained_types: HashMap<&'program str, &'program str>,
    member_names: HashSet<&'program str>,
    state_names: HashSet<&'program str>,
}

impl<'program> MachineSymbols<'program> {
    pub fn build(machine: &'program Machine, diagnostics: &mut Vec<Diagnostic>) -> Self {
        let mut commands = HashMap::new();
        let mut contained_types = HashMap::new();
        let mut member_names = HashSet::new();
        let mut owned_data_names = HashSet::new();
        let mut state_names = HashSet::new();

        for command in &machine.commands {
            if commands
                .insert(command.signature.name.as_str(), command)
                .is_some()
            {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` has duplicate command `{}`",
                    machine.name, command.signature.name
                )));
            }
        }

        for contained_object in &machine.contains {
            if !member_names.insert(contained_object.name.as_str()) {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` has duplicate member `{}`",
                    machine.name, contained_object.name
                )));
            }

            if contained_types
                .insert(
                    contained_object.name.as_str(),
                    contained_object.type_name.as_str(),
                )
                .is_some()
            {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` has duplicate contained object `{}`",
                    machine.name, contained_object.name
                )));
            }
        }

        for owned_data in &machine.owned_data {
            if !member_names.insert(owned_data.name.as_str()) {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` has duplicate member `{}`",
                    machine.name, owned_data.name
                )));
            }

            if !owned_data_names.insert(owned_data.name.as_str()) {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` has duplicate owned data `{}`",
                    machine.name, owned_data.name
                )));
            }
        }

        for state in &machine.states {
            if !state_names.insert(state.name.as_str()) {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` has duplicate state `{}`",
                    machine.name, state.name
                )));
            }
        }

        Self {
            commands,
            contained_types,
            member_names,
            state_names,
        }
    }

    pub fn command(&self, name: &str) -> Option<&'program CommandDefinition> {
        self.commands.get(name).copied()
    }

    pub fn contained_type(&self, name: &str) -> Option<&'program str> {
        self.contained_types.get(name).copied()
    }

    pub fn member_names(&self) -> impl Iterator<Item = &'program str> + '_ {
        self.member_names.iter().copied()
    }

    pub fn has_state(&self, name: &str) -> bool {
        self.state_names.contains(name)
    }
}
