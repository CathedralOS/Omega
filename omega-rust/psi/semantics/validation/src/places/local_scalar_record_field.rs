//! Exact declaration custody for a scalar read from an immutable local record.

use language_semantics::Multiplicity;
use numerics::arithmetic::ArithmeticDomain;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::DataMember;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::StatementNode;
use typed_trees::types::{PrimitiveType, TypeReferenceHandle, TypeReferenceNode};

#[cfg(test)]
mod tests;

/// Source identities only; this does not grant ownership or establish loan
/// validity. Checked flow and the consuming replay still validate availability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocalScalarRecordField {
    pub local: SymbolHandle,
    pub local_statement_ordinal: u32,
    pub type_reference: TypeReferenceHandle,
    pub field: SymbolHandle,
    pub primitive_type: PrimitiveType,
}

/// Resolve a direct field of one prior, whole immutable plain-owned local.
/// The caller must bind the expression to this authored statement's evaluation
/// graph. Nested projections, generic/qualified owners, erased fields and
/// qualified or policy-bearing scalar fields require additional retained facts.
pub fn local_scalar_record_field(
    program: &TypedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_ordinal: u32,
    expression: ExpressionHandle,
) -> Option<LocalScalarRecordField> {
    let ExpressionNode::Member(member) = program.expression_table.expression(expression) else {
        return None;
    };
    if member.case_variant.is_some() {
        return None;
    }
    let local = local_plain_record(
        program,
        machine,
        state,
        statement_ordinal,
        expression,
        member.receiver,
    )?;
    let record = program
        .data_definitions()
        .iter()
        .find(|record| record.symbol == program.type_reference_symbol(local.type_reference))?;
    let field = super::exact_data_member_field(
        program,
        record,
        member.member_symbol,
        member.member.as_str(),
        None,
    )?;
    if program.symbols.get(field.symbol).kind != symbols::SymbolKind::Field
        || program.symbols.get(field.symbol).parent != record.symbol
        || program.data_members(record).iter().filter(|member| matches!(member, DataMember::Field(candidate) if candidate.symbol == field.symbol)).count() != 1
        || field.relevance.is_erased()
        || !matches!(program.type_reference_table.type_reference(field.type_reference), TypeReferenceNode::Named { .. })
        || program.arithmetic_domain_for_type_reference(field.type_reference) != ArithmeticDomain::Exact
    {
        return None;
    }
    let primitive_type = crate::recasts::exact_primitive_type(program, field.type_reference)?;
    if !matches!(
        primitive_type,
        PrimitiveType::Bool
            | PrimitiveType::I8
            | PrimitiveType::I16
            | PrimitiveType::I32
            | PrimitiveType::I64
            | PrimitiveType::U8
            | PrimitiveType::U16
            | PrimitiveType::U32
            | PrimitiveType::U64
    ) {
        return None;
    }
    Some(LocalScalarRecordField {
        local: local.local,
        local_statement_ordinal: local.local_statement_ordinal,
        type_reference: local.type_reference,
        field: field.symbol,
        primitive_type,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LocalRecordSource {
    pub local: SymbolHandle,
    pub local_statement_ordinal: u32,
    pub type_reference: TypeReferenceHandle,
}

/// Rejoin a runtime expression to this exact authored statement. This does not
/// replace selective execution replay or grant validity to an unevaluated arm.
fn expression_at_statement(
    program: &TypedTrees,
    statement: &StatementNode,
    expression: ExpressionHandle,
) -> bool {
    if !program.expression_table.expression_is_valid(expression) {
        return false;
    }
    let mut pending = crate::calls::statement_value_expression_roots(program, statement);
    let mut visited = Vec::new();
    while let Some(handle) = pending.pop() {
        if handle == expression {
            return true;
        }
        if !program.expression_table.expression_is_valid(handle) || visited.contains(&handle) {
            continue;
        }
        visited.push(handle);
        crate::literals::expression_children::children(
            program,
            program.expression_table.expression(handle),
            |child| pending.push(child),
        );
    }
    false
}

fn local_plain_record(
    program: &TypedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_ordinal: u32,
    expression: ExpressionHandle,
    receiver_expression: ExpressionHandle,
) -> Option<LocalRecordSource> {
    if !machine.is_valid()
        || !state.is_valid()
        || !program.expression_table.expression_is_valid(expression)
    {
        return None;
    }
    let mut machines = program
        .machines()
        .iter()
        .filter(|owner| owner.symbol == machine);
    let machine = machines.next()?;
    if machines.next().is_some() {
        return None;
    }
    let mut states = program
        .machine_states(machine)
        .iter()
        .filter(|owner| owner.symbol == state);
    let state = states.next()?;
    if states.next().is_some() {
        return None;
    }
    let statements = program.statement_table.statements(state.statement_nodes);
    if !expression_at_statement(
        program,
        statements.get(statement_ordinal as usize)?,
        expression,
    ) {
        return None;
    }
    let ExpressionNode::Name(receiver) = program.expression_table.expression(receiver_expression)
    else {
        return None;
    };
    if !receiver.symbol.is_valid()
        || receiver.head_symbol != receiver.symbol
        || program
            .expression_table
            .name_path_members(receiver.members)
            .len()
            != 1
        || program
            .expression_table
            .name_path_member_symbols(receiver.member_symbols)
            != [receiver.symbol]
    {
        return None;
    }
    let mut locals =
        statements
            .iter()
            .enumerate()
            .filter_map(|(ordinal, statement)| match statement {
                StatementNode::LocalData(local) if local.symbol == receiver.symbol => {
                    Some((ordinal, local))
                }
                _ => None,
            });
    let (ordinal, local) = locals.next()?;
    if locals.next().is_some()
        || ordinal >= statement_ordinal as usize
        || local.is_mutable
        || !program
            .expression_table
            .expression_is_valid(local.initial_value)
        || !matches!(
            program.type_multiplicity(local.type_reference),
            Multiplicity::Affine | Multiplicity::Unrestricted
        )
        || !crate::has_plain_owned_contents(program, local.type_reference)
    {
        return None;
    }
    let TypeReferenceNode::Named { symbol, .. } = program
        .type_reference_table
        .type_reference(local.type_reference)
    else {
        return None;
    };
    if !symbol.is_valid() || program.symbols.get(*symbol).kind != symbols::SymbolKind::Data {
        return None;
    }
    let mut records = program
        .data_definitions()
        .iter()
        .filter(|record| record.symbol == *symbol);
    let record = records.next()?;
    if records.next().is_some()
        || !program.data_type_parameters(record).is_empty()
        || !has_realized_scalar_fields(program, record)
    {
        return None;
    }
    Some(LocalRecordSource {
        local: local.symbol,
        local_statement_ordinal: u32::try_from(ordinal).ok()?,
        type_reference: local.type_reference,
    })
}

/// The current local record constructor retains one runtime scalar per field.
/// Plain ownership alone also permits nested, erased and qualified storage;
/// those shapes remain outside this local field route until their construction
/// and custody are represented. Call receiver admission has its own owner.
fn has_realized_scalar_fields(
    program: &TypedTrees,
    record: &typed_trees::data::DataDefinition,
) -> bool {
    let members = program.data_members(record);
    members.iter().enumerate().all(|(ordinal, member)| {
        let DataMember::Field(field) = member else {
            return false;
        };
        !field.relevance.is_erased()
            && program.symbols.get(field.symbol).kind == symbols::SymbolKind::Field
            && program.symbols.get(field.symbol).parent == record.symbol
            && !members[..ordinal].iter().any(
                |prior| matches!(prior, DataMember::Field(prior) if prior.symbol == field.symbol),
            )
            && matches!(
                program
                    .type_reference_table
                    .type_reference(field.type_reference),
                TypeReferenceNode::Named { .. }
            )
            && matches!(
                crate::recasts::exact_primitive_type(program, field.type_reference),
                Some(
                    typed_trees::types::PrimitiveType::Bool
                        | typed_trees::types::PrimitiveType::I8
                        | typed_trees::types::PrimitiveType::I16
                        | typed_trees::types::PrimitiveType::I32
                        | typed_trees::types::PrimitiveType::I64
                        | typed_trees::types::PrimitiveType::U8
                        | typed_trees::types::PrimitiveType::U16
                        | typed_trees::types::PrimitiveType::U32
                        | typed_trees::types::PrimitiveType::U64
                        | typed_trees::types::PrimitiveType::F32
                        | typed_trees::types::PrimitiveType::F64
                )
            )
    })
}
