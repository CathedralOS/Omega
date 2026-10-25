//! Resolve projected types only beneath an exact, in-scope value declaration.

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::{
    expression::{ExpressionHandle, ExpressionNode},
    name::Identifier,
    state::State,
    types::TypeReferenceHandle,
};

pub struct Projection {
    pub root_symbol: SymbolHandle,
    pub symbol: SymbolHandle,
    pub type_reference: TypeReferenceHandle,
}

pub fn root(
    program: &dyn crate::CheckingInput,
    state: &State,
    before: usize,
    symbol: SymbolHandle,
    name: &Identifier,
) -> Option<Projection> {
    if !symbol.is_valid() {
        return None;
    }
    let type_reference = program
        .state_parameter_by_symbol(state, symbol)
        // A parameter owns its place root outright, so the resolved symbol
        // IS the identity: the spelling beside it adds no discrimination.
        .map(|parameter| parameter.type_reference)
        .or_else(|| {
            program
                .state_local_declaration(state, symbol)
                .filter(|&(ordinal, local)| ordinal < before && local.name == *name)
                .map(|(_, local)| local.type_reference)
        })?;
    Some(Projection {
        root_symbol: symbol,
        symbol,
        type_reference,
    })
}

pub fn field(
    program: &dyn crate::CheckingInput,
    receiver: Projection,
    name: &Identifier,
) -> Option<Projection> {
    let nominal =
        crate::machine_symbol_from_type_reference_handle(program, receiver.type_reference);
    let definition = program.data_definition_by_symbol(nominal)?;
    let selected = super::exact_data_member_field(
        program,
        definition,
        SymbolHandle::invalid(),
        name.as_str(),
        None,
    )?;
    Some(Projection {
        root_symbol: receiver.root_symbol,
        symbol: selected.symbol,
        type_reference: selected.type_reference,
    })
}

pub fn expression(
    program: &dyn crate::CheckingInput,
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
                    handle,
                    symbols
                        .get(index)
                        .copied()
                        .unwrap_or_else(SymbolHandle::invalid),
                    &selected,
                )?;
            }
            check_symbol(program, handle, path.symbol, &selected)?;
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
            check_symbol(program, handle, member.member_symbol, &selected)?;
            Some(selected)
        }
        ExpressionNode::Indexed(indexed) => {
            let Some(receiver) = expression(program, state, before, indexed.collection, visited)?
            else {
                return Ok(None);
            };
            let Some(machine) = crate::find_machine(program, state.symbol) else {
                return Ok(None);
            };
            if !crate::place_has_builtin_coordinates(program, machine, Some(state), handle) {
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
            let Some(element_type) =
                super::collection_element_type_reference(program, receiver.type_reference)
            else {
                return Ok(None);
            };
            Some(Projection {
                root_symbol: receiver.root_symbol,
                // An element has no field declaration of its own. Its exact
                // type selects the method; place and access checks retain the index.
                symbol: SymbolHandle::invalid(),
                type_reference: element_type,
            })
        }
        _ => None,
    };
    Ok(retained)
}

fn check_symbol(
    program: &dyn crate::CheckingInput,
    expression: ExpressionHandle,
    authored: SymbolHandle,
    selected: &Projection,
) -> Result<(), Vec<Diagnostic>> {
    if authored.is_valid() && authored != selected.symbol {
        return Err(vec![
            Diagnostic::error("projected call receiver disagrees with its exact declared field")
                .with_source_span(program.expression_table.source_span(expression)),
        ]);
    }
    Ok(())
}

pub fn projected_statement_receiver_place(
    program: &dyn crate::CheckingInput,
    state: &typed_trees::state::State,
    before: usize,
    call: &typed_trees::statement::TableCall,
) -> Option<(SymbolHandle, Vec<facts::PlaceSegment>)> {
    let members = program.statement_table.name_path_members(call.receiver);
    let mut receiver = root(
        program,
        state,
        before,
        call.receiver_root_symbol,
        members.first()?,
    )?;
    let root = receiver.symbol;
    let mut segments = Vec::new();
    for member in members.iter().skip(1) {
        receiver = field(program, receiver, member)?;
        segments.push(facts::PlaceSegment::Field {
            symbol: receiver.symbol,
        });
    }
    (receiver.symbol == call.receiver_symbol).then_some((root, segments))
}

/// The exact receiver occurrence's stored application, after replaying all
/// field substitutions. Namespace qualifiers have no value place or authority.
pub fn projected_statement_receiver_type_reference(
    program: &dyn crate::CheckingInput,
    state: &typed_trees::state::State,
    before: usize,
    call: &typed_trees::statement::TableCall,
) -> Option<TypeReferenceHandle> {
    let (root, segments) = projected_statement_receiver_place(program, state, before, call)?;
    super::canonical_place_type_reference(
        program,
        state.symbol,
        before,
        &super::CanonicalPlace {
            root: facts::PlaceRoot::Symbol(root),
            segments,
        },
    )
}
