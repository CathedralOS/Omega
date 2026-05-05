use std::collections::HashMap;

use crate::diagnostics::Diagnostic;
use crate::ir::Program;
use crate::ir::data::DataDefinition;
use crate::ir::machine::Machine;
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
