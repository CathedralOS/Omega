//! Resolve field paths only beneath an exact, in-scope value declaration.

use diagnostics::Diagnostic;
use symbols::SymbolHandle;
use typed_trees::{
    TypedTrees,
    data::DataMember,
    expression::{ExpressionHandle, ExpressionNode},
    name::Identifier,
    state::State,
    statement::StatementNode,
    types::TypeReferenceHandle,
};

pub(super) struct Projection {
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
        .find(|parameter| parameter.symbol == symbol && parameter.name == *name)
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
        })?;
    Some(Projection {
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
    let nominal =
        super::super::machine_symbol_from_type_reference_handle(program, receiver.type_reference);
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
                    handle,
                    symbols
                        .get(index)
                        .copied()
                        .unwrap_or_else(SymbolHandle::invalid),
                    selected.symbol,
                )?;
            }
            check_symbol(program, handle, path.symbol, selected.symbol)?;
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
            check_symbol(program, handle, member.member_symbol, selected.symbol)?;
            Some(selected)
        }
        _ => None,
    };
    Ok(retained)
}

fn check_symbol(
    program: &TypedTrees,
    expression: ExpressionHandle,
    authored: SymbolHandle,
    selected: SymbolHandle,
) -> Result<(), Vec<Diagnostic>> {
    if authored.is_valid() && authored != selected {
        return Err(vec![
            Diagnostic::error("projected call receiver disagrees with its exact declared field")
                .with_source_span(program.expression_table.source_span(expression)),
        ]);
    }
    Ok(())
}
