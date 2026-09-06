//! Resolve projected types only beneath an exact, in-scope value declaration.

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::{
    TypedTrees,
    data::DataMember,
    expression::{ExpressionHandle, ExpressionNode},
    name::Identifier,
    state::State,
    statement::StatementNode,
    types::{TypeReferenceHandle, TypeReferenceNode},
};

pub(super) struct Projection {
    pub root_symbol: SymbolHandle,
    pub symbol: SymbolHandle,
    pub type_reference: TypeReferenceHandle,
    pub depth: usize,
}

pub(super) fn root(
    program: &TypedTrees,
    state: &State,
    before: usize,
    symbol: SymbolHandle,
    name: &Identifier,
) -> Option<Projection> {
    if !symbol.is_valid() {
        return None;
    }
    let type_reference = program
        .state_parameters(state)
        .iter()
        .find(|parameter| {
            parameter.name == *name
                && (parameter.symbol == symbol
                    || (parameter.is_self
                        && program.machines().iter().any(|machine| {
                            machine.symbol == symbol
                                && program
                                    .machine_states(machine)
                                    .iter()
                                    .any(|candidate| candidate.symbol == state.symbol)
                        })))
        })
        .map(|parameter| parameter.type_reference)
        .or_else(|| {
            program.statement_table.statements(state.statement_nodes)[..before]
                .iter()
                .find_map(|statement| match statement {
                    StatementNode::LocalData(local)
                        if local.symbol == symbol && local.name == *name =>
                    {
                        Some(local.type_reference)
                    }
                    _ => None,
                })
        })
        .or_else(|| {
            if !program
                .state_parameters(state)
                .iter()
                .any(|parameter| parameter.is_self)
            {
                return None;
            }
            let machine = program.machines().iter().find(|machine| {
                program
                    .machine_states(machine)
                    .iter()
                    .any(|candidate| candidate.symbol == state.symbol)
            })?;
            validation::exact_attached_field(program, machine, symbol, name.as_str())
                .map(|field| field.type_reference)
        })?;
    Some(Projection {
        root_symbol: symbol,
        symbol,
        type_reference,
        depth: 0,
    })
}

pub(super) fn field(
    program: &TypedTrees,
    receiver: Projection,
    name: &Identifier,
) -> Option<Projection> {
    let mut nominal =
        super::super::machine_symbol_from_type_reference_handle(program, receiver.type_reference);
    // A self reference names its attached machine, not the data declaration.
    // Rejoin only that machine's actual self root before walking record fields;
    // ordinary values cannot borrow a same-spelled machine's attachment.
    if receiver.depth == 0
        && let Some(machine) = program.machines().iter().find(|machine| {
            machine.symbol == nominal
                && (receiver.symbol == machine.symbol
                    || program.machine_states(machine).iter().any(|state| {
                        program.state_parameters(state).iter().any(|parameter| {
                            parameter.is_self && parameter.symbol == receiver.symbol
                        })
                    }))
        })
    {
        nominal = machine.attached_data_symbol;
    }
    let definition = program
        .data_definitions()
        .iter()
        .find(|definition| nominal.is_valid() && definition.symbol == nominal)?;
    let mut fields =
        program
            .data_members(definition)
            .iter()
            .filter_map(|candidate| match candidate {
                DataMember::Field(field) if field.name == *name => Some(field),
                _ => None,
            });
    let selected = fields.next()?;
    if fields.next().is_some() || !selected.symbol.is_valid() {
        return None;
    }
    Some(Projection {
        root_symbol: receiver.root_symbol,
        symbol: selected.symbol,
        type_reference: selected.type_reference,
        depth: receiver.depth + 1,
    })
}

pub(super) fn expression(
    program: &TypedTrees,
    state: &State,
    before: usize,
    handle: ExpressionHandle,
    visited: &mut Vec<ExpressionHandle>,
) -> Result<Option<Projection>, Vec<Diagnostic>> {
    if !handle.is_valid() || visited.contains(&handle) {
        return Ok(None);
    }
    visited.push(handle);
    let retained = match program.expression_table.expression(handle) {
        ExpressionNode::Borrow(borrow) => {
            expression(program, state, before, borrow.target, visited)?
        }
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            let Some(first) = members.first() else {
                return Ok(None);
            };
            let Some(mut selected) = root(program, state, before, path.head_symbol, first) else {
                return Ok(None);
            };
            let symbols = program
                .expression_table
                .name_path_member_symbols(path.member_symbols);
            for (index, member) in members.iter().enumerate() {
                if index != 0 {
                    let Some(projected) = field(program, selected, member) else {
                        return Ok(None);
                    };
                    selected = projected;
                }
                check_symbol(
                    program,
                    state,
                    handle,
                    symbols
                        .get(index)
                        .copied()
                        .unwrap_or_else(SymbolHandle::invalid),
                    &selected,
                )?;
            }
            check_symbol(program, state, handle, path.symbol, &selected)?;
            Some(selected)
        }
        ExpressionNode::Member(member) if member.case_variant.is_none() => {
            let Some(receiver) = expression(program, state, before, member.receiver, visited)?
            else {
                return Ok(None);
            };
            let Some(selected) = field(program, receiver, &member.member) else {
                return Ok(None);
            };
            check_symbol(program, state, handle, member.member_symbol, &selected)?;
            Some(selected)
        }
        ExpressionNode::Indexed(indexed) => {
            let Some(receiver) = expression(program, state, before, indexed.collection, visited)?
            else {
                return Ok(None);
            };
            let Some(machine) = program
                .machines()
                .iter()
                .find(|machine| machine.symbol == program.symbols.get(state.symbol).parent)
            else {
                return Ok(None);
            };
            if !validation::place_has_builtin_coordinates(program, machine, Some(state), handle) {
                // Authored indexing retains its selected result declaration;
                // the collection's element type cannot replace that meaning.
                return Ok(None);
            }
            if matches!(
                program.expression_table.expression(indexed.index),
                ExpressionNode::Range(_)
            ) {
                return Ok(None);
            }
            let Some(collection) =
                validation::unwrapped_type_reference(program, receiver.type_reference)
            else {
                return Ok(None);
            };
            let (TypeReferenceNode::FixedArray { element_type, .. }
            | TypeReferenceNode::Slice { element_type }) =
                program.type_reference_table.type_reference(collection)
            else {
                return Ok(None);
            };
            Some(Projection {
                root_symbol: receiver.root_symbol,
                // An element has no field declaration of its own. Its exact
                // type selects the method; place and access checks retain the index.
                symbol: SymbolHandle::invalid(),
                type_reference: *element_type,
                depth: receiver.depth + 1,
            })
        }
        _ => None,
    };
    Ok(retained)
}

fn check_symbol(
    program: &TypedTrees,
    state: &State,
    expression: ExpressionHandle,
    authored: SymbolHandle,
    selected: &Projection,
) -> Result<(), Vec<Diagnostic>> {
    if authored.is_valid()
        && !matches_symbol(
            program,
            state,
            selected.root_symbol,
            authored,
            selected.symbol,
        )
    {
        return Err(vec![
            Diagnostic::error("projected call receiver disagrees with its exact declared field")
                .with_source_span(program.expression_table.source_span(expression)),
        ]);
    }
    Ok(())
}

/// Original data fields and their inherited self slots identify the same
/// declaration only beneath this state's exact attached receiver root.
pub(super) fn matches_symbol(
    program: &TypedTrees,
    state: &State,
    root: SymbolHandle,
    authored: SymbolHandle,
    selected: SymbolHandle,
) -> bool {
    authored == selected
        || program.machines().iter().any(|machine| {
            program
                .machine_states(machine)
                .iter()
                .any(|candidate| candidate.symbol == state.symbol)
                && program.state_parameters(state).iter().any(|parameter| {
                    parameter.is_self && (root == machine.symbol || root == parameter.symbol)
                })
                && validation::exact_attached_field(
                    program,
                    machine,
                    authored,
                    program.symbols.name(authored),
                )
                .is_some_and(|field| field.symbol == selected)
        })
}
