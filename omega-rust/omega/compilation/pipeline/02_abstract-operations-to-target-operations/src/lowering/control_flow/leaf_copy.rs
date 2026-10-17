//! Read-only leaf copies retain their root, resolved extent, and result home.
//!
//! The same row also realizes a borrowed-window move (`borrowed_windows.rs`):
//! both copy one resolved extent of a root into a fresh home, and differ only
//! in the custody the abstract operation already verified.
use super::LiveDefinitions;
use crate::LoweringError;
use crate::lowering::structural_type_lookup::StructuralTypeLookup;
use abstract_operations::{AbstractFunction, AbstractOperation};
use semantic_vocabulary::{
    IntegerSign, IntegerType, OperationId, PlaceId, ScalarType, StructuralTypeId,
};
use std::collections::{BTreeMap, BTreeSet};
use target_operations::{TargetUnitOperation, TerminalPsiProvenance};
use terminal_psi::{StructuralOperationResult, StructuralPathSegment, StructuralTypeShape};

/// A leaf copy reads one currently observable structural root. The view stays
/// the source identity; selection separately resolves its backing pointer
/// rather than copying its descriptor. Owned homes retain their additional
/// claim and qualification obligations below.
pub(super) fn copy(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
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
    let root_type = live
        .structural_observation_type(function, *source)
        .ok_or_else(|| LoweringError::unsupported_control_flow(function.machine))?;
    if let Some(home) = live.structural_homes.get(source)
        && home.has_claims()
    {
        return Err(LoweringError::unsupported_control_flow(function.machine));
    }
    let copied = super::references::copied_leaves(function, types, live, result, *source, path)?;
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
    )?;
    live.references.extend(copied);
    Ok(())
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
    path: &abstract_operations::StructuralCopyPath,
    root_type: StructuralTypeId,
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    live: &mut LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
    let (endpoint, shape, byte_offset, indices) = match path {
        abstract_operations::StructuralCopyPath::Case(path) => {
            let (endpoint, shape, offset) = crate::lowering::structural_layout::case_projection(
                root_type,
                path,
                types,
                &mut BTreeMap::new(),
                &mut BTreeSet::new(),
            )?;
            (endpoint, shape, offset, Vec::new())
        }
        abstract_operations::StructuralCopyPath::Ordinary(path) if path.is_empty() => (
            root_type,
            crate::lowering::structural_layout::structural_shape(
                root_type,
                types,
                &mut BTreeMap::new(),
                &mut BTreeSet::new(),
            )?,
            0,
            Vec::new(),
        ),
        abstract_operations::StructuralCopyPath::Ordinary(path) => {
            crate::lowering::structural_layout::runtime_leaf_copy_projection(
                root_type,
                path,
                result.structural_type,
                types,
                &mut BTreeMap::new(),
                &mut BTreeSet::new(),
            )?
        }
    };
    // A byte-carrier copy projects a `u8` array element into the overlaid
    // record the verifier's recast window admitted; its copied extent is the
    // record's own layout, not the element's scalar shape.
    let byte_carrier = endpoint != result.structural_type
        && byte_carrier_leaf_copy(types, root_type, path, result.structural_type);
    let shape = if byte_carrier {
        crate::lowering::structural_layout::structural_shape(
            result.structural_type,
            types,
            &mut BTreeMap::new(),
            &mut BTreeSet::new(),
        )?
    } else {
        shape
    };
    let quotient = matches!(path, abstract_operations::StructuralCopyPath::Ordinary(path) if path.is_empty())
        && abstract_operations::quotients::licensed_copy(
            types.catalog(),
            function.machine,
            psi_operation,
            source,
            root_type,
            result,
            function.operations.iter(),
        );
    let occurrence_access = match path {
        abstract_operations::StructuralCopyPath::Ordinary(path) if path.is_empty() => function
            .arrival_parameter(source)
            .map(|parameter| parameter.access)
            .or_else(|| {
                function.operations.iter().find_map(|operation| {
                    let declaration = operation.structural_storage()?;
                    (declaration.place == source).then(|| {
                        terminal_semantics::descriptor_results::descriptor_value_access(
                            types.catalog().iter(),
                            declaration.structural_type,
                            declaration.multiplicity,
                        )
                        .unwrap_or(declaration.access)
                    })
                })
            }),
        _ => None,
    };
    if endpoint != result.structural_type
        && !byte_carrier
        && !quotient
        && !terminal_semantics::shared_descriptor_copy_matches(
            types.catalog().iter(),
            endpoint,
            occurrence_access,
            result.structural_type,
            result.multiplicity,
        )
    {
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
        path: path.clone(),
        byte_offset,
        indices,
    });
    provenance.operations.push(psi_operation);
    Ok(())
}

/// Whether `path` ends at a literal index inside a `u8` fixed array whose
/// remaining elements host `expected`'s fixed scalar layout — the same
/// window the verifier's `byte_carrier_leaf_copy` admission proved. A
/// runtime-selected index cannot prove the wider `index + extent` bound, so
/// it stays outside this arm.
fn byte_carrier_leaf_copy(
    types: &StructuralTypeLookup<'_>,
    root: StructuralTypeId,
    path: &abstract_operations::StructuralCopyPath,
    expected: StructuralTypeId,
) -> bool {
    let abstract_operations::StructuralCopyPath::Ordinary(path) = path else {
        return false;
    };
    let Some((StructuralPathSegment::FixedIndex(index), prefix)) = path.split_last() else {
        return false;
    };
    let Some(parent) = types.subtree(root, prefix) else {
        return false;
    };
    let Some(declaration) = types.get(&parent) else {
        return false;
    };
    let StructuralTypeShape::FixedArray { element, length } = &declaration.shape else {
        return false;
    };
    let byte_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 is valid"));
    if !types.values().any(|candidate| {
        candidate.id == *element
            && candidate.shape == StructuralTypeShape::PrimitiveScalar(byte_type)
    }) {
        return false;
    };
    let Some(layout) =
        terminal_semantics::byte_carrier_scalar_layout(types.values().copied(), expected)
    else {
        return false;
    };
    index
        .checked_add(layout.size)
        .is_some_and(|end| end <= *length)
}
