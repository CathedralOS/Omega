use diagnostics::Diagnostic;
use symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees;
use symbol_resolved_trees_to_typed_trees::typed_trees::data::DataMember;
use symbol_resolved_trees_to_typed_trees::typed_trees::machine::Machine;
use symbol_resolved_trees_to_typed_trees::typed_trees::state::State;
use symbol_resolved_trees_to_typed_trees::typed_trees::types::{
    TypeReferenceHandle, TypeReferenceNode,
};
use symbols::SymbolHandle;

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
}

impl<'program> MachineSymbols<'program> {
    pub fn build(
        program: &'program TypedTrees,
        machine: &'program Machine,
        diagnostics: &mut Vec<Diagnostic>,
    ) -> Self {
        let machine_symbol = retained_machine_symbol(program, machine);
        // One child index per machine build: a fresh `find_child_by_name`
        // walk per member is O(children) each, so member lookups composed to
        // O(members * children). First match wins, matching that lookup.
        let mut children_by_name: std::collections::HashMap<&str, SymbolHandle> =
            std::collections::HashMap::new();
        if machine_symbol.is_valid()
            && let Some(children) = program.symbols.child_handles(machine_symbol)
        {
            for child in children {
                children_by_name
                    .entry(program.symbols.name(child))
                    .or_insert(child);
            }
        }
        // Duplicate verdicts read whether a same-named earlier member carries
        // a valid symbol, so the seen maps store name -> symbol.is_valid()
        // rather than the names alone.
        let mut member_verdicts: std::collections::HashMap<&str, bool> =
            std::collections::HashMap::new();
        let mut owned_data_verdicts: std::collections::HashMap<&str, bool> =
            std::collections::HashMap::new();
        let mut state_verdicts: std::collections::HashMap<&str, bool> =
            std::collections::HashMap::new();
        let mut symbols = Self {
            callable_fields: Vec::new(),
            member_symbols: Vec::with_capacity(program.machine_owned_data(machine).len()),
            owned_data_symbols: Vec::with_capacity(program.machine_owned_data(machine).len()),
            states: Vec::with_capacity(program.machine_states(machine).len()),
        };

        if let Some(data_definition) =
            crate::validation::value_custody::places::machine_attached_data(program, machine)
        {
            for member in program.data_members(data_definition) {
                let DataMember::Field(field) = member else {
                    continue;
                };

                if member_verdicts
                    .get(field.name.as_str())
                    .copied()
                    .unwrap_or(false)
                {
                    diagnostics.push(Diagnostic::error(format!(
                        "machine `{}` has duplicate member `{}`",
                        machine.name, field.name
                    )));
                    continue;
                }

                let symbol = children_by_name
                    .get(field.name.as_str())
                    .copied()
                    .unwrap_or_else(SymbolHandle::invalid);
                member_verdicts
                    .entry(field.name.as_str())
                    .or_insert(symbol.is_valid());
                owned_data_verdicts
                    .entry(field.name.as_str())
                    .or_insert(symbol.is_valid());
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
            if member_verdicts
                .get(owned_data.name.as_str())
                .copied()
                .unwrap_or(false)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` has duplicate member `{}`",
                    machine.name, owned_data.name
                )));
            }

            if owned_data_verdicts
                .get(owned_data.name.as_str())
                .copied()
                .unwrap_or(false)
            {
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
            member_verdicts
                .entry(owned_data.name.as_str())
                .or_insert(symbol.is_valid());
            owned_data_verdicts
                .entry(owned_data.name.as_str())
                .or_insert(symbol.is_valid());
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
            if state_verdicts
                .get(state.name.as_str())
                .copied()
                .unwrap_or(false)
            {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` has duplicate state `{}`",
                    machine.name, state.name
                )));
            }

            let symbol =
                retained_child_symbol(program, machine_symbol, state.symbol, state.name.as_str());
            state_verdicts
                .entry(state.name.as_str())
                .or_insert(symbol.is_valid());
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

    pub fn callable_field_type(&self, name: &str) -> Option<&'program str> {
        self.callable_fields
            .iter()
            .find(|symbol| symbol.name == name)
            .map(|symbol| symbol.type_name)
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

/// The machine's own retained declaration symbol. Authored machines are
/// top-level children of root; compiler-generated specializations are
/// generated roots whose parent is invalid and whose `generated_from` carries
/// the authored derivation. Their generated field, owned-data, and state
/// children still resolve under the machine symbol itself.
fn retained_machine_symbol(program: &TypedTrees, machine: &Machine) -> SymbolHandle {
    let spelling = machine.symbol_spelling();
    let authored =
        retained_child_symbol(program, program.symbols.root(), machine.symbol, &spelling);
    if authored.is_valid() {
        return authored;
    }
    let symbol = machine.symbol;
    if symbol.is_valid()
        && program.symbols.get(symbol).kind == symbols::SymbolKind::Machine
        && program.symbols.get(symbol).generated_from.is_valid()
        && program.symbols.name(symbol) == spelling
    {
        symbol
    } else {
        SymbolHandle::invalid()
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
        symbol_resolved_trees_to_typed_trees::typed_trees::service::exact_bound_service_requirement(
            program,
            type_reference,
        )
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
