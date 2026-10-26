//! Preserve atomic accesses as events while resolving their scalar operands
//! and aligned field locations. Splitting a fetch into a read and a store
//! would lose the instruction-observed prior before instruction selection.
//! Fences retain their location-free ordering event; single-attempt exchange
//! retains its complete structural result and independently derived home.

use super::{LiveDefinitions, RowSite};
use crate::LoweringError;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::{AbstractAtomicEvent, AbstractFunction};
use semantic_vocabulary::OperationId;
use target_operations::{TargetUnitOperation, TerminalPsiProvenance};
use terminal_psi::{AtomicAccessEvent, StructuralAccess};

pub(super) fn lower(
    operation: OperationId,
    event: &AbstractAtomicEvent,
    site: &mut RowSite<'_>,
    prepared: &crate::lowering::function_signature::PreparedFunctionSignature,
) -> Result<(), LoweringError> {
    let (function, types, live, operations, provenance) = site.parts();
    if let AbstractAtomicEvent::Fence { ordering } = event {
        operations.push(TargetUnitOperation::AtomicFence {
            psi_operation: operation,
            ordering: *ordering,
        });
        provenance.operations.push(operation);
        return Ok(());
    }
    let location = event
        .location()
        .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
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
                operation: (*operation).into(),
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
        AbstractAtomicEvent::CompareExchangeOnce {
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
        AbstractAtomicEvent::Fence { .. } => {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
    };
    let (root, root_access) =
        super::primitive_storage::field_root(function, types, prepared, live, location.root)?;
    if !access.ordering_is_legal()
        || access.observes_resident() && root_access == StructuralAccess::WriteOnlyBorrow
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let field = terminal_semantics::projected_field_declaration(
        types.values().copied(),
        root,
        &location.path,
        location.field,
    )
    .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
    let scalar_type = field
        .field_type
        .atomic_scalar_type()
        .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
    let integer = target_operations::operations::atomic_storage_integer_type(access, scalar_type)
        .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
    let regions = crate::lowering::structural_layout::reference_projection_regions(
        root,
        &location.path,
        types,
    )
    .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
    if !regions.readable
        || regions
            .pointer_slots
            .iter()
            .any(|slot| !slot.indices.is_empty())
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let (field_byte_offset, bytes) = crate::lowering::structural_layout::static_field_geometry(
        regions.root,
        regions.path,
        location.field,
        scalar_type,
        types,
    )?;
    if bytes != u32::from(integer.bits() / 8) || field_byte_offset % bytes != 0 {
        return Err(LoweringError::unsupported_control_flow(function.machine));
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
            // An atomic observation is an ordinary typed scalar definition.
            // Retain it in the Boolean or integer home roster according to
            // its logical type, rather than classifying its physical byte lane.
            super::primitive_storage::retain_result(operation, result, live)
        })
        .transpose()?;
    let outcome_home = if let AbstractAtomicEvent::CompareExchangeOnce { outcome, .. } = event {
        if !event.custody_is_consistent()
            || !outcome.is_unrestricted_plain()
            || !outcome.qualification_establishments.is_empty()
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        let layout = types
            .get(&outcome.structural_type)
            .and_then(|row| target_operations::conventional_atomic_once_layout(row, integer))
            .ok_or(LoweringError::unsupported_control_flow(function.machine))?;
        let home = target_operations::TargetStructuralHomeRequirement {
            origin: target_operations::TargetStructuralHomeOrigin::OperationResult {
                operation,
                result: outcome.clone(),
            },
            layout: target_operations::TargetStructuralHomeLayout::Sum(layout),
        };
        if live
            .structural_homes
            .insert(outcome.place, home.clone())
            .is_some()
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        Some(home)
    } else {
        None
    };
    operations.push(TargetUnitOperation::AtomicAccess {
        psi_operation: operation,
        location: terminal_psi::StructuralArgument {
            place: location.root,
            access: root_access,
            path: location.path.clone(),
        },
        field: location.field,
        scalar_type: integer,
        field_byte_offset,
        event: access,
        operands,
        result_home,
        outcome_home,
    });
    provenance.operations.push(operation);
    Ok(())
}
