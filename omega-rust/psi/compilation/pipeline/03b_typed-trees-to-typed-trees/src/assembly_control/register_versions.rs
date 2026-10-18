//! Each scratch write creates an immutable value version. This is semantic
//! register data flow, not an optimization: retaining the pre-write symbol
//! keeps arithmetic premises and ZF's exact result relation alive independently
//! of later writes to that register. Edges transport only the latest versions.
use arena::{Handle, HandleSpan};
use symbols::{SymbolHandle, SymbolKind};
use typed_trees::statement::{StatementNode, TableLocalData};
use typed_trees::types::TypeReferenceHandle;
use typed_trees::{TypedTrees, name::Identifier};

pub(super) struct RegisterVersion {
    pub source: SymbolHandle,
    pub current: SymbolHandle,
    pub carrier: TypeReferenceHandle,
    pub name: Identifier,
}

pub(super) fn statement(
    program: &mut TypedTrees,
    owner: SymbolHandle,
    handle: Handle<StatementNode>,
    registers: &mut [RegisterVersion],
    ordinal: usize,
) -> StatementNode {
    let mapping: Vec<_> = registers
        .iter()
        .map(|register| (register.source, register.current))
        .collect();
    program
        .tables
        .remap_statement_symbols(HandleSpan::from_parts(handle, 1), &mapping);
    let statement = program.statement_table.statement(handle).clone();
    // This is SSA renaming, not a clone that retains authored spellings.
    // Each changed reference must name its new declaration as well as carry
    // its identity. An old parameter spelling must not denote a fresh value.
    let mut expressions = Vec::new();
    validation::expression_traversal::collect_statement_expression_trees(
        program,
        &statement,
        &mut expressions,
    );
    for expression in expressions {
        let typed_trees::expression::ExpressionNode::Name(path) =
            program.expression_table.expression(expression)
        else {
            continue;
        };
        if !registers
            .iter()
            .any(|register| register.current != register.source && register.current == path.symbol)
        {
            continue;
        }
        let mut members = HandleSpan::empty();
        let identifier = Identifier::generated(program.symbols.name(path.symbol));
        program
            .expression_table
            .push_name_path_member(&mut members, identifier);
        let typed_trees::expression::ExpressionNode::Name(path) =
            program.expression_table.expression_mut(expression)
        else {
            unreachable!()
        };
        path.members = members;
    }
    let StatementNode::Assignment(assignment) = &statement else {
        return statement;
    };
    let Some(root) = super::normalization::root(program, assignment.target) else {
        return statement;
    };
    let Some(register) = registers
        .iter_mut()
        .find(|register| register.current == root)
    else {
        return statement;
    };
    let name = Identifier::generated(format!("{}$version${ordinal}", register.name));
    let symbol = program
        .symbols
        .append_generated_children_from(
            owner,
            [(register.source, SymbolKind::Local, name.as_str())],
        )
        .handle_at(0);
    register.current = symbol;
    StatementNode::LocalData(TableLocalData {
        symbol,
        name,
        type_reference: register.carrier,
        initial_value: assignment.value,
        ..Default::default()
    })
}
