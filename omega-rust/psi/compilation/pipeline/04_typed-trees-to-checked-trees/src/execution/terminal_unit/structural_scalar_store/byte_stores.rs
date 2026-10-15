//! Byte-sequence leaves: a whole borrowed byte view, an established mutable
//! bounded byte home, or a bounded-owned field below an exclusive root. One
//! indexed byte is a `ByteElement` store leaf and replaced live contents a
//! `ByteContents` leaf; the destination and the `u8` value are resolved by
//! the same owners as every other store.
use super::super::{
    CheckFacts, CheckedStructuralAccess, CheckedUnitEffectOperationPlan, ExpressionNode,
    Multiplicity, TypeReferenceNode,
};
use super::destination::{Destination, Root};
use super::value::{self, AssignmentSource};
use crate::execution::terminal_unit::control::LocalConstructionTrace;
use crate::execution::terminal_unit::types::{
    byte_sequence_carrier, structural_access_for_type_reference, terminal_field_identity,
};

/// `view[index] = byte` through a whole mutable borrowed view or owned home.
/// Replacing a byte observes the current length, so a borrowed root needs an
/// ordinary unrestricted mutable borrow rather than write-only authority.
/// Byte predicates stay attached to the owner and become store obligations;
/// borrowing its backing does not turn it into an unqualified byte view.
/// Entry, state and local owners retain their declared binding mutability;
/// where the home was established does not change the byte operation.
pub(super) fn byte_write(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    destination: &Destination<'_>,
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
    source: AssignmentSource,
    trace: &LocalConstructionTrace,
) -> Option<CheckedUnitEffectOperationPlan> {
    trace.phase("indexed store: byte backing");
    let index = destination.dynamic_index?;
    if !destination.place.segments.is_empty() {
        return None;
    }
    // Resolve declaration properties once, then apply the same byte-store
    // contract to every root. A local `let view: &mut [u8]` owns its descriptor,
    // not its borrowed contents: binding mutability controls replacement of
    // the descriptor, while the loan grants mutation of its bytes. Destination
    // resolution already retained that live loan rather than its initializer.
    let (declared_type, access, multiplicity, mutable_owner) = match destination.root {
        Root::Parameter { plan, parameter } => {
            if plan.binding_carrier.is_some() {
                return None;
            }
            (
                parameter.type_reference,
                plan.access,
                plan.multiplicity,
                parameter.is_mutable && !parameter.is_const,
            )
        }
        Root::Local { symbol } => {
            let (_, local) = program.state_local_declaration(state, symbol)?;
            (
                local.type_reference,
                structural_access_for_type_reference(program, local.type_reference)?,
                program.type_multiplicity(local.type_reference),
                local.is_mutable,
            )
        }
    };
    let carrier = byte_sequence_carrier(program, declared_type, &[])?;
    let writable = match access {
        CheckedStructuralAccess::MutableBorrow => true,
        CheckedStructuralAccess::Owned => {
            mutable_owner
                && matches!(
                    carrier,
                    checked_trees::CheckedByteSequenceCarrier::BoundedOwned { .. }
                )
        }
        _ => false,
    };
    if !writable || multiplicity != Multiplicity::Unrestricted {
        return None;
    }
    let domains = validation::field_domains::domain_constraint_symbols(program, declared_type);
    if !domains.into_iter().all(|symbol| {
        typed_trees::domain::domain_by_symbol(program, symbol).is_some_and(|domain| {
            domain.establishment_routes.is_empty()
                && domain.semantic_roles == Default::default()
                && typed_trees::byte_predicates::domain_byte_predicate(program, symbol).is_some()
        })
    }) {
        return None;
    }
    let indexed = indexed_byte_value(
        program,
        facts,
        machine,
        state,
        statement_index,
        index,
        assignment,
        source,
        trace,
    )?;
    Some(indexed.store(statement_index, destination.root.destination(), Vec::new()))
}

/// A whole borrowed owner retains its caller's capacity and qualifications.
pub(super) fn whole_store(
    program: &dyn ::validation::CheckingInput,
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
        || plan.binding_carrier.is_some()
    {
        return None;
    }
    let checked_trees::CheckedByteSequenceCarrier::BoundedOwned { capacity } =
        byte_sequence_carrier(program, parameter.type_reference, &[])?
    else {
        return None;
    };
    let domains =
        validation::field_domains::domain_constraint_symbols(program, parameter.type_reference);
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
    Some(contents_store(
        statement_index,
        checked_trees::CheckedStoreRoot::Parameter {
            position: plan.position,
        },
        Vec::new(),
        value,
    ))
}

/// A byte field leaf: `field[index] = byte` replaces one live byte of a
/// bounded-owned field or of a mutable borrowed view, and `field =
/// "literal"` replaces a bounded-owned field's live prefix (`capacity` is
/// that field's bound; a borrowed view has none and hosts no literal). A
/// domain on the field must be a plain predicate domain; a whole-literal
/// store must also satisfy it from the literal's own bytes.
pub(super) fn field_store(
    program: &dyn ::validation::CheckingInput,
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
    let mut field_path = carrier_path;
    field_path.push(checked_trees::CheckedUnitStructuralPathSegment::Field(
        terminal_field_identity(program, field.symbol)?,
    ));
    let plain_domain = |symbol, literal_grant: bool| {
        program.domain_definition(symbol).is_some_and(|domain| {
            domain.establishment_routes.is_empty()
                && domain.semantic_roles == Default::default()
                && (!literal_grant
                    || validation::field_domains::string_literal_expression_grants_domain(
                        program,
                        assignment.value,
                        symbol,
                    ))
        })
    };
    let domains =
        validation::field_domains::domain_constraint_symbols(program, field.type_reference);
    if let Some(index) = destination.dynamic_index {
        // Indexed byte mutation retains its separate parameter-only contract.
        destination.root.parameter_position()?;
        if !domains
            .into_iter()
            .all(|symbol| plain_domain(symbol, false))
        {
            return None;
        }
        let indexed = indexed_byte_value(
            program,
            facts,
            machine,
            state,
            statement_index,
            index,
            assignment,
            source,
            trace,
        )?;
        return Some(indexed.store(statement_index, store_root, field_path));
    }
    let capacity = capacity?;
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
        AssignmentSource::StructuralValue {
            binding_ordinal,
            result_type,
        } => {
            let source_capacity = validation::bounded_byte_buffer_capacity(program, result_type)?;
            let source_domains =
                validation::field_domains::domain_constraint_symbols(program, result_type);
            if program.type_multiplicity(result_type) != Multiplicity::Unrestricted
                || !validation::has_contents(
                    program,
                    result_type,
                    &[],
                    validation::ContentsRequirement::RepresentedOwned,
                )
                || !domains.iter().all(|domain| {
                    source_domains.iter().any(|source| {
                        validation::field_domains::declared_domain_implies(
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
    Some(contents_store(
        statement_index,
        store_root,
        field_path,
        value,
    ))
}

/// The store replacing a bounded-owned carrier's live contents: the whole
/// root (an empty path) or the byte field `path` ends at.
fn contents_store(
    statement_index: u32,
    root: checked_trees::CheckedStoreRoot,
    path: Vec<checked_trees::CheckedUnitStructuralPathSegment>,
    value: checked_trees::CheckedByteSequenceValuePlan,
) -> CheckedUnitEffectOperationPlan {
    CheckedUnitEffectOperationPlan::new_store(
        statement_index,
        root,
        path,
        checked_trees::CheckedStoreLeaf::ByteContents,
        checked_trees::CheckedStoreValue::Bytes(value),
    )
}

/// Preserve only the selected built-in byte law. Leaves are immutable literals
/// or readable bounded places; there is no delayed call evaluation in this
/// plan. A whole place observes its current live extent,
/// rather than padding the value to its carrier capacity.
pub(in crate::execution::terminal_unit) fn byte_value_plan(
    program: &dyn ::validation::CheckingInput,
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
                validation::field_domains::string_literal_expression_grants_domain(
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
                validation::field_domains::domain_is_concat_preserving(program, *domain)
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
            let place = validation::canonical_place_from_expression_in_state(
                program,
                state.symbol,
                statement_index as usize,
                expression,
            )?;
            let root = place.root;
            let parameter_position = program
                .state_parameters(state)
                .iter()
                .position(|parameter| (facts::PlaceRoot::Symbol(parameter.symbol)) == root);
            let type_reference = validation::canonical_place_type_reference(
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
                validation::field_domains::domain_constraint_symbols(program, type_reference);
            if !domains.iter().all(|domain| {
                source_domains.iter().any(|source| {
                    validation::field_domains::declared_domain_implies(program, *source, *domain)
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
                    let symbol = root.as_symbol()?;
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
                                | checked_trees::checked_borrowed_access_pair_pattern!()
                        )
                    )
                    || !source_domains.iter().all(|domain| {
                        typed_trees::byte_predicates::domain_byte_predicate(program, *domain)
                            .is_some()
                            && program
                                .domain_definition(*domain)
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
                let symbol = root.as_symbol()?;
                let (_, local) = program
                    .state_local_declaration(state, symbol)
                    .filter(|&(ordinal, _)| ordinal < statement_index as usize)?;
                if structural_access_for_type_reference(program, local.type_reference)
                    != Some(CheckedStructuralAccess::Owned)
                    || !validation::has_contents(
                        program,
                        local.type_reference,
                        &[],
                        validation::ContentsRequirement::RepresentedOwned,
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
    program: &dyn ::validation::CheckingInput,
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

/// The index, value and decisions an indexed byte store evaluates against one
/// destination root — shared by the whole-carrier and field-element lanes.
struct IndexedByte {
    index: checked_trees::CheckedScalarExpression,
    value: checked_trees::CheckedScalarStoreValue,
    decisions: checked_trees::CheckedIndexedWriteDecisions,
}

impl IndexedByte {
    /// The store of this byte into the element of the byte carrier `path`
    /// names below `root`.
    fn store(
        self,
        statement_index: u32,
        root: checked_trees::CheckedStoreRoot,
        path: Vec<checked_trees::CheckedUnitStructuralPathSegment>,
    ) -> CheckedUnitEffectOperationPlan {
        CheckedUnitEffectOperationPlan::new_store(
            statement_index,
            root,
            path,
            checked_trees::CheckedStoreLeaf::ByteElement {
                index: self.index,
                decisions: self.decisions,
            },
            checked_trees::CheckedStoreValue::assigned(self.value),
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn indexed_byte_value(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    statement_index: u32,
    index: typed_trees::expression::ExpressionHandle,
    assignment: &typed_trees::statement::TableAssignment,
    source: AssignmentSource,
    trace: &LocalConstructionTrace,
) -> Option<IndexedByte> {
    Some(IndexedByte {
        index: value::runtime_index(facts, state, statement_index, index)?,
        value: value::byte_value(
            program,
            facts,
            machine,
            state,
            statement_index,
            assignment,
            source,
            trace,
        )?,
        // Both byte leaves index the authored target itself.
        decisions: checked_trees::CheckedIndexedWriteDecisions {
            index_in_bounds: facts.scalar_index_is_proven(assignment.target),
            predicates_preserved: facts.byte_write_is_proven(assignment.target),
        },
    })
}
