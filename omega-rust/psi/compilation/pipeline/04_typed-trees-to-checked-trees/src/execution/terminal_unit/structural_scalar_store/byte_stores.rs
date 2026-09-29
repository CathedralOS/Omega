//! Byte-sequence leaves: a whole borrowed byte view, or a bounded-owned byte
//! field below an exclusive root. These leaves have their own Terminal
//! operations (`ByteSequenceWrite`, `StructuralByteSequenceFieldStore`,
//! `StructuralByteSequenceFieldByteStore`), but the destination and the `u8`
//! value are resolved by the same owners as every other store.
use super::super::{
    CheckFacts, CheckedStructuralAccess, CheckedUnitEffectOperationPlan, ExpressionNode,
    Multiplicity, TypeReferenceNode, TypedTrees,
};
use super::destination::{Destination, Root};
use super::value::{self, AssignmentSource};
use crate::execution::terminal_unit::control::LocalConstructionTrace;
use crate::execution::terminal_unit::types::{byte_sequence_carrier, terminal_field_identity};

/// `view[index] = byte` through a whole mutable borrowed byte view. Replacing
/// a byte observes the view's current length, so the root needs an ordinary
/// unrestricted, unqualified mutable borrow rather than write-only authority.
pub(super) fn view_write(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    destination: &Destination<'_>,
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
    source: AssignmentSource,
    trace: &LocalConstructionTrace,
) -> Option<CheckedUnitEffectOperationPlan> {
    let index = destination.dynamic_index?;
    if !destination.place.segments.is_empty() {
        return None;
    }
    // `view[index] = element` through a mutable `&mut [T]` view: the same
    // runtime-indexed write shape as the byte view above, but the carrier
    // observes an element, so the plan names the element's primitive rather
    // than hardcoding `u8`. A parameter destination is the signature's
    // borrowed slice; a local destination is an immutable `&mut` binding
    // whose single Mutable loan the destination root already re-read as the
    // write authority.
    if let Some(element) = element_view_element(program, state, destination) {
        return Some(CheckedUnitEffectOperationPlan::ElementViewWrite(
            checked_trees::CheckedElementViewWritePlan {
                statement_index,
                destination: destination.root.destination(),
                index: value::runtime_index(facts, state, statement_index, index)?,
                element,
                value: value::assignment_value(
                    program,
                    facts,
                    machine,
                    state,
                    &destination.root,
                    statement_index,
                    assignment,
                    source,
                    element,
                    trace,
                )?,
            },
        ));
    }
    let Root::Parameter { plan, parameter } = destination.root else {
        return None;
    };
    if plan.is_self
        || plan.access != CheckedStructuralAccess::MutableBorrow
        || plan.multiplicity != Multiplicity::Unrestricted
        || !plan.qualifications.is_empty()
        || plan.fused_service_erasure.is_some()
        || !byte_sequence_carrier(program, parameter.type_reference, &[])
            .is_some_and(checked_trees::CheckedByteSequenceCarrier::is_borrowed)
    {
        return None;
    }
    Some(CheckedUnitEffectOperationPlan::ByteSequenceWrite(
        checked_trees::CheckedByteSequenceWritePlan {
            statement_index,
            destination_parameter_position: plan.position,
            index: value::runtime_index(facts, state, statement_index, index)?,
            value: value::byte_value(
                program,
                facts,
                machine,
                state,
                &destination.root,
                statement_index,
                assignment,
                source,
                trace,
            )?,
        },
    ))
}

/// The scalar element a `&mut [T]` view destination stores, or `None` when
/// the root names no mutable slice view or its element is not a primitive.
/// A parameter root re-checks the signature's admitted borrow; a local root
/// re-reads the declared `&mut` type of the binding its root resolved.
fn element_view_element(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    destination: &Destination<'_>,
) -> Option<typed_trees::types::PrimitiveType> {
    let type_reference = match &destination.root {
        Root::Parameter { plan, parameter } => {
            if plan.is_self
                || plan.access != CheckedStructuralAccess::MutableBorrow
                || plan.multiplicity != Multiplicity::Unrestricted
                || !plan.qualifications.is_empty()
                || plan.fused_service_erasure.is_some()
            {
                return None;
            }
            parameter.type_reference
        }
        Root::Local { symbol } => program
            .statement_table
            .statements(state.statement_nodes)
            .iter()
            .find_map(|statement| match statement {
                typed_trees::statement::StatementNode::LocalData(local)
                    if local.symbol == *symbol =>
                {
                    Some(local.type_reference)
                }
                _ => None,
            })?,
    };
    let (referee, access) =
        super::super::reference_results::mutable_borrowed_parts(program, type_reference)?;
    if access != CheckedStructuralAccess::MutableBorrow {
        return None;
    }
    let TypeReferenceNode::Slice { element_type } =
        program.type_reference_table.type_reference(referee)
    else {
        return None;
    };
    // The byte element keeps its established `ByteSequence` carrier and
    // `ByteSequenceWrite` vocabulary; the element arm covers only the
    // non-byte view family.
    let element = program.primitive_type_reference(*element_type)?;
    (element != typed_trees::types::PrimitiveType::U8).then_some(element)
}

/// A whole borrowed owner retains its caller's capacity and qualifications.
pub(super) fn whole_store(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    destination: &Destination<'_>,
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
    source: AssignmentSource,
) -> Option<CheckedUnitEffectOperationPlan> {
    let Root::Parameter { plan, parameter } = destination.root else {
        return None;
    };
    if !destination.place.segments.is_empty()
        || destination.dynamic_index.is_some()
        || !matches!(source, AssignmentSource::Authored)
        || !matches!(
            plan.access,
            CheckedStructuralAccess::MutableBorrow | CheckedStructuralAccess::WriteOnlyBorrow
        )
        || plan.multiplicity != Multiplicity::Unrestricted
        || plan.fused_service_erasure.is_some()
    {
        return None;
    }
    let checked_trees::CheckedByteSequenceCarrier::BoundedOwned { capacity } =
        byte_sequence_carrier(program, parameter.type_reference, &[])?
    else {
        return None;
    };
    let domains =
        crate::facts::field_domain::domain_constraint_symbols(program, parameter.type_reference);
    if !domains.iter().all(|symbol| {
        typed_trees::byte_predicates::domain_byte_predicate(program, *symbol).is_some()
    }) {
        return None;
    }
    let mut value = checked_trees::CheckedByteSequenceValuePlan::default();
    let (root, _) = byte_value_plan(
        program,
        facts,
        state,
        statement_index,
        assignment.value,
        &domains,
        &mut value,
        0,
    )?;
    value.root = root;
    if let checked_trees::CheckedByteSequenceValue::Literal(bytes) = value.nodes.get(root)
        && u64::try_from(bytes.len()).ok()? > capacity
    {
        return None;
    }
    Some(CheckedUnitEffectOperationPlan::ByteSequenceStore(
        checked_trees::CheckedByteSequenceStorePlan {
            statement_index,
            destination_parameter_position: plan.position,
            destination: checked_trees::CheckedByteSequenceStoreDestination::Whole,
            value,
        },
    ))
}

/// A byte field leaf: `field[index] = byte` replaces one live byte of a
/// bounded-owned field or of a mutable borrowed view, and `field =
/// "literal"` replaces a bounded-owned field's live prefix (`capacity` is
/// that field's bound; a borrowed view has none and hosts no literal). A
/// domain on the field must be a plain predicate domain; a whole-literal
/// store must also satisfy it from the literal's own bytes.
pub(super) fn field_store(
    program: &TypedTrees,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    destination: &Destination<'_>,
    carrier_path: Vec<checked_trees::CheckedUnitStructuralPathSegment>,
    field: &typed_trees::data::DataField,
    capacity: Option<u64>,
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
    source: AssignmentSource,
    trace: &LocalConstructionTrace,
) -> Option<CheckedUnitEffectOperationPlan> {
    let destination_parameter_position = destination.root.parameter_position()?;
    let plain_domain = |symbol, literal_grant: bool| {
        program
            .domain_definitions()
            .iter()
            .find(|domain| domain.symbol == symbol)
            .is_some_and(|domain| {
                domain.establishment_routes.is_empty()
                    && domain.semantic_roles == Default::default()
                    && (!literal_grant
                        || crate::facts::field_domain::string_literal_expression_grants_domain(
                            program,
                            assignment.value,
                            symbol,
                        ))
            })
    };
    let domains =
        crate::facts::field_domain::domain_constraint_symbols(program, field.type_reference);
    if let Some(index) = destination.dynamic_index {
        if !domains
            .into_iter()
            .all(|symbol| plain_domain(symbol, false))
        {
            return None;
        }
        return Some(
            CheckedUnitEffectOperationPlan::StructuralByteSequenceFieldByteStore(
                checked_trees::CheckedStructuralByteSequenceFieldByteStorePlan {
                    statement_index,
                    destination_parameter_position,
                    carrier_path,
                    field_identity: terminal_field_identity(program, field.symbol)?,
                    index: value::runtime_index(facts, state, statement_index, index)?,
                    value: value::byte_value(
                        program,
                        facts,
                        machine,
                        state,
                        &destination.root,
                        statement_index,
                        assignment,
                        source,
                        trace,
                    )?,
                },
            ),
        );
    }
    let (AssignmentSource::Authored, Some(capacity)) = (source, capacity) else {
        return None;
    };
    if !domains.iter().all(|symbol| plain_domain(*symbol, false)) {
        return None;
    }
    let mut value = checked_trees::CheckedByteSequenceValuePlan::default();
    let (root, _) = byte_value_plan(
        program,
        facts,
        state,
        statement_index,
        assignment.value,
        &domains,
        &mut value,
        0,
    )?;
    value.root = root;
    // Source checking owns the receiving bound. Each concat owns its separate
    // checked sum bound; never reject a short value merely for carrier capacity.
    if let checked_trees::CheckedByteSequenceValue::Literal(bytes) = value.nodes.get(root)
        && u64::try_from(bytes.len()).ok()? > capacity
    {
        return None;
    }
    Some(CheckedUnitEffectOperationPlan::ByteSequenceStore(
        checked_trees::CheckedByteSequenceStorePlan {
            statement_index,
            destination_parameter_position,
            destination: checked_trees::CheckedByteSequenceStoreDestination::Field {
                carrier_path,
                field_identity: terminal_field_identity(program, field.symbol)?,
            },
            value,
        },
    ))
}

/// Preserve only the selected built-in byte law. Leaves are immutable literals
/// or readable declared fields; there is no delayed call evaluation in this plan.
fn byte_value_plan(
    program: &TypedTrees,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    statement_index: u32,
    expression: typed_trees::expression::ExpressionHandle,
    domains: &[symbols::SymbolHandle],
    plan: &mut checked_trees::CheckedByteSequenceValuePlan,
    depth: usize,
) -> Option<(arena::Handle<checked_trees::CheckedByteSequenceValue>, u64)> {
    use checked_trees::{
        CheckedByteSequenceValue as Value, CheckedUnitStructuralPathSegment as Segment,
    };
    if depth > 128 {
        return None;
    }
    let (value, capacity) = match program.expression_table.expression(expression) {
        ExpressionNode::String(bytes) => {
            if !domains.iter().all(|domain| {
                crate::facts::field_domain::string_literal_expression_grants_domain(
                    program, expression, *domain,
                )
            }) {
                return None;
            }
            (
                Value::Literal(bytes.to_vec()),
                u64::try_from(bytes.len()).ok()?,
            )
        }
        ExpressionNode::Binary(binary)
            if binary.operator == typed_trees::expression::BinaryOperator::Add
                && crate::values::operator_is_builtin(&facts.operators, expression) =>
        {
            if !domains.iter().all(|domain| {
                crate::facts::field_domain::domain_is_concat_preserving(program, *domain)
            }) {
                return None;
            }
            let (left, left_capacity) = byte_value_plan(
                program,
                facts,
                state,
                statement_index,
                binary.left,
                domains,
                plan,
                depth + 1,
            )?;
            let (right, right_capacity) = byte_value_plan(
                program,
                facts,
                state,
                statement_index,
                binary.right,
                domains,
                plan,
                depth + 1,
            )?;
            let capacity = left_capacity.checked_add(right_capacity)?;
            (
                Value::Concatenation {
                    left,
                    right,
                    capacity,
                },
                capacity,
            )
        }
        _ => {
            let place = crate::flow::canonical_place_from_expression_in_state(
                program,
                state.symbol,
                statement_index as usize,
                expression,
            )?;
            let root = crate::flow::normalized_event_place_root(program, place.root);
            let parameter_position =
                program
                    .state_parameters(state)
                    .iter()
                    .position(|parameter| {
                        crate::flow::normalized_event_place_root(
                            program,
                            facts::PlaceRoot::Symbol(parameter.symbol),
                        ) == root
                    })?;
            let type_reference = crate::flow::canonical_place_type_reference(
                program,
                state.symbol,
                statement_index as usize,
                &place,
            )?;
            let checked_trees::CheckedByteSequenceCarrier::BoundedOwned { capacity } =
                byte_sequence_carrier(program, type_reference, &[])?
            else {
                return None;
            };
            let source_domains =
                crate::facts::field_domain::domain_constraint_symbols(program, type_reference);
            if !domains.iter().all(|domain| {
                source_domains.iter().any(|source| {
                    crate::facts::field_domain::declared_domain_implies(program, *source, *domain)
                })
            }) {
                return None;
            }
            let mut carrier_path = Vec::new();
            for segment in &place.segments {
                carrier_path.push(match segment {
                    facts::PlaceSegment::Field { symbol } => {
                        Segment::Field(terminal_field_identity(program, *symbol)?)
                    }
                    facts::PlaceSegment::FixedIndex { index } => {
                        Segment::FixedIndex(u64::try_from(*index).ok()?)
                    }
                    _ => return None,
                });
            }
            let Segment::Field(field_identity) = carrier_path.pop()? else {
                return None;
            };
            (
                Value::Field {
                    parameter_position: u32::try_from(parameter_position).ok()?,
                    carrier_path,
                    field_identity,
                    capacity,
                },
                capacity,
            )
        }
    };
    Some((plan.nodes.insert(value), capacity))
}

/// Whether the field's declared type passes through an exclusive `&mut`
/// reference. The byte-sequence carrier collapses `&'r [u8]` and `&'r mut
/// [u8]` to the same `BorrowedView`, so the access distinction is read here
/// while the Reference node still exists.
pub(super) fn field_view_is_mutable(
    program: &TypedTrees,
    mut type_reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    loop {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Constrained { base_type, .. } => {
                type_reference = *base_type;
            }
            TypeReferenceNode::Reference { access, .. } => {
                return *access == language_semantics::ReferenceAccess::Mutable;
            }
            _ => return false,
        }
    }
}
