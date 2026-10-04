//! Exact declaration custody for scalar observations of established structural storage.
//!
//! Named locals and saved expression results share the same declaration walk.
//! A saved dispatch subject retains its authored occurrence until execution
//! planning assigns storage; looking up a field neither executes that subject
//! nor grants its case membership or ownership. Those remain use-site checks.

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
pub struct StructuralScalarField {
    pub root: facts::PlaceRoot,
    pub statement_ordinal: u32,
    /// Whole storage declaration, before any field projection.
    pub type_reference: TypeReferenceHandle,
    /// Data declaration containing the final scalar field or selected payload.
    pub carrier_type_reference: TypeReferenceHandle,
    /// Declared case and carrier-field identities and literal fixed-array
    /// selections in authored order, excluding the final scalar field.
    pub path: Vec<checked_trees::CheckedUnitStructuralPathSegment>,
    pub field: SymbolHandle,
    pub primitive_type: PrimitiveType,
    /// Governs subsequent arithmetic; observing storage does not erase policy.
    pub domain: ArithmeticDomain,
}

/// Resolve a field path below a prior local or an evaluated dispatch subject. Storage is
/// observed at this statement, never equated with its initializer: member
/// writes need not rebind the local, so absence of `mut` does not freeze fields.
/// The caller must bind the expression to this authored statement's evaluation
/// graph. Open generic/qualified owners, erased fields and scalar qualifications
/// without a complete supported predicate require additional facts. Arithmetic
/// policy changes operations, not storage: retain it alongside the observation.
/// Restrictions stay on the independently reconstructed field declaration;
/// construction and every store prove them. This query grants no membership:
/// observing the established field reads its scalar carrier.
pub fn structural_scalar_field(
    program: &TypedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_ordinal: u32,
    expression: ExpressionHandle,
) -> Option<StructuralScalarField> {
    let mut steps = Vec::new();
    let mut receiver = expression;
    loop {
        if steps.len() >= program.expression_table.expression_count() {
            return None;
        }
        match program.expression_table.expression(receiver) {
            ExpressionNode::Member(member) => {
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
    let local = structural_source(
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
        if !program.data_type_parameters(record).is_empty() {
            return None;
        }
        let field = selected_member_field(program, record, member, &mut path)?;
        path.push(checked_trees::CheckedUnitStructuralPathSegment::Field(
            field.path_identity(),
        ));
        carrier = field.type_reference;
    }
    let record = crate::program_scope::data_definitions::data_definition_by_symbol(
        program,
        program.type_reference_symbol(carrier),
    )?;
    let field = selected_member_field(program, record, leaf, &mut path)?;
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
    Some(StructuralScalarField {
        root: local.root,
        statement_ordinal: local.statement_ordinal,
        type_reference: local.type_reference,
        carrier_type_reference: carrier,
        path,
        field: field.symbol,
        primitive_type,
        domain: program.arithmetic_domain_for_type_reference(field.type_reference),
    })
}

/// Select under the current receiver declaration, retaining the case edge
/// before its payload field. A selected case is a path coordinate, not a
/// synthetic record type; execution still needs its established case.
fn selected_member_field<'program>(
    program: &'program TypedTrees,
    record: &'program typed_trees::data::DataDefinition,
    member: &typed_trees::expression::TableMemberExpression,
    path: &mut Vec<checked_trees::CheckedUnitStructuralPathSegment>,
) -> Option<&'program typed_trees::data::DataField> {
    let field = super::exact_data_member_field(
        program,
        record,
        member.member_symbol,
        member.member.as_str(),
        member.case_variant.as_ref().map(|variant| variant.as_str()),
    )?;
    if field.relevance.is_erased()
        || program.symbols.get(field.symbol).kind != symbols::SymbolKind::Field
    {
        return None;
    }
    let owner = if let Some(case_name) = &member.case_variant {
        let mut variants = program.data_members(record).iter().filter_map(|member| {
            let DataMember::Variant(variant) = member else {
                return None;
            };
            (variant.name == *case_name).then_some(variant)
        });
        let variant = variants.next()?;
        if variants.next().is_some()
            || !variant.symbol.is_valid()
            || program.symbols.get(variant.symbol).kind != symbols::SymbolKind::Variant
            || program.symbols.get(variant.symbol).parent != record.symbol
            || program
                .data_payload_fields(variant)
                .iter()
                .filter(|candidate| candidate.symbol == field.symbol)
                .count()
                != 1
        {
            return None;
        }
        path.push(checked_trees::CheckedUnitStructuralPathSegment::Case(
            variant.path_identity(),
        ));
        variant.symbol
    } else {
        if program
            .data_members(record)
            .iter()
            .filter(|member| matches!(member, DataMember::Field(candidate) if candidate.symbol == field.symbol))
            .count()
            != 1
        {
            return None;
        }
        record.symbol
    };
    (program.symbols.get(field.symbol).parent == owner).then_some(field)
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
struct StructuralSource {
    pub root: facts::PlaceRoot,
    pub statement_ordinal: u32,
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

/// Rejoin a copied dispatch subject to the first arm that evaluates it.
/// The argument is the receiver expression, not its payload projection.
/// Identical spelling alone is insufficient: separately authored calls retain
/// different selection occurrences even when their arguments are identical.
/// This describes source identity; a consumer must still prove the unique
/// producing operation completed before reading its established storage.
pub fn saved_dispatch_subject(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<(usize, ExpressionHandle)> {
    let table = &program.expression_table;
    if !table.expression_is_valid(expression)
        || !matches!(table.expression(expression), ExpressionNode::Call(_))
    {
        return None;
    }
    let statements = program.statement_table.statements(state.statement_nodes);
    let statement = statements.get(statement_index)?;
    if !expression_at_statement(program, statement, expression) {
        return None;
    }
    let same_subject = |subject| same_authored_call(program, subject, expression);
    if !same_subject(dispatch_case_subject(program, statement)?) {
        return None;
    }
    statements[..=statement_index]
        .iter()
        .enumerate()
        .find_map(|(ordinal, statement)| {
            let subject = dispatch_case_subject(program, statement)?;
            same_subject(subject).then_some((ordinal, subject))
        })
}

/// Rejoin a call anywhere beneath the saved subject to its canonical
/// evaluation, retaining its exact position inside that subject's tree.
/// Guards and payload receivers can contain separate copies of the entire
/// tree. Every copied call keeps its authored selection occurrence; a fresh
/// equal-looking call in an arm does not become a saved observation.
///
/// A shared expression handle may also occur on an independently evaluated
/// edge. Track containment per path, and decline that ambiguity rather than
/// letting a copied occurrence erase the independent edge's effects.
pub fn saved_dispatch_call(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    statement_index: usize,
    expression: ExpressionHandle,
) -> Option<(usize, ExpressionHandle)> {
    let table = &program.expression_table;
    if !table.expression_is_valid(expression)
        || !matches!(table.expression(expression), ExpressionNode::Call(_))
    {
        return None;
    }
    let statement = program
        .statement_table
        .statements(state.statement_nodes)
        .get(statement_index)?;
    let mut pending =
        crate::machine_calls::calls::statement_value_expression_roots(program, statement)
            .into_iter()
            .map(|node| (node, None))
            .collect::<Vec<_>>();
    let mut visited = Vec::new();
    let mut found = None;
    while let Some((node, inherited)) = pending.pop() {
        if node == ExpressionHandle::invalid() {
            if inherited.is_some_and(|(_, counterpart)| counterpart != ExpressionHandle::invalid())
            {
                return None;
            }
            continue;
        }
        if !table.expression_is_valid(node) {
            return None;
        }
        let canonical =
            inherited.or_else(|| saved_dispatch_subject(program, state, statement_index, node));
        if visited.contains(&(node, canonical)) {
            continue;
        }
        visited.push((node, canonical));
        if let Some((_, counterpart)) = canonical
            && (!table.expression_is_valid(counterpart)
                || !table.expressions_structurally_equal(node, counterpart)
                || (matches!(table.expression(node), ExpressionNode::Call(_))
                    && !same_authored_call(program, node, counterpart)))
        {
            return None;
        }
        if node == expression {
            let canonical = canonical?;
            if found.is_some_and(|prior| prior != canonical) {
                return None;
            }
            found = Some(canonical);
            continue;
        }
        let mut children = Vec::new();
        crate::expression_traversal::children(program, table.expression(node), |child| {
            children.push(child);
        });
        if let Some((ordinal, counterpart)) = canonical {
            let mut counterparts = Vec::new();
            crate::expression_traversal::children(
                program,
                table.expression(counterpart),
                |child| {
                    counterparts.push(child);
                },
            );
            if children.len() != counterparts.len() {
                return None;
            }
            pending.extend(
                children
                    .into_iter()
                    .zip(counterparts)
                    .map(|(child, counterpart)| (child, Some((ordinal, counterpart)))),
            );
        } else {
            pending.extend(children.into_iter().map(|child| (child, None)));
        }
    }
    found
}

fn same_authored_call(
    program: &TypedTrees,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> bool {
    let table = &program.expression_table;
    if !matches!(table.expression(left), ExpressionNode::Call(_))
        || !matches!(table.expression(right), ExpressionNode::Call(_))
        || !table.expressions_structurally_equal(left, right)
    {
        return false;
    }
    let mut occurrences = table.authored_selection_occurrences(left).peekable();
    occurrences.peek().is_some() && occurrences.eq(table.authored_selection_occurrences(right))
}

fn dispatch_case_subject(
    program: &TypedTrees,
    statement: &StatementNode,
) -> Option<ExpressionHandle> {
    use typed_trees::expression::BinaryOperator;
    use typed_trees::statement::TransitionGuardNode;
    let StatementNode::Transition(transition) = statement else {
        return None;
    };
    let TransitionGuardNode::When(mut guard) = transition.guard else {
        return None;
    };
    let table = &program.expression_table;
    for _ in 0..table.expression_count() {
        let ExpressionNode::Binary(binary) = table.expression(guard) else {
            return None;
        };
        if !matches!(
            binary.operator,
            BinaryOperator::Equal | BinaryOperator::CaseMembership
        ) {
            return None;
        }
        if binary.operator == BinaryOperator::Equal {
            if matches!(table.expression(binary.left), ExpressionNode::Boolean(true)) {
                guard = binary.right;
                continue;
            }
            if matches!(
                table.expression(binary.right),
                ExpressionNode::Boolean(true)
            ) {
                guard = binary.left;
                continue;
            }
        }
        if crate::exact_case_reference_owner(program, binary.right).is_some() {
            return Some(binary.left);
        }
        if binary.operator == BinaryOperator::Equal
            && crate::exact_case_reference_owner(program, binary.left).is_some()
        {
            return Some(binary.right);
        }
        return None;
    }
    None
}

fn structural_source(
    program: &TypedTrees,
    machine: SymbolHandle,
    state: SymbolHandle,
    statement_ordinal: u32,
    expression: ExpressionHandle,
    receiver_expression: ExpressionHandle,
) -> Option<StructuralSource> {
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
    let (root, ordinal, source_type) =
        match program.expression_table.expression(receiver_expression) {
            ExpressionNode::Name(receiver) => {
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
                // A zeroed declaration owns storage even without an initializer.
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
                {
                    return None;
                }
                (
                    facts::PlaceRoot::Symbol(local.symbol),
                    ordinal,
                    local.type_reference,
                )
            }
            ExpressionNode::Call(_) => {
                let (ordinal, subject) = saved_dispatch_subject(
                    program,
                    state,
                    statement_ordinal as usize,
                    receiver_expression,
                )?;
                let ExpressionNode::Call(call) = program.expression_table.expression(subject)
                else {
                    return None;
                };
                let result_type =
                    crate::machine_calls::calls::resolved_call_result_type(program, call)?;
                (facts::PlaceRoot::Expression(subject), ordinal, result_type)
            }
            _ => return None,
        };
    if !matches!(
        program.type_multiplicity(source_type),
        Multiplicity::Affine | Multiplicity::Unrestricted
    ) {
        return None;
    }
    // A borrow local observes its referent without moving its reference.
    // An expression result must already own its home before it can be read.
    let carrier = match program.type_reference_table.type_reference(source_type) {
        TypeReferenceNode::Reference {
            referee,
            access:
                language_semantics::ReferenceAccess::Shared
                | language_semantics::ReferenceAccess::Mutable,
            ..
        } if matches!(root, facts::PlaceRoot::Symbol(_)) => *referee,
        _ => source_type,
    };
    // A scalar observation moves neither its record nor neighboring reference
    // permissions. Establishment and use-site custody are checked separately;
    // a whole-content transfer classifier is not a storage-read judgment.
    // This route still carries no linear-claim requirements.
    if !crate::linear_claim_frontier(program, carrier).is_empty() {
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
    Some(StructuralSource {
        root,
        statement_ordinal: u32::try_from(ordinal).ok()?,
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
