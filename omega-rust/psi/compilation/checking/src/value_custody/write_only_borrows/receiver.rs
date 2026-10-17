//! Rejoin a write-only receiver with its attached storage declaration.
use super::{
    DataDefinition, DataShapeKind, Diagnostic, ExpressionHandle, ExpressionNode, Machine,
    SymbolHandle, WriteOnlyRoot, closed_write_only_data_by_symbol,
    closed_write_only_data_shape_by_symbol, write_only_record_shape,
};
use typed_trees::data::DataField;
use typed_trees::expression::TableNamePath;
use typed_trees::statement::TableCall;

mod projections;
pub(super) use projections::nonobserving_projection;
pub(super) use projections::projected;
pub(super) use projections::storage_projection;

pub(super) fn record<'program>(
    program: &'program dyn crate::CheckingInput,
    root: &WriteOnlyRoot,
) -> Option<&'program DataDefinition> {
    let machine = machine(program, root)?;
    let definition = closed_write_only_data_by_symbol(program, machine.attached_data_symbol)?;
    (DataDefinition::shape_kind_from_members(program.data_members(definition))
        == DataShapeKind::Record)
        .then_some(definition)
}

pub(super) fn field<'program>(
    program: &'program dyn crate::CheckingInput,
    root: &WriteOnlyRoot,
    expression: ExpressionHandle,
) -> Option<&'program DataField> {
    let definition = projection_record(program, root)?;
    let field = crate::exact_self_field(program, machine(program, root)?, expression)?;
    crate::declarations::data_queries::data_field_is_where_independent(program, definition, field)
        .then_some(field)
}

fn projection_record<'program>(
    program: &'program dyn crate::CheckingInput,
    root: &WriteOnlyRoot,
) -> Option<&'program DataDefinition> {
    let definition = closed_write_only_data_shape_by_symbol(
        program,
        machine(program, root)?.attached_data_symbol,
    )?;
    (DataDefinition::shape_kind_from_members(program.data_members(definition))
        == DataShapeKind::Record)
        .then_some(definition)
}

fn machine<'program>(
    program: &'program dyn crate::CheckingInput,
    root: &WriteOnlyRoot,
) -> Option<&'program Machine> {
    if !root.receiver_machine.is_valid() {
        return None;
    }
    program.machine(root.receiver_machine)
}

pub(super) fn matches_name(root: &WriteOnlyRoot, path: &TableNamePath) -> bool {
    path.head_symbol == root.symbol
}

pub(super) fn mentions_name(
    program: &dyn crate::CheckingInput,
    root: &WriteOnlyRoot,
    path: &TableNamePath,
) -> bool {
    matches_name(root, path)
        || program
            .expression_table
            .name_path_members(path.members)
            .first()
            .is_some_and(|name| {
                attached_field(program, root, path.head_symbol, name.as_str()).is_some()
            })
}

pub(super) fn bare_field<'program, 'roots>(
    program: &'program dyn crate::CheckingInput,
    expression: ExpressionHandle,
    roots: &'roots [WriteOnlyRoot],
) -> Option<(&'roots WriteOnlyRoot, &'program DataField)> {
    let Some((path, name)) = program.expression_table.single_name_member(expression) else {
        return None;
    };
    roots.iter().find_map(|root| {
        attached_field(program, root, path.head_symbol, name.as_str()).map(|field| (root, field))
    })
}

fn attached_field<'program>(
    program: &'program dyn crate::CheckingInput,
    root: &WriteOnlyRoot,
    symbol: SymbolHandle,
    name: &str,
) -> Option<&'program DataField> {
    if !symbol.is_valid() {
        return None;
    }
    let definition = projection_record(program, root)?;
    let field = crate::value_custody::places::exact_attached_field(
        program,
        machine(program, root)?,
        symbol,
        name,
    )?;
    crate::declarations::data_queries::data_field_is_where_independent(program, definition, field)
        .then_some(field)
}

pub(super) fn validate_statement_call(
    machine: &str,
    state: &str,
    call: &TableCall,
    roots: &[WriteOnlyRoot],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for root in roots {
        if call.receiver_root_symbol == root.symbol {
            diagnostics.push(Diagnostic::error(format!(
                "machine `{machine}` state `{state}` calls through write-only parameter `{}`; dispatch requires an exact checked `&write self` target on the whole receiver or an eligible record-field projection",
                root.name,
            )));
        }
    }
}
