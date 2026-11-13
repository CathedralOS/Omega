//! Resolved names retain their exact declaration and lexical source scope.
//! These queries explain missing source bindings; they grant no body facts.

use crate::TypedTrees;
use crate::expression::{ExpressionHandle, ExpressionNode};
use crate::machine::Machine;
use crate::state::State;
use crate::statement::StatementNode;
use crate::types::TypeReferenceHandle;
use diagnostics::Diagnostic;
use symbols::{SymbolHandle, SymbolKind};

pub fn declared_receiver_type_reference(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    receiver: SymbolHandle,
) -> Option<TypeReferenceHandle> {
    if !receiver.is_valid() {
        return None;
    }
    program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == receiver)
        .map(|parameter| parameter.type_reference)
        .or_else(|| {
            program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .find_map(|statement| {
                    let local = statement.as_local_data()?;
                    (local.symbol == receiver).then_some(local.type_reference)
                })
        })
        .or_else(|| {
            program
                .machine_owned_data(machine)
                .iter()
                .find(|owned| owned.symbol == receiver)
                .map(|owned| owned.type_reference)
        })
        .or_else(|| {
            let data = program.attached_data_definition(machine)?;
            program.data_members(data).iter().find_map(|member| {
                let field = member.as_field()?;
                (field.symbol == receiver).then_some(field.type_reference)
            })
        })
}

pub fn receiver_member_chain(
    program: &TypedTrees,
    receiver: ExpressionHandle,
) -> Option<Vec<String>> {
    if !program.expression_table.expression_is_valid(receiver) {
        return None;
    }
    match program.expression_table.expression(receiver) {
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            (!members.is_empty()).then(|| {
                members
                    .iter()
                    .map(|member| member.as_str().to_owned())
                    .collect()
            })
        }
        ExpressionNode::Member(member) => {
            let mut chain = receiver_member_chain(program, member.receiver)?;
            chain.push(member.member.as_str().to_owned());
            Some(chain)
        }
        ExpressionNode::Borrow(borrow) => receiver_member_chain(program, borrow.target),
        _ => None,
    }
}

fn state_binding_type(
    program: &TypedTrees,
    state: &State,
    prior_statements: &[StatementNode],
    symbol: SymbolHandle,
) -> Option<TypeReferenceHandle> {
    if !symbol.is_valid() || program.symbols.get(symbol).parent != state.symbol {
        return None;
    }
    match program.symbols.get(symbol).kind {
        SymbolKind::Parameter => program
            .state_parameters(state)
            .iter()
            .find(|parameter| parameter.symbol == symbol)
            .map(|parameter| parameter.type_reference),
        SymbolKind::Local => prior_statements.iter().find_map(|statement| {
            let local = statement.as_local_data()?;
            (local.symbol == symbol).then_some(local.type_reference)
        }),
        _ => None,
    }
}

fn value_root_is_known(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    prior_statements: &[StatementNode],
    root: SymbolHandle,
) -> bool {
    if !root.is_valid() {
        return false;
    }
    match program.symbols.get(root).kind {
        SymbolKind::Local | SymbolKind::Parameter => {
            return state_binding_type(program, state, prior_statements, root).is_some();
        }
        SymbolKind::Unknown => return false,
        SymbolKind::Const => {
            return program
                .const_declarations()
                .iter()
                .filter(|declaration| declaration.symbol == root)
                .count()
                == 1;
        }
        SymbolKind::Conformance | SymbolKind::ConformanceParameter => return true,
        _ => {}
    }
    if program
        .data_definitions()
        .iter()
        .any(|row| row.symbol == root)
        || program.machines().iter().any(|row| row.symbol == root)
        || program.traits().iter().any(|row| row.symbol == root)
        || program.symbols.get(root).kind == SymbolKind::BuiltinType
    {
        return true;
    }
    let (parameters, _) =
        program.machine_type_reference_scope(machine, |left, right| left.name == right.name);
    if parameters
        .iter()
        .filter(|parameter| parameter.symbol == root && parameter.kind.is_const_or_value())
        .count()
        == 1
    {
        return true;
    }
    if let Some(data) = program.attached_data_definition(machine)
        && program
            .data_type_parameters(data)
            .iter()
            .any(|parameter| parameter.symbol == root && parameter.kind.is_const_or_value())
    {
        return true;
    }
    program.data_definitions().iter().any(|data| {
        program.data_members(data).iter().any(|member| {
            member
                .as_variant()
                .is_some_and(|variant| variant.symbol == root)
        })
    })
}

pub fn report_unresolved_name_use(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    prior_statements: &[StatementNode],
    expression: ExpressionHandle,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let Some(path) = program.expression_table.name_path_node(expression) else {
        return;
    };
    let members = program.expression_table.name_path_members(path.members);
    if let Some(first) = members.first() {
        let name = first.as_str();
        let root = if path.head_symbol.is_valid() {
            path.head_symbol
        } else {
            path.symbol
        };
        let bare_identity_matches =
            members.len() != 1 || !path.head_symbol.is_valid() || path.head_symbol == path.symbol;
        if language_semantics::const_value::boolean_literal_spelling(name).is_none()
            && (!bare_identity_matches
                || !value_root_is_known(program, machine, state, prior_statements, root))
        {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` state `{}` uses `{name}`, which is not a declared local, parameter, field, or type in this state (check the spelling)", machine.name, state.name,
            )));
        }
        if members.len() > 1 && state_binding_type(program, state, prior_statements, root).is_some()
        {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{}` state `{}` uses a static path through runtime binding `{name}`; use `.` to select a field or callable through a value", machine.name, state.name,
            )));
        }
    }
    if let [scope, tail] = members
        && !path.head_symbol.is_valid()
        && !path.symbol.is_valid()
    {
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` state `{}` uses `{}::{}`, which resolves to no case or constant -- it would silently read 0 (ZII). Check the spelling and ensure the declaration is imported", machine.name, state.name, scope.as_str(), tail.as_str(),
        )));
    }
}
