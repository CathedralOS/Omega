//! Read-only leaf copies retain their root, resolved extent, and result home.
//!
//! The same row also realizes a borrowed-window move (`borrowed_windows.rs`):
//! both copy one resolved extent of a root into a fresh home, and differ only
//! in the custody the abstract operation already verified.
use super::LiveDefinitions;
use crate::LoweringError;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::{AbstractFunction, AbstractOperation};
use semantic_vocabulary::{OperationId, PlaceId, StructuralTypeId};
use std::collections::{BTreeMap, BTreeSet};
use target_operations::{TargetUnitOperation, TerminalPsiProvenance};
use terminal_psi::{StructuralOperationResult, StructuralPathSegment};

/// A leaf copy reads an owned home or a readable parameter root; the
/// verifier's copyable-path replay already excluded every other source kind.
pub(super) fn copy(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    prepared: &crate::lowering::function_signature::PreparedFunctionSignature,
    live: &mut LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let AbstractOperation::StructuralLeafCopy {
        psi_operation,
        result,
        source,
        path,
    } = operation
    else {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    };
    let root_type = if let Some(home) = live.structural_homes.get(source) {
        if home.has_claims()
            || !home.qualifications().is_empty()
            || !home.projected_qualifications().is_empty()
        {
            return Err(LoweringError::unsupported_control_flow(function.machine));
        }
        home.structural_type()
    } else {
        super::structural_case::parameter_root(prepared, *source)
            .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?
            .structural_type
    };
    copy_extent(
        *psi_operation,
        result,
        *source,
        path,
        root_type,
        function,
        types,
        live,
        operations,
        provenance,
    )
}

/// Copy the extent `path` names beneath a `root_type` root at `source` into
/// `result`'s fresh home. The static projection must land on the declared
/// result type, and the extent's canonical shape must equal the result home's
/// own shape, or a raw copy would write a different layout than the home
/// carries.
#[allow(clippy::too_many_arguments)]
pub(super) fn copy_extent(
    psi_operation: OperationId,
    result: &StructuralOperationResult,
    source: PlaceId,
    path: &[StructuralPathSegment],
    root_type: StructuralTypeId,
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    live: &mut LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
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
        crate::lowering::structural_layout::runtime_projection(
            root_type,
            path,
            types,
            &mut BTreeMap::new(),
            &mut BTreeSet::new(),
        )?
    };
    if endpoint != result.structural_type {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    // A selector resolves to its exact dominating scalar source — an
    // incoming parameter, a block parameter, or a stored/computed value in
    // its durable home — the same admission the primitive store and read
    // siblings use. The segment's verified bound obligation stays attached;
    // only the operand's home is found here.
    let indices = super::scalar_sources::runtime_indices(
        indices,
        function,
        &super::scalar_sources::ScalarSources::from(&*live),
    )?;
    let result_home = super::aggregate_results::home(psi_operation, result, types)?;
    if result_home.layout.shape() != shape {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    if live
        .structural_homes
        .insert(result.place, result_home.clone())
        .is_some()
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    operations.push(TargetUnitOperation::StructuralLeafCopy {
        psi_operation,
        result_home,
        source,
        path: path.to_vec(),
        byte_offset,
        indices,
    });
    provenance.operations.push(psi_operation);
    Ok(())
}
