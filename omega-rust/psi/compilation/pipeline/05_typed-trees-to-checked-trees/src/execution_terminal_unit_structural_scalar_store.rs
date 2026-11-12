/*
    Projected store planning for parameter and local storage.

    One authored assignment `place = value` is one composition. The
    destination (`destination.rs`) is resolved from the target alone: its root
    and that root's write authority, the checked carrier path, and the leaf the
    path ends at. The value (`value.rs`) is resolved from the right-hand side
    alone: the already-checked scalar source at the value's own coordinate --
    a computation root, a bound pure expression, or the result of the call the
    statement performs. Every assignment publishes one
    `CheckedUnitEffectOperationPlan::Store` over that destination and value;
    the backing representation and the leaf's declared type decide which
    `CheckedStoreLeaf` names the replaced place:

    - a primitive field is a `ScalarField` leaf carrying its receiving range;
    - a scalar leaf below an element-view root is a `ViewElement` leaf,
      independent of whether that leaf is a field, array element, or whole element;
    - a primitive-array literal completes as one owned array before whole-place
      replacement in `control::statement_sequence`;
    - a sum field is a `SumField` leaf receiving a whole owned parameter of its
      type or a case literal;
    - a bounded-owned byte field or a whole borrowed byte view is a
      `ByteElement` or `ByteContents` leaf (`byte_stores.rs`);
    - a shared view field is the `ByteDescriptor` leaf of `descriptor_store.rs`;
      whole local descriptors use ordinary value establishment and rebinding.

    Whole record assignments stay with ordinary structural construction and
    replacement in `control::statement_sequence`: operands complete before
    destination mutation, regardless of whether the destination is a root,
    field, or indexed element. This planner does not decompose constructors.

    Mutation analysis owns write-frame completeness and fact invalidation.
    An opaque call summary is not an empty frame, but neither prevents emitting
    independently checked assignments. Replaying the enclosing state's frame
    here rejected a valid store merely because a later foreign call was opaque.
    What remains refused is a custody or proof condition, not an arrangement:
    unsupported parameter custody, permission crossings, and fields
    whose declared domain the stored value carries no evidence for.
*/

use super::calls::projected_argument_path;
use super::control::scalar_arrays::assignment_type;
use super::types::ShapeCollector;
use super::types::base_type_identity;
use super::{
    CheckFacts, CheckedScalarExpressionRole, CheckedStructuralAccess,
    CheckedStructuralScalarParameterPlan, CheckedUnitEffectOperationPlan,
    CheckedUnitStructuralParameterPlan, CheckedUnitStructuralPathSegment, DataMember,
    DataShapeKind, ExpressionNode, Multiplicity, PrimitiveType, StatementNode, TypeReferenceNode,
};
use crate::execution::terminal_unit::calls::ProjectedArgumentPath;
use crate::execution::terminal_unit::control::LocalConstructionTrace;
use crate::execution::terminal_unit::types::retain_scalar_field;
use crate::execution::terminal_unit::types::{byte_sequence_carrier, terminal_field_identity};
use crate::published::places::place_at;
use crate::values::occupies_scalar_position;
use crate::values::resolve_structural_parameter_path;
use destination::{Destination, Root, StoreRoots, plain_record};
use symbols::SymbolHandle;
pub(super) use value::AssignmentSource;

pub(super) use primitive::{primitive_local_before, scalar_custody_is_exact};
pub(super) use selectors::TargetSelectors;

mod byte_stores {
    /*
        Byte-sequence leaves: a whole borrowed byte view, an established mutable
        bounded byte home, or a bounded-owned field below an exclusive root. One
        indexed byte is a `ByteElement` store leaf and replaced live contents a
        `ByteContents` leaf; the destination and the `u8` value are resolved by
        the same owners as every other store.
    */
    use super::super::{
        CheckedStructuralAccess as StructuralAccess,
        CheckedUnitEffectOperationPlan as OperationPlan, ExpressionNode, Multiplicity,
    };
    use super::destination::Root;
    use super::value::{self, AssignmentSource};
    use crate::checks::current_scalar_values::byte_length_bound;
    use crate::checks::reference_access;
    use crate::execution::terminal_unit::types::{
        byte_sequence_carrier, structural_access_for_type_reference, terminal_field_identity,
    };
    use crate::published::places::place_at;
    use crate::values::operator_is_builtin;

    /*
        `view[index] = byte` through a whole mutable borrowed view or owned home.
        Replacing a byte observes the current length, so a borrowed root needs an
        ordinary unrestricted mutable borrow rather than write-only authority.
        Byte predicates stay attached to the owner and become store obligations;
        borrowing its backing does not turn it into an unqualified byte view.
        Entry, state and local owners retain their declared binding mutability;
        where the home was established does not change the byte operation.
    */
    pub(super) fn byte_write(site: &super::Site<'_, '_, '_>) -> Option<OperationPlan> {
        let super::Occurrence {
            program,
            state,
            statement_index,
            trace,
            ..
        } = site.occurrence;
        let destination = site.destination;
        trace.phase("indexed store: byte backing");
        let index = destination.dynamic_index?;
        if !destination.place.segments.is_empty() {
            return None;
        }
        let (declared_type, access, multiplicity, mutable_owner) = match destination.root {
            Root::Parameter { plan, parameter } => (
                parameter.type_reference,
                plan.access,
                plan.multiplicity,
                parameter.is_mutable && !parameter.is_const,
            ),
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
            StructuralAccess::MutableBorrow => true,
            StructuralAccess::Owned => {
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
                    && typed_trees::byte_predicates::domain_byte_predicate(program, symbol)
                        .is_some()
            })
        }) {
            return None;
        }
        let indexed = indexed_byte_value(&site.occurrence, index)?;
        Some(indexed.store(statement_index, destination.root.destination(), Vec::new()))
    }

    /* A whole borrowed owner retains its caller's capacity and qualifications. */
    pub(super) fn whole_store(site: &super::Site<'_, '_, '_>) -> Option<OperationPlan> {
        let super::Occurrence {
            program,
            facts,
            state,
            statement_index,
            assignment,
            source,
            ..
        } = site.occurrence;
        let destination = site.destination;
        if !destination.place.segments.is_empty()
            || destination.dynamic_index.is_some()
            || !matches!(source, AssignmentSource::Authored)
        {
            return None;
        }
        let reference = match destination.root {
            Root::Parameter { plan, parameter } => {
                if !matches!(
                    plan.access,
                    StructuralAccess::MutableBorrow | StructuralAccess::WriteOnlyBorrow
                ) || plan.multiplicity != Multiplicity::Unrestricted
                {
                    return None;
                }
                parameter.type_reference
            }
            Root::Local { symbol } => {
                let (_, local) = program.state_local_declaration(state, symbol)?;
                if !local.is_mutable
                    || program.type_multiplicity(local.type_reference) != Multiplicity::Unrestricted
                    || structural_access_for_type_reference(program, local.type_reference)
                        != Some(StructuralAccess::Owned)
                {
                    return None;
                }
                local.type_reference
            }
        };
        let checked_trees::CheckedByteSequenceCarrier::BoundedOwned { .. } =
            byte_sequence_carrier(program, reference, &[])?
        else {
            return None;
        };
        let domains = validation::field_domains::domain_constraint_symbols(program, reference);
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
        Some(contents_store(
            statement_index,
            destination.root.destination(),
            Vec::new(),
            value,
        ))
    }

    /*
        A byte field leaf: `field[index] = byte` replaces one live byte of a
        bounded-owned field or of a mutable borrowed view, and `field =
        "literal"` replaces a bounded-owned field's live prefix (`capacity` is
        that field's bound; a borrowed view has none and hosts no literal). A
        domain on the field must be a plain predicate domain; a whole-literal
        store must also satisfy it from the literal's own bytes.
    */
    pub(super) fn field_store(
        site: &super::Site<'_, '_, '_>,
        carrier_path: Vec<checked_trees::CheckedUnitStructuralPathSegment>,
        field: &typed_trees::data::DataField,
        capacity: Option<u64>,
    ) -> Option<OperationPlan> {
        let super::Occurrence {
            program,
            facts,
            state,
            statement_index,
            assignment,
            source,
            ..
        } = site.occurrence;
        let destination = site.destination;
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
            destination.root.parameter_position()?;
            if !domains
                .into_iter()
                .all(|symbol| plain_domain(symbol, false))
            {
                return None;
            }
            let indexed = indexed_byte_value(&site.occurrence, index)?;
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
                let source_capacity =
                    validation::bounded_byte_buffer_capacity(program, result_type)?;
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

    /*
        The store replacing a bounded-owned carrier's live contents: the whole
        root (an empty path) or the byte field `path` ends at.
    */
    fn contents_store(
        statement_index: u32,
        root: checked_trees::CheckedStoreRoot,
        path: Vec<checked_trees::CheckedUnitStructuralPathSegment>,
        value: checked_trees::CheckedByteSequenceValuePlan,
    ) -> OperationPlan {
        OperationPlan::new_store(
            statement_index,
            root,
            path,
            checked_trees::CheckedStoreLeaf::ByteContents,
            checked_trees::CheckedStoreValue::Bytes(value),
        )
    }

    /*
        Preserve only the selected built-in byte law. Leaves are immutable literals
        or readable bounded places; there is no delayed call evaluation in this
        plan. A whole place observes its current live extent,
        rather than padding the value to its carrier capacity.
    */
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
                    && operator_is_builtin(operators, expression) =>
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
                let place = place_at(program, state.symbol, statement_index as usize, expression)?;
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
                let capacity = match byte_sequence_carrier(program, type_reference, &[])? {
                    checked_trees::CheckedByteSequenceCarrier::BoundedOwned { capacity }
                    | checked_trees::CheckedByteSequenceCarrier::BoundedView { capacity, .. } => {
                        capacity
                    }
                    checked_trees::CheckedByteSequenceCarrier::BorrowedView { .. } => {
                        byte_length_bound(program, state, statement_index as usize, expression)?
                    }
                };
                let source_domains =
                    validation::field_domains::domain_constraint_symbols(program, type_reference);
                if !domains.iter().all(|domain| {
                    source_domains.iter().any(|source| {
                        validation::field_domains::domain_membership_implies(
                            program, *source, *domain,
                        )
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
                        if !matches!(
                            structural_access_for_type_reference(program, local.type_reference),
                            Some(
                                StructuralAccess::Owned
                                    | checked_trees::checked_borrowed_access_pair_pattern!()
                            )
                        ) {
                            return None;
                        }
                        checked_trees::CheckedStoreRoot::Local { symbol }
                    };
                    if program.type_multiplicity(type_reference) != Multiplicity::Unrestricted
                        || !matches!(
                            structural_access_for_type_reference(program, type_reference),
                            Some(
                                StructuralAccess::Owned
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
                        != Some(StructuralAccess::Owned)
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

    /*
        Whether the field's declared type passes through an exclusive `&mut`
        reference. The byte-sequence carrier collapses `&'r [u8]` and `&'r mut
        [u8]` to the same `BorrowedView`, so the access distinction is read here
        while the Reference node still exists.
    */
    pub(super) fn field_view_is_mutable(
        program: &dyn ::validation::CheckingInput,
        type_reference: typed_trees::types::TypeReferenceHandle,
    ) -> bool {
        reference_access(program, type_reference)
            .is_some_and(|access| access == language_semantics::ReferenceAccess::Mutable)
    }

    /*
        The index, value and decisions an indexed byte store evaluates against one
        destination root — shared by the whole-carrier and field-element lanes.
    */
    struct IndexedByte {
        index: checked_trees::CheckedScalarExpression,
        value: checked_trees::CheckedScalarStoreValue,
        decisions: checked_trees::CheckedIndexedWriteDecisions,
    }

    impl IndexedByte {
        /*
            The store of this byte into the element of the byte carrier `path`
            names below `root`.
        */
        fn store(
            self,
            statement_index: u32,
            root: checked_trees::CheckedStoreRoot,
            path: Vec<checked_trees::CheckedUnitStructuralPathSegment>,
        ) -> OperationPlan {
            OperationPlan::new_store(
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
        occurrence: &super::Occurrence<'_, '_>,
        index: typed_trees::expression::ExpressionHandle,
    ) -> Option<IndexedByte> {
        let super::Occurrence {
            facts,
            state,
            statement_index,
            assignment,
            ..
        } = *occurrence;
        Some(IndexedByte {
            index: value::runtime_index(facts, state, statement_index, index)?,
            value: value::byte_value(occurrence)?,
            decisions: checked_trees::CheckedIndexedWriteDecisions {
                index_in_bounds: facts.scalar_index_is_proven(assignment.target),
                predicates_preserved: facts.byte_write_is_proven(assignment.target),
            },
        })
    }
}
pub(super) use byte_stores::byte_value_plan;
mod descriptor_store {
    /*
        Shared view descriptor replacement keeps the RHS descriptor's actual origin.

        A record field retains its declared slot and byte-predicate obligation.
        Whole local descriptors use ordinary value establishment and rebinding.

        The common byte operand planner supplies the same completed operand used
        by comparisons and predicate scans. This operation retains the exact source
        expression; source loans and Terminal replay prove backing lifetime, while
        a separate current-value obligation proves the slot's byte predicates.
    */
    use super::super::CheckFacts;
    use super::destination::StoreRoots;
    use super::value::AssignmentSource;
    use crate::execution::terminal_unit::types::terminal_field_identity;

    /*
        The descriptor slot one store replaces: the carrier path from the root,
        the slot's declared type, and the field naming it.
    */
    pub(super) struct DescriptorSlot<'a> {
        pub(super) path: Vec<checked_trees::CheckedUnitStructuralPathSegment>,
        pub(super) type_reference: typed_trees::types::TypeReferenceHandle,
        pub(super) field: &'a typed_trees::data::DataField,
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn store(
        site: &super::Site<'_, '_, '_>,
        roots: StoreRoots<'_>,
        slot: DescriptorSlot<'_>,
    ) -> Option<checked_trees::CheckedUnitEffectOperationPlan> {
        let super::Occurrence {
            program,
            facts,
            machine,
            state,
            statement_index,
            assignment,
            source,
            trace,
        } = site.occurrence;
        let destination = site.destination;
        trace.phase("structural field store: borrowed view field: descriptor source");
        if destination.dynamic_index.is_some() {
            return None;
        }
        descriptor_store(
            program,
            facts,
            machine,
            state,
            &|position| {
                roots
                    .structural
                    .iter()
                    .position(|parameter| parameter.position as usize == position)
                    .and_then(|dense| u32::try_from(dense).ok())
            },
            destination.root.destination(),
            slot,
            statement_index,
            assignment.value,
            source,
        )
    }

    /*
        Replace one descriptor slot with the completed descriptor `expression`
        denotes. The root's write authority was resolved by its caller;
        `dense_parameter` names the lane's dense structural argument
        for an authored parameter position the source reads.
    */
    #[allow(clippy::too_many_arguments)]
    pub(super) fn descriptor_store(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        dense_parameter: &dyn Fn(usize) -> Option<u32>,
        root: checked_trees::CheckedStoreRoot,
        slot: DescriptorSlot<'_>,
        statement_index: u32,
        expression: typed_trees::expression::ExpressionHandle,
        source: AssignmentSource,
    ) -> Option<checked_trees::CheckedUnitEffectOperationPlan> {
        for (_, predicate) in typed_trees::byte_predicates::type_reference_domain_predicates(
            program,
            slot.type_reference,
        ) {
            predicate?;
        }
        let target = if let AssignmentSource::StructuralValue { result_type, .. } = source {
            result_type
        } else if matches!(
            program.expression_table.expression(expression),
            typed_trees::expression::ExpressionNode::String(_)
        ) {
            slot.type_reference
        } else {
            validation::expression_result_type_reference(program, machine, state, expression)?
        };
        let source = match source {
            AssignmentSource::StructuralValue {
                binding_ordinal,
                result_type,
            } => {
                use crate::execution::terminal_unit::types;
                if types::structural_access_for_type_reference(program, result_type)
                    != Some(checked_trees::CheckedStructuralAccess::SharedBorrow)
                    || !matches!(
                        types::byte_sequence_carrier(program, result_type, &[]),
                        Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView { .. })
                    )
                {
                    return None;
                }
                checked_trees::CheckedUnitStructuralArgumentPlan {
                    projection_evaluations: Default::default(),
                    source:
                        checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralResult {
                            binding_ordinal,
                        },
                    path: Vec::new(),
                    type_identity: types::byte_sequence_type_identity(
                        program,
                        result_type,
                        &[],
                        &[],
                    )?,
                    access: checked_trees::CheckedStructuralAccess::SharedBorrow,
                }
            }
            AssignmentSource::CallResult { .. } => return None,
            AssignmentSource::Authored => crate::execution::terminal_unit::byte_operands::plan(
                program,
                &facts.operators,
                &facts.values.scalar_expressions,
                &facts.values.scalar_computations,
                &facts.borrow,
                machine,
                state,
                statement_index as usize,
                expression,
                target,
                false,
                dense_parameter,
                &|selector| {
                    let checked_trees::CheckedScalarExpression::Local { position, .. } =
                        super::value::runtime_index(facts, state, statement_index, selector)?
                    else {
                        return None;
                    };
                    Some(checked_trees::CheckedRuntimeIndex::Local {
                        position: u32::try_from(position).ok()?,
                    })
                },
            )?,
        };
        let mut path = slot.path;
        path.push(checked_trees::CheckedUnitStructuralPathSegment::Field(
            terminal_field_identity(program, slot.field.symbol)?,
        ));
        let mut value = checked_trees::CheckedByteSequenceValuePlan::default();
        value.root = value
            .nodes
            .insert(checked_trees::CheckedByteSequenceValue::Descriptor { expression, source });
        Some(checked_trees::CheckedUnitEffectOperationPlan::new_store(
            statement_index,
            root,
            path,
            checked_trees::CheckedStoreLeaf::ByteDescriptor,
            checked_trees::CheckedStoreValue::Bytes(value),
        ))
    }
}
mod destination {
    /*
        Resolve where one authored assignment writes: the storage root it names,
        the root's write authority, and the checked carrier path from that root.

        The destination is resolved without looking at the right-hand side. Each
        store kind in `super` then reads the resolved place and decides whether
        its own leaf and value compose with it.
    */
    use super::super::{
        CheckFacts, CheckedStructuralAccess as StructuralAccess,
        CheckedStructuralScalarParameterPlan, CheckedUnitStructuralParameterPlan,
        CheckedUnitStructuralPathSegment as PathSegment, DataMember, DataShapeKind, ExpressionNode,
        StatementNode, SymbolHandle, TypeReferenceNode,
    };
    use crate::execution::terminal_unit::control::LocalConstructionTrace;
    use crate::execution::terminal_unit::control::statement_sequence::bounded_byte_values;
    use crate::execution::terminal_unit::types::terminal_field_identity;
    use crate::flow::expressions;
    use crate::published::places::place_at;

    /*
        The storage roots retained for source-validated stores: owned or exclusive
        borrowed structural parameters, established locals, and the `mut` scalar
        formals the body owns storage for.
    */
    #[derive(Clone, Copy)]
    pub(super) struct StoreRoots<'a> {
        pub(super) structural: &'a [CheckedUnitStructuralParameterPlan],
        pub(super) scalar: &'a [CheckedStructuralScalarParameterPlan],
        pub(super) storage: &'a [checked_trees::CheckedUnitParameterStoragePlan],
    }

    /* The resolved root of one destination place. */
    pub(super) enum Root<'a> {
        Parameter {
            plan: &'a CheckedUnitStructuralParameterPlan,
            parameter: &'a typed_trees::signature::StateParameter,
        },
        Local {
            symbol: SymbolHandle,
        },
    }

    impl Root<'_> {
        pub(super) fn destination(&self) -> checked_trees::CheckedStoreRoot {
            match self {
                Self::Parameter { plan, .. } => checked_trees::CheckedStoreRoot::Parameter {
                    position: plan.position,
                },
                Self::Local { symbol } => {
                    checked_trees::CheckedStoreRoot::Local { symbol: *symbol }
                }
            }
        }

        pub(super) fn parameter_position(&self) -> Option<u32> {
            match self {
                Self::Parameter { plan, .. } => Some(plan.position),
                Self::Local { .. } => None,
            }
        }
    }

    /*
        One authored destination: a canonical place below a resolved root. A
        trailing selector is split off as `dynamic_index` for the byte stores,
        whose live-length operations take it as an operand; every other runtime
        selector of the target is a `RuntimeIndex` carrier segment named by
        `selectors`.
    */
    pub(super) struct Destination<'a> {
        pub(super) root: Root<'a>,
        pub(super) place: validation::CanonicalPlace,
        pub(super) dynamic_index: Option<typed_trees::expression::ExpressionHandle>,
        /*
            The declared referee primitive of a recast loan local the target
            names (`let view: &mut u32 = &mut self.buf[k] as &mut u32`): the
            carrier the authored store writes at, distinct from the rejoined
            leaf's own primitive.
        */
        pub(super) recast_view: Option<typed_trees::types::PrimitiveType>,
        /*
            The exact exclusive reference local's declared type, or zero for an
            owned/parameter root. Keep its shell: the referent alone cannot decide
            whether storage is a pointer slot or a byte/element descriptor.
        */
        pub(super) loan_type: typed_trees::types::TypeReferenceHandle,
        /*
            The target's admitted element selectors, when every indexed step is
            an element of a declared fixed array.
        */
        selectors: Option<super::selectors::TargetSelectors>,
        /*
            Declared type of the root's referent (the borrowed record, or the
            local's own type) and its record owner when the root is a record.
        */
        pub(super) root_type: typed_trees::types::TypeReferenceHandle,
        root_owner: Option<&'a typed_trees::data::DataDefinition>,
    }

    /* The checked carrier path to a record, and that record's declaration. */
    pub(super) struct Carrier<'a> {
        pub(super) path: Vec<PathSegment>,
        pub(super) owner: &'a typed_trees::data::DataDefinition,
        selected_case: Option<&'a typed_trees::data::DataVariant>,
    }

    pub(super) struct StorageProjection<'a> {
        pub(super) path: Vec<PathSegment>,
        /*
            Position of the last element-view descriptor crossing. Earlier
            selectors belong to the full structural operand locating this view;
            later selectors locate the scalar leaf inside its element.
        */
        pub(super) element_view_index: Option<usize>,
        pub(super) type_reference: typed_trees::types::TypeReferenceHandle,
        /*
            A declared scalar field keeps its field-store range obligation. An
            array element or dereferenced primitive instead names primitive storage.
        */
        pub(super) ends_at_field: bool,
        owner: Option<&'a typed_trees::data::DataDefinition>,
        selected_case: Option<&'a typed_trees::data::DataVariant>,
    }

    /* Resolve the authored target, its root and the root's write authority. */
    pub(super) fn resolve<'a>(
        program: &'a dyn ::validation::CheckingInput,
        facts: &'a CheckFacts,
        machine: &'a typed_trees::machine::Machine,
        state: &'a typed_trees::state::State,
        roots: StoreRoots<'a>,
        statement_index: u32,
        assignment: &typed_trees::statement::TableAssignment,
        trace: &LocalConstructionTrace,
    ) -> Option<Destination<'a>> {
        trace.phase("structural field store: target place");
        let ordinal = usize::try_from(statement_index).ok()?;
        let (target, dynamic_index) = match program.expression_table.expression(assignment.target) {
            ExpressionNode::Indexed(indexed) => {
                if !validation::place_has_builtin_coordinates(
                    program,
                    machine,
                    Some(state),
                    assignment.target,
                ) || matches!(
                    program.expression_table.expression(indexed.index),
                    ExpressionNode::Range(_)
                ) {
                    return None;
                }
                (indexed.collection, Some(indexed.index))
            }
            _ => (assignment.target, None),
        };
        let place = place_at(program, state.symbol, ordinal, target)?;
        let materialized = |root: facts::PlaceRoot| {
            let Some(symbol) = root.as_symbol() else {
                return false;
            };
            let Some((declared, local)) = program.state_local_declaration(state, symbol) else {
                return false;
            };
            let Ok(declared_index) = u32::try_from(declared) else {
                return false;
            };
            facts
                .values
                .structural_values
                .root_for_expression(state.symbol, declared_index, local.initial_value)
                .is_some()
                || crate::execution::terminal_unit::control::reference_captures::plan(
                    program, facts, machine, state, declared, local,
                )
                .is_some()
        };
        let aliases = super::super::receiver_aliases::aliases(program, facts, machine, state)
            .unwrap_or_default();
        let place = super::super::receiver_aliases::resolve(&aliases, &place, materialized)
            .unwrap_or(place);
        let (place, recast_view) = if materialized(place.root) {
            (place, None)
        } else {
            rejoin_loan_local(program, machine, state, ordinal, place)?
        };
        let root_symbol = place.root.as_symbol()?;
        let selectors = super::selectors::TargetSelectors::resolve(
            program,
            facts,
            machine,
            state,
            statement_index,
            assignment.target,
        );
        trace.phase("structural field store: destination parameter");
        let StoreRoots {
            structural,
            storage,
            ..
        } = roots;
        let source_parameters = program.state_parameters(state);
        let mut destinations = structural.iter().filter_map(|plan| {
            let parameter = source_parameters.get(plan.position as usize)?;
            (parameter.symbol == root_symbol).then_some((plan, parameter))
        });
        let Some((plan, parameter)) = destinations.next() else {
            if storage.iter().any(|row| row.symbol == root_symbol) {
                let formal = source_parameters
                    .iter()
                    .find(|parameter| parameter.symbol == root_symbol)?;
                return (dynamic_index.is_none() && place.segments.is_empty()).then(|| {
                    local_destination(
                        program,
                        root_symbol,
                        formal.type_reference,
                        place,
                        dynamic_index,
                        selectors,
                    )
                });
            }
            return exclusive_reference_local_root(
                program,
                state,
                root_symbol,
                ordinal,
                dynamic_index,
                place.clone(),
                selectors.clone(),
            )
            .or_else(|| {
                owned_local_root(
                    program,
                    &facts.operators,
                    state,
                    root_symbol,
                    ordinal,
                    dynamic_index,
                    place,
                    selectors,
                )
            });
        };
        if destinations.next().is_some() {
            return None;
        }
        let root_type = match plan.access {
            StructuralAccess::Owned => parameter.type_reference,
            _ => validation::reference_shell(program, parameter.type_reference)?.referee,
        };
        let root_owner =
            validation::field_domains::data_definition_for_field_type(program, root_type);
        Some(Destination {
            root: Root::Parameter { plan, parameter },
            place,
            dynamic_index,
            recast_view,
            loan_type: Default::default(),
            selectors,
            root_type,
            root_owner,
        })
    }

    /*
        A store rooted at a declared local: the unique `LocalData` symbol names
        the root and its declared type fixes the owner and projection base.
    */
    fn local_destination<'a>(
        program: &'a dyn ::validation::CheckingInput,
        root_symbol: SymbolHandle,
        root_type: typed_trees::types::TypeReferenceHandle,
        place: validation::CanonicalPlace,
        dynamic_index: Option<typed_trees::expression::ExpressionHandle>,
        selectors: Option<super::selectors::TargetSelectors>,
    ) -> Destination<'a> {
        Destination {
            root: Root::Local {
                symbol: root_symbol,
            },
            place,
            dynamic_index,
            recast_view: None,
            loan_type: Default::default(),
            selectors,
            root_type,
            root_owner: validation::field_domains::data_definition_for_field_type(
                program, root_type,
            ),
        }
    }

    /*
        An owned local store root under the parameter lane: a unique `LocalData`
        declaration earlier in the state that establishes its own storage.
        Unlike the scalar-graph lane's record local this root needs no
        disposition row — the attached lane's emission writes through the live
        home place directly rather than rebuilding the local's scalar ledger.
    */
    fn owned_local_root<'a>(
        program: &'a dyn ::validation::CheckingInput,
        operators: &'a checked_trees::CheckedOperatorFacts,
        state: &'a typed_trees::state::State,
        root_symbol: SymbolHandle,
        ordinal: usize,
        dynamic_index: Option<typed_trees::expression::ExpressionHandle>,
        place: validation::CanonicalPlace,
        selectors: Option<super::selectors::TargetSelectors>,
    ) -> Option<Destination<'a>> {
        let (local_ordinal, local) = program.state_local_declaration(state, root_symbol)?;
        let bounded_byte_local = local.is_mutable
            && place.segments.is_empty()
            && bounded_byte_values::value_plan(
                program,
                operators,
                state,
                u32::try_from(local_ordinal).ok()?,
                local,
            )
            .is_some();
        let declared_type = validation::peel_declared_constraints(program, local.type_reference);
        let declared_home = matches!(
            program.type_reference_table.type_reference(declared_type),
            TypeReferenceNode::Named { .. }
                | TypeReferenceNode::Generic { .. }
                | TypeReferenceNode::FixedArray { .. }
        );
        let primitive_local =
            super::primitive::primitive_local_before(program, state, ordinal, root_symbol)
                .is_some();
        if local_ordinal >= ordinal
            || (dynamic_index.is_some() && selectors.is_none() && !bounded_byte_local)
            || !(bounded_byte_local || primitive_local || declared_home)
            || validation::reference_shell(program, local.type_reference).is_some()
        {
            return None;
        }
        Some(local_destination(
            program,
            root_symbol,
            local.type_reference,
            place,
            dynamic_index,
            selectors,
        ))
    }

    impl<'a> Destination<'a> {
        /*
            The full projection includes the outer selector split off for byte-view
            operations. All other stores consume this same root and selector roster.
        */
        pub(super) fn full_segments(
            &self,
            program: &dyn ::validation::CheckingInput,
        ) -> Option<Vec<facts::PlaceSegment>> {
            let mut segments = self.place.segments.clone();
            if let Some(index) = self.dynamic_index {
                segments.push(
                    match program.expression_table.constant_integer_value(index) {
                        Some(value) => facts::PlaceSegment::FixedIndex {
                            index: usize::try_from(value).ok()?,
                        },
                        None => facts::PlaceSegment::Index { expression: index },
                    },
                );
            }
            Some(segments)
        }

        /*
            Runtime storage authority belongs to the exact owned home or exclusive
            borrowed referent, independent of entry versus continuation spelling.
            Source assignment validation already judged binding mutability and the
            stored-reference access route. This join retains the complete signature
            roster and declared access; leaf planning keeps qualification and live
            custody checks. A second binding-mutability test would reject writes
            through references held inside an owned parameter.
        */
        pub(super) fn exclusive_authority(
            &self,
            program: &dyn ::validation::CheckingInput,
            state: &typed_trees::state::State,
            roots: StoreRoots<'_>,
            trace: &LocalConstructionTrace,
        ) -> Option<()> {
            let Root::Parameter { plan, parameter } = self.root else {
                return Some(());
            };
            let StoreRoots {
                structural, scalar, ..
            } = roots;
            if !matches!(
                plan.access,
                StructuralAccess::Owned
                    | StructuralAccess::MutableBorrow
                    | StructuralAccess::WriteOnlyBorrow
            ) {
                return None;
            }
            trace.phase("structural field store: parameter access");
            let mut planned_parameters = 0;
            for (position, parameter) in program.state_parameters(state).iter().enumerate() {
                let rows = structural
                    .iter()
                    .filter(|plan| plan.position as usize == position)
                    .count()
                    + scalar
                        .iter()
                        .filter(|plan| plan.source_position as usize == position)
                        .count();
                planned_parameters += rows;
                if parameter.relevance.is_erased() {
                    if rows != 0 {
                        return None;
                    }
                } else if rows != 1 {
                    return None;
                }
            }
            if planned_parameters != scalar.len() + structural.len() || parameter.is_const {
                return None;
            }
            let expected_access =
                match validation::reference_shell(program, parameter.type_reference) {
                    Some(shell) if shell.access.is_exclusive() => {
                        StructuralAccess::from(shell.access)
                    }
                    Some(_) => return None,
                    None => StructuralAccess::Owned,
                };
            (plan.access == expected_access).then_some(())
        }

        /*
            Resolve the leaf's exact declaration namespace. A trailing case keeps
            its payload roster; otherwise the owner supplies its common fields.
            The same path and scalar-field obligations govern either namespace.
        */
        pub(super) fn leaf_carrier(
            &self,
            program: &'a dyn ::validation::CheckingInput,
            segments: &[facts::PlaceSegment],
        ) -> Option<Carrier<'a>> {
            let projection = self.walk(program, segments)?;
            let owner = projection.owner?;
            plain_checked_shape(owner, program).then_some(Carrier {
                path: projection.path,
                owner,
                selected_case: projection.selected_case,
            })
        }

        pub(super) fn storage_projection(
            &self,
            program: &'a dyn ::validation::CheckingInput,
        ) -> Option<StorageProjection<'a>> {
            let mut projection = self.walk(program, &self.full_segments(program)?)?;
            let descriptor = self.loan_type.is_valid()
                && (validation::slice_view_shell(program, self.loan_type, &[]).is_some()
                    || matches!(
                        validation::byte_sequence_carrier(program, self.loan_type, &[]),
                        Some(
                            checked_trees::CheckedByteSequenceCarrier::BorrowedView { .. }
                                | checked_trees::CheckedByteSequenceCarrier::BoundedView { .. }
                        )
                    ));
            if self.loan_type.is_valid() && !descriptor {
                projection.path.insert(0, PathSegment::Referent);
                if let Some(position) = &mut projection.element_view_index {
                    *position += 1;
                }
            }
            Some(projection)
        }

        fn walk(
            &self,
            program: &'a dyn ::validation::CheckingInput,
            segments: &[facts::PlaceSegment],
        ) -> Option<StorageProjection<'a>> {
            if let Some(owner) = self.root_owner
                && !segments.is_empty()
                && !plain_checked_shape(owner, program)
            {
                return None;
            }
            let mut path = Vec::with_capacity(segments.len());
            let mut element_view_index = None;
            let mut carrier_type = self.root_type;
            let mut owner = self.root_owner;
            let mut ends_at_field = false;
            let mut selected_case = None;
            for segment in segments {
                match segment {
                    facts::PlaceSegment::Field { symbol } => {
                        let field_owner = owner?;
                        if !plain_checked_shape(field_owner, program) {
                            return None;
                        }
                        let carrier = relevant_field_in_namespace(
                            program,
                            field_owner,
                            selected_case.take(),
                            *symbol,
                        )?;
                        path.push(PathSegment::Field(terminal_field_identity(
                            program,
                            carrier.symbol,
                        )?));
                        carrier_type = carrier.type_reference;
                        ends_at_field = true;
                    }
                    facts::PlaceSegment::Case { variant } => {
                        let case_owner = owner?;
                        if selected_case.is_some() || !plain_checked_shape(case_owner, program) {
                            return None;
                        }
                        let mut cases = program.data_members(case_owner).iter().filter_map(
                            |member| match member {
                                DataMember::Variant(case)
                                    if variant.is_valid() && case.symbol == *variant =>
                                {
                                    Some(case)
                                }
                                _ => None,
                            },
                        );
                        let case = cases.next()?;
                        if cases.next().is_some() {
                            return None;
                        }
                        path.push(PathSegment::Case(
                            checked_trees::CheckedMemberIdentity::of_member(
                                case.identity,
                                case.name.clone(),
                            ),
                        ));
                        selected_case = Some(case);
                        ends_at_field = false;
                        continue;
                    }
                    facts::PlaceSegment::FixedIndex { .. } | facts::PlaceSegment::Index { .. } => {
                        if selected_case.is_some() {
                            return None;
                        }
                        let array = validation::unwrapped_type_reference(program, carrier_type)?;
                        let (element_type, length) = match program
                            .type_reference_table
                            .type_reference(array)
                        {
                            TypeReferenceNode::FixedArray { element_type, .. } => (
                                *element_type,
                                program
                                    .declared_array_extent(array)?
                                    .literal()
                                    .map(usize::try_from)
                                    .transpose()
                                    .ok()?,
                            ),
                            TypeReferenceNode::Slice { element_type } => {
                                if matches!(program.type_reference_table.type_reference(carrier_type),
                                        TypeReferenceNode::Reference { access, .. }
                                            if !access.is_exclusive())
                                {
                                    return None;
                                }
                                if program.primitive_type_reference(*element_type)
                                    != Some(typed_trees::types::PrimitiveType::U8)
                                {
                                    element_view_index = Some(path.len());
                                }
                                (*element_type, None)
                            }
                            _ => return None,
                        };
                        path.push(match segment {
                            facts::PlaceSegment::FixedIndex { index }
                                if length.is_none_or(|length| *index < length) =>
                            {
                                PathSegment::FixedIndex(u64::try_from(*index).ok()?)
                            }
                            facts::PlaceSegment::Index { expression } => {
                                self.selectors.as_ref()?.runtime_segment(*expression)?
                            }
                            _ => return None,
                        });
                        carrier_type = element_type;
                        ends_at_field = false;
                    }
                    _ => return None,
                }
                let mut reference_types = Vec::new();
                loop {
                    carrier_type = validation::peel_declared_constraints(program, carrier_type);
                    let TypeReferenceNode::Reference {
                        referee, access, ..
                    } = program.type_reference_table.type_reference(carrier_type)
                    else {
                        break;
                    };
                    if reference_types.contains(&carrier_type) {
                        return None;
                    }
                    reference_types.push(carrier_type);
                    if validation::slice_view_shell(program, carrier_type, &[]).is_some() {
                        break;
                    }
                    if !access.is_exclusive() {
                        return None;
                    }
                    path.push(PathSegment::Referent);
                    carrier_type = *referee;
                    ends_at_field = false;
                }
                owner = validation::field_domains::data_definition_for_field_type(
                    program,
                    carrier_type,
                );
            }
            Some(StorageProjection {
                path,
                element_view_index,
                type_reference: carrier_type,
                ends_at_field,
                owner,
                selected_case,
            })
        }
    }

    /*
        A record whose scalar leaves a store may write through.

        Lifetime binders are deliberately not part of this test. A declared
        lifetime is an erased region, "separate from runtime generic arity, layout
        and monomorphization" (wiki/spec/language/lifetimes.md, Binders and source
        applications), and store admission retains "the complete home declaration,
        path, field, access, qualifications/claims, scalar type, and dominating
        definition" (wiki/spec/terminal-psi/structural_access.md, Store
        vocabulary) -- a binder changes none of them. `exact_relevant_field` skips
        erased fields. Pointer crossings remain explicit `Referent` steps, with
        separate leaf-custody checks; canonical field facts cannot cross them.
        Runtime generic arity is still
        excluded by the type-parameter and owner-application tests below.
    */
    pub(super) fn plain_record(
        data: &typed_trees::data::DataDefinition,
        program: &dyn ::validation::CheckingInput,
    ) -> bool {
        plain_checked_shape(data, program)
            && typed_trees::data::DataDefinition::shape_kind_from_members(
                program.data_members(data),
            ) == DataShapeKind::Record
    }

    fn plain_checked_shape(
        data: &typed_trees::data::DataDefinition,
        program: &dyn ::validation::CheckingInput,
    ) -> bool {
        data.supply_mode == language_semantics::DataSupplyMode::CheckedShape
            && program.data_type_parameters(data).is_empty()
            && retained_record_owner_application(data, program)
            && data.quotient.is_none()
    }

    fn retained_record_owner_application(
        data: &typed_trees::data::DataDefinition,
        program: &dyn ::validation::CheckingInput,
    ) -> bool {
        let Some(application) = data.generic_instance else {
            return true;
        };
        program
            .closed_data_constructor_definition(application, data.symbol)
            .is_some()
    }

    /*
        Whether a declared field type names storage behind a reference.

        Field facts describe inline storage of one referent. A pointer hop is not
        an inline offset: primitive-store projection retains an explicit `Referent`
        step and checks the reference leaf's authority separately. Consumers that
        build canonical field facts use this query to exclude that crossing.
    */
    pub(super) fn crosses_reference(
        program: &dyn ::validation::CheckingInput,
        type_reference: typed_trees::types::TypeReferenceHandle,
    ) -> bool {
        match program.type_reference_table.type_reference(type_reference) {
            TypeReferenceNode::Reference { .. } => true,
            TypeReferenceNode::Constrained { base_type, .. } => {
                crosses_reference(program, *base_type)
            }
            _ => false,
        }
    }

    pub(super) fn exact_relevant_field<'a>(
        program: &'a dyn ::validation::CheckingInput,
        owner: &'a typed_trees::data::DataDefinition,
        symbol: SymbolHandle,
    ) -> Option<&'a typed_trees::data::DataField> {
        let mut fields = program
            .data_members(owner)
            .iter()
            .filter_map(|member| match member {
                DataMember::Field(field)
                    if field.symbol == symbol && !field.relevance.is_erased() =>
                {
                    Some(field)
                }
                _ => None,
            });
        let field = fields.next()?;
        fields.next().is_none().then_some(field)
    }

    impl<'a> Carrier<'a> {
        pub(super) fn field(
            &self,
            program: &'a dyn validation::CheckingInput,
            symbol: SymbolHandle,
        ) -> Option<&'a typed_trees::data::DataField> {
            relevant_field_in_namespace(program, self.owner, self.selected_case, symbol)
        }
    }

    fn relevant_field_in_namespace<'a>(
        program: &'a dyn validation::CheckingInput,
        owner: &'a typed_trees::data::DataDefinition,
        selected_case: Option<&'a typed_trees::data::DataVariant>,
        symbol: SymbolHandle,
    ) -> Option<&'a typed_trees::data::DataField> {
        let Some(case) = selected_case else {
            return exact_relevant_field(program, owner, symbol);
        };
        let mut fields = program.data_payload_fields(case).iter().filter(|field| {
            symbol.is_valid() && field.symbol == symbol && !field.relevance.is_erased()
        });
        let field = fields.next()?;
        fields.next().is_none().then_some(field)
    }

    /*
        Rejoin a write through an exclusive loan local with the place it loans.

        `let seen: &mut u8 = &mut self.byte; seen = 0;` writes `self.byte`. The
        local is immutable and a reference cannot be reseated, so the name denotes
        one place for its whole scope and the store is the ordinary parameter-rooted
        store of that place with the loan's own segments in front. This adds no
        authority: the loan's exclusivity, lifetime and conflicting-access rules are
        already checked, and a shared loan is not a write destination.

        A place that is not rooted at such a local is returned unchanged.
    */
    fn rejoin_loan_local(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        ordinal: usize,
        place: validation::CanonicalPlace,
    ) -> Option<(
        validation::CanonicalPlace,
        Option<typed_trees::types::PrimitiveType>,
    )> {
        let Some(symbol) = place.root.as_symbol() else {
            return Some((place, None));
        };
        let statements = program.statement_table.statements(state.statement_nodes);
        let mut declarations =
            statements
                .iter()
                .enumerate()
                .take(ordinal)
                .filter_map(|(declared, statement)| match statement {
                    StatementNode::LocalData(local) if local.symbol == symbol => {
                        Some((declared, local))
                    }
                    _ => None,
                });
        let Some((_, local)) = declarations.next() else {
            return Some((place, None));
        };
        if declarations.next().is_some() {
            return None;
        }
        let mut initial = local.initial_value;
        let mut recast = false;
        while let Some(cast) = program.expression_table.cast_node(initial) {
            if !cast.form.is_recast() {
                break;
            }
            initial = cast.value;
            recast = true;
        }
        let Some(borrow) = program.expression_table.borrow_expression(initial) else {
            return Some((place, None));
        };
        if local.is_mutable
            || !matches!(
                borrow.access,
                language_core::ReferenceAccess::Mutable | language_core::ReferenceAccess::WriteOnly
            )
        {
            return None;
        }
        let mut target = borrow.target;
        while let Some(cast) = program.expression_table.cast_node(target) {
            if !cast.form.is_recast() {
                break;
            }
            target = cast.value;
            recast = true;
        }
        let recast_view = if recast {
            match program
                .type_reference_table
                .type_reference(local.type_reference)
            {
                TypeReferenceNode::Reference { referee, .. } => {
                    match program.primitive_type_reference(*referee) {
                        None => {
                            if let Some((leaf_place, view)) = record_view_leaf(
                                program, machine, state, ordinal, target, *referee, &place,
                            ) {
                                return Some((leaf_place, view));
                            }
                            return Some((place, None));
                        }
                        primitive => primitive,
                    }
                }
                _ => None,
            }
        } else {
            None
        };
        let mut loaned = place_at(program, state.symbol, ordinal, target)?;
        loaned.segments.extend(place.segments);
        Some((loaned, recast_view))
    }

    /*
        The source leaf a store through a typed record view writes: the loaned
        place followed by the leaf's own segments, and the view's carrier when it
        differs from the leaf's. Only a direct field of the view rejoins.
    */
    fn record_view_leaf(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        ordinal: usize,
        target: typed_trees::expression::ExpressionHandle,
        referee: typed_trees::types::TypeReferenceHandle,
        place: &validation::CanonicalPlace,
    ) -> Option<(
        validation::CanonicalPlace,
        Option<typed_trees::types::PrimitiveType>,
    )> {
        let [facts::PlaceSegment::Field { symbol: field }] = place.segments.as_slice() else {
            return None;
        };
        let source = validation::declared_place_type_raw(program, machine, Some(state), target)?;
        let leaf = validation::recast_record_source_leaf(program, source, referee, *field)?;
        let mut loaned = place_at(program, state.symbol, ordinal, target)?;
        for segment in leaf.path {
            loaned.segments.push(match segment {
                validation::RecastLeafSegment::Field(symbol) => {
                    facts::PlaceSegment::Field { symbol }
                }
                validation::RecastLeafSegment::Element(index) => {
                    facts::PlaceSegment::FixedIndex { index }
                }
            });
        }
        let view = (leaf.view_primitive != leaf.primitive).then_some(leaf.view_primitive);
        Some((loaned, view))
    }

    /*
        An exclusive reference local names its current materialized carrier. The declaration
        supplies its referent shape and access; source checking already admitted
        this assignment's loan compatibility. Replaying the initializer's loan here
        would confuse its old backing with the descriptor installed by a rebind.
    */
    fn exclusive_reference_local_root<'a>(
        program: &'a dyn ::validation::CheckingInput,
        state: &'a typed_trees::state::State,
        root_symbol: SymbolHandle,
        ordinal: usize,
        dynamic_index: Option<typed_trees::expression::ExpressionHandle>,
        place: validation::CanonicalPlace,
        selectors: Option<super::selectors::TargetSelectors>,
    ) -> Option<Destination<'a>> {
        let statements = program.statement_table.statements(state.statement_nodes);
        let mut locals = statements
            .iter()
            .enumerate()
            .filter_map(|(local_ordinal, statement)| match statement {
                StatementNode::LocalData(local) if local.symbol == root_symbol => {
                    Some((local_ordinal, local))
                }
                _ => None,
            });
        let (local_ordinal, local) = locals.next()?;
        if locals.next().is_some()
            || local_ordinal >= ordinal
            || !expressions(program).expression_is_valid(local.initial_value)
        {
            return None;
        }
        let shell = validation::reference_shell(program, local.type_reference)?;
        if !matches!(
            shell.access,
            language_semantics::ReferenceAccess::Mutable
                | language_semantics::ReferenceAccess::WriteOnly
        ) {
            return None;
        }
        let referee = shell.referee;
        Some(Destination {
            root: Root::Local {
                symbol: root_symbol,
            },
            place,
            dynamic_index,
            recast_view: None,
            loan_type: local.type_reference,
            selectors,
            root_type: referee,
            root_owner: validation::field_domains::data_definition_for_field_type(program, referee),
        })
    }
}
mod element_stores {
    /*
        Scalar stores below mutable element-view descriptors.

        The resolved storage walk owns field identity, reference crossings and
        selector occurrences. Its typed descriptor crossing selects the view element;
        the rest selects a scalar leaf within that element. Separating these two
        coordinates keeps a nested fixed array from treating descriptor storage as
        inline primitive backing. Whole elements and record fields use this same
        operation, with no dependency on the right-hand side's source arrangement.
        Byte views retain their independent predicate-preservation contract.
    */
    use super::value;
    use super::{CheckedUnitEffectOperationPlan, CheckedUnitStructuralPathSegment, ExpressionNode};

    pub(super) fn store(site: &super::Site<'_, '_, '_>) -> Option<CheckedUnitEffectOperationPlan> {
        let super::Occurrence {
            program,
            facts,
            machine,
            state,
            statement_index,
            assignment,
            ..
        } = site.occurrence;
        let destination = site.destination;
        let projection = destination.storage_projection(program)?;
        let primitive_type = program.primitive_type_reference(projection.type_reference)?;
        if projection.ends_at_field {
            let (facts::PlaceSegment::Field { symbol }, carriers) =
                destination.place.segments.split_last()?
            else {
                return None;
            };
            let carrier = destination.leaf_carrier(program, carriers)?;
            let field = carrier.field(program, *symbol)?;
            if super::scalar_leaf(program, field)? != primitive_type {
                return None;
            }
        }
        let (descriptor_path, element_path) =
            projection.path.split_at(projection.element_view_index?);
        let (selector, path) = element_path.split_first()?;
        let element_selector = match selector {
            CheckedUnitStructuralPathSegment::FixedIndex(element_index) => {
                checked_trees::CheckedScalarExpression::IntegerLiteral {
                    literal: numerics::literals::IntegerLiteral::from_value(
                        i64::try_from(*element_index).ok()?,
                    )
                    .with_landing(numerics::literals::IntegerLanding {
                        landed_type: numerics::literals::LandedIntegerType::U64,
                        domain: numerics::arithmetic::ArithmeticDomain::Exact,
                    }),
                }
            }
            CheckedUnitStructuralPathSegment::RuntimeIndex(selector) => {
                super::runtime_index_expression(program, facts, state, statement_index, selector)?
            }
            _ => return None,
        };
        let selected = element_selector_target(program, state, assignment.target, path)?;
        let value = value::assignment_value(&site.occurrence, primitive_type)?;
        Some(CheckedUnitEffectOperationPlan::new_store(
            statement_index,
            destination.root.destination(),
            descriptor_path.to_vec(),
            checked_trees::CheckedStoreLeaf::ViewElement {
                index: element_selector,
                element_path: path.to_vec(),
                element: primitive_type,
                range_decision: facts.proof.integer_assignment_at(
                    machine.symbol,
                    state.symbol,
                    statement_index as usize,
                    assignment.target,
                    assignment.value,
                ),
                decisions: checked_trees::CheckedIndexedWriteDecisions {
                    index_in_bounds: facts.scalar_index_is_proven(selected),
                    predicates_preserved: false,
                },
            },
            checked_trees::CheckedStoreValue::assigned(value),
        ))
    }

    /*
        The authored `collection[index]` whose selector this write's element
        coordinate names; index checking records its judgment against it. The
        target's member and fixed-index steps within the element (`path`) peel
        off first. A `local.field = value` store through a `&mut collection[index]`
        element view local has no indexed step of its own, so the local's
        borrowed `collection[index]` stands in.
    */
    fn element_selector_target(
        program: &dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        target: typed_trees::expression::ExpressionHandle,
        path: &[CheckedUnitStructuralPathSegment],
    ) -> Option<typed_trees::expression::ExpressionHandle> {
        let mut target = target;
        for segment in path.iter().rev() {
            target = match (segment, program.expression_table.expression(target)) {
                (CheckedUnitStructuralPathSegment::Field(_), ExpressionNode::Member(member))
                    if member.case_variant.is_none() =>
                {
                    member.receiver
                }
                (
                    CheckedUnitStructuralPathSegment::FixedIndex(_),
                    ExpressionNode::Indexed(indexed),
                ) => indexed.collection,
                _ => return None,
            };
        }
        if let Some(name) = program.expression_table.name_path_node(target)
            && let Some((_, local)) = program.state_local_declaration(state, name.symbol)
            && let ExpressionNode::Borrow(borrow) =
                program.expression_table.expression(local.initial_value)
            && borrow.access == language_core::ReferenceAccess::Mutable
        {
            return Some(borrow.target);
        }
        Some(target)
    }
}
mod primitive {
    /*
        Primitive leaves: whole primitive storage and primitive elements.

        Restricted primitive stores retain their complete declaration and the exact
        source assignment judgment. Bounded declarations additionally retain the
        declaration-selected interval; general scalar predicates use the same
        completed source-check cut without manufacturing a numeric receipt.
        Terminal obligations keep the original restriction and assignment identity,
        so carrier normalization cannot erase the receiving domain. Declared record
        fields remain field operations with their own retained restrictions.

        A primitive leaf is the `Primitive` store leaf over one checked
        path from the shared destination resolver's borrowed or owned storage.
        The path composes fields, explicit reference crossings, literal elements, and the
        target's runtime elements: `self.cells[self.i] = v` is the store over
        `[cells, RuntimeIndex(AssignmentIndex { depth: 0 })]` and
        `self.grid[i][j] = v` the store over `[grid, RuntimeIndex(depth 1),
        RuntimeIndex(depth 0)]`, not a separate indexed store with a selector as
        an operand. Each runtime selector retains the statement's exact
        `AssignmentIndex { depth }` scalar and its separate bounds obligation.
        The planner creates no bound from the selector's syntactic shape.

        Whole primitive array construction and replacement are sequenced by the
        statement owner: its complete value is established before receiving storage
        changes, so stored reads cannot observe a partial replacement.
    */
    use super::super::{
        CheckFacts, CheckedScalarExpression as ScalarExpression,
        CheckedScalarExpressionRole as ExpressionRole, CheckedUnitEffectOperationPlan,
        Multiplicity, PrimitiveType, StatementNode, SymbolHandle, TypeConstraintNode,
        TypeReferenceNode,
    };
    use crate::flow::expressions;
    use crate::published::places::place_at;

    /*
        Find primitive storage established by an initializer or its declaration's
        plain zero value before this occurrence.
        Both binding kinds own contents; replacement permission is checked by
        the store owner, while exact borrow facts decide the access a loan lends.
    */
    pub(in crate::execution::terminal_unit) fn primitive_local_before<'program>(
        program: &'program dyn ::validation::CheckingInput,
        state: &typed_trees::state::State,
        statement_index: usize,
        symbol: SymbolHandle,
    ) -> Option<&'program typed_trees::statement::TableLocalData> {
        let (_, local) = program
            .state_local_declaration(state, symbol)
            .filter(|&(ordinal, _)| ordinal < statement_index)?;
        if (!expressions(program).expression_is_valid(local.initial_value)
            && crate::values::zeroed_scalar_local_initializer(program, local).is_none())
            || !super::super::types::supports_primitive_storage(program, local.type_reference)
            || program
                .primitive_type_reference(local.type_reference)
                .is_none()
        {
            return None;
        }
        Some(local)
    }

    /*
        The primitive store of one authored assignment, or `None` when its target
        is not a primitive leaf this lane writes. The shared statement roster
        validates the complete state write frame.
    */
    #[allow(clippy::too_many_arguments)]
    pub(super) fn store_at(
        site: &super::Site<'_, '_, '_>,
    ) -> Option<CheckedUnitEffectOperationPlan> {
        let super::Occurrence {
            program,
            facts,
            machine,
            state,
            statement_index,
            assignment,
            source,
            ..
        } = site.occurrence;
        let destination = site.destination;
        let projection = destination.storage_projection(program)?;
        let primitive_type = program.primitive_type_reference(projection.type_reference)?;
        if projection.ends_at_field || destination.recast_view.is_some() {
            return None;
        }
        let leaf = primitive_leaf_type_reference(program, machine, state, assignment.target)?;
        let receiving_range = match super::super::types::retain_scalar_field(
            program,
            leaf,
            &[],
            primitive_type,
            None,
        )? {
            checked_trees::CheckedUnitStructuralFieldType::Scalar(_)
                if validation::unrestricted_builtin_primitive(program, leaf)
                    == Some(primitive_type) =>
            {
                None
            }
            checked_trees::CheckedUnitStructuralFieldType::BoundedInteger(integer) => {
                let decision = facts.proof.integer_assignment_at(
                    machine.symbol,
                    state.symbol,
                    statement_index as usize,
                    assignment.target,
                    assignment.value,
                );
                if !decision.is_valid() {
                    return None;
                }
                Some(checked_trees::CheckedPrimitiveReceivingRange {
                    primitive: primitive_type,
                    integer,
                    decision,
                })
            }
            checked_trees::CheckedUnitStructuralFieldType::RefinedScalar { .. } => None,
            _ => return None,
        };
        if projection.path.is_empty() {
            match destination.root {
                super::destination::Root::Parameter { plan, .. }
                    if plan.multiplicity != Multiplicity::Unrestricted =>
                {
                    return None;
                }
                super::destination::Root::Local { symbol }
                    if primitive_local_before(program, state, statement_index as usize, symbol)
                        .is_none_or(|local| !local.is_mutable)
                        && !program.state_parameters(state).iter().any(|parameter| {
                            parameter.symbol == symbol && parameter.is_mutable
                        }) =>
                {
                    return None;
                }
                _ => {}
            }
        }
        let authored = place_at(
            program,
            state.symbol,
            statement_index as usize,
            assignment.target,
        )?;
        let authored_symbol = authored.root.as_symbol()?;
        let value = match source {
            super::value::AssignmentSource::Authored => primitive_store_value(
                program,
                facts,
                machine,
                state,
                statement_index,
                assignment,
                authored_symbol,
                authored.segments.is_empty(),
                primitive_type,
            )?,
            super::value::AssignmentSource::CallResult { .. } => {
                super::value::assignment_value(&site.occurrence, primitive_type)?
            }
            super::value::AssignmentSource::StructuralValue { .. } => return None,
        };
        Some(CheckedUnitEffectOperationPlan::new_store(
            statement_index,
            destination.root.destination(),
            projection.path,
            checked_trees::CheckedStoreLeaf::Primitive {
                primitive_type,
                receiving_range,
                preserves_byte_memberships: facts.byte_write_is_proven(assignment.target),
            },
            checked_trees::CheckedStoreValue::assigned(value),
        ))
    }

    /*
        The stored value of one primitive store: the statement's own call result
        or the pure scalar bound at `AssignmentValue`, retagged to the leaf's
        declared primitive. Whole-root stores keep the authored root symbol as
        the bound destination; every element or field store binds `invalid`.
    */
    fn primitive_store_value(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: u32,
        assignment: &typed_trees::statement::TableAssignment,
        authored_symbol: SymbolHandle,
        authored_whole: bool,
        primitive_type: PrimitiveType,
    ) -> Option<checked_trees::CheckedScalarStoreValue> {
        let computations = &facts.values.scalar_computations;
        let mut roots = computations
            .roots
            .iter()
            .map(|(_, root)| root)
            .filter(|root| {
                root.state == state.symbol
                    && root.statement_ordinal == statement_index
                    && root.role == ExpressionRole::AssignmentValue
            });
        if let Some(root) = roots.next() {
            if roots.next().is_some()
                || !computations.is_authored_root(
                    root,
                    machine.symbol,
                    assignment.value,
                    primitive_type,
                )
                || facts
                    .values
                    .scalar_expressions
                    .expression_at(
                        state.symbol,
                        statement_index,
                        ExpressionRole::AssignmentValue,
                    )
                    .is_some()
            {
                return None;
            }
            return Some(checked_trees::CheckedScalarStoreValue::Computation(
                root.root,
            ));
        }
        let (binding, value) = facts.values.scalar_expressions.bound_expression_at(
            state.symbol,
            statement_index,
            ExpressionRole::AssignmentValue,
        )?;
        if binding.expression != assignment.value
            || binding.destination
                != if authored_whole {
                    authored_symbol
                } else {
                    SymbolHandle::invalid()
                }
            || crate::values::scalar_expression_type(value) != Some(primitive_type)
            || !scalar_store_value_is_exact(program, facts, state, binding, value, primitive_type)
        {
            return None;
        }
        Some(checked_trees::CheckedScalarStoreValue::Pure(value.clone()))
    }

    /*
        The type of the primitive leaf an assignment through `expression` writes.
        An arithmetic-policy shell (`i32 in Wrapping`) qualifies the element's
        operations, not its storage identity, so it peels here like the primitive
        leaf of the record-field store route. A represented integer interval keeps
        its receiving judgment on the checked store leaf; other constraints decline.
        Storage identity itself looks through qualification (the shared structural
        identity contract); a successful source receiving judgment is required
        before lowering that store. This does not create a Terminal field invariant.
        An exclusive borrow carrier peels to its referee the same way: `borrowed`
        in `let borrowed: &mut u8 = &mut self.buf[i]` spells the reference type,
        while the store reaches the element behind it. Whether that reach is
        lawful is the destination resolver's call, not the leaf walker's — a
        carrier without validated formation keeps its own root and declines
        there. A shared borrow never reaches this lane.
    */
    fn primitive_leaf_type_reference(
        program: &dyn ::validation::CheckingInput,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        expression: typed_trees::expression::ExpressionHandle,
    ) -> Option<typed_trees::types::TypeReferenceHandle> {
        let mut leaf =
            validation::declared_place_type_raw(program, machine, Some(state), expression)?;
        loop {
            match program.type_reference_table.type_reference(leaf) {
                TypeReferenceNode::Constrained {
                    base_type,
                    constraints,
                } => {
                    if !program
                        .type_reference_table
                        .constraints(*constraints)
                        .iter()
                        .all(|constraint| {
                            matches!(constraint, TypeConstraintNode::ArithmeticDomain(_))
                        })
                    {
                        break;
                    }
                    leaf = *base_type;
                }
                TypeReferenceNode::Reference {
                    access, referee, ..
                } => {
                    if matches!(access, language_semantics::ReferenceAccess::Shared) {
                        break;
                    }
                    leaf = *referee;
                }
                _ => break,
            }
        }
        Some(leaf)
    }

    /*
        The scalar-binding closure both store custody proofs share: the binding
        must close over exactly the scalar-position parameters and the immutable
        primitive locals declared before its statement — nothing missing,
        nothing extra.
    */
    fn scalar_binding_symbols_exact(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &typed_trees::state::State,
        binding: &checked_trees::CheckedScalarExpressionBindings,
    ) -> bool {
        let Ok(statement_index) = usize::try_from(binding.statement_ordinal) else {
            return false;
        };
        let Some(symbols) =
            crate::values::unit_scalar_binding_symbols(program, state, statement_index)
        else {
            return false;
        };
        symbols.iter().copied().eq(facts
            .values
            .scalar_expressions
            .binding_symbols
            .span_or_empty(binding.symbols)
            .iter()
            .copied())
    }

    pub(in crate::execution::terminal_unit) fn scalar_custody_is_exact(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &typed_trees::state::State,
        binding: &checked_trees::CheckedScalarExpressionBindings,
        value: &ScalarExpression,
        primitive_type: PrimitiveType,
    ) -> bool {
        if !scalar_binding_symbols_exact(program, facts, state, binding) {
            return false;
        }
        let Ok(statement_index) = usize::try_from(binding.statement_ordinal) else {
            return false;
        };
        if binding.role == ExpressionRole::StorageInitializer && !binding.expression.is_valid() {
            return matches!(program.statement_table.statement_at(state, statement_index),
                Some(StatementNode::LocalData(local)) if local.symbol == binding.destination
                    && crate::values::zeroed_scalar_local_initializer(program, local).as_ref() == Some(value));
        }
        crate::values::lower_unit_scalar_argument(
            program,
            &facts.operators,
            state,
            statement_index,
            binding.expression,
            primitive_type,
        )
        .as_ref()
            == Some(value)
    }

    /*
        A store's scalar value is exact when its binding closes over precisely
        the scalar frame (as `scalar_custody_is_exact` demands) and the retained
        expression either re-lowers through the unit-argument vocabulary — as
        local custody requires — or simply cannot re-lower there at all. The
        scalar-expression table is itself the canonical lowering of this
        authored root; named-domain casts and other shapes the narrower
        argument lowerer cannot spell keep their retained value, as long as
        that value still lowers through the ordinary scalar vocabulary.
    */
    fn scalar_store_value_is_exact(
        program: &dyn ::validation::CheckingInput,
        facts: &CheckFacts,
        state: &typed_trees::state::State,
        binding: &checked_trees::CheckedScalarExpressionBindings,
        value: &ScalarExpression,
        primitive_type: PrimitiveType,
    ) -> bool {
        if !scalar_binding_symbols_exact(program, facts, state, binding) {
            return false;
        }
        let Ok(statement_index) = usize::try_from(binding.statement_ordinal) else {
            return false;
        };
        match crate::values::lower_unit_scalar_argument(
            program,
            &facts.operators,
            state,
            statement_index,
            binding.expression,
            primitive_type,
        ) {
            Some(relowered) => relowered == *value,
            None => scalar_expression_needs_no_bindings(value),
        }
    }

    /*
        A retained scalar value stands on its own when every node lowers through
        the ordinary scalar vocabulary — parameters, locals, literals and their
        compositions. Structural field/indexed reads, storage reads, erased
        formals and trapping casts resolve through binding lists or runtime
        policy that the store's value lane does not carry, so a retained
        expression built from any of them must keep declining.
    */
    fn scalar_expression_needs_no_bindings(
        expression: &checked_trees::CheckedScalarExpression,
    ) -> bool {
        match expression {
            ScalarExpression::Parameter { .. }
            | ScalarExpression::Local { .. }
            | ScalarExpression::IntegerLiteral { .. }
            | ScalarExpression::IeeeFloatLiteral { .. }
            | ScalarExpression::ImplementationSelection { .. } => true,
            ScalarExpression::IntegerBinary { left, right, .. } => {
                scalar_expression_needs_no_bindings(left)
                    && scalar_expression_needs_no_bindings(right)
            }
            checked_trees::integer_widen_triple_pattern!(operand, ..)
            | ScalarExpression::RepresentationView { operand, .. } => {
                scalar_expression_needs_no_bindings(operand)
            }
            ScalarExpression::Boolean(expression) => {
                boolean_expression_needs_no_bindings(expression)
            }
            ScalarExpression::ConstantApplication { .. }
            | ScalarExpression::StructuralSequenceLength { .. }
            | ScalarExpression::StorageRead { .. }
            | ScalarExpression::ErasedParameter { .. }
            | ScalarExpression::StructuralParameterField { .. }
            | ScalarExpression::StructuralLocalField { .. }
            | ScalarExpression::StructuralParameterIndexedRead { .. } => false,
        }
    }

    fn boolean_expression_needs_no_bindings(
        expression: &checked_trees::CheckedBooleanExpression,
    ) -> bool {
        use checked_trees::CheckedBooleanExpression;
        match expression {
            CheckedBooleanExpression::Constant(_)
            | CheckedBooleanExpression::Parameter { .. }
            | CheckedBooleanExpression::Local { .. } => true,
            CheckedBooleanExpression::Not(operand) => boolean_expression_needs_no_bindings(operand),
            facts::checked_boolean_triple_pattern!(left, right, ..) => {
                boolean_expression_needs_no_bindings(left)
                    && boolean_expression_needs_no_bindings(right)
            }
            CheckedBooleanExpression::IntegerComparison { left, right, .. } => {
                scalar_expression_needs_no_bindings(left)
                    && scalar_expression_needs_no_bindings(right)
            }
            CheckedBooleanExpression::StorageRead { .. }
            | CheckedBooleanExpression::ErasedParameter { .. }
            | CheckedBooleanExpression::StructuralParameterField { .. }
            | CheckedBooleanExpression::ScalarIeeeFloatComparison { .. }
            | CheckedBooleanExpression::IntegerMathComparison { .. }
            | CheckedBooleanExpression::IeeeFloatComparison { .. }
            | CheckedBooleanExpression::StructuralIdentity { .. }
            | CheckedBooleanExpression::ByteSequenceEqual { .. }
            | CheckedBooleanExpression::ByteSequencePredicate { .. }
            | CheckedBooleanExpression::StructuralCaseMembership { .. } => false,
        }
    }
}
mod selectors {
    /*
        The indexed steps of one assignment target.

        Every indexed step of a target is an element selection: a literal selector
        names a static element and must lie within its array's declared extent; a
        runtime selector is the statement's retained `AssignmentIndex { depth }`
        scalar (depth counted from the target inward, the coordinate
        `validation::assignment_target_selectors` defines) and becomes a
        `RuntimeIndex` path segment. The planner proves no bound for a runtime
        element: Terminal re-proves `index < extent` for the segment's obligation.
    */
    use super::super::{
        CheckFacts, CheckedScalarExpressionRole, CheckedUnitStructuralPathSegment, ExpressionNode,
        PrimitiveType, TypeReferenceNode,
    };
    use typed_trees::expression::ExpressionHandle;

    #[derive(Clone)]
    pub(in crate::execution::terminal_unit) struct TargetSelectors {
        /* The target's selector expressions from the target inward. */
        selectors: Vec<ExpressionHandle>,
        /*
            Segments resolved for `Index` selectors an erased borrow alias
            injects into the walk — expressions the authored target never
            spelled, so no `AssignmentIndex` coordinate exists for them.
        */
        resolved: Vec<(ExpressionHandle, CheckedUnitStructuralPathSegment)>,
    }

    impl TargetSelectors {
        /*
            Admit every indexed step of `target`, or `None` when one of them is
            not an element of a declared fixed array or slice: a range (a
            subslice), a literal outside a declared extent, a selector into a
            non-collection carrier, or a runtime selector whose scalar coordinate the
            statement does not retain. A slice carries no declared literal
            extent, so its steps leave every index unproven here for the
            segment's Terminal obligation.
        */
        pub(in crate::execution::terminal_unit) fn resolve(
            program: &dyn ::validation::CheckingInput,
            facts: &CheckFacts,
            machine: &typed_trees::machine::Machine,
            state: &typed_trees::state::State,
            statement_index: u32,
            target: ExpressionHandle,
        ) -> Option<Self> {
            let selectors = validation::assignment_target_selectors(program, target);
            let mut cursor = target;
            let mut depth = 0usize;
            loop {
                match program.expression_table.expression(cursor) {
                    ExpressionNode::Indexed(indexed) => {
                        if selectors.get(depth) != Some(&indexed.index) {
                            return None;
                        }
                        let collection = validation::declared_place_type_raw(
                            program,
                            machine,
                            Some(state),
                            indexed.collection,
                        )?;
                        let collection = validation::unwrapped_type_reference(program, collection)?;
                        let literal_length =
                            match program.type_reference_table.type_reference(collection) {
                                TypeReferenceNode::FixedArray { .. } => program
                                    .declared_array_extent(collection)?
                                    .literal()
                                    .map(usize::try_from)
                                    .transpose()
                                    .ok()?,
                                TypeReferenceNode::Slice { .. } => None,
                                _ => return None,
                            };
                        match program.expression_table.expression(indexed.index) {
                            ExpressionNode::Integer(literal) => {
                                let index =
                                    usize::try_from(literal.value_bignum()?.to_u64()?).ok()?;
                                if literal_length.is_some_and(|length| index >= length) {
                                    return None;
                                }
                            }
                            _ => retained(
                                facts,
                                machine,
                                state,
                                statement_index,
                                u32::try_from(depth).ok()?,
                                indexed.index,
                            )?,
                        }
                        depth += 1;
                        cursor = indexed.collection;
                    }
                    ExpressionNode::Member(member) => cursor = member.receiver,
                    ExpressionNode::Name(_) => break,
                    _ => return None,
                }
            }
            if depth != selectors.len() {
                return None;
            }
            let mut resolved = Vec::new();
            if let Some((_, borrow_state)) = facts
                .borrow
                .first_state_borrow(machine.symbol, state.symbol)
            {
                for loan in facts.borrow.loans.span_or_empty(borrow_state.loans) {
                    for segment in facts.borrow.loan_segments(loan) {
                        let facts::PlaceSegment::Index { expression } = segment else {
                            continue;
                        };
                        if resolved
                            .iter()
                            .any(|(candidate, _): &(ExpressionHandle, _)| candidate == expression)
                        {
                            continue;
                        }
                        if let Some(segment) = super::super::calls::borrowed_index_segment(
                            program,
                            state.symbol,
                            loan.statement_index,
                            *expression,
                        ) {
                            resolved.push((*expression, segment));
                        }
                    }
                }
            }
            Some(Self {
                selectors,
                resolved,
            })
        }

        /*
            The checked segment of the runtime element `expression` selects. An
            authored target selector always carries the `AssignmentIndex`
            coordinate; an alias-injected borrow selector resolves through the
            side table `resolve` built.
        */
        pub(in crate::execution::terminal_unit) fn runtime_segment(
            &self,
            expression: ExpressionHandle,
        ) -> Option<CheckedUnitStructuralPathSegment> {
            if let Some(depth) = self
                .selectors
                .iter()
                .position(|selector| *selector == expression)
            {
                return Some(CheckedUnitStructuralPathSegment::RuntimeIndex(
                    checked_trees::CheckedRuntimeIndex::AssignmentIndex {
                        depth: u32::try_from(depth).ok()?,
                    },
                ));
            }
            self.resolved.iter().find_map(|(candidate, segment)| {
                (*candidate == expression).then(|| segment.clone())
            })
        }
    }

    /*
        The statement's `AssignmentIndex { depth }` coordinate retains exactly this
        authored selector at an integer carrier: a bound pure expression, or the
        computation root the ordinary scalar evaluator completes.
    */
    fn retained(
        facts: &CheckFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: u32,
        depth: u32,
        index: ExpressionHandle,
    ) -> Option<()> {
        let integer = |primitive_type: Option<PrimitiveType>| {
            primitive_type.is_some_and(|primitive| primitive.is_fixed_integer())
        };
        let role = CheckedScalarExpressionRole::AssignmentIndex { depth };
        if let Some((binding, value)) =
            facts
                .values
                .scalar_expressions
                .bound_expression_at(state.symbol, statement_index, role)
        {
            let primitive_type = crate::values::scalar_expression_type(value);
            return (binding.expression == index && integer(primitive_type)).then_some(());
        }
        let computations = &facts.values.scalar_computations;
        let root = computations.root_at(state.symbol, statement_index, role)?;
        if root.machine != machine.symbol || !computations.nodes.is_valid(root.root) {
            return None;
        }
        let node = computations.nodes.get(root.root);
        (node.authored_root == index && integer(Some(node.primitive_type))).then_some(())
    }
}
mod value {
    /*
        Resolve what one store writes: an already-checked scalar source, selected
        independently of where it is stored.

        The store vocabulary asks for an "already-defined, exactly typed SSA value"
        (wiki/spec/terminal-psi/structural_access.md, Store vocabulary). Checking
        records that value in exactly one of three places: a scalar computation
        rooted at the value's own role coordinate, a bound pure scalar expression,
        or -- when the right-hand side is the call this same statement performs --
        the call's result in the dense scalar namespace.
    */
    use super::super::{
        CheckFacts, CheckedScalarExpression, CheckedScalarExpressionRole, ExpressionNode,
        PrimitiveType,
    };
    use crate::execution::terminal_unit::control::LocalConstructionTrace;
    use crate::values::scalar_expression_type;

    /* Where the assignment's right-hand side was evaluated. */
    #[derive(Clone, Copy)]
    pub(in crate::execution::terminal_unit) enum AssignmentSource {
        /* An authored expression with its own checked value facts. */
        Authored,
        /* The completed, exactly typed structural value of this assignment. */
        StructuralValue {
            binding_ordinal: u32,
            result_type: typed_trees::types::TypeReferenceHandle,
        },
        /*
            The SSA result of the scalar call this same statement performs, by its
            dense scalar-namespace position and declared result type.
        */
        CallResult {
            position: u32,
            primitive_type: PrimitiveType,
        },
    }

    /*
        Select the scalar value an authored assignment stores into a leaf of
        `primitive_type`, then apply the value policy.
    */
    pub(super) fn assignment_value(
        occurrence: &super::Occurrence<'_, '_>,
        primitive_type: PrimitiveType,
    ) -> Option<checked_trees::CheckedScalarStoreValue> {
        let super::Occurrence {
            program,
            facts,
            machine,
            state,
            statement_index,
            assignment,
            source,
            trace,
        } = *occurrence;
        let value = match source {
            AssignmentSource::StructuralValue { .. } => return None,
            AssignmentSource::CallResult {
                position,
                primitive_type: result_type,
            } => {
                trace.phase("structural field store: pure source: call result: result type");
                if result_type != primitive_type {
                    return None;
                }
                trace.phase("structural field store: pure source: call result: authored call");
                if !matches!(
                    program.expression_table.expression(assignment.value),
                    ExpressionNode::Call(_)
                ) {
                    return None;
                }
                checked_trees::CheckedScalarStoreValue::ScalarResult { position }
            }
            AssignmentSource::Authored => authored_value(
                facts,
                machine,
                state,
                statement_index,
                CheckedScalarExpressionRole::AssignmentValue,
                assignment.value,
                primitive_type,
                trace,
            )?,
        };
        trace.phase("structural field store: pure source: value shape");
        admits(primitive_type, &value).then_some(value)
    }

    /*
        The checked value at one role coordinate: a computation rooted there, or
        otherwise the bound pure expression. A computation that exists but belongs
        to a different machine, authored expression or carrier is a custody
        mismatch, not permission to fall back to another source.
    */
    pub(super) fn authored_value(
        facts: &CheckFacts,
        machine: &typed_trees::machine::Machine,
        state: &typed_trees::state::State,
        statement_index: u32,
        role: CheckedScalarExpressionRole,
        authored: typed_trees::expression::ExpressionHandle,
        primitive_type: PrimitiveType,
        trace: &LocalConstructionTrace,
    ) -> Option<checked_trees::CheckedScalarStoreValue> {
        let computations = &facts.values.scalar_computations;
        if let Some(root) = computations.root_at(state.symbol, statement_index, role) {
            trace.phase("structural field store: computation source");
            if !computations.is_authored_root(root, machine.symbol, authored, primitive_type)
                || facts
                    .values
                    .scalar_expressions
                    .expression_at(state.symbol, statement_index, role)
                    .is_some()
            {
                return None;
            }
            return Some(checked_trees::CheckedScalarStoreValue::Computation(
                root.root,
            ));
        }
        trace.phase("structural field store: pure source: scalar expression row");
        let (binding, value) = facts.values.scalar_expressions.bound_expression_at(
            state.symbol,
            statement_index,
            role,
        )?;
        trace.phase("structural field store: pure source: value shape");
        if binding.expression != authored || scalar_expression_type(value) != Some(primitive_type) {
            return None;
        }
        Some(checked_trees::CheckedScalarStoreValue::Pure(value.clone()))
    }

    /*
        Floating stores forward existing bits: an authored IEEE literal or an
        already-defined scalar (parameter, local, or a leaf read out of existing
        structural storage). A floating operation in a pure expression has no
        selected provider, so it is refused. A checked computation retains its own
        selected operations and call correspondence regardless of whether the
        destination is borrowed parameter storage or an owned local. Destination
        authority is checked independently by the store planner.
    */
    fn admits(
        primitive_type: PrimitiveType,
        value: &checked_trees::CheckedScalarStoreValue,
    ) -> bool {
        if !matches!(primitive_type, PrimitiveType::F32 | PrimitiveType::F64) {
            return true;
        }
        match value {
            checked_trees::CheckedScalarStoreValue::Pure(expression) => matches!(
                expression,
                CheckedScalarExpression::IeeeFloatLiteral { .. }
                    | CheckedScalarExpression::Parameter { .. }
                    | CheckedScalarExpression::Local { .. }
                    | CheckedScalarExpression::StructuralParameterField { .. }
                    | CheckedScalarExpression::StructuralParameterIndexedRead { .. }
            ),
            checked_trees::CheckedScalarStoreValue::Computation(_) => true,
            checked_trees::CheckedScalarStoreValue::ScalarResult { .. } => true,
        }
    }

    /*
        The retained runtime selector of an indexed store: the bound
        `AssignmentIndex` expression of exactly this authored index, of an integer
        carrier.
    */
    pub(super) fn runtime_index(
        facts: &CheckFacts,
        state: &typed_trees::state::State,
        statement_index: u32,
        index: typed_trees::expression::ExpressionHandle,
    ) -> Option<CheckedScalarExpression> {
        let (binding, value) = facts.values.scalar_expressions.bound_expression_at(
            state.symbol,
            statement_index,
            CheckedScalarExpressionRole::AssignmentIndex { depth: 0 },
        )?;
        (binding.expression == index
            && scalar_expression_type(value).is_some_and(|primitive| primitive.is_integer()))
        .then(|| value.clone())
    }

    /*
        The `u8` a byte store writes: the same scalar source selection as every
        other store, including a computation that owns the value's calls.
    */
    pub(super) fn byte_value(
        occurrence: &super::Occurrence<'_, '_>,
    ) -> Option<checked_trees::CheckedScalarStoreValue> {
        assignment_value(occurrence, PrimitiveType::U8)
    }
}
mod view_stores {
    /*
        One scalar store through a representation view: `view = v` where `view`
        is an immutable `&mut`/`&write` local bound by `&mut place as &mut T`.

        The checked plan stays one `RepresentationView` store leaf per authored
        statement, the store mirror of `CheckedScalarExpression::RepresentationView`.
        The destination's rejoined place decides the leaf family:

        - a whole primitive field of the same integer width (`&mut self.x as
          &mut u8` over an `i8` field, or a direct field of a record view whose
          source leaf has the other sign) lowers to one primitive store whose
          value is retagged to the leaf's declared carrier at emission;
        - one `u8` element selected by `FixedIndex`/`RuntimeIndex` below a
          bounded-owned byte field (`&mut self.buf[k] as &mut u32`) lowers to one
          primitive store per little-endian footprint byte, each byte's `index +
          j` re-proving its own bound against the field's declared capacity —
          the same contract the view's read side composes from.
    */
    use super::super::{
        CheckedUnitEffectOperationPlan, CheckedUnitStructuralPathSegment as PathSegment,
        PrimitiveType,
    };
    use super::destination::Destination;
    use super::value;
    use crate::execution::terminal_unit::calls::borrowed_index_segment;
    use crate::execution::terminal_unit::types::{byte_sequence_carrier, terminal_field_identity};

    /*
        The checked row for `view = v`, or `None` when the leaf family stays
        with another store lane. The leaf's primitive width selects the arm: a
        view equal to its leaf writes one whole store; a view wider than its
        leaf decomposes only over a bounded-owned `u8` carrier — a borrowed
        view's live extent has no declared footprint bound to distribute the
        per-byte obligations against, so it stays refused.
    */
    pub(super) fn plan(
        site: &super::Site<'_, '_, '_>,
        view: PrimitiveType,
    ) -> Option<CheckedUnitEffectOperationPlan> {
        let super::Occurrence {
            program,
            state,
            statement_index,
            ..
        } = site.occurrence;
        let destination = site.destination;
        let store_root = checked_trees::CheckedStoreRoot::Parameter {
            position: destination.root.parameter_position()?,
        };
        let view_bytes = store_width(view)?;
        let (leaf, carriers) = destination.place.segments.split_last()?;
        let (path, element) = match leaf {
            facts::PlaceSegment::Field { symbol } => {
                let carrier = destination.leaf_carrier(program, carriers)?;
                let field = carrier.field(program, *symbol)?;
                let element = program.primitive_type_reference(field.type_reference)?;
                if store_width(element)? != view_bytes {
                    return None;
                }
                let mut path = carrier.path;
                path.push(PathSegment::Field(terminal_field_identity(
                    program, *symbol,
                )?));
                (path, element)
            }
            facts::PlaceSegment::FixedIndex { index } => {
                let mut path = byte_field(program, destination, carriers)?;
                path.push(PathSegment::FixedIndex(u64::try_from(*index).ok()?));
                (path, PrimitiveType::U8)
            }
            facts::PlaceSegment::Index { expression } => {
                let mut path = byte_field(program, destination, carriers)?;
                let segment = borrowed_index_segment(
                    program,
                    state.symbol,
                    usize::try_from(statement_index).ok()?,
                    *expression,
                );
                let segment = segment?;
                path.push(segment);
                (path, PrimitiveType::U8)
            }
            _ => {
                return None;
            }
        };
        if element != PrimitiveType::U8 && store_width(element)? != view_bytes {
            return None;
        }
        let value = value::assignment_value(&site.occurrence, view);
        let value = value?;
        match value {
            checked_trees::CheckedScalarStoreValue::Computation(_) => return None,
            checked_trees::CheckedScalarStoreValue::Pure(_)
            | checked_trees::CheckedScalarStoreValue::ScalarResult { .. } => {}
        }
        Some(CheckedUnitEffectOperationPlan::new_store(
            statement_index,
            store_root,
            path,
            checked_trees::CheckedStoreLeaf::RepresentationView { view },
            checked_trees::CheckedStoreValue::assigned(value),
        ))
    }

    /*
        The bounded-owned byte field one indexed recast leaf selects inside.
        `carriers` ends at the field segment; the checked path it yields is the
        record prefix only — the field's own segment is appended here, and the
        caller appends the literal or runtime element segment after it.
    */
    fn byte_field(
        program: &dyn ::validation::CheckingInput,
        destination: &Destination<'_>,
        carriers: &[facts::PlaceSegment],
    ) -> Option<Vec<PathSegment>> {
        let (facts::PlaceSegment::Field { symbol }, prefix) = carriers.split_last()? else {
            return None;
        };
        let carrier = destination.leaf_carrier(program, prefix)?;
        let field = carrier.field(program, *symbol)?;
        let mut path = carrier.path;
        let bounded = matches!(
            byte_sequence_carrier(program, field.type_reference, &[]),
            Some(checked_trees::CheckedByteSequenceCarrier::BoundedOwned { .. })
        ) || matches!(
            program.type_reference_table.type_reference(field.type_reference),
            typed_trees::types::TypeReferenceNode::FixedArray { element_type, .. }
                if program.primitive_type_reference(*element_type) == Some(PrimitiveType::U8)
                    && program.declared_array_extent(field.type_reference).is_some()
        );
        if !bounded {
            return None;
        }
        path.push(PathSegment::Field(terminal_field_identity(
            program, *symbol,
        )?));
        Some(path)
    }

    /*
        The storage width of one primitive carrier the store decomposes
        against: a Boolean rides one byte beside the integers, so the view and
        the leaf compare by footprint rather than by arithmetic family. Wider
        than the element's own width the little-endian byte decomposition still
        names only a bounded-owned `u8` carrier.
    */
    fn store_width(primitive_type: PrimitiveType) -> Option<u64> {
        Some(match primitive_type {
            PrimitiveType::Bool | PrimitiveType::U8 | PrimitiveType::I8 => 1,
            PrimitiveType::U16 | PrimitiveType::I16 => 2,
            PrimitiveType::U32 | PrimitiveType::I32 => 4,
            PrimitiveType::U64 | PrimitiveType::I64 => 8,
            _ => return None,
        })
    }
}

/*
    One assignment statement and the checked value source it stores: the
    coordinate every store lane reads. Each lane used to restate it as the same
    eight positional arguments; one owner carries it instead.
*/
#[derive(Clone, Copy)]
pub(super) struct Occurrence<'a, 'program> {
    pub(super) program: &'program dyn ::validation::CheckingInput,
    pub(super) facts: &'program CheckFacts,
    pub(super) machine: &'program typed_trees::machine::Machine,
    pub(super) state: &'program typed_trees::state::State,
    pub(super) statement_index: u32,
    pub(super) assignment: &'program typed_trees::statement::TableAssignment,
    pub(super) source: AssignmentSource,
    pub(super) trace: &'a LocalConstructionTrace,
}

/*
    The occurrence together with the destination dispatch resolved for it: the
    leaf family a lane judges is a property of the resolved place, not of the
    statement, so the two travel as one site.
*/
#[derive(Clone, Copy)]
struct Site<'a, 'program, 'destination> {
    occurrence: Occurrence<'a, 'program>,
    destination: &'a Destination<'destination>,
}

/*
    Capture scalar storage through the same destination resolution as its
    store. A recast's physical carrier and the authored value's view carrier
    are distinct: only the latter belongs to the subsequent conversion.
*/
#[allow(clippy::too_many_arguments)]
pub(super) fn scalar_capture_destination(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    shapes: &mut ShapeCollector<'_>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural: &[CheckedUnitStructuralParameterPlan],
    scalar: &[CheckedStructuralScalarParameterPlan],
    storage: &[checked_trees::CheckedUnitParameterStoragePlan],
    binders: &[(SymbolHandle, String)],
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
    trace: &LocalConstructionTrace,
) -> Option<checked_trees::CheckedUnitStructuralArgumentPlan> {
    let destination = destination::resolve(
        program,
        facts,
        machine,
        state,
        StoreRoots {
            structural,
            scalar,
            storage,
        },
        statement_index,
        assignment,
        trace,
    )?;
    let projection = destination.storage_projection(program)?;
    program.primitive_type_reference(projection.type_reference)?;
    destination.exclusive_authority(
        program,
        state,
        StoreRoots {
            structural,
            scalar,
            storage,
        },
        trace,
    )?;
    let source = match destination.root {
        Root::Parameter { plan, .. } => {
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index: u32::try_from(
                    structural
                        .iter()
                        .position(|row| row.position == plan.position)?,
                )
                .ok()?,
            }
        }
        Root::Local { symbol } => {
            checked_trees::CheckedUnitStructuralArgumentSourcePlan::StructuralLocal { symbol }
        }
    };
    Some(checked_trees::CheckedUnitStructuralArgumentPlan {
        projection_evaluations: Default::default(),
        source,
        path: projection.path,
        type_identity: shapes.add_stored_type(projection.type_reference, binders, &[])?,
        access: CheckedStructuralAccess::Owned,
    })
}

/*
    The ordered stores after the sequence's verified construction prefix.
    Each assignment composes its independently checked destination and value.
*/
pub(super) fn build_structural_scalar_field_store_sequence_traced(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    scalar_parameters: &[CheckedStructuralScalarParameterPlan],
    parameter_storage: &[checked_trees::CheckedUnitParameterStoragePlan],
    construction_statement_count: usize,
    trace: &LocalConstructionTrace,
) -> Option<Vec<CheckedUnitEffectOperationPlan>> {
    let statements = program.statement_table.statements(state.statement_nodes);
    if !statements
        .iter()
        .skip(construction_statement_count)
        .any(|statement| matches!(statement, StatementNode::Assignment(_)))
    {
        return Some(Vec::new());
    }
    let roots = StoreRoots {
        structural: structural_parameters,
        scalar: scalar_parameters,
        storage: parameter_storage,
    };
    let mut stores = Vec::new();
    let execution_order = program
        .statement_table
        .admitted_execution_order(state.statement_nodes);
    let start = construction_statement_count;
    for statement_index in execution_order.indices()[start..].iter().copied() {
        let statement = &statements[statement_index];
        let Some(assignment) = statement.as_assignment() else {
            continue;
        };
        if validation::atomic_assignment_carrier(program, assignment).is_some() {
            continue;
        }
        if assignment_type(program, machine, state, assignment).is_some() {
            continue;
        }
        let statement_index = u32::try_from(statement_index).ok()?;
        trace.phase("scalar field store sequence: assignment store");
        trace.statement(Some(statement_index));
        let baseline = trace.mark();
        if let Some(planned) = plan_assignment(
            program,
            facts,
            machine,
            state,
            roots,
            statement_index,
            assignment,
            AssignmentSource::Authored,
            trace,
        ) {
            stores.extend(planned);
            continue;
        }
        if matches!(
            program.expression_table.expression(assignment.value),
            ExpressionNode::Call(_)
        ) || facts
            .values
            .structural_values
            .root_for_expression(state.symbol, statement_index, assignment.value)
            .is_some()
        {
            trace.restore(&baseline);
            continue;
        }
        trace.statement(Some(statement_index));
        return None;
    }
    Some(stores)
}

/*
    The store consuming the scalar result of the call this same statement
    performs: the same composition, with the call's SSA result as the value.
*/
pub(super) fn build_structural_call_result_field_store(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    scalar_parameters: &[CheckedStructuralScalarParameterPlan],
    parameter_storage: &[checked_trees::CheckedUnitParameterStoragePlan],
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
    (position, primitive_type): (u32, PrimitiveType),
    trace: &LocalConstructionTrace,
) -> Option<CheckedUnitEffectOperationPlan> {
    single_parameter_store(
        program,
        facts,
        machine,
        state,
        structural_parameters,
        scalar_parameters,
        parameter_storage,
        statement_index,
        assignment,
        AssignmentSource::CallResult {
            position,
            primitive_type,
        },
        trace,
    )
}

/*
    A completed byte value uses the destination's ordinary replacement:
    owned contents copy bytes and length; shared fields copy the descriptor.
    Destination selection is independent of how the RHS value was produced.
*/
pub(super) fn build_completed_byte_value_store(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    scalar_parameters: &[CheckedStructuralScalarParameterPlan],
    parameter_storage: &[checked_trees::CheckedUnitParameterStoragePlan],
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
    binding_ordinal: u32,
    result_type: typed_trees::types::TypeReferenceHandle,
    trace: &LocalConstructionTrace,
) -> Option<CheckedUnitEffectOperationPlan> {
    single_parameter_store(
        program,
        facts,
        machine,
        state,
        structural_parameters,
        scalar_parameters,
        parameter_storage,
        statement_index,
        assignment,
        AssignmentSource::StructuralValue {
            binding_ordinal,
            result_type,
        },
        trace,
    )
    .filter(|store| {
        matches!(
            store,
            CheckedUnitEffectOperationPlan::Store(checked_trees::CheckedStorePlan {
                value: checked_trees::CheckedStoreValue::Bytes(_),
                ..
            })
        )
    })
}

/*
    The scalar coordinate one carrier-path runtime index names at this
    statement. Every `CheckedRuntimeIndex` variant resolves to the scalar
    expression the same selector spells as a value: an assignment selector
    keeps its bound row, a parameter or local its dense position's declared
    type, and a field snapshot its leaf type below the declared root.
*/
fn runtime_index_expression(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    state: &typed_trees::state::State,
    statement_index: u32,
    index: &checked_trees::CheckedRuntimeIndex,
) -> Option<checked_trees::CheckedScalarExpression> {
    match index {
        checked_trees::CheckedRuntimeIndex::StorageRead { symbol } => {
            let reference = program
                .state_parameter_by_symbol(state, *symbol)
                .filter(|parameter| parameter.is_mutable)
                .map(|parameter| parameter.type_reference)
                .or_else(|| {
                    program
                        .state_local_declaration(state, *symbol)
                        .filter(|(ordinal, local)| {
                            *ordinal < statement_index as usize && local.is_mutable
                        })
                        .map(|(_, local)| local.type_reference)
                })?;
            Some(checked_trees::CheckedScalarExpression::StorageRead {
                symbol: *symbol,
                primitive_type: program.primitive_type_reference(reference)?,
            })
        }
        checked_trees::CheckedRuntimeIndex::AssignmentIndex { depth } => Some(
            facts
                .values
                .scalar_expressions
                .bound_expression_at(
                    state.symbol,
                    statement_index,
                    CheckedScalarExpressionRole::AssignmentIndex { depth: *depth },
                )?
                .1
                .clone(),
        ),
        checked_trees::CheckedRuntimeIndex::Parameter { position } => {
            let position = usize::try_from(*position).ok()?;
            let parameter = program
                .state_parameters(state)
                .iter()
                .filter(|parameter| occupies_scalar_position(program, parameter))
                .nth(position)?;
            Some(checked_trees::CheckedScalarExpression::Parameter {
                position,
                primitive_type: program.primitive_type_reference(parameter.type_reference)?,
            })
        }
        checked_trees::CheckedRuntimeIndex::Local { position } => {
            let position = usize::try_from(*position).ok()?;
            let mut dense = 0usize;
            for statement in program.statement_table.statements(state.statement_nodes) {
                let Some(local) = statement.as_local_data() else {
                    continue;
                };
                let primitive_type = program.primitive_type_reference(local.type_reference);
                let Some(primitive_type) = primitive_type else {
                    continue;
                };
                if dense == position {
                    return Some(checked_trees::CheckedScalarExpression::Local {
                        position,
                        primitive_type,
                    });
                }
                dense += 1;
            }
            None
        }
        checked_trees::CheckedRuntimeIndex::StructuralField(selector) => {
            let (_, _, leaf, _) = resolve_structural_parameter_path(
                program,
                program.state_parameters(state),
                selector.parameter_position,
                &selector.path,
            )?;
            Some(
                checked_trees::CheckedScalarExpression::StructuralParameterField {
                    observation: Default::default(),
                    parameter_position: selector.parameter_position,
                    path: selector.path.clone(),
                    primitive_type: program.primitive_type_reference(leaf)?,
                },
            )
        }
    }
}

/*
    Compose one assignment: resolve the destination, then let the leaf's
    declared type select the store its value joins.
*/
fn plan_assignment(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    roots: StoreRoots<'_>,
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
    source: AssignmentSource,
    trace: &LocalConstructionTrace,
) -> Option<Vec<CheckedUnitEffectOperationPlan>> {
    if validation::assignment_stored_value_type(program, machine, state, assignment)
        .is_some_and(|installed| validation::reference_shell(program, installed).is_some())
        && place_at(
            program,
            state.symbol,
            usize::try_from(statement_index).ok()?,
            assignment.target,
        )
        .is_some_and(|place| place.segments.is_empty())
    {
        return None;
    }
    let destination = destination::resolve(
        program,
        facts,
        machine,
        state,
        roots,
        statement_index,
        assignment,
        trace,
    )?;
    let site = Site {
        occurrence: Occurrence {
            program,
            facts,
            machine,
            state,
            statement_index,
            assignment,
            source,
            trace,
        },
        destination: &destination,
    };
    if destination
        .storage_projection(program)
        .is_some_and(|projection| {
            projection.element_view_index.is_some()
                && program
                    .primitive_type_reference(projection.type_reference)
                    .is_some()
        })
    {
        destination.exclusive_authority(program, state, roots, trace)?;
        return element_stores::store(&site).map(|store| vec![store]);
    }
    if destination.place.segments.is_empty()
        && destination.dynamic_index.is_some()
        && (matches!(
            program
                .type_reference_table
                .type_reference(validation::unwrapped_type_reference(
                    program,
                    destination.root_type
                )?),
            TypeReferenceNode::Slice { .. }
        ) || matches!(
            byte_sequence_carrier(program, destination.root_type, &[]),
            Some(checked_trees::CheckedByteSequenceCarrier::BoundedOwned { .. })
        ) || match destination.root {
            destination::Root::Parameter { parameter, .. } => matches!(
                byte_sequence_carrier(program, parameter.type_reference, &[]),
                Some(checked_trees::CheckedByteSequenceCarrier::BoundedView { .. })
            ),
            destination::Root::Local { .. } => false,
        })
    {
        return byte_stores::byte_write(&site).map(|write| vec![write]);
    }
    if destination.place.segments.is_empty()
        && destination.dynamic_index.is_none()
        && let Some(store) = byte_stores::whole_store(&site)
    {
        return Some(vec![store]);
    }
    destination.exclusive_authority(program, state, roots, trace)?;
    if let Some(view) = destination.recast_view {
        trace.phase("structural field store: representation view");
        return view_stores::plan(&site, view).map(|store| vec![store]);
    }
    if let Some(store) = primitive::store_at(&site) {
        return Some(vec![store]);
    }
    trace.phase("structural field store: carrier path");
    let (leaf, carriers) = destination.place.segments.split_last()?;
    let facts::PlaceSegment::Field { symbol } = leaf else {
        return None;
    };
    let carrier = destination.leaf_carrier(program, carriers)?;
    trace.phase("structural field store: field owner record");
    let field = carrier.field(program, *symbol)?;
    trace.phase("structural field store: byte sequence carrier");
    let byte_leaf = match byte_sequence_carrier(program, field.type_reference, &[]) {
        Some(checked_trees::CheckedByteSequenceCarrier::BoundedOwned { capacity }) => {
            Some(Some(capacity))
        }
        Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView { .. })
            if byte_stores::field_view_is_mutable(program, field.type_reference) =>
        {
            Some(None)
        }
        Some(checked_trees::CheckedByteSequenceCarrier::BoundedView { capacity, .. })
            if byte_stores::field_view_is_mutable(program, field.type_reference) =>
        {
            Some(Some(capacity))
        }
        _ => None,
    };
    let static_carrier = !carrier
        .path
        .iter()
        .any(|segment| matches!(segment, CheckedUnitStructuralPathSegment::RuntimeIndex(_)));
    if !static_carrier
        && matches!(destination.root, Root::Local { .. })
        && !destination.loan_type.is_valid()
    {
        return None;
    }
    if let Some(capacity) = byte_leaf {
        if !static_carrier {
            return None;
        }
        return byte_stores::field_store(&site, carrier.path, field, capacity)
            .map(|store| vec![store]);
    }
    if is_shared_view_field(program, field.type_reference) {
        return descriptor_store::store(
            &site,
            roots,
            descriptor_store::DescriptorSlot {
                path: carrier.path,
                type_reference: field.type_reference,
                field,
            },
        )
        .map(|store| vec![store]);
    }
    trace.phase("structural field store: borrowed view field");
    if destination::crosses_reference(program, field.type_reference) {
        return None;
    }
    trace.phase("structural field store: scalar field type");
    if destination.dynamic_index.is_some() {
        return None;
    }
    if program
        .primitive_type_reference(field.type_reference)
        .is_some()
    {
        let primitive_type = scalar_leaf(program, field)?;
        let value = value::assignment_value(&site.occurrence, primitive_type)?;
        let mut path = carrier.path;
        path.push(CheckedUnitStructuralPathSegment::Field(
            terminal_field_identity(program, field.symbol)?,
        ));
        return Some(vec![CheckedUnitEffectOperationPlan::new_store(
            statement_index,
            destination.root.destination(),
            path,
            checked_trees::CheckedStoreLeaf::ScalarField {
                primitive_type,
                range_decision: facts.proof.integer_assignment_at(
                    machine.symbol,
                    state.symbol,
                    statement_index as usize,
                    assignment.target,
                    assignment.value,
                ),
            },
            checked_trees::CheckedStoreValue::assigned(value),
        )]);
    }
    let field_data =
        validation::field_domains::data_definition_for_field_type(program, field.type_reference)?;
    if plain_record(field_data, program) {
        return None;
    }
    trace.phase("structural field store: case field type");
    if !static_carrier {
        return None;
    }
    case_field_store(&site, roots, carrier.path.clone(), field, field_data)
        .or_else(|| case_literal_field_store(&site, carrier.path, field, field_data))
        .map(|store| vec![store])
}

/*
    The shared field retention judgment keeps every restriction independently
    of whether the destination is a parameter or an owned local. Atomic storage
    retains a different access contract and cannot become an ordinary store.
*/
fn scalar_leaf(
    program: &dyn ::validation::CheckingInput,
    field: &typed_trees::data::DataField,
) -> Option<PrimitiveType> {
    let primitive = program.primitive_type_reference(field.type_reference)?;
    let retained = retain_scalar_field(program, field.type_reference, &[], primitive, None)?;
    matches!(
        retained,
        checked_trees::CheckedUnitStructuralFieldType::Scalar(_)
            | checked_trees::CheckedUnitStructuralFieldType::BoundedInteger(_)
            | checked_trees::CheckedUnitStructuralFieldType::RefinedScalar { .. }
    )
    .then_some(primitive)
}

/*
    `place.<sum field> = <parameter path>`: an initialized unrestricted-sum
    field overwritten by a whole parameter or a member read below one, both of
    the same declared type. The write copies whole, moves nothing, and needs no
    carrier borrow window -- call results, payload sums, records and affine
    carriers keep declining.
*/
fn case_field_store(
    site: &Site<'_, '_, '_>,
    roots: StoreRoots<'_>,
    carrier_path: Vec<CheckedUnitStructuralPathSegment>,
    field: &typed_trees::data::DataField,
    field_data: &typed_trees::data::DataDefinition,
) -> Option<CheckedUnitEffectOperationPlan> {
    let Occurrence {
        program,
        state,
        statement_index,
        assignment,
        source,
        ..
    } = site.occurrence;
    let destination = site.destination;
    let AssignmentSource::Authored = source else {
        return None;
    };
    let structural_parameters = roots.structural;
    if !unrestricted_sum(field_data, program) {
        return None;
    }
    let value_place = place_at(
        program,
        state.symbol,
        statement_index as usize,
        assignment.value,
    )?;
    let value_root = value_place.root.as_symbol()?;
    let source_parameters = program.state_parameters(state);
    let value_position =
        typed_trees::signature::state_parameter_position(source_parameters, value_root)?;
    let _value_state_parameter = source_parameters.get(value_position)?;

    let ProjectedArgumentPath {
        storage_type: projected,
        path: value_path,
        ..
    } = projected_argument_path(
        program,
        state.symbol,
        statement_index as usize,
        &value_place,
        None,
        None,
    )?;
    let same_named_type = matches!(
        (
            program
                .type_reference_table
                .type_reference(field.type_reference),
            program.type_reference_table.type_reference(projected),
        ),
        (
            TypeReferenceNode::Named { symbol: field_name, .. },
            TypeReferenceNode::Named { symbol: value_name, .. }
        ) if field_name == value_name
    );
    if !same_named_type {
        return None;
    }
    let (value_index, value_plan) = structural_parameters
        .iter()
        .enumerate()
        .find(|(_, parameter)| parameter.position as usize == value_position)?;
    if !matches!(
        value_plan.access,
        CheckedStructuralAccess::Owned | CheckedStructuralAccess::SharedBorrow
    ) || value_plan.multiplicity != Multiplicity::Unrestricted
        || !value_plan.qualifications.is_empty()
        || !value_plan.projected_qualifications.is_empty()
    {
        return None;
    }
    sum_field_store(
        program,
        destination,
        carrier_path,
        field,
        statement_index,
        checked_trees::CheckedStoreValue::Place(checked_trees::CheckedUnitStructuralArgumentPlan {
            projection_evaluations: Default::default(),
            source: checked_trees::CheckedUnitStructuralArgumentSourcePlan::Parameter {
                parameter_index: u32::try_from(value_index).ok()?,
            },
            path: value_path,
            type_identity: base_type_identity(program, projected, &[], &[])?,
            access: value_plan.access,
        }),
    )
}

/*
    `place.field = Sum::Case { .. }` — the literal carries no custody root, so
    the store itself establishes the construction into the field's storage:
    the tag, every authored scalar operand at its declared carrier, and a zero
    for each common field the literal leaves unnamed. The destination still
    proves the same owned place, write-frame and static-carrier custody the
    whole-place store proves; only the source's shape differs — construction
    operands where a copy named a structural place.
*/
fn case_literal_field_store(
    site: &Site<'_, '_, '_>,
    carrier_path: Vec<CheckedUnitStructuralPathSegment>,
    field: &typed_trees::data::DataField,
    field_data: &typed_trees::data::DataDefinition,
) -> Option<CheckedUnitEffectOperationPlan> {
    let Occurrence {
        program,
        facts,
        state,
        statement_index,
        assignment,
        source,
        ..
    } = site.occurrence;
    let destination = site.destination;
    if !matches!(source, AssignmentSource::Authored) {
        return None;
    }
    if !closed_sum(field_data, program) {
        return None;
    }
    let constructor =
        validation::structural_case_constructor(program, assignment.value, field.type_reference)?;
    if !constructor.omitted_fields.is_empty() {
        return None;
    }
    if program.normalized_type_identity(constructor.type_reference)
        != program.normalized_type_identity(field.type_reference)
    {
        return None;
    }
    let variant = program
        .data_members(field_data)
        .iter()
        .find_map(|member| match member {
            DataMember::Variant(variant) if variant.symbol == constructor.case => Some(variant),
            _ => None,
        })?;
    let declarations = program.data_payload_fields(variant);
    let commons = program
        .data_members(field_data)
        .iter()
        .filter_map(|member| match member {
            DataMember::Field(field) => Some(field),
            DataMember::Variant(_) => None,
        })
        .collect::<Vec<_>>();
    let mut fields = Vec::with_capacity(constructor.fields.len());
    for (ordinal, (field_symbol, source_expression, declared)) in
        constructor.fields.iter().enumerate()
    {
        let primitive_type = program.primitive_type_reference(*declared)?;
        let identity = declarations
            .iter()
            .find(|declaration| declaration.symbol == *field_symbol)
            .map(|declaration| {
                checked_trees::CheckedMemberIdentity::of_member(
                    declaration.identity,
                    declaration.name.clone(),
                )
            })
            .or_else(|| {
                commons
                    .iter()
                    .find(|declaration| declaration.symbol == *field_symbol)
                    .map(|declaration| {
                        checked_trees::CheckedMemberIdentity::of_member(
                            declaration.identity,
                            declaration.name.clone(),
                        )
                    })
            })?;
        let field_ordinal = u32::try_from(ordinal).ok()?;
        let (binding, expression) = facts.values.scalar_expressions.bound_expression_at(
            state.symbol,
            statement_index,
            CheckedScalarExpressionRole::StructuralValueField {
                expression: assignment.value,
                field_ordinal,
            },
        )?;
        if binding.expression != *source_expression || binding.destination.is_valid() {
            return None;
        }
        fields.push(checked_trees::CheckedScalarCaseFieldPlan {
            field_ordinal,
            field_identity: identity,
            primitive_type,
            expression: expression.clone(),
        });
    }
    sum_field_store(
        program,
        destination,
        carrier_path,
        field,
        statement_index,
        checked_trees::CheckedStoreValue::CaseLiteral(checked_trees::CheckedCaseLiteralValuePlan {
            literal: assignment.value,
            case_identity: checked_trees::CheckedMemberIdentity::of_member(
                variant.identity,
                variant.name.clone(),
            ),
            type_identity: base_type_identity(program, field.type_reference, &[], &[])?,
            fields,
        }),
    )
}

/*
    One whole sum field's store: the exact field below the destination's
    carrier, receiving a copied place or an established case literal.
*/
fn sum_field_store(
    program: &dyn ::validation::CheckingInput,
    destination: &Destination<'_>,
    mut carrier_path: Vec<CheckedUnitStructuralPathSegment>,
    field: &typed_trees::data::DataField,
    statement_index: u32,
    value: checked_trees::CheckedStoreValue,
) -> Option<CheckedUnitEffectOperationPlan> {
    carrier_path.push(CheckedUnitStructuralPathSegment::Field(
        terminal_field_identity(program, field.symbol)?,
    ));
    Some(CheckedUnitEffectOperationPlan::new_store(
        statement_index,
        destination.root.destination(),
        carrier_path,
        checked_trees::CheckedStoreLeaf::SumField,
        value,
    ))
}

/*
    A closed payload-free pure-sum data declaration: CheckedShape supply, no
    lifetimes or type parameters, no quotient or where facts, unrestricted
    multiplicity, and every member a payload-free case. Copying a whole owned
    place into such a field moves nothing, so the store needs no carrier
    borrow window.
*/
fn unrestricted_sum(
    data: &typed_trees::data::DataDefinition,
    program: &dyn ::validation::CheckingInput,
) -> bool {
    closed_sum(data, program)
        && typed_trees::data::DataDefinition::shape_kind_from_members(program.data_members(data))
            == DataShapeKind::Enum
        && program.data_members(data).iter().all(|member| {
            let Some(variant) = member.as_variant() else {
                return false;
            };
            program.data_payload_fields(variant).is_empty()
        })
}

/*
    The closed Unrestricted family both sum stores share: CheckedShape supply,
    no lifetimes or type parameters, no quotient or where facts, and no zero
    gate — the whole-place arm adds its payload-free Enum shape on top.
*/
fn closed_sum(
    data: &typed_trees::data::DataDefinition,
    program: &dyn ::validation::CheckingInput,
) -> bool {
    data.supply_mode == language_semantics::DataSupplyMode::CheckedShape
        && data.lifetime_parameters.is_empty()
        && program.data_type_parameters(data).is_empty()
        && data.quotient.is_none()
        && data.requires_facts.is_empty()
        && !data.zero_gated
        && data.properties.multiplicity == language_semantics::Multiplicity::Unrestricted
}

/*
    Whether a declared slot type is a shared `&[u8]` view, through any domain
    constraint shells: the slot stores a relocatable descriptor.
*/
pub(super) fn is_shared_view_field(
    program: &dyn ::validation::CheckingInput,
    type_reference: typed_trees::types::TypeReferenceHandle,
) -> bool {
    matches!(
        byte_sequence_carrier(program, type_reference, &[]),
        Some(checked_trees::CheckedByteSequenceCarrier::BorrowedView { .. })
    ) && !byte_stores::field_view_is_mutable(program, type_reference)
}

/*
    Plan an assignment against the parameter roster and keep its single store.
    Each call-result lane supplies only its `AssignmentSource` spelling; the
    byte lane further narrows the produced plan kind.
*/
#[allow(clippy::too_many_arguments)]
fn single_parameter_store(
    program: &dyn ::validation::CheckingInput,
    facts: &CheckFacts,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    structural_parameters: &[CheckedUnitStructuralParameterPlan],
    scalar_parameters: &[CheckedStructuralScalarParameterPlan],
    parameter_storage: &[checked_trees::CheckedUnitParameterStoragePlan],
    statement_index: u32,
    assignment: &typed_trees::statement::TableAssignment,
    source: AssignmentSource,
    trace: &LocalConstructionTrace,
) -> Option<CheckedUnitEffectOperationPlan> {
    let [store] = <[_; 1]>::try_from(plan_assignment(
        program,
        facts,
        machine,
        state,
        StoreRoots {
            structural: structural_parameters,
            scalar: scalar_parameters,
            storage: parameter_storage,
        },
        statement_index,
        assignment,
        source,
        trace,
    )?)
    .ok()?;
    Some(store)
}
