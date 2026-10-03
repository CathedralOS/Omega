//! Reconstruct an atomic access from the current abstract graph, independently
//! of target planning. Geometry is shared with ordinary structural accesses;
//! authority, exact scalar type and ordering remain atomic-event obligations.
//! An observed prior is the result of this event, never an added scalar load.

use abstract_operations::{AbstractAtomicEvent, AbstractAtomicReadModifyWrite, AbstractResult};
use semantic_vocabulary::{IntegerType, ScalarType, StructuralFieldId};
use terminal_psi::{
    AtomicAccessEvent, AtomicReadModifyWrite, StructuralAccess, StructuralArgument,
    StructuralFieldType, StructuralTypeDeclaration, StructuralTypeShape,
};

pub(in crate::legalization) struct Access {
    pub location: StructuralArgument,
    pub field: StructuralFieldId,
    pub scalar_type: IntegerType,
    pub field_byte_offset: u32,
    pub event: AtomicAccessEvent,
    pub result: Option<AbstractResult>,
}

/// Fences and single-attempt exchange need distinct native contracts. Do not
/// erase either into this scalar-result access route.
pub(super) fn event(
    source: &AbstractAtomicEvent,
) -> Option<(AtomicAccessEvent, Option<AbstractResult>)> {
    use AbstractAtomicEvent as Source;
    Some(match source {
        Source::Load {
            ordering, result, ..
        } => (
            AtomicAccessEvent::Load {
                ordering: *ordering,
            },
            Some(*result),
        ),
        Source::Store {
            ordering, value, ..
        } => (
            AtomicAccessEvent::Store {
                ordering: *ordering,
                value: *value,
            },
            None,
        ),
        Source::ReadModifyWrite {
            ordering,
            operation,
            operand,
            prior,
            ..
        } => (
            AtomicAccessEvent::ReadModifyWrite {
                ordering: *ordering,
                operation: match operation {
                    AbstractAtomicReadModifyWrite::FetchAdd => AtomicReadModifyWrite::FetchAdd,
                    AbstractAtomicReadModifyWrite::FetchSub => AtomicReadModifyWrite::FetchSub,
                    AbstractAtomicReadModifyWrite::FetchAnd => AtomicReadModifyWrite::FetchAnd,
                    AbstractAtomicReadModifyWrite::FetchOr => AtomicReadModifyWrite::FetchOr,
                    AbstractAtomicReadModifyWrite::FetchXor => AtomicReadModifyWrite::FetchXor,
                },
                operand: *operand,
            },
            Some(*prior),
        ),
        Source::Swap {
            ordering,
            value,
            prior,
            ..
        } => (
            AtomicAccessEvent::Swap {
                ordering: *ordering,
                value: *value,
            },
            Some(*prior),
        ),
        Source::CompareExchange {
            success,
            failure,
            expected,
            replacement,
            observed,
            ..
        } => (
            AtomicAccessEvent::CompareExchange {
                success: *success,
                failure: *failure,
                expected: *expected,
                replacement: *replacement,
            },
            Some(*observed),
        ),
        Source::CompareExchangeOnce { .. } | Source::Fence { .. } => return None,
    })
}

pub(in crate::legalization) fn access(
    source: &AbstractAtomicEvent,
    function: &optimization_unit::PsiOptimizationFunction,
    types: &[StructuralTypeDeclaration],
) -> Option<Access> {
    let location = source.location()?;
    let (event, result) = event(source)?;
    let parameter = function.arrival_parameter(location.root)?;
    if !event.ordering_is_legal()
        || !parameter.qualifications.is_empty()
        || !parameter.projected_qualifications.is_empty()
        || event.observes_resident() && parameter.access == StructuralAccess::WriteOnlyBorrow
    {
        return None;
    }
    let (carrier, _, indices) =
        crate::structural_inputs::structural_reference_input::runtime_projection(
            parameter.structural_type,
            &location.path,
            types,
        )?;
    if !indices.is_empty() {
        return None;
    }
    let StructuralTypeShape::Record { fields } = &types
        .iter()
        .find(|declaration| declaration.id == carrier)?
        .shape
    else {
        return None;
    };
    let field = fields
        .iter()
        .find(|field| field.id == location.field && !field.relevance.is_erased())?;
    // A bounded or qualified leaf owes additional evidence; matching its
    // machine width does not discharge the restriction on an atomic write.
    let StructuralFieldType::Atomic(ScalarType::Integer(integer)) = field.field_type else {
        return None;
    };
    let scalar_type = ScalarType::Integer(integer);
    super::scalar_shape(scalar_type)?;
    if event
        .operands()
        .iter()
        .any(|value| super::value_type(function, *value) != Some(scalar_type))
        || result.is_some_and(|result| result.scalar_type != scalar_type)
    {
        return None;
    }
    let (offset, byte_size, indices) = crate::structural_inputs::structural_reference_input::store(
        parameter.structural_type,
        &location.path,
        location.field,
        scalar_type,
        types,
    )?;
    if !indices.is_empty() || offset % u32::from(byte_size) != 0 {
        return None;
    }
    Some(Access {
        location: StructuralArgument {
            place: location.root,
            access: parameter.access,
            path: location.path.clone(),
        },
        field: location.field,
        scalar_type: integer,
        field_byte_offset: offset,
        event,
        result,
    })
}
