use super::shared::{child_symbol, top_level_symbol};
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::TypedTrees;
use omega_typed_trees::data::DataMember;
use omega_typed_trees::machine::Machine;
use omega_typed_trees::state::State;
use omega_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

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
            contained_objects: Vec::with_capacity(program.machine_contained_objects(machine).len()),
            member_symbols: Vec::with_capacity(
                program.machine_contained_objects(machine).len()
                    + program.machine_owned_data(machine).len(),
            ),
            owned_data_symbols: Vec::with_capacity(program.machine_owned_data(machine).len()),
            states: Vec::with_capacity(program.machine_states(machine).len()),
        };

        if let Some(data_definition) = program
            .data_definitions()
            .iter()
            .find(|definition| Some(&definition.name) == machine.attached_data.as_ref())
        {
            for member in program.data_members(data_definition) {
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

                if let Some(type_name) = callable_receiver_type_name(program, field.type_reference)
                {
                    symbols.contained_objects.push(ContainedObjectSymbol {
                        name: field.name.as_str(),
                        type_name,
                        symbol,
                    });
                }
            }
        }

        for contained_object in program.machine_contained_objects(machine) {
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

        for owned_data in program.machine_owned_data(machine) {
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

        for state in program.machine_states(machine) {
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

fn callable_receiver_type_name(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<&str> {
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            callable_receiver_type_name(program, *referee)
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            callable_receiver_type_name(program, *base_type)
        }
        TypeReferenceNode::FixedArray { .. } | TypeReferenceNode::Slice { .. } => None,
        TypeReferenceNode::Generic { .. } => None,
        TypeReferenceNode::DynamicTrait { name, .. } => Some(name.as_str()),
        TypeReferenceNode::Named { name, .. } => Some(name.as_str()),
        TypeReferenceNode::Unit => None,
    }
}
