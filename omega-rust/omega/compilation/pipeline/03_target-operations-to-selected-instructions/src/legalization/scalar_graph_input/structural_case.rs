//! Input-only case custody. Construction and replay produce separate outputs.
use super::{
    AbstractOperation, AbstractOperationPlan, OptimizationNode, PsiOptimizationFunction,
    PsiProvenance,
};
use crate::LegalizationError;
use semantic_vocabulary::{
    IntegerSign, IntegerType, OperationId, PlaceId, ScalarType, StructuralTypeId,
};
use terminal_psi::{
    StructuralOperationResult, StructuralPathSegment, StructuralTypeDeclaration,
    StructuralTypeShape, find_structural_type,
};

pub(in crate::legalization) fn source_result(
    function: &PsiOptimizationFunction,
    source: PlaceId,
) -> Result<(OperationId, &StructuralOperationResult), LegalizationError> {
    let mut matching = function.block_nodes().filter_map(|node| {
        let (producer, result) = node.operation.structural_result()?;
        (result.place == source).then_some((producer, result))
    });
    let result = matching.next().ok_or(LegalizationError::custody())?;
    if matching.next().is_some() {
        return Err(LegalizationError::custody());
    }
    Ok(result)
}

pub(in crate::legalization) fn source_owner(
    function: &PsiOptimizationFunction,
    place: PlaceId,
) -> Result<legalized_operations::LegalizedStructuralCaseSource, LegalizationError> {
    if let Ok((operation, result)) = source_result(function, place) {
        return Ok(
            legalized_operations::LegalizedStructuralCaseSource::OperationResult {
                operation,
                result: result.clone(),
            },
        );
    }
    let mut parameters = function.blocks.iter().flat_map(|block| {
        block
            .structural_parameters
            .iter()
            .filter(move |parameter| parameter.place == place)
            .map(move |parameter| (block.id, parameter))
    });
    let (block, declaration) = parameters.next().ok_or(LegalizationError::custody())?;
    if parameters.next().is_some() || declaration.access != terminal_psi::StructuralAccess::Owned {
        return Err(LegalizationError::custody());
    }
    Ok(
        legalized_operations::LegalizedStructuralCaseSource::BlockParameter {
            block,
            declaration: declaration.clone(),
        },
    )
}

/// The owner a case dispatch inspects: a stored result or block arrival as
/// `source_owner` resolves it, otherwise the function's own owned incoming
/// parameter under the same owned-arrival contract a block arrival meets. A
/// readable borrowed parameter dispatches the caller's referent through its
/// borrow pointer instead of an activation value copy. Other structural
/// consumers keep `source_owner` and never see a parameter.
pub(in crate::legalization) fn case_source(
    function: &PsiOptimizationFunction,
    place: PlaceId,
) -> Result<legalized_operations::LegalizedStructuralCaseSource, LegalizationError> {
    if let Ok(owner) = source_owner(function, place) {
        return Ok(owner);
    }
    let mut parameters = function
        .structural_parameters
        .iter()
        .filter(|parameter| parameter.place == place);
    if let Some(declaration) = parameters.next() {
        if parameters.next().is_some() {
            return Err(LegalizationError::custody());
        }
        return match declaration.access {
            terminal_psi::StructuralAccess::SharedBorrow
            | terminal_psi::StructuralAccess::MutableBorrow => Ok(
                legalized_operations::LegalizedStructuralCaseSource::BorrowedParameter {
                    declaration: declaration.clone(),
                },
            ),
            terminal_psi::StructuralAccess::Owned => Ok(
                legalized_operations::LegalizedStructuralCaseSource::Parameter {
                    declaration: declaration.clone(),
                },
            ),
            _ => Err(LegalizationError::custody()),
        };
    }
    // A readable borrowed block parameter joins as its referent's address: the
    // dispatch reads through the edge-retained pointer the join minted, under
    // the borrowed-parameter contract rather than an owned arrival's.
    let mut parameters = function.blocks.iter().flat_map(|block| {
        block
            .structural_parameters
            .iter()
            .filter(move |parameter| parameter.place == place)
    });
    let declaration = parameters.next().ok_or(LegalizationError::custody())?;
    if parameters.next().is_some()
        || !matches!(
            declaration.access,
            terminal_psi::StructuralAccess::SharedBorrow
                | terminal_psi::StructuralAccess::MutableBorrow
        )
    {
        return Err(LegalizationError::custody());
    }
    Ok(
        legalized_operations::LegalizedStructuralCaseSource::BorrowedParameter {
            declaration: declaration.clone(),
        },
    )
}

/// The inspected root's structural type: an established result or a readable
/// function/block parameter. Inspecting a tag retains the carrier's claims,
/// cleanup and refinements; it does not copy or dispose of that carrier.
fn source_identity(
    function: &PsiOptimizationFunction,
    source: PlaceId,
) -> Result<semantic_vocabulary::StructuralTypeId, LegalizationError> {
    if let Ok((_, result)) = source_result(function, source) {
        return Ok(result.structural_type);
    }
    // Literal carriers have a declared home and Unit establishment. Reuse
    // the same exact producer join as other descriptor geometry consumers.
    if let Some((_, identity)) = super::literals::declaration_producer(function, source) {
        return Ok(identity);
    }
    let parameter = function
        .structural_parameters
        .iter()
        .chain(function.block_structural_parameters())
        .find(|parameter| parameter.place == source)
        .ok_or(LegalizationError::custody())?;
    if parameter.access == terminal_psi::StructuralAccess::WriteOnlyBorrow {
        return Err(LegalizationError::custody());
    }
    Ok(parameter.structural_type)
}

/// Resolve a captured dispatch projection under the root's existing custody.
/// A reference root retains its descriptor extent; the sum is bounded by the
/// final backing region, with offsets restarting after each pointer crossing.
#[allow(clippy::type_complexity)]
pub(in crate::legalization) fn dispatch_projection(
    function: &PsiOptimizationFunction,
    source: PlaceId,
    path: &[StructuralPathSegment],
    plan: &AbstractOperationPlan,
) -> Result<
    (
        StructuralTypeId,
        u32,
        calling_conventions::ValueShape,
        calling_conventions::ConventionalSumLayout,
    ),
    LegalizationError,
> {
    let root = source_identity(function, source)?;
    let geometry = crate::structural_inputs::structural_reference_input::reference_source_geometry(
        root,
        path,
        &plan.structural_types,
    )
    .ok_or(LegalizationError::custody())?;
    if !geometry.elements.is_empty()
        || !geometry.readable
        || geometry
            .pointer_slots
            .iter()
            .any(|slot| slot.scalar_descriptor)
    {
        return Err(LegalizationError::custody());
    }
    let selected = geometry.selected;
    let byte_offset = geometry.byte_offset;
    let root_shape = super::aggregate_results::type_home_layout(root, plan)?.shape();
    let region_shape =
        super::aggregate_results::type_home_layout(geometry.region_root, plan)?.shape();
    let layout = super::aggregate_results::sum_type_layout(selected, plan)?;
    if byte_offset
        .checked_add(layout.shape.byte_size)
        .is_none_or(|end| end > region_shape.byte_size)
    {
        return Err(LegalizationError::custody());
    }
    Ok((selected, byte_offset, root_shape, layout))
}

#[allow(clippy::type_complexity)]
pub(in crate::legalization) fn membership_layout(
    function: &PsiOptimizationFunction,
    source: PlaceId,
    path: &[terminal_psi::StructuralPathSegment],
    case: semantic_vocabulary::StructuralCaseId,
    plan: &AbstractOperationPlan,
) -> Result<
    (
        u32,
        u32,
        Vec<crate::structural_inputs::structural_reference_input::RuntimeElement>,
    ),
    LegalizationError,
> {
    let identity = source_identity(function, source)?;
    let geometry = crate::structural_inputs::structural_reference_input::reference_source_geometry(
        identity,
        path,
        &plan.structural_types,
    )
    .ok_or(LegalizationError::custody())?;
    if !geometry.readable
        || geometry
            .pointer_slots
            .iter()
            .any(|slot| slot.scalar_descriptor)
    {
        return Err(LegalizationError::custody());
    }
    let identity = geometry.selected;
    let byte_offset = geometry.byte_offset;
    let layout = super::aggregate_results::sum_type_layout(identity, plan)?;
    if layout.tag_byte_offset != 0
        || layout.tag_shape != calling_conventions::ValueShape::integer(4, 4)
    {
        return Err(LegalizationError::custody());
    }
    let declaration = find_structural_type(plan.structural_types.as_slice(), identity)
        .ok_or(LegalizationError::custody())?;
    let cases: &[terminal_psi::StructuralCaseDeclaration] = match &declaration.shape {
        terminal_psi::StructuralTypeShape::Sum { cases }
        | terminal_psi::StructuralTypeShape::Mixed { cases, .. } => cases,
        _ => return Err(LegalizationError::custody()),
    };
    cases
        .iter()
        .position(|candidate| candidate.id == case)
        .and_then(|ordinal| u32::try_from(ordinal).ok())
        .map(|tag| (tag, byte_offset, geometry.elements))
        .ok_or(LegalizationError::custody())
}

/// Resolve the copied leaf's byte offset inside the readable root and its
/// canonical shape. The path's endpoint type must be the declared result type
/// and the result must keep the copy's unrestricted, claim-free custody.
/// Domain memberships neither change storage geometry nor grant ownership.
/// `RuntimeIndex` segments additionally yield their runtime elements in path
/// order: each index must resolve to a dominating integer scalar source no
/// wider than the address model — an incoming parameter, a block parameter,
/// or a computed definition — the operand contract target lowering applies
/// to leaf copies.
pub(in crate::legalization) fn leaf_copy_layout(
    function: &PsiOptimizationFunction,
    operation: OperationId,
    source: PlaceId,
    path: &abstract_operations::StructuralCopyPath,
    result: &StructuralOperationResult,
    plan: &AbstractOperationPlan,
) -> Result<
    (
        u32,
        calling_conventions::ValueShape,
        Vec<crate::structural_inputs::structural_reference_input::RuntimeElement>,
    ),
    LegalizationError,
> {
    // Every copy retains source-checked memberships independently of geometry.
    // This layout join checks readable storage and the selected carrier; it
    // neither invents predicates nor imposes a separate byte-only proof mode.
    let identity = source_identity(function, source)?;
    if !result.qualification_establishments.is_empty() {
        return Err(LegalizationError::custody());
    }
    let (endpoint, byte_offset, indices) = match path {
        abstract_operations::StructuralCopyPath::Case(path) => {
            let (endpoint, offset) =
                crate::structural_inputs::structural_reference_input::case_projection(
                    identity,
                    path,
                    &plan.structural_types,
                )
                .ok_or(LegalizationError::custody())?;
            (endpoint, offset, Vec::new())
        }
        abstract_operations::StructuralCopyPath::Ordinary(path) if path.is_empty() => {
            (identity, 0, Vec::new())
        }
        abstract_operations::StructuralCopyPath::Ordinary(path) => {
            let geometry =
                crate::structural_inputs::structural_reference_input::leaf_copy_geometry(
                    identity,
                    path,
                    result.structural_type,
                    &plan.structural_types,
                )
                .filter(|geometry| geometry.readable)
                .ok_or(LegalizationError::custody())?;
            (geometry.selected, geometry.byte_offset, geometry.elements)
        }
    };
    // A case-qualified path also carries the verifier's owned-copy shape: an
    // `Affine` non-reference leaf is copied out of the proven-active payload
    // into fresh storage the successor takes custody of.
    let owned_case_copy = matches!(path, abstract_operations::StructuralCopyPath::Case(_))
        && result.multiplicity == terminal_psi::StructuralMultiplicity::Affine;
    let shared_descriptor = terminal_semantics::is_shared_reference_carrier(
        plan.structural_types.iter(),
        result.structural_type,
    );
    if (result.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
        && !owned_case_copy
        && !(result.multiplicity == terminal_psi::StructuralMultiplicity::Affine
            && shared_descriptor))
        || !result.claims.is_empty()
    {
        return Err(LegalizationError::custody());
    }
    // A byte-carrier copy projects a `u8` array element into the overlaid
    // record the verifier's recast window admitted; the legalized extent is
    // the record's own layout, not the element's scalar shape.
    let byte_carrier = endpoint != result.structural_type
        && byte_carrier_leaf_copy(
            &plan.structural_types,
            identity,
            path,
            result.structural_type,
        );
    let copy_type = if byte_carrier {
        result.structural_type
    } else {
        endpoint
    };
    let quotient = matches!(path, abstract_operations::StructuralCopyPath::Ordinary(path) if path.is_empty())
        && abstract_operations::quotients::licensed_copy(
            &plan.structural_types,
            function.machine,
            operation,
            source,
            identity,
            result,
            function.block_nodes().map(|node| &node.operation),
        );
    let occurrence_access = match path {
        abstract_operations::StructuralCopyPath::Ordinary(path) if path.is_empty() => function
            .arrival_parameter(source)
            .map(|parameter| parameter.access)
            .or_else(|| {
                function.block_nodes().find_map(|node| {
                    let (_, result) = node.operation.structural_result()?;
                    (result.place == source)
                        .then(|| {
                            terminal_semantics::descriptor_results::descriptor_value_access(
                                plan.structural_types.iter(),
                                result.structural_type,
                                result.multiplicity,
                            )
                        })
                        .flatten()
                })
            }),
        _ => None,
    };
    if endpoint != result.structural_type
        && !byte_carrier
        && !quotient
        && !terminal_semantics::shared_descriptor_copy_matches(
            plan.structural_types.iter(),
            endpoint,
            occurrence_access,
            result.structural_type,
            result.multiplicity,
        )
    {
        return Err(LegalizationError::custody());
    }
    // Geometry copies descriptor bytes; the independent custody replay joins
    // their exact shared leaves and preserves the original live source roster.
    // This never authorizes duplication of an exclusive reference or claims.
    if !super::reference_custody::shared_copy_type(&plan.structural_types, copy_type) {
        return Err(LegalizationError::custody());
    }
    for element in &indices {
        crate::legalization::runtime_indices::defined_type(function, element.index)
            .filter(|scalar| {
                matches!(scalar, semantic_vocabulary::ScalarType::Integer(integer)
                    if integer.bits() <= 64)
            })
            .ok_or(LegalizationError::custody())?;
    }
    let shape = crate::structural_inputs::structural_reference_input::shape(
        copy_type,
        &plan.structural_types,
    )
    .ok_or(LegalizationError::custody())?;
    // Nominal adaptation is licensed by its exact retained occurrence, while
    // the physical copy independently preserves both endpoints' geometry.
    if quotient
        && crate::structural_inputs::structural_reference_input::shape(
            result.structural_type,
            &plan.structural_types,
        ) != Some(shape.clone())
    {
        return Err(LegalizationError::custody());
    }
    Ok((byte_offset, shape, indices))
}

/// Whether `path` ends at a literal index inside a `u8` fixed array whose
/// remaining elements host `expected`'s fixed scalar layout — the same
/// window the verifier's `byte_carrier_leaf_copy` admission proved. A
/// runtime-selected index cannot prove the wider `index + extent` bound, so
/// it stays outside this arm.
fn byte_carrier_leaf_copy(
    declarations: &[StructuralTypeDeclaration],
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
    let Some((parent, _, _)) =
        crate::structural_inputs::structural_reference_input::runtime_projection(
            root,
            prefix,
            declarations,
        )
    else {
        return false;
    };
    let Some(declaration) = find_structural_type(declarations, parent) else {
        return false;
    };
    let StructuralTypeShape::FixedArray { element, length } = &declaration.shape else {
        return false;
    };
    let byte_type =
        ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 is valid"));
    if !declarations.iter().any(|candidate| {
        candidate.id == *element
            && candidate.shape == StructuralTypeShape::PrimitiveScalar(byte_type)
    }) {
        return false;
    };
    let Some(layout) =
        terminal_semantics::byte_carrier_scalar_layout(declarations.iter(), expected)
    else {
        return false;
    };
    index
        .checked_add(layout.size)
        .is_some_and(|end| end <= *length)
}

pub(super) fn validate(
    node: &OptimizationNode,
    function: &PsiOptimizationFunction,
    structural_types: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<(), LegalizationError> {
    let AbstractOperation::StructuralCase { source, cases, .. } = &node.operation else {
        return Err(LegalizationError::custody());
    };
    case_source(function, *source)?;
    if !node.provenance.is_empty()
        || !node.fuel.is_empty()
        || !node.definitions.is_empty()
        || cases.len() != node.successors.len()
    {
        return Err(LegalizationError::custody());
    }
    for (case, edge) in cases.iter().zip(&node.successors) {
        if edge.psi_edge != case.psi_edge
            || edge.target != case.target
            || !edge.bindings.is_empty()
            || !edge.structural_bindings.is_empty()
            || edge.cleanup_actions != case.cleanup_actions
            || edge.provenance != [PsiProvenance::Edge(case.psi_edge)]
            || edge.fuel.is_empty()
            || edge
                .fuel
                .iter()
                .any(|fuel| fuel.site != PsiProvenance::Edge(case.psi_edge))
        {
            return Err(LegalizationError::custody());
        }
        // A case edge's discards are admitted by the same rule as every other
        // edge's and every return's: a plain affine result, block arrival, or
        // the function's own owned parameter — which is what a dispatch on an
        // owned parameter leaves behind on each arm.
        if !super::aggregate_results::cleanup(function, &case.cleanup_actions, structural_types) {
            return Err(LegalizationError::custody());
        }
    }
    Ok(())
}
