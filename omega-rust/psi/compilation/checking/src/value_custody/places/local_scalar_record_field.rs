//! Exact declaration custody for a scalar observation of local record storage.

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
    /// Declared carrier-field identities and literal fixed-array element
    /// selections in authored order, excluding the final scalar field.
    pub path: Vec<checked_trees::CheckedUnitStructuralPathSegment>,
    pub field: SymbolHandle,
    pub primitive_type: PrimitiveType,
    /// Governs subsequent arithmetic; observing storage does not erase policy.
    pub domain: ArithmeticDomain,
}

/// Resolve a field path below one prior, whole plain-owned local. Storage is
/// observed at this statement, never equated with its initializer: member
/// writes need not rebind the local, so absence of `mut` does not freeze fields.
/// The caller must bind the expression to this authored statement's evaluation
/// graph. Open generic/qualified owners, erased fields and scalar qualifications
/// without a complete supported predicate require additional facts. Arithmetic
/// policy changes operations, not storage: retain it alongside the observation.
/// Restrictions stay on the independently reconstructed field declaration;
/// construction and every store prove them. This query grants no membership:
/// observing the established field reads its scalar carrier.
pub fn local_scalar_record_field(
    program: &TypedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_ordinal: u32,
    expression: ExpressionHandle,
) -> Option<LocalScalarRecordField> {
    let mut steps = Vec::new();
    let mut receiver = expression;
    loop {
        if steps.len() >= program.expression_table.expression_count() {
            return None;
        }
        match program.expression_table.expression(receiver) {
            ExpressionNode::Member(member) => {
                if member.case_variant.is_some() {
                    return None;
                }
                steps.push(Step::Member(member));
                receiver = member.receiver;
            }
            ExpressionNode::Indexed(indexed) => {
                steps.push(Step::Element(indexed.index));
                receiver = indexed.collection;
            }
            _ => break,
        }
    }
    let Some(Step::Member(leaf)) = steps.first().copied() else {
        return None;
    };
    let local = local_plain_record(
        program,
        machine,
        state,
        statement_ordinal,
        expression,
        receiver,
    )?;
    // A literal element selection is a coordinate only when `[]` keeps its
    // builtin meaning along the whole place spine.
    if steps.iter().any(|step| matches!(step, Step::Element(_)))
        && !builtin_place_coordinates(program, machine, state, expression)
    {
        return None;
    }
    let mut carrier = local.type_reference;
    let mut path = Vec::new();
    // Every step is resolved under its own declaration. Reusing only the final
    // field spelling would confuse identically shaped children of this root.
    for step in steps.iter().rev().take(steps.len() - 1) {
        let member = match step {
            Step::Member(member) => member,
            // The checker's place facts fold a constant selector to its
            // element; a runtime selector is not a field path.
            Step::Element(index) => {
                let TypeReferenceNode::FixedArray {
                    element_type,
                    length: typed_trees::types::FixedArrayLength::Literal(length),
                } = program.type_reference_table.type_reference(carrier)
                else {
                    return None;
                };
                let index = program
                    .expression_table
                    .constant_integer_value(*index)
                    .and_then(|index| u64::try_from(index).ok())?;
                if usize::try_from(index).ok()? >= *length {
                    return None;
                }
                path.push(checked_trees::CheckedUnitStructuralPathSegment::FixedIndex(
                    index,
                ));
                carrier = *element_type;
                continue;
            }
        };
        let record = crate::program_scope::data_definitions::data_definition_by_symbol(
            program,
            program.type_reference_symbol(carrier),
        )?;
        let field = super::exact_data_member_field(
            program,
            record,
            member.member_symbol,
            member.member.as_str(),
            None,
        )?;
        if field.relevance.is_erased()
            || program.symbols.get(field.symbol).kind != symbols::SymbolKind::Field
            || program.symbols.get(field.symbol).parent != record.symbol
            || !program.data_type_parameters(record).is_empty()
            || program.data_members(record).iter().filter(|member| matches!(member, DataMember::Field(candidate) if candidate.symbol == field.symbol)).count() != 1
        {
            return None;
        }
        path.push(checked_trees::CheckedUnitStructuralPathSegment::Field(
            field.path_identity(),
        ));
        carrier = field.type_reference;
    }
    let record = crate::program_scope::data_definitions::data_definition_by_symbol(
        program,
        program.type_reference_symbol(carrier),
    )?;
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
            | PrimitiveType::F32
            | PrimitiveType::F64
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
        domain: program.arithmetic_domain_for_type_reference(field.type_reference),
    })
}

/// One projection between the local root and the observed scalar field.
#[derive(Clone, Copy)]
enum Step<'a> {
    Member(&'a typed_trees::expression::TableMemberExpression),
    Element(ExpressionHandle),
}

/// The exact authored machine and state, when each symbol names exactly one,
/// asked whether the place spine keeps builtin coordinates.
fn builtin_place_coordinates(
    program: &TypedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    expression: ExpressionHandle,
) -> bool {
    let mut machines = program
        .machines()
        .iter()
        .filter(|owner| owner.symbol == machine);
    let (Some(machine), None) = (machines.next(), machines.next()) else {
        return false;
    };
    let mut states = program
        .machine_states(machine)
        .iter()
        .filter(|owner| owner.symbol == state);
    let (Some(state), None) = (states.next(), states.next()) else {
        return false;
    };
    super::place_has_builtin_coordinates(program, machine, Some(state), expression)
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
    let mut pending =
        crate::machine_calls::calls::statement_value_expression_roots(program, statement);
    let mut visited = Vec::new();
    while let Some(handle) = pending.pop() {
        if handle == expression {
            return true;
        }
        if !program.expression_table.expression_is_valid(handle) || visited.contains(&handle) {
            continue;
        }
        visited.push(handle);
        crate::expression_traversal::children(
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
    let (ordinal, local) = program.state_local_declaration(state, receiver.symbol)?;
    // A declared-but-uninitialized record carries no initializer to
    // observe; its storage is the zeroed establishment minted at declaration,
    // which the ledger and the ordered Unit effect independently proved.
    let declared_zeroed = !local.initial_value.is_valid()
        && matches!(
            program
                .type_reference_table
                .type_reference(local.type_reference),
            TypeReferenceNode::Named { .. } | TypeReferenceNode::Generic { .. }
        )
        && crate::zeroed_structural_parts(program, local.type_reference);
    if ordinal >= statement_ordinal as usize
        || !(program
            .expression_table
            .expression_is_valid(local.initial_value)
            || declared_zeroed)
        || !matches!(
            program.type_multiplicity(local.type_reference),
            Multiplicity::Affine | Multiplicity::Unrestricted
        )
    {
        return None;
    }
    // A borrow local presents its referent's record storage: `view.f` reads
    // field `f` of `T` out of the borrowed place without moving custody.
    // The returned `type_reference` stays the referent so downstream carriers
    // and structural type identities keep naming the record, not `&T`.
    let carrier = match program
        .type_reference_table
        .type_reference(local.type_reference)
    {
        TypeReferenceNode::Reference {
            referee,
            access:
                language_semantics::ReferenceAccess::Shared
                | language_semantics::ReferenceAccess::Mutable,
            ..
        } => *referee,
        _ => local.type_reference,
    };
    // The storage judgment owns the complete record's custody restrictions.
    // Observation only selects a scalar path: unrelated arrays and sums do
    // not need to satisfy a second, scalar-record-only construction roster.
    // Their establishment remains the producer's independently checked work.
    // A record mixing plain fields with shared views keeps that judgment:
    // reading a scalar member moves no view custody, and the view carrier
    // classifies the same owned-carrier shape the disposal and return paths
    // already accept.
    if !crate::has_plain_owned_contents_with_numeric_constraints(program, carrier)
        && !crate::has_owned_or_shared_view_fields(program, carrier)
    {
        return None;
    }
    let TypeReferenceNode::Named { symbol, .. } =
        program.type_reference_table.type_reference(carrier)
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
    if records.next().is_some() || !program.data_type_parameters(record).is_empty() {
        return None;
    }
    Some(LocalRecordSource {
        local: local.symbol,
        local_statement_ordinal: u32::try_from(ordinal).ok()?,
        type_reference: carrier,
    })
}

/// Policy-only integer fields use their ordinary scalar storage, while the
/// returned observation separately retains their arithmetic policy. Closed
/// ranges and exact declared intervals share the bounded-field storage contract;
/// complete scalar predicates retain their declaration invariant. Unsupported
/// predicates and restricted non-Exact policies remain unsupported.
fn realized_scalar_carrier(
    program: &TypedTrees,
    mut reference: TypeReferenceHandle,
) -> Option<PrimitiveType> {
    if crate::is_arithmetic_policy_only_integer(program, reference) {
        return program.primitive_type_reference(reference);
    }
    let primitive = program.primitive_type_reference(reference)?;
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
            match constraint {
                TypeConstraintNode::Range {
                    minimum,
                    maximum,
                    end_inclusive,
                } => {
                    crate::closed_integer_range_bound(program, *minimum)?;
                    crate::closed_integer_range_maximum(program, *maximum, *end_inclusive)?;
                    bounded = true;
                }
                TypeConstraintNode::Domain(domain) => {
                    if let Some((minimum, maximum)) =
                        crate::exact_declared_domain_carrier_interval(program, primitive, domain)
                    {
                        if minimum > maximum {
                            return None;
                        }
                        bounded = true;
                    } else {
                        crate::scalar_field_domain_predicate(program, domain, primitive)?;
                    }
                }
                _ => return None,
            }
        }
        reference = *base_type;
    }
    let primitive = crate::value_custody::recasts::exact_primitive_type(program, reference)?;
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
