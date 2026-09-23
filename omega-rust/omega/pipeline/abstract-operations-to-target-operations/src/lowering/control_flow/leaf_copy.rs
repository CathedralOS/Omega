//! Read-only leaf copies retain their root, resolved extent, and result home.
use super::LiveDefinitions;
use crate::LoweringError;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::{AbstractFunction, AbstractOperation};
use semantic_vocabulary::{IntegerSign, IntegerType, ScalarType};
use std::collections::{BTreeMap, BTreeSet};
use target_operations::{TargetUnitOperation, TerminalPsiProvenance};

/// A leaf copy reads an owned home or a readable parameter root; the
/// verifier's copyable-path replay already excluded every other source kind.
/// The static projection must land on the declared leaf type, and the leaf's
/// canonical extent must equal the result home's own shape, or a raw copy
/// would write a different layout than the home carries.
pub(super) fn copy(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    prepared: &crate::lowering::function_signature::PreparedFunctionSignature,
    live: &mut LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let invalid = || LoweringError::UnsupportedControlFlow(function.machine);
    let AbstractOperation::StructuralLeafCopy {
        psi_operation,
        result,
        source,
        path,
    } = operation
    else {
        return Err(invalid());
    };
    let root_type = if let Some(home) = live.structural_homes.get(source) {
        if home.has_claims()
            || !home.qualifications().is_empty()
            || !home.projected_qualifications().is_empty()
        {
            return Err(invalid());
        }
        home.structural_type()
    } else {
        super::structural_case::parameter_root(prepared, *source)
            .ok_or_else(invalid)?
            .structural_type
    };
    let (endpoint, shape, byte_offset, indices) = if path.is_empty() {
        (
            root_type,
            crate::lowering::structural_layout::structural_shape(
                root_type,
                types,
                &mut BTreeMap::new(),
                &mut BTreeSet::new(),
            )?,
            0,
            Vec::new(),
        )
    } else {
        crate::lowering::structural_layout::leaf_copy_projection(
            root_type,
            path,
            types,
            &mut BTreeMap::new(),
            &mut BTreeSet::new(),
        )?
    };
    if endpoint != result.structural_type {
        return Err(invalid());
    }
    let indices = indices
        .into_iter()
        .map(|(selector, stride)| {
            let parameter = usize::try_from(selector)
                .ok()
                .and_then(|position| function.parameters.get(position).copied())
                .ok_or_else(invalid)?;
            let unsigned_64 = IntegerType::new(IntegerSign::Unsigned, 64).map_err(|_| invalid())?;
            if parameter.scalar_type != ScalarType::Integer(unsigned_64) {
                return Err(invalid());
            }
            Ok(target_operations::TargetStructuralRuntimeIndex {
                operand: target_operations::TargetUnitScalarArgumentSource::Parameter {
                    parameter_index: selector,
                    source_value: parameter.value,
                    scalar_type: parameter.scalar_type,
                },
                stride,
            })
        })
        .collect::<Result<Vec<_>, LoweringError>>()?;
    let result_home = super::aggregate_results::home(*psi_operation, result, types)?;
    if result_home.layout.shape() != shape {
        return Err(invalid());
    }
    if live
        .structural_homes
        .insert(result.place, result_home.clone())
        .is_some()
    {
        return Err(invalid());
    }
    operations.push(TargetUnitOperation::StructuralLeafCopy {
        psi_operation: *psi_operation,
        result_home,
        source: *source,
        path: path.clone(),
        byte_offset,
        indices,
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}
