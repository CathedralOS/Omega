//! Preserve atomic accesses as events while resolving their scalar operands
//! and aligned field locations. Splitting a fetch into a read and a store
//! would lose the instruction-observed prior before instruction selection.
//! This is the existing serial scalar-result route; fences and single-attempt
//! exchange retain their explicit unsupported boundary.

use super::{KnownUnitInteger, LiveDefinitions};
use crate::LoweringError;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::{AbstractAtomicEvent, AbstractAtomicReadModifyWrite, AbstractFunction};
use semantic_vocabulary::{OperationId, ScalarType};
use std::collections::{BTreeMap, BTreeSet};
use target_operations::{
    TargetUnitOperation, TargetUnitScalarHomeRequirement, TerminalPsiProvenance,
};
use terminal_psi::{
    AtomicAccessEvent, AtomicReadModifyWrite, StructuralAccess, StructuralFieldType,
    StructuralTypeShape,
};

pub(super) fn lower(
    operation: OperationId,
    event: &AbstractAtomicEvent,
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    live: &mut LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let invalid = || LoweringError::unsupported_control_flow(function.machine);
    let location = event.location().ok_or_else(invalid)?;
    let (access, result) = match event {
        AbstractAtomicEvent::Load {
            ordering, result, ..
        } => (
            AtomicAccessEvent::Load {
                ordering: *ordering,
            },
            Some(*result),
        ),
        AbstractAtomicEvent::Store {
            ordering, value, ..
        } => (
            AtomicAccessEvent::Store {
                ordering: *ordering,
                value: *value,
            },
            None,
        ),
        AbstractAtomicEvent::ReadModifyWrite {
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
        AbstractAtomicEvent::Swap {
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
        AbstractAtomicEvent::CompareExchange {
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
        AbstractAtomicEvent::CompareExchangeOnce { .. } | AbstractAtomicEvent::Fence { .. } => {
            return Err(invalid());
        }
    };
    let parameter = function
        .arrival_parameter(location.root)
        .ok_or_else(invalid)?;
    if !access.ordering_is_legal()
        || access.observes_resident() && parameter.access == StructuralAccess::WriteOnlyBorrow
    {
        return Err(invalid());
    }
    let (carrier, _, prefix, indices) = crate::lowering::structural_layout::runtime_projection(
        parameter.structural_type,
        &location.path,
        types,
        &mut BTreeMap::new(),
        &mut BTreeSet::new(),
    )?;
    // Atomic location identity currently admits static projections only.
    if !indices.is_empty() {
        return Err(invalid());
    }
    let StructuralTypeShape::Record { fields } = &types.get(&carrier).ok_or_else(invalid)?.shape
    else {
        return Err(invalid());
    };
    let field = fields
        .iter()
        .find(|field| field.id == location.field && !field.relevance.is_erased())
        .ok_or_else(invalid)?;
    let StructuralFieldType::Atomic(ScalarType::Integer(integer)) = field.field_type else {
        // A bounded field additionally owes a range proof; an atomic event
        // cannot invent that proof by erasing the field's restriction.
        return Err(invalid());
    };
    if !matches!(integer.bits(), 8 | 16 | 32 | 64) {
        return Err(invalid());
    }
    let scalar_type = ScalarType::Integer(integer);
    let offset = crate::lowering::structural_layout::direct_integer_field_offset(
        carrier,
        location.field,
        integer,
        types,
    )?;
    let field_byte_offset = prefix.checked_add(offset).ok_or_else(invalid)?;
    if field_byte_offset % u32::from(integer.bits() / 8) != 0 {
        return Err(invalid());
    }
    let operands = access
        .operands()
        .into_iter()
        .map(|value| {
            let source = super::scalar_sources::source(value, function, live)?;
            if source.scalar_type() != scalar_type {
                return Err(LoweringError::ValueTypeMismatch(value));
            }
            Ok(source)
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let result_home = result
        .map(|result| {
            if result.scalar_type != scalar_type {
                return Err(LoweringError::ValueTypeMismatch(result.value));
            }
            Ok(TargetUnitScalarHomeRequirement {
                defining_operation: operation,
                source_value: result.value,
                scalar_type,
                shape: crate::lowering::scalar::scalar_shape(result.value, scalar_type, true)?,
            })
        })
        .transpose()?;
    if let Some(home) = result_home {
        crate::lowering::unit::known_integers::insert_known_unit_integer(
            &mut live.integers,
            home.source_value,
            KnownUnitInteger::Home(home),
        )?;
    }
    operations.push(TargetUnitOperation::AtomicAccess {
        psi_operation: operation,
        location: terminal_psi::StructuralArgument {
            place: location.root,
            access: parameter.access,
            path: location.path.clone(),
        },
        field: location.field,
        scalar_type: integer,
        field_byte_offset,
        event: access,
        operands,
        result_home,
    });
    provenance.operations.push(operation);
    Ok(())
}
