//! Concatenation retains its two ordered, measured sources and establishes a
//! fresh owned home. Its read descriptor never changes ownership of that home.
//! Capacity sizes backing storage; only the checked live lengths authorize reads.

use super::LiveDefinitions;
use crate::LoweringError;
use crate::lowering::function_signature::PreparedFunctionSignature;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::{AbstractFunction, AbstractOperation};
use semantic_vocabulary::IntegerSign;
use target_operations::{TargetUnitOperation, TerminalPsiProvenance};
use terminal_psi::control_flow::ByteSequenceOperand;
use terminal_psi::{
    ByteSequenceCarrier, StructuralAccess, StructuralMultiplicity, StructuralTypeShape,
};

pub(super) fn lower(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    prepared: &PreparedFunctionSignature,
    live: &mut LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let invalid = || LoweringError::unsupported_control_flow(function.machine);
    let AbstractOperation::EstablishByteSequenceConcatenation {
        psi_operation,
        result,
        left,
        right,
        capacity_obligation,
    } = operation
    else {
        return Err(invalid());
    };
    if result.multiplicity != StructuralMultiplicity::Unrestricted
        || !result.claims.is_empty()
        || !result.projected_qualifications.is_empty()
        || !result.qualification_establishments.is_empty()
        || !types
            .get(&result.structural_type)
            .is_some_and(|declaration| {
                matches!(
                    declaration.shape,
                    StructuralTypeShape::ByteSequence(ByteSequenceCarrier::BoundedOwned { .. })
                )
            })
    {
        return Err(invalid());
    }
    for operand in [left, right] {
        let (source, length, whole_source) = match operand {
            ByteSequenceOperand::Whole { source, length } => {
                if live.lengths.get(length) != Some(source)
                    || !(live.views.contains_key(source)
                        || live.block_views.contains(source)
                        || prepared
                            .parameters
                            .iter()
                            .any(|parameter| parameter.place == *source)
                        || super::observations::bounded_owner_storage(
                            function, types, live, *source,
                        ))
                {
                    return Err(invalid());
                }
                (*source, *length, true)
            }
            ByteSequenceOperand::Field {
                source,
                field,
                length,
            } => {
                let authority = super::primitive_storage::readable_byte_field(
                    function,
                    types,
                    prepared,
                    live,
                    source.place,
                    &source.path,
                    *field,
                )?;
                if source.access != StructuralAccess::SharedBorrow
                    || authority.access == StructuralAccess::WriteOnlyBorrow
                    || !function.operations.iter().any(|candidate| {
                        matches!(candidate,
                        AbstractOperation::StructuralByteSequenceFieldLength {
                            source: measured, path, field: measured_field, result, ..
                        } if *measured == source.place && *path == source.path
                            && measured_field == field && result.value == *length)
                    })
                {
                    return Err(invalid());
                }
                (source.place, *length, false)
            }
        };
        if source == result.place
            || (whole_source && super::references::check_root_access(live, source))
        {
            return Err(invalid());
        }
        let scalar = live
            .integers
            .get(&length)
            .ok_or_else(invalid)?
            .scalar_type();
        if scalar.sign() != IntegerSign::Unsigned || scalar.bits() != 64 || scalar.is_address() {
            return Err(invalid());
        }
    }
    let home = super::aggregate_results::home(*psi_operation, result, types)?;
    if live.structural_homes.insert(result.place, home).is_some()
        || live
            .views
            .insert(result.place, (*psi_operation, result.structural_type))
            .is_some()
    {
        return Err(invalid());
    }
    operations.push(TargetUnitOperation::EstablishByteSequenceConcatenation {
        psi_operation: *psi_operation,
        result: result.clone(),
        left: left.clone(),
        right: right.clone(),
        capacity_obligation: *capacity_obligation,
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}
