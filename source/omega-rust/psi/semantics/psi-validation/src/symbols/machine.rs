use super::shared::child_symbol;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;
use psi_typed_trees::TypedTrees;
use psi_typed_trees::data::DataMember;
use psi_typed_trees::machine::Machine;
use psi_typed_trees::state::State;
use psi_typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

#[derive(Debug)]
pub struct MachineSymbols<'program> {
    callable_fields: Vec<CallableFieldSymbol<'program>>,
    member_symbols: Vec<MemberSymbol<'program>>,
    owned_data_symbols: Vec<MemberSymbol<'program>>,
    states: Vec<StateSymbol<'program>>,
}

#[derive(Debug)]
struct CallableFieldSymbol<'program> {
    name: &'program str,
    type_name: &'program str,
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
        let machine_symbol = retained_child_symbol(
            program,
            program.symbols.root(),
            machine.symbol,
            machine.name.as_str(),
        );
        let mut symbols = Self {
            callable_fields: Vec::new(),
            member_symbols: Vec::with_capacity(program.machine_owned_data(machine).len()),
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
                    symbols.callable_fields.push(CallableFieldSymbol {
                        name: field.name.as_str(),
                        type_name,
                    });
                }
            }
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

            let symbol = retained_child_symbol(
                program,
                machine_symbol,
                owned_data.symbol,
                owned_data.name.as_str(),
            );
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
                symbol: retained_child_symbol(
                    program,
                    machine_symbol,
                    state.symbol,
                    state.name.as_str(),
                ),
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

    pub fn callable_field_type(&self, name: &str) -> Option<&'program str> {
        self.callable_fields
            .iter()
            .find(|symbol| symbol.name == name)
            .map(|symbol| symbol.type_name)
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

fn retained_child_symbol(
    program: &TypedTrees,
    parent: SymbolHandle,
    symbol: SymbolHandle,
    name: &str,
) -> SymbolHandle {
    if symbol.is_valid()
        && program.symbols.get(symbol).parent == parent
        && program.symbols.name(symbol) == name
    {
        symbol
    } else {
        SymbolHandle::invalid()
    }
}

fn callable_receiver_type_name(
    program: &TypedTrees,
    type_reference: TypeReferenceHandle,
) -> Option<&str> {
    if let Some(requirement) =
        psi_typed_trees::service::exact_bound_service_requirement(program, type_reference)
    {
        return program
            .traits()
            .iter()
            .find(|definition| definition.symbol == requirement)
            .map(|definition| definition.name.as_str());
    }
    match program.type_reference_table.type_reference(type_reference) {
        TypeReferenceNode::Reference { referee, .. } => {
            callable_receiver_type_name(program, *referee)
        }
        TypeReferenceNode::Constrained { base_type, .. } => {
            callable_receiver_type_name(program, *base_type)
        }
        TypeReferenceNode::FixedArray { .. } | TypeReferenceNode::Slice { .. } => None,
        TypeReferenceNode::Generic { .. } => None,
        TypeReferenceNode::ConstExpression(_) => None,
        TypeReferenceNode::DynamicTrait { name, .. } => Some(name.as_str()),
        TypeReferenceNode::Named { name, .. } => Some(name.as_str()),
        TypeReferenceNode::Unit => None,
    }
}
