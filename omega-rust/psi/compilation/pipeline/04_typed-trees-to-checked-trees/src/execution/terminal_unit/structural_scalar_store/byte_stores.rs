//! Byte-sequence leaves: a whole borrowed byte view, an established mutable
//! bounded byte local, or a bounded-owned field below an exclusive root. These
//! leaves have their own Terminal operations (`ByteSequenceWrite`,
//! `StructuralByteSequenceFieldStore`, `StructuralByteSequenceFieldByteStore`),
//! but the destination and the `u8` value are resolved by the same owners as
//! every other store.
use super::super::{
    CheckFacts, CheckedStructuralAccess, CheckedUnitEffectOperationPlan, ExpressionNode,
    Multiplicity, TypeReferenceNode, TypedTrees,
};
use super::destination::{Destination, Root};
use super::value::{self, AssignmentSource};
use crate::execution::terminal_unit::control::LocalConstructionTrace;
use crate::execution::terminal_unit::types::{
    byte_sequence_carrier, structural_access_for_type_reference, terminal_field_identity,
};

/// `view[index] = byte` through a whole mutable borrowed view or owned local.
/// Replacing a byte observes the current length, so a borrowed root needs an
/// ordinary unrestricted, unqualified mutable borrow rather than write-only
/// authority. An owned root retains its exact mutable local establishment.
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
                path: Vec::new(),
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
    match destination.root {
        Root::Parameter { plan, parameter } => {
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
        }
        Root::Local { symbol } => {
            // Destination resolution independently rejoined the earlier literal
            // establishment. Keep its declared carrier and mutation authority;
            // an immutable bounded owner or an unrelated view is not writable.
            let (_, local) = program.state_local_declaration(state, symbol)?;
            if !local.is_mutable
                || program.type_multiplicity(local.type_reference) != Multiplicity::Unrestricted
                || structural_access_for_type_reference(program, local.type_reference)
                    != Some(CheckedStructuralAccess::Owned)
                || !matches!(
                    byte_sequence_carrier(program, local.type_reference, &[]),
                    Some(checked_trees::CheckedByteSequenceCarrier::BoundedOwned { .. })
                )
            {
                return None;
            }
        }
    }
    Some(CheckedUnitEffectOperationPlan::ByteSequenceWrite(
        checked_trees::CheckedByteSequenceWritePlan {
            statement_index,
            destination: destination.root.destination(),
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
    program.primitive_type_reference(element_view_element_reference(program, state, destination)?)
}

/// Exact mutable element-view root, independent of its selected scalar leaf.
/// Byte slices retain their separate byte-sequence storage contract.
pub(super) fn element_view_element_reference(
    program: &TypedTrees,
    state: &typed_trees::state::State,
    destination: &Destination<'_>,
) -> Option<typed_trees::types::TypeReferenceHandle> {
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
    (program.primitive_type_reference(*element_type) != Some(typed_trees::types::PrimitiveType::U8))
        .then_some(*element_type)
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
        &facts.operators,
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
            root: checked_trees::CheckedStoreRoot::Parameter {
                position: plan.position,
            },
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
    // Destination resolution already authenticated the local's mutable loan.
    // Retain that identity: a returned reference is not a new owned record or
    // a synthetic formal parameter.
    let store_root = destination.root.destination();
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
        // Indexed byte mutation retains its separate parameter-only contract.
        destination.root.parameter_position()?;
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
                    destination: store_root,
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
    let Some(capacity) = capacity else {
        return None;
    };
    if !domains.iter().all(|symbol| plain_domain(*symbol, false)) {
        return None;
    }
    let mut value = checked_trees::CheckedByteSequenceValuePlan::default();
    let root = match source {
        AssignmentSource::Authored => {
            byte_value_plan(
                program,
                &facts.operators,
                state,
                statement_index,
                assignment.value,
                &domains,
                &mut value,
                0,
            )?
            .0
        }
        AssignmentSource::StructuralCallResult {
            binding_ordinal,
            result_type,
        } => {
            let source_capacity = validation::bounded_byte_buffer_capacity(program, result_type)?;
            let source_domains =
                crate::facts::field_domain::domain_constraint_symbols(program, result_type);
            if program.type_multiplicity(result_type) != Multiplicity::Unrestricted
                || !validation::has_plain_owned_contents_with_represented_constraints(
                    program,
                    result_type,
                )
                || !matches!(
                    program.expression_table.expression(assignment.value),
                    ExpressionNode::Call(_)
                )
                || !domains.iter().all(|domain| {
                    source_domains.iter().any(|source| {
                        crate::facts::field_domain::declared_domain_implies(
                            program, *source, *domain,
                        )
                    })
                })
            {
                return None;
            }
            value
                .nodes
                .insert(checked_trees::CheckedByteSequenceValue::CallResult {
                    state: state.symbol,
                    statement_index,
                    binding_ordinal,
                    capacity: u64::try_from(source_capacity).ok()?,
                })
        }
        AssignmentSource::CallResult { .. } => return None,
    };

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
            root: store_root,
            destination: checked_trees::CheckedByteSequenceStoreDestination::Field {
                carrier_path,
                field_identity: terminal_field_identity(program, field.symbol)?,
            },
            value,
        },
    ))
}

/// Preserve only the selected built-in byte law. Leaves are immutable literals
/// or readable bounded places; there is no delayed call evaluation in this
/// plan. A whole place observes its current live extent,
/// rather than padding the value to its carrier capacity.
pub(in crate::execution::terminal_unit) fn byte_value_plan(
    program: &TypedTrees,
    operators: &checked_trees::CheckedOperatorFacts,
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
                && crate::values::operator_is_builtin(operators, expression) =>
        {
            if !domains.iter().all(|domain| {
                crate::facts::field_domain::domain_is_concat_preserving(program, *domain)
            }) {
                return None;
            }
            let (left, left_capacity) = byte_value_plan(
                program,
                operators,
                state,
                statement_index,
                binary.left,
                domains,
                plan,
                depth + 1,
            )?;
            let (right, right_capacity) = byte_value_plan(
                program,
                operators,
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
            let parameter_position = program
                .state_parameters(state)
                .iter()
                .position(|parameter| {
                    crate::flow::normalized_event_place_root(
                        program,
                        facts::PlaceRoot::Symbol(parameter.symbol),
                    ) == root
                });
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
            if place.segments.is_empty() {
                let source_root = if let Some(position) = parameter_position {
                    checked_trees::CheckedStoreRoot::Parameter {
                        position: u32::try_from(position).ok()?,
                    }
                } else {
                    let facts::PlaceRoot::Symbol(symbol) = root else {
                        return None;
                    };
                    let (_, local) = program
                        .state_local_declaration(state, symbol)
                        .filter(|&(ordinal, _)| ordinal < statement_index as usize)?;
                    if structural_access_for_type_reference(program, local.type_reference)
                        != Some(CheckedStructuralAccess::Owned)
                    {
                        return None;
                    }
                    checked_trees::CheckedStoreRoot::Local { symbol }
                };
                if program.type_multiplicity(type_reference) != Multiplicity::Unrestricted
                    || !matches!(
                        structural_access_for_type_reference(program, type_reference),
                        Some(
                            CheckedStructuralAccess::Owned
                                | CheckedStructuralAccess::SharedBorrow
                                | CheckedStructuralAccess::MutableBorrow
                        )
                    )
                    || !source_domains.iter().all(|domain| {
                        typed_trees::byte_predicates::domain_byte_predicate(program, *domain)
                            .is_some()
                            && program
                                .domain_definitions()
                                .iter()
                                .find(|definition| definition.symbol == *domain)
                                .is_some_and(|definition| {
                                    definition.establishment_routes.is_empty()
                                        && definition.semantic_roles == Default::default()
                                })
                    })
                {
                    return None;
                }
                return Some((
                    plan.nodes.insert(Value::Whole {
                        root: source_root,
                        capacity,
                    }),
                    capacity,
                ));
            }
            // A field read observes the live home after earlier operations,
            // including calls that mutated a local. Keep that storage identity
            // rather than substituting the local's original value plan.
            let source_root = if let Some(position) = parameter_position {
                checked_trees::CheckedStoreRoot::Parameter {
                    position: u32::try_from(position).ok()?,
                }
            } else {
                let facts::PlaceRoot::Symbol(symbol) = root else {
                    return None;
                };
                let (_, local) = program
                    .state_local_declaration(state, symbol)
                    .filter(|&(ordinal, _)| ordinal < statement_index as usize)?;
                if structural_access_for_type_reference(program, local.type_reference)
                    != Some(CheckedStructuralAccess::Owned)
                    || !validation::has_plain_owned_contents_with_represented_constraints(
                        program,
                        local.type_reference,
                    )
                {
                    return None;
                }
                checked_trees::CheckedStoreRoot::Local { symbol }
            };
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
                    root: source_root,
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
