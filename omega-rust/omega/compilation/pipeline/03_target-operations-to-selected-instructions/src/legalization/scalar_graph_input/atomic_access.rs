//! Reconstruct an atomic access from the current abstract graph, independently
//! of target planning. Geometry is shared with ordinary structural accesses;
//! authority, exact scalar type and ordering remain atomic-event obligations.
//! An observed prior is the result of this event, never an added scalar load.

use abstract_operations::{AbstractAtomicEvent, AbstractResult};
use semantic_vocabulary::{IntegerType, StructuralFieldId};
use terminal_psi::{
    AtomicAccessEvent, StructuralAccess, StructuralArgument, StructuralTypeDeclaration,
};

pub(in crate::legalization) struct Access {
    pub location: StructuralArgument,
    pub field: StructuralFieldId,
    pub scalar_type: IntegerType,
    pub field_byte_offset: u32,
    pub event: AtomicAccessEvent,
    pub result: Option<AbstractResult>,
    pub outcome: Option<(
        terminal_psi::StructuralOperationResult,
        calling_conventions::ConventionalSumLayout,
    )>,
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
                operation: (*operation).into(),
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
        Source::CompareExchangeOnce {
            success,
            failure,
            expected,
            replacement,
            ..
        } => (
            AtomicAccessEvent::CompareExchangeOnce {
                success: *success,
                failure: *failure,
                expected: *expected,
                replacement: *replacement,
            },
            None,
        ),
        Source::Fence { .. } => return None,
    })
}

pub(in crate::legalization) fn access(
    source: &AbstractAtomicEvent,
    function: &optimization_unit::PsiOptimizationFunction,
    types: &[StructuralTypeDeclaration],
) -> Option<Access> {
    let location = source.location()?;
    let (event, result) = event(source)?;
    let root = function
        .arrival_parameter(location.root)
        .map(|parameter| parameter.structural_type)
        .or_else(|| {
            super::structural_case::source_result(function, location.root)
                .ok()
                .map(|(_, result)| result.structural_type)
        })?;
    let root_access =
        optimization_unit_semantics::structural_place_access(function, location.root)?;
    if !event.ordering_is_legal()
        || event.observes_resident() && root_access == StructuralAccess::WriteOnlyBorrow
    {
        return None;
    }
    let field = terminal_semantics::projected_field_declaration(
        types.iter(),
        root,
        &location.path,
        location.field,
    )?;
    // Dedicated cell identity and logical type are independent from the
    // integer lane that the native instruction transfers.
    let scalar_type = field.field_type.atomic_scalar_type()?;
    let integer = target_operations::operations::atomic_storage_integer_type(event, scalar_type)?;
    super::scalar_shape(scalar_type)?;
    if event
        .operands()
        .iter()
        .any(|value| super::value_type(function, *value) != Some(scalar_type))
        || result.is_some_and(|result| result.scalar_type != scalar_type)
    {
        return None;
    }
    let geometry = crate::structural_inputs::structural_reference_input::atomic_field_geometry(
        root,
        &location.path,
        location.field,
        scalar_type,
        types,
    )?;
    let offset = geometry.byte_offset;
    let byte_size = geometry.byte_size;
    if byte_size != u32::from(integer.bits() / 8) || offset % byte_size != 0 {
        return None;
    }
    let outcome = if let AbstractAtomicEvent::CompareExchangeOnce { outcome, .. } = source {
        if !source.custody_is_consistent()
            || !outcome.is_unrestricted_plain()
            || !outcome.qualification_establishments.is_empty()
        {
            return None;
        }
        let declaration = types.iter().find(|row| row.id == outcome.structural_type)?;
        Some((
            outcome.clone(),
            target_operations::conventional_atomic_once_layout(declaration, integer)?,
        ))
    } else {
        None
    };
    Some(Access {
        location: StructuralArgument {
            place: location.root,
            access: root_access,
            path: location.path.clone(),
        },
        field: location.field,
        scalar_type: integer,
        field_byte_offset: offset,
        event,
        result,
        outcome,
    })
}
