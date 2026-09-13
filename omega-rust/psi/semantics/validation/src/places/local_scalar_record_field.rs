//! Exact declaration custody for a scalar read from an immutable local record.

use language_semantics::Multiplicity;
use numerics::arithmetic::ArithmeticDomain;
use symbols::SymbolHandle;
use typed_trees::TypedTrees;
use typed_trees::data::DataMember;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::statement::StatementNode;
use typed_trees::types::{
    PrimitiveType, TypeConstraintNode, TypeReferenceHandle, TypeReferenceNode,
};

#[cfg(test)]
mod tests;

/// Source identities only; this does not grant ownership or establish loan
/// validity. Checked flow and the consuming replay still validate availability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalScalarRecordField {
    pub local: SymbolHandle,
    pub local_statement_ordinal: u32,
    /// Whole local declaration, before any field projection.
    pub type_reference: TypeReferenceHandle,
    /// Record containing the final scalar field.
    pub carrier_type_reference: TypeReferenceHandle,
    /// Declared carrier-field identities, excluding the final scalar field.
    pub path: Vec<String>,
    pub field: SymbolHandle,
    pub primitive_type: PrimitiveType,
}

/// Resolve a field path below one prior, whole immutable plain-owned local.
/// The caller must bind the expression to this authored statement's evaluation
/// graph. Open generic/qualified owners, erased fields and
/// nominally qualified or policy-bearing scalar fields require additional facts.
/// Closed integer ranges keep their bounded declaration and construction proof;
/// observing the established field uses its underlying scalar carrier.
pub fn local_scalar_record_field(
    program: &TypedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_ordinal: u32,
    expression: ExpressionHandle,
) -> Option<LocalScalarRecordField> {
    let mut members = Vec::new();
    let mut receiver = expression;
    while let ExpressionNode::Member(member) = program.expression_table.expression(receiver) {
        if members.len() >= program.expression_table.expression_count()
            || member.case_variant.is_some()
        {
            return None;
        }
        members.push(member);
        receiver = member.receiver;
    }
    let leaf = *members.first()?;
    let local = local_plain_record(
        program,
        machine,
        state,
        statement_ordinal,
        expression,
        receiver,
    )?;
    let mut carrier = local.type_reference;
    let mut path = Vec::new();
    // Every step is resolved under its own declaration. Reusing only the final
    // field spelling would confuse identically shaped children of this root.
    for member in members.iter().rev().take(members.len() - 1) {
        let record = program
            .data_definitions()
            .iter()
            .find(|record| record.symbol == program.type_reference_symbol(carrier))?;
        let field = super::exact_data_member_field(
            program,
            record,
            member.member_symbol,
            member.member.as_str(),
            None,
        )?;
        if field.relevance.is_erased() {
            return None;
        }
        path.push(
            field
                .identity
                .map(|identity| format!("#{identity}"))
                .unwrap_or_else(|| field.name.as_str().to_owned()),
        );
        carrier = field.type_reference;
    }
    let record = program
        .data_definitions()
        .iter()
        .find(|record| record.symbol == program.type_reference_symbol(carrier))?;
    let field = super::exact_data_member_field(
        program,
        record,
        leaf.member_symbol,
        leaf.member.as_str(),
        None,
    )?;
    if program.symbols.get(field.symbol).kind != symbols::SymbolKind::Field
        || program.symbols.get(field.symbol).parent != record.symbol
        || program.data_members(record).iter().filter(|member| matches!(member, DataMember::Field(candidate) if candidate.symbol == field.symbol)).count() != 1
        || field.relevance.is_erased()
        || program.arithmetic_domain_for_type_reference(field.type_reference) != ArithmeticDomain::Exact
    {
        return None;
    }
    let primitive_type = realized_scalar_carrier(program, field.type_reference)?;
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
        carrier_type_reference: carrier,
        path,
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
        || !crate::has_plain_owned_contents_with_numeric_constraints(program, local.type_reference)
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
        || !has_realized_record_fields(program, record, &mut Vec::new())
    {
        return None;
    }
    Some(LocalRecordSource {
        local: local.symbol,
        local_statement_ordinal: u32::try_from(ordinal).ok()?,
        type_reference: local.type_reference,
    })
}

/// Construction retains the entire nested record, including unread siblings.
/// Validate their declaration rosters as well; selecting a scalar cannot erase
/// an unsupported sibling, an ownership shell or a recursive storage cycle.
fn has_realized_record_fields(
    program: &TypedTrees,
    record: &typed_trees::data::DataDefinition,
    active: &mut Vec<SymbolHandle>,
) -> bool {
    if active.contains(&record.symbol)
        || !program.data_type_parameters(record).is_empty()
        || program.symbols.get(record.symbol).kind != symbols::SymbolKind::Data
        || program
            .data_definitions()
            .iter()
            .filter(|candidate| candidate.symbol == record.symbol)
            .count()
            != 1
    {
        return false;
    }
    active.push(record.symbol);
    let members = program.data_members(record);
    let valid = members.len() == record.members.count() as usize
        && members.iter().enumerate().all(|(ordinal, member)| {
            let DataMember::Field(field) = member else {
                return false;
            };
            !field.relevance.is_erased()
            && program.symbols.get(field.symbol).kind == symbols::SymbolKind::Field
            && program.symbols.get(field.symbol).parent == record.symbol
            && !members[..ordinal].iter().any(
                |prior| matches!(prior, DataMember::Field(prior) if prior.symbol == field.symbol),
            )
            && (matches!(
                realized_scalar_carrier(program, field.type_reference),
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
            ) || matches!(program.type_reference_table.type_reference(field.type_reference),
                TypeReferenceNode::Named { symbol, .. } if program.data_definitions().iter()
                    .find(|nested| nested.symbol == *symbol)
                    .is_some_and(|nested| has_realized_record_fields(program, nested, active))))
        });
    active.pop();
    valid
}

/// Only range shells may use the bounded-record construction/replay route.
/// Peeling an arbitrary constraint here would erase a qualification or policy.
fn realized_scalar_carrier(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
) -> Option<PrimitiveType> {
    let mut bounded = false;
    while let TypeReferenceNode::Constrained {
        base_type,
        constraints,
    } = program.type_reference_table.type_reference(reference)
    {
        let rows = program.type_reference_table.constraints(*constraints);
        if rows.is_empty() || rows.len() != constraints.count() as usize {
            return None;
        }
        for constraint in rows {
            let TypeConstraintNode::Range {
                minimum,
                maximum,
                end_inclusive,
            } = constraint
            else {
                return None;
            };
            crate::closed_integer_range_bound(program, *minimum)?;
            crate::closed_integer_range_maximum(program, *maximum, *end_inclusive)?;
        }
        bounded = true;
        reference = *base_type;
    }
    let primitive = crate::recasts::exact_primitive_type(program, reference)?;
    (!bounded
        || matches!(
            primitive,
            PrimitiveType::I8
                | PrimitiveType::I16
                | PrimitiveType::I32
                | PrimitiveType::I64
                | PrimitiveType::U8
                | PrimitiveType::U16
                | PrimitiveType::U32
                | PrimitiveType::U64
        ))
    .then_some(primitive)
}
