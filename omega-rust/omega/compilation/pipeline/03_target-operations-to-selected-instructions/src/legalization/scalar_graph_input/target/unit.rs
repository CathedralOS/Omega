//! Replay of one ordered target Unit operation against the abstract
//! operation it was lowered from, as part of legalization's input check of a
//! scalar graph.
//!
//! `validate_operation` is the entry; `control_flow::validate` calls it for
//! each non-terminator row of each block in order, with the scalar sources
//! available so far and the block's reference custody state, and applies
//! the operation to that custody afterwards. A boundary call whose origin is
//! a selected ordinary operation (`call_origin::selected_boundary_operation`) is first
//! rebound as an authored call and replayed against that operation.
//! Otherwise `validate_operation` matches the target row with its abstract
//! operation. It compares simple pairs field by field here (constants,
//! primitive and empty-record locals, references, byte sequence and element
//! view rows, structural case membership, and structural field reads and
//! stores), and
//! passes the other families to a child, to a sibling under `target`
//! (`scalar_definitions`, `normalized_foreign`, `hosted_scalar`) or to
//! `read_byte`. A row that defines a value adds it to the available sources,
//! and a pair with no matching rule is a custody error.
//!
//! Each child checks one family: `aggregate_results` (aggregate constructors
//! and structural leaf copies), `borrowed_windows` (structural field moves
//! and stores), `direct_calls`, `ieee_float` (float compare, constant and fused
//! multiply-add) and `primitive_store` (write-only primitive stores).
//! `runtime_indices_rejoin` is used by `primitive_store`.

use super::super::ScalarType;
use super::{
    AbstractOperation, AbstractOperationPlan, PsiOptimizationFunction, PsiOptimizationUnit,
    TargetFunction, TargetOperationPlan, TargetUnitOperation, ValueId,
};
use crate::LegalizationError;
use crate::legalization::scalar_graph_input::callee_plan;
use crate::legalization::scalar_graph_input::target::Checker;
use target_operations::{
    ScalarAbiValue, TargetStructuralParameter, TargetUnitScalarArgumentSource as Source,
};
mod aggregate_results;
mod borrowed_windows;
mod direct_calls;
mod ieee_float;
mod primitive_store;
/// Replay one ordered Unit operation with only the SSA sources available here.
pub(super) fn validate_operation(
    function: &TargetFunction,
    target: &TargetUnitOperation,
    abstracted: &AbstractOperation,
    scalar_parameters: &[ScalarAbiValue],
    parameters: &[TargetStructuralParameter],
    sources: &mut Vec<(ValueId, Source)>,
    custody: &super::super::reference_custody::Custody,
    optimized: &PsiOptimizationFunction,
    native: &TargetOperationPlan,
    plan: &AbstractOperationPlan,
    unit: &PsiOptimizationUnit,
) -> Result<(), LegalizationError> {
    if matches!(abstracted, AbstractOperation::BoundaryCall { .. }) {
        let node = optimized
            .block_nodes()
            .find(|node| &node.operation == abstracted)
            .ok_or(LegalizationError::custody())?;
        if let Some((operation, _)) =
            super::super::call_origin::selected_boundary_operation(node, optimized, native, plan)?
        {
            let mut ordinary = target.clone();
            match &mut ordinary {
                TargetUnitOperation::Call { origin, .. } => {
                    *origin = target_operations::NativeCallOrigin::Authored
                }
                _ => return Err(LegalizationError::custody()),
            }
            return validate_operation(
                function,
                &ordinary,
                &operation,
                scalar_parameters,
                parameters,
                sources,
                custody,
                optimized,
                native,
                plan,
                unit,
            );
        }
    }
    let checker = Checker {
        function,
        available: Some(sources),
        optimized,
        types: unit.structural_types.as_slice(),
    };
    let projected_bytes = match target {
        TargetUnitOperation::StructuralByteSequenceFieldLength {
            source,
            field,
            indices,
            ..
        }
        | TargetUnitOperation::StructuralByteSequenceFieldRead {
            source,
            field,
            indices,
            ..
        } => Some((source, Some(*field), indices)),
        TargetUnitOperation::StructuralByteViewFieldStore {
            destination,
            field,
            indices,
            ..
        } => Some((destination, *field, indices)),
        _ => None,
    };
    if let Some((source, field, indices)) = projected_bytes {
        let geometry = super::super::structural_fields::view_slot_geometry(
            optimized,
            source,
            field,
            &plan.structural_types,
        )
        .ok_or(LegalizationError::custody())?;
        if !runtime_indices_rejoin(indices, &geometry.elements, sources) {
            return Err(LegalizationError::custody());
        }
    }
    match (target, abstracted) {
        (
            TargetUnitOperation::PlacedScalarAccess {
                psi_operation,
                boundary,
                realization,
                location,
                interpretation,
                operand,
                result_home,
            },
            AbstractOperation::BoundaryCall {
                psi_operation: expected_operation,
                boundary: expected_boundary,
                arguments,
                structural_arguments,
                result,
                completion_claim_sources,
                completion_receipts,
            },
        ) => {
            let demand = terminal_semantics::placed_scalar_boundary_demand(
                unit.structural_types.as_slice(),
                &plan.identities,
                unit.structural_types.placed_views(),
                &plan.boundary_machines,
                *boundary,
            )
            .map_err(|_| LegalizationError::custody())?;
            let [argument] = structural_arguments.as_slice() else {
                return Err(LegalizationError::custody());
            };
            let mut parent = argument.clone();
            if parent.path.pop()
                != Some(terminal_psi::StructuralPathSegment::Field(
                    demand.view_field,
                ))
                || &parent != location
                || psi_operation != expected_operation
                || boundary != expected_boundary
                || realization.demand != demand
                || argument.access != demand.access
                || !completion_claim_sources.is_empty()
                || !completion_receipts.is_empty()
                || &super::super::reference_custody::placed_scalar_custody(
                    optimized,
                    custody,
                    unit.structural_types.as_slice(),
                    location,
                )? != interpretation
            {
                return Err(LegalizationError::custody());
            }
            match (
                demand.operation,
                arguments.as_slice(),
                operand,
                result,
                result_home,
            ) {
                (
                    terminal_psi::TerminalPlacedAccessorKind::Read,
                    [],
                    None,
                    abstract_operations::AbstractBoundaryResult::Scalar(result),
                    Some(home),
                ) if result.scalar_type == demand.scalar
                    && home.scalar_type == demand.scalar
                    && home.defining_operation == *psi_operation
                    && home.source_value == result.value
                    && Some(home.shape) == super::super::scalar_shape(demand.scalar) =>
                {
                    sources.push((result.value, Source::Home(*home)));
                }
                (
                    terminal_psi::TerminalPlacedAccessorKind::Write,
                    [value],
                    Some(operand),
                    abstract_operations::AbstractBoundaryResult::Unit,
                    None,
                ) if operand.scalar_type() == demand.scalar
                    && sources
                        .iter()
                        .any(|(identity, source)| identity == value && source == operand) => {}
                _ => return Err(LegalizationError::custody()),
            }
        }
        (
            TargetUnitOperation::SelectImplementation { result_home, table },
            AbstractOperation::SelectImplementation {
                psi_operation,
                result,
                table: expected_table,
            },
        ) => {
            let expected = target_operations::TargetUnitScalarHomeRequirement {
                defining_operation: *psi_operation,
                source_value: result.value,
                scalar_type: result.scalar_type,
                shape: super::super::scalar_shape(result.scalar_type)
                    .ok_or(LegalizationError::custody())?,
            };
            if result.scalar_type != target_operations::selector_carrier_type()
                || result_home != &expected
                || table != expected_table
                || plan.dynamic_dispatch.table(*table).is_none()
            {
                return Err(LegalizationError::custody());
            }
            sources.push((result.value, Source::Home(*result_home)));
        }
        (
            TargetUnitOperation::AtomicFence {
                psi_operation,
                ordering,
            },
            AbstractOperation::AtomicEvent {
                psi_operation: expected_operation,
                event:
                    abstract_operations::AbstractAtomicEvent::Fence {
                        ordering: expected_ordering,
                    },
                reads_from: None,
                modification_after: None,
            },
        ) if psi_operation == expected_operation && ordering == expected_ordering => {}
        (
            TargetUnitOperation::AtomicAccess {
                psi_operation,
                location,
                field,
                scalar_type,
                field_byte_offset,
                event,
                operands,
                result_home,
            },
            AbstractOperation::AtomicEvent {
                psi_operation: expected_operation,
                event: source,
                ..
            },
        ) => {
            let expected =
                super::super::atomic_access::access(source, optimized, &unit.structural_types)
                    .ok_or(LegalizationError::custody())?;
            // Source-unit custody owns legal access to the original atomic
            // cell. A stored descriptor's external pointee does not suspend
            // the entire enclosing record: that whole-root rule belongs to
            // cleanup, not this event. Rejoin the exact cell, aligned address,
            // ordering, operands and observed result independently below.
            if psi_operation != expected_operation
                || location != &expected.location
                || *field != expected.field
                || *scalar_type != expected.scalar_type
                || *field_byte_offset != expected.field_byte_offset
                || *event != expected.event
                || operands.len() != event.operands().len()
                || !event
                    .operands()
                    .iter()
                    .zip(operands)
                    .all(|(value, operand)| {
                        sources
                            .iter()
                            .any(|(identity, source)| identity == value && source == operand)
                    })
            {
                return Err(LegalizationError::custody());
            }
            match (result_home, expected.result) {
                (None, None) => {}
                (Some(home), Some(result))
                    if home.defining_operation == *psi_operation
                        && home.source_value == result.value
                        && home.scalar_type == result.scalar_type
                        && Some(home.shape) == super::super::scalar_shape(result.scalar_type) =>
                {
                    sources.push((result.value, Source::Home(*home)));
                }
                _ => return Err(LegalizationError::custody()),
            }
        }
        (
            TargetUnitOperation::StructuralCaseMembership {
                psi_operation,
                result,
                source,
                path,
                case,
                case_tag,
                tag_byte_offset,
                indices,
            },
            AbstractOperation::StructuralCaseMembership {
                psi_operation: expected_operation,
                result: expected_result,
                source: expected_source,
                path: expected_path,
                case: expected_case,
            },
        ) if psi_operation == expected_operation
            && result == expected_result
            && source == expected_source
            && path == expected_path
            && case == expected_case
            && result.scalar_type == ScalarType::Boolean =>
        {
            // The tag, its static offset and every runtime element's stride
            // are rebuilt from declarations; the row supplies none of them.
            let (expected_tag, expected_offset, elements) =
                super::super::structural_case::membership_layout(
                    optimized,
                    *expected_source,
                    expected_path,
                    *expected_case,
                    plan,
                )?;
            if (*case_tag, *tag_byte_offset) != (expected_tag, expected_offset)
                || !runtime_indices_rejoin(indices, &elements, sources)
            {
                return Err(LegalizationError::custody());
            }
            sources.push((
                result.value,
                Source::Home(target_operations::TargetUnitScalarHomeRequirement {
                    defining_operation: *psi_operation,
                    source_value: result.value,
                    scalar_type: result.scalar_type,
                    shape: calling_conventions::ValueShape::integer(1, 1),
                }),
            ));
        }
        (
            _,
            AbstractOperation::IeeeFloatCompare { .. }
            | AbstractOperation::FloatResultPolicy { .. }
            | AbstractOperation::IeeeFloatConstant { .. }
            | AbstractOperation::NearestIeeeFloatFusedMultiplyAdd { .. }
            | AbstractOperation::ScalarRepresentationView { .. },
        ) => {
            ieee_float::validate(target, abstracted, sources)?;
        }
        (
            TargetUnitOperation::EstablishPlacedReference {
                psi_operation,
                result_home,
                source,
                correspondence,
            },
            AbstractOperation::EstablishPlacedReference {
                psi_operation: expected_operation,
                result,
                source: expected_source,
                correspondence: expected_correspondence,
            },
        ) if psi_operation == expected_operation
            && source == expected_source
            && correspondence == expected_correspondence
            && *result_home
                == super::super::aggregate_results::result_home(optimized, result.place, plan)? => {
        }
        (
            TargetUnitOperation::EstablishReference {
                psi_operation,
                result_home,
                source,
                storage,
            },
            AbstractOperation::EstablishReference {
                psi_operation: expected_operation,
                result,
                source: expected_source,
                storage: expected_storage,
            },
        ) if psi_operation == expected_operation
            && source == expected_source
            && storage == expected_storage
            && *result_home
                == super::super::aggregate_results::result_home(optimized, result.place, plan)? => {
        }
        (
            TargetUnitOperation::ReleaseReference {
                psi_operation,
                source,
                path,
            },
            AbstractOperation::ReleaseReference {
                psi_operation: expected_operation,
                source: expected_source,
                path: expected_path,
            },
        ) if psi_operation == expected_operation
            && source == expected_source
            && path == expected_path => {}
        (
            _,
            AbstractOperation::EstablishStructuralArray { .. }
            | AbstractOperation::EstablishScalarArray { .. }
            | AbstractOperation::EstablishRecord { .. }
            | AbstractOperation::StructuralLeafCopy { .. }
            | AbstractOperation::EstablishCase { .. },
        ) => {
            aggregate_results::validate(target, abstracted, sources, optimized, plan, unit)?;
        }
        (
            TargetUnitOperation::EstablishZeroedStructural {
                psi_operation,
                result_home,
            },
            AbstractOperation::EstablishZeroedStructural {
                psi_operation: expected_operation,
                result,
            },
        ) if psi_operation == expected_operation
            && *result_home
                == super::super::aggregate_results::result_home(optimized, result.place, plan)?
            && terminal_semantics::zeroed_structural_shape(
                plan.structural_types.iter(),
                result.structural_type,
            ) => {}
        // A memberwise-established local names any structural home the
        // aggregate layout describes; its stores arrive as their own rows.
        (
            TargetUnitOperation::EstablishStructuralLocal {
                psi_operation,
                result_home,
            },
            AbstractOperation::EstablishStructuralLocal {
                psi_operation: expected_operation,
                result,
            },
        ) if psi_operation == expected_operation
            && *result_home
                == super::super::aggregate_results::result_home(optimized, result.place, plan)?
            && super::super::aggregate_results::home_layout(result, plan).is_ok() => {}
        (_, abstract_operations::move_or_store_structural_field_pattern!()) => {
            borrowed_windows::validate(target, abstracted, optimized, sources, plan, custody)?;
        }
        // An empty-record local realizes no storage: the row is its exact
        // establishment, of the module's own empty record, at the trivial
        // place that type declares.
        (
            TargetUnitOperation::EstablishTrivialAffineLocal {
                psi_operation,
                place,
                structural_type,
            },
            AbstractOperation::EstablishTrivialAffineLocal {
                psi_operation: expected_operation,
                place: expected_place,
                structural_type: expected_type,
            },
        ) if psi_operation == expected_operation
            && place == expected_place
            && structural_type == expected_type
            && plan.structural_types.contains(structural_type)
            && matches!(
                &structural_type.shape,
                terminal_psi::StructuralTypeShape::Record { fields, .. } if fields.is_empty()
            )
            && matches!(
                place.kind,
                semantic_vocabulary::StructuralPlaceKind::TrivialAffineLocal {
                    structural_type: declared,
                    construction: None,
                    ..
                } if declared == structural_type.id
            ) => {}
        (
            TargetUnitOperation::StructuralByteSequenceFieldByteStore {
                psi_operation,
                destination,
                field,
                index,
                value,
                length,
                obligation,
            },
            AbstractOperation::StructuralByteSequenceFieldByteStore {
                psi_operation: expected_operation,
                field: expected_field,
                index: expected_index,
                value: expected_value,
                length: expected_length,
                obligation: expected_obligation,
                ..
            },
        ) if psi_operation == expected_operation
            && field == expected_field
            && length == expected_length
            && obligation == expected_obligation
            && Some(destination.clone())
                == super::super::structural_fields::replacement(
                    optimized,
                    abstracted,
                    &plan.structural_types,
                    custody,
                )
            && sources
                .iter()
                .any(|(identity, actual)| identity == expected_index && actual == index)
            && sources
                .iter()
                .any(|(identity, actual)| identity == expected_value && actual == value) => {}
        (
            TargetUnitOperation::StructuralByteSequenceFieldRead {
                psi_operation,
                result,
                source,
                field,
                index,
                length,
                obligation,
                ..
            },
            AbstractOperation::StructuralByteSequenceFieldRead {
                psi_operation: expected_operation,
                result: expected_result,
                field: expected_field,
                index: expected_index,
                length: expected_length,
                obligation: expected_obligation,
                ..
            },
        ) => {
            if psi_operation != expected_operation
                || result != expected_result
                || field != expected_field
                || length != expected_length
                || obligation != expected_obligation
                || Some(source.clone())
                    != super::super::structural_fields::byte_read(
                        optimized,
                        abstracted,
                        &plan.structural_types,
                    )
                || !sources
                    .iter()
                    .any(|(identity, actual)| identity == expected_index && actual == index)
            {
                return Err(LegalizationError::custody());
            }
            sources.push(scalar_home(psi_operation, result)?);
        }
        (
            TargetUnitOperation::EstablishByteSequenceConcatenation {
                psi_operation,
                result,
                left,
                right,
                capacity_obligation,
            },
            AbstractOperation::EstablishByteSequenceConcatenation {
                psi_operation: expected_operation,
                result: expected_result,
                left: expected_left,
                right: expected_right,
                capacity_obligation: expected_obligation,
            },
        ) if psi_operation == expected_operation
            && result == expected_result
            && left == expected_left
            && right == expected_right
            && capacity_obligation == expected_obligation => {}
        (
            TargetUnitOperation::ByteSequenceStore {
                psi_operation,
                destination,
                source,
                length,
                obligation,
            },
            AbstractOperation::ByteSequenceStore {
                psi_operation: expected_operation,
                destination: expected_destination,
                source: expected_source,
                length: expected_length,
                obligation: expected_obligation,
            },
        ) if psi_operation == expected_operation
            && destination == expected_destination
            && source == expected_source
            && obligation == expected_obligation
            && super::super::structural_fields::replacement(
                optimized,
                abstracted,
                &plan.structural_types,
                custody,
            )
            .is_some()
            && sources
                .iter()
                .any(|(value, actual)| value == expected_length && actual == length) => {}
        (
            TargetUnitOperation::StructuralByteViewFieldStore {
                psi_operation,
                destination,
                field,
                source,
                ..
            },
            AbstractOperation::StructuralByteViewFieldStore {
                psi_operation: expected_operation,
                field: expected_field,
                source: expected_source,
                ..
            },
        ) if psi_operation == expected_operation
            && field == expected_field
            && source == expected_source
            && Some(destination.clone())
                == super::super::structural_fields::view_replacement(
                    optimized,
                    abstracted,
                    &plan.structural_types,
                    custody,
                ) => {}
        (
            TargetUnitOperation::StructuralByteSequenceFieldStore {
                psi_operation,
                destination,
                field,
                source,
                length,
                obligation,
            },
            AbstractOperation::StructuralByteSequenceFieldStore {
                psi_operation: expected_operation,
                field: expected_field,
                source: expected_source,
                length: expected_length,
                obligation: expected_obligation,
                ..
            },
        ) if psi_operation == expected_operation
            && field == expected_field
            && source == expected_source
            && obligation == expected_obligation
            && Some(destination.clone())
                == super::super::structural_fields::replacement(
                    optimized,
                    abstracted,
                    &plan.structural_types,
                    custody,
                )
            && sources
                .iter()
                .any(|(value, actual)| value == expected_length && actual == length) => {}
        (
            TargetUnitOperation::ByteSequenceWrite {
                psi_operation,
                destination,
                view,
                index,
                value,
                length,
                obligation,
            },
            AbstractOperation::ByteSequenceWrite {
                psi_operation: expected_operation,
                destination: expected_destination,
                index: expected_index,
                value: expected_value,
                length: expected_length,
                obligation: expected_obligation,
            },
        ) if psi_operation == expected_operation
            && destination == expected_destination
            && length == expected_length
            && obligation == expected_obligation
            && checker.mutable_byte_view(view, *expected_destination)
            && sources
                .iter()
                .any(|(identity, source)| identity == expected_index && source == index)
            && sources
                .iter()
                .any(|(identity, source)| identity == expected_value && source == value) => {}
        (
            TargetUnitOperation::ElementViewWrite {
                path,
                psi_operation,
                destination,
                view,
                index,
                value,
                length,
                obligation,
            },
            AbstractOperation::ElementViewWrite {
                path: expected_path,
                psi_operation: expected_operation,
                destination: expected_destination,
                index: expected_index,
                value: expected_value,
                length: expected_length,
                obligation: expected_obligation,
            },
        ) if path == expected_path
            && psi_operation == expected_operation
            && destination == expected_destination
            && length == expected_length
            && obligation == expected_obligation
            && checker.mutable_element_view(view, *expected_destination)
            && sources
                .iter()
                .any(|(identity, source)| identity == expected_index && source == index)
            && sources
                .iter()
                .any(|(identity, source)| identity == expected_value && source == value) => {}
        (
            TargetUnitOperation::EstablishPrimitiveLocal {
                psi_operation,
                result,
                value,
                shape,
            },
            AbstractOperation::EstablishPrimitiveLocal {
                psi_operation: expected_operation,
                result: expected,
                value: expected_value,
                ..
            },
        ) if psi_operation == expected_operation
            && result == expected
            && value == expected_value
            && super::super::scalar_shape(value.scalar_type) == Some(*shape)
            && sources.iter().any(|(identity, source)| {
                *identity == value.value && source.scalar_type() == value.scalar_type
            }) => {}
        (
            TargetUnitOperation::PrimitiveLocalStore {
                psi_operation,
                destination,
                value,
            },
            AbstractOperation::PrimitiveLocalStore {
                psi_operation: expected_operation,
                destination: expected,
                value: expected_value,
            },
        ) if psi_operation == expected_operation
            && destination == expected
            && value == expected_value
            && sources.iter().any(|(identity, source)| {
                *identity == value.value && source.scalar_type() == value.scalar_type
            }) => {}
        (
            TargetUnitOperation::PrimitiveScalarRead {
                psi_operation,
                result,
                source,
                path,
                indices,
            },
            AbstractOperation::PrimitiveScalarRead {
                psi_operation: expected_operation,
                result: expected,
                source: expected_source,
                path: expected_path,
            },
        ) if psi_operation == expected_operation
            && result == expected
            && source == expected_source
            && path == expected_path =>
        {
            let elements = if terminal_psi::is_static_structural_path(expected_path) {
                Vec::new()
            } else {
                let root_type =
                    crate::legalization::scalar_graph_input::primitive_locals::read_root_type(
                        optimized,
                        *expected_source,
                    )
                    .ok_or(LegalizationError::custody())?;
                crate::structural_inputs::structural_reference_input::primitive_read_geometry(
                    root_type,
                    expected_path,
                    expected.scalar_type,
                    &unit.structural_types,
                )
                .ok_or(LegalizationError::custody())?
                .elements
            };
            if !runtime_indices_rejoin(indices, &elements, sources) {
                return Err(LegalizationError::custody());
            }
            sources.push(scalar_home(psi_operation, result)?);
        }
        (
            TargetUnitOperation::StructuralScalarFieldRead {
                psi_operation,
                result,
                source,
                field,
                ..
            },
            AbstractOperation::StructuralScalarFieldRead { .. },
        )
        | (
            TargetUnitOperation::StructuralByteSequenceFieldLength {
                psi_operation,
                result,
                source,
                field,
                ..
            },
            AbstractOperation::StructuralByteSequenceFieldLength { .. },
        ) => {
            // Only a scalar field read names a payload case; the replayed
            // carrier decides whether one is required, and which.
            let case = match target {
                TargetUnitOperation::StructuralScalarFieldRead { case, .. } => *case,
                _ => None,
            };
            let expected = super::super::structural_fields::read(
                optimized,
                abstracted,
                &plan.structural_types,
                custody,
            )
            .ok_or(LegalizationError::custody())?;
            if expected != (*psi_operation, *result, source.clone(), case, *field) {
                return Err(LegalizationError::custody());
            }
            sources.push(scalar_home(psi_operation, result)?);
        }
        (
            TargetUnitOperation::EstablishByteSequenceLiteral {
                psi_operation,
                place,
                structural_type,
                bytes,
                qualifications,
            },
            AbstractOperation::EstablishByteSequenceLiteral {
                psi_operation: expected_operation,
                place: expected_place,
                structural_type: expected_type,
                bytes: expected_bytes,
                qualifications: expected_qualifications,
            },
        ) if psi_operation == expected_operation
            && place == expected_place
            && structural_type == expected_type
            && bytes == expected_bytes
            && qualifications == expected_qualifications => {}
        (
            TargetUnitOperation::WriteOnlyPrimitiveStore { .. },
            AbstractOperation::WriteOnlyPrimitiveStore { .. },
        ) => {
            primitive_store::validate(target, abstracted, parameters, sources, optimized, unit)?;
        }
        (TargetUnitOperation::ScalarDefinition { result_home, .. }, abstracted)
            if super::scalar_definitions::observed_family(abstracted) =>
        {
            super::scalar_definitions::observation(target, abstracted, &checker)?;
            sources.push((result_home.source_value, Source::Home(*result_home)));
        }
        (
            TargetUnitOperation::ByteSequenceSubslice { result, view },
            AbstractOperation::ByteSequenceSubslice {
                result: expected,
                psi_operation,
                ..
            },
        ) => {
            if result != expected
                || !matches!(view, target_operations::TargetByteView::Subslice { psi_operation: operation, .. } if operation == psi_operation)
                || !checker.byte_view(view, result.place, &[])
            {
                return Err(LegalizationError::custody());
            }
        }
        (
            TargetUnitOperation::ElementViewSubslice { result, view },
            AbstractOperation::ElementViewSubslice {
                result: expected,
                psi_operation,
                ..
            },
        ) => {
            if result != expected
                || !matches!(view, target_operations::TargetElementView::Subslice { psi_operation: operation, .. } if operation == psi_operation)
                || !checker.element_view(view, result.place, &[])
            {
                return Err(LegalizationError::custody());
            }
        }
        (
            TargetUnitOperation::EstablishElementView { result, view },
            AbstractOperation::EstablishElementView {
                result: expected,
                destination,
                psi_operation,
                ..
            },
        ) => {
            if result != expected
                || !matches!(view, target_operations::TargetElementView::Established { psi_operation: operation, .. } if operation == psi_operation)
                || !checker.element_view(view, *destination, &[])
            {
                return Err(LegalizationError::custody());
            }
        }
        (
            TargetUnitOperation::ScalarDefinition { result_home, .. },
            AbstractOperation::IntegerWiden { .. },
        ) => {
            super::scalar_definitions::validate(target, abstracted, scalar_parameters, sources)?;
            sources.push((result_home.source_value, Source::Home(*result_home)));
        }
        (
            TargetUnitOperation::NormalizedForeignCall { .. },
            AbstractOperation::BoundaryCall { .. },
        ) => {
            super::normalized_foreign::validate(
                target, abstracted, parameters, native, optimized, plan, unit, sources,
            )?;
        }
        (
            TargetUnitOperation::NearestIeeeFloatArithmetic { .. }
            | TargetUnitOperation::IeeeFloatClassify { .. },
            AbstractOperation::BoundaryCall { .. },
        ) => {
            super::super::ieee_float_arithmetic::validate(
                target,
                abstracted,
                native.target,
                plan,
                unit,
                sources,
            )?;
        }
        (
            TargetUnitOperation::BoundarySettlement { .. },
            AbstractOperation::BoundaryCall {
                result: abstract_operations::AbstractBoundaryResult::Structural(_),
                ..
            },
        ) => {
            super::super::read_byte::validate(target, abstracted, native.target, plan, unit)?;
        }
        (
            TargetUnitOperation::BoundarySettlement { .. },
            AbstractOperation::BoundaryCall { .. },
        ) => {
            super::hosted_scalar::validate(target, abstracted, native.target, plan, unit, sources)?;
        }
        (
            TargetUnitOperation::StructuralScalarFieldStore {
                psi_operation,
                destination,
                path,
                field,
                destination_storage,
                field_byte_offset,
                source,
                indices,
            },
            AbstractOperation::StructuralScalarFieldStore {
                psi_operation: expected_operation,
                destination: expected_destination,
                path: expected_path,
                field: expected_field,
                value,
                ..
            },
        ) => {
            let (expected_destination, expected_path) =
                super::super::structural_fields::scalar_store_destination(
                    optimized,
                    expected_destination,
                    expected_path,
                    &unit.structural_types,
                    custody,
                )
                .ok_or(LegalizationError::custody())?;
            // The physical home must retain the same semantic owner: either an
            // incoming borrow or the exact fresh result, never a forged parameter.
            let storage_matches = match (destination, destination_storage) {
                (
                    abstract_operations::StructuralScalarStoreDestination::Parameter(parameter),
                    target_operations::TargetStructuralArgumentSource::Placement(placement),
                ) => {
                    optimized.structural_parameters.contains(parameter)
                        && parameters.iter().any(|declared| {
                            declared.place == parameter.place && &declared.placement == placement
                        })
                }
                (
                    abstract_operations::StructuralScalarStoreDestination::Parameter(parameter),
                    target_operations::TargetStructuralArgumentSource::BlockParameter {
                        block,
                        place,
                    },
                ) => {
                    *place == parameter.place
                        && matches!(
                            parameter.access,
                            terminal_psi::StructuralAccess::Owned
                                | terminal_psi::StructuralAccess::MutableBorrow
                        )
                        && optimized.blocks.iter().any(|entry| {
                            entry.id == *block && entry.structural_parameters.contains(parameter)
                        })
                        && optimized.arrival_parameter(*place) == Some(parameter)
                        && parameter.multiplicity != terminal_psi::StructuralMultiplicity::Linear
                        && parameter.qualifications.is_empty()
                        && parameter.projected_qualifications.is_empty()
                        && optimized
                            .entry_claim_declarations
                            .iter()
                            .all(|claim| claim.input != *place)
                        && optimized
                            .content_entry_claims
                            .iter()
                            .all(|claim| claim.input.root != *place)
                }
                (
                    abstract_operations::StructuralScalarStoreDestination::OwnedResult {
                        producer,
                        result,
                    },
                    target_operations::TargetStructuralArgumentSource::StructuralHome {
                        psi_operation,
                    },
                ) => {
                    psi_operation == producer
                        && super::super::structural_case::source_result(optimized, result.place)
                            .is_ok_and(|(operation, declared)| {
                                operation == *producer && declared == result
                            })
                }
                // An owned arrival's storage is its own block home: the same
                // block and declaration the owned-arrival owner resolves to,
                // never a predecessor's producer home.
                (
                    abstract_operations::StructuralScalarStoreDestination::OwnedBlockParameter {
                        block,
                        parameter,
                    },
                    target_operations::TargetStructuralArgumentSource::BlockParameter {
                        block: storage_block,
                        place,
                    },
                ) => {
                    storage_block == block
                        && *place == parameter.place
                        && matches!(
                            super::super::structural_case::source_owner(optimized, parameter.place),
                            Ok(legalized_operations::LegalizedStructuralCaseSource::BlockParameter {
                                block: owner,
                                declaration,
                            }) if owner == *block && declaration == *parameter
                        )
                }
                _ => false,
            };
            let (offset, _, elements) =
                crate::structural_inputs::structural_reference_input::store(
                    expected_destination.structural_type(),
                    &expected_path,
                    *expected_field,
                    value.scalar_type,
                    &unit.structural_types,
                )
                .ok_or(LegalizationError::custody())?;
            if psi_operation != expected_operation
                || destination != &expected_destination
                || path != &expected_path
                || field != expected_field
                || !storage_matches
                || *field_byte_offset != offset
                || !runtime_indices_rejoin(indices, &elements, sources)
                || !sources
                    .iter()
                    .any(|(identity, expected)| *identity == value.value && expected == source)
            {
                return Err(LegalizationError::custody());
            }
        }
        (
            TargetUnitOperation::BooleanConstant {
                psi_operation,
                result,
                value,
            },
            AbstractOperation::BooleanConstant {
                psi_operation: actual,
                result: defined,
                value: literal,
            },
        ) if psi_operation == actual && result == defined && value == literal => {
            sources.push((
                *result,
                Source::BooleanImmediate {
                    defining_operation: *psi_operation,
                    source_value: *result,
                    value: *value,
                },
            ));
        }
        (TargetUnitOperation::Call { .. }, _) => {
            direct_calls::validate(
                target, abstracted, sources, custody, optimized, native, plan, unit,
            )?;
        }
        (
            TargetUnitOperation::IntegerConstant {
                psi_operation,
                result,
                scalar_type,
                value,
            },
            AbstractOperation::IntegerConstant {
                psi_operation: actual,
                result: defined,
                scalar_type: integer,
                value: literal,
            },
        ) if psi_operation == actual
            && result == defined
            && *integer == ScalarType::Integer(*scalar_type)
            && value == literal =>
        {
            sources.push((
                *result,
                Source::IntegerImmediate {
                    defining_operation: *psi_operation,
                    source_value: *result,
                    scalar_type: *scalar_type,
                    value: *value,
                },
            ));
        }
        (
            TargetUnitOperation::Return {
                psi_edge,
                cleanup_actions,
            },
            AbstractOperation::ReturnUnit {
                psi_edge: edge,
                cleanup_actions: cleanup,
            },
        ) if psi_edge == edge
            && cleanup_actions == cleanup
            && (cleanup.is_empty() || super::super::read_byte::cleanup(optimized, cleanup)) => {}
        _ => return Err(LegalizationError::custody()),
    }
    Ok(())
}

/// A target row's runtime traversals rejoin the projection's runtime elements
/// one for one, in path order: the same selector at the reconstructed stride,
/// resolved to the exact source already available for that value. The
/// element's bound is its path segment's obligation, which the row keeps.
pub(super) fn runtime_indices_rejoin(
    indices: &[target_operations::TargetStructuralRuntimeIndex],
    elements: &[crate::structural_inputs::structural_reference_input::RuntimeElement],
    sources: &[(ValueId, Source)],
) -> bool {
    indices.len() == elements.len()
        && indices.iter().zip(elements).all(|(index, element)| {
            index.stride == element.stride
                && index.operand.source_value() == element.index
                && sources
                    .iter()
                    .any(|(identity, known)| *identity == element.index && *known == index.operand)
        })
}

/// One scalar result's home-requirement row: the unit source every
/// scalar-producing operation contributes — its defining operation, value,
/// type, and storage shape.
fn scalar_home(
    psi_operation: &semantic_vocabulary::OperationId,
    result: &abstract_operations::AbstractResult,
) -> Result<(ValueId, Source), LegalizationError> {
    Ok((
        result.value,
        Source::Home(target_operations::TargetUnitScalarHomeRequirement {
            defining_operation: *psi_operation,
            source_value: result.value,
            scalar_type: result.scalar_type,
            shape: super::super::scalar_shape(result.scalar_type)
                .ok_or(LegalizationError::custody())?,
        }),
    ))
}
