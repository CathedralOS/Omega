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
    StructuralTypeShape,
};

pub(in crate::legalization) fn source_result(
    function: &PsiOptimizationFunction,
    source: PlaceId,
) -> Result<(OperationId, &StructuralOperationResult), LegalizationError> {
    let mut matching = function
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .filter_map(|node| {
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
    if parameters.next().is_some()
        || declaration.access != terminal_psi::StructuralAccess::Owned
        || declaration.multiplicity == terminal_psi::StructuralMultiplicity::Linear
        || !declaration.qualifications.is_empty()
        || !declaration.projected_qualifications.is_empty()
    {
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
        if parameters.next().is_some()
            || declaration.multiplicity == terminal_psi::StructuralMultiplicity::Linear
            || !declaration.qualifications.is_empty()
            || !declaration.projected_qualifications.is_empty()
            || !function.entry_claims.is_empty()
        {
            return Err(LegalizationError::custody());
        }
        return match declaration.access {
            terminal_psi::StructuralAccess::SharedBorrow
            | terminal_psi::StructuralAccess::MutableBorrow => Ok(
                legalized_operations::LegalizedStructuralCaseSource::BorrowedParameter {
                    declaration: declaration.clone(),
                },
            ),
            terminal_psi::StructuralAccess::Owned if !declaration.is_self => Ok(
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
        || declaration.multiplicity == terminal_psi::StructuralMultiplicity::Linear
        || !declaration.qualifications.is_empty()
        || !declaration.projected_qualifications.is_empty()
        || !function.entry_claims.is_empty()
    {
        return Err(LegalizationError::custody());
    }
    Ok(
        legalized_operations::LegalizedStructuralCaseSource::BorrowedParameter {
            declaration: declaration.clone(),
        },
    )
}

/// The inspected root's structural type: a live result home under the
/// same empty-custody contract, or a readable function/block parameter.
fn source_identity(
    function: &PsiOptimizationFunction,
    source: PlaceId,
) -> Result<semantic_vocabulary::StructuralTypeId, LegalizationError> {
    if let Ok((_, result)) = source_result(function, source) {
        if result.multiplicity == terminal_psi::StructuralMultiplicity::Linear
            || !result.claims.is_empty()
            || !result.qualifications.is_empty()
            || !result.projected_qualifications.is_empty()
        {
            return Err(LegalizationError::custody());
        }
        return Ok(result.structural_type);
    }
    let parameter = function
        .structural_parameters
        .iter()
        .chain(
            function
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters),
        )
        .find(|parameter| parameter.place == source)
        .ok_or(LegalizationError::custody())?;
    if parameter.access == terminal_psi::StructuralAccess::WriteOnlyBorrow
        || parameter.multiplicity == terminal_psi::StructuralMultiplicity::Linear
        || !parameter.qualifications.is_empty()
        || !parameter.projected_qualifications.is_empty()
        || !function.entry_claims.is_empty()
    {
        return Err(LegalizationError::custody());
    }
    Ok(parameter.structural_type)
}

/// Resolve the exact nominal case under the independently validated root
/// access: its tag, the tag's static byte offset, and each runtime element the
/// path selects through (`items[i]`), rebuilt from declarations as the
/// element's selector, obligation, stride and extent.
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
    let (identity, byte_offset, elements) = if path.is_empty() {
        (identity, 0, Vec::new())
    } else if terminal_psi::is_static_structural_path(path) {
        let (identity, byte_offset) =
            crate::structural_inputs::structural_reference_input::project(
                identity,
                path,
                &plan.structural_types,
            )
            .ok_or(LegalizationError::custody())?;
        (identity, byte_offset, Vec::new())
    } else {
        crate::structural_inputs::structural_reference_input::runtime_projection(
            identity,
            path,
            &plan.structural_types,
        )
        .ok_or(LegalizationError::custody())?
    };
    let layout = super::aggregate_results::sum_type_layout(identity, plan)?;
    if layout.tag_byte_offset != 0
        || layout.tag_shape != calling_conventions::ValueShape::integer(4, 4)
    {
        return Err(LegalizationError::custody());
    }
    let declaration = plan
        .structural_types
        .iter()
        .find(|declaration| declaration.id == identity)
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
        .map(|tag| (tag, byte_offset, elements))
        .ok_or(LegalizationError::custody())
}

/// Resolve the copied leaf's byte offset inside the readable root and its
/// canonical shape. The path's endpoint type must be the declared result type
/// and the result must keep the copy's unrestricted empty custody.
/// `RuntimeIndex` segments additionally yield their runtime elements in path
/// order: each index must resolve to a dominating integer scalar source no
/// wider than the address model — an incoming parameter, a block parameter,
/// or a computed definition — the operand contract target lowering applies
/// to leaf copies.
pub(in crate::legalization) fn leaf_copy_layout(
    function: &PsiOptimizationFunction,
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
    // Byte snapshots retain established qualifications rather than demand the
    // empty contract of case observations. Rejoin the exact source roster here;
    // this layout layer has no authority to establish a new predicate.
    let whole_bytes = matches!(path, abstract_operations::StructuralCopyPath::Ordinary(path) if path.is_empty())
        && plan.structural_types.iter().any(|declaration| {
            declaration.id == result.structural_type
                && matches!(
                    declaration.shape,
                    StructuralTypeShape::ByteSequence(
                        terminal_psi::ByteSequenceCarrier::BoundedOwned { .. }
                    )
                )
        });
    let identity = if whole_bytes {
        let exact = if let Ok((_, previous)) = source_result(function, source) {
            previous.structural_type == result.structural_type
                && previous.multiplicity == terminal_psi::StructuralMultiplicity::Unrestricted
                && previous.qualifications == result.qualifications
                && previous.projected_qualifications.is_empty()
                && previous.qualification_establishments.is_empty()
                && previous.claims.is_empty()
        } else {
            function
                .structural_parameters
                .iter()
                .chain(
                    function
                        .blocks
                        .iter()
                        .flat_map(|block| &block.structural_parameters),
                )
                .any(|parameter| {
                    parameter.place == source
                        && parameter.access != terminal_psi::StructuralAccess::WriteOnlyBorrow
                        && parameter.structural_type == result.structural_type
                        && parameter.multiplicity
                            == terminal_psi::StructuralMultiplicity::Unrestricted
                        && parameter.qualifications == result.qualifications
                        && parameter.projected_qualifications.is_empty()
                })
        };
        if !exact || !result.qualification_establishments.is_empty() {
            return Err(LegalizationError::custody());
        }
        result.structural_type
    } else {
        source_identity(function, source)?
    };
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
            crate::structural_inputs::structural_reference_input::runtime_leaf_copy_projection(
                identity,
                path,
                &plan.structural_types,
            )
            .ok_or(LegalizationError::custody())?
        }
    };
    // A case-qualified path also carries the verifier's owned-copy shape: an
    // `Affine` non-reference leaf is copied out of the proven-active payload
    // into fresh storage the successor takes custody of.
    let owned_case_copy = matches!(path, abstract_operations::StructuralCopyPath::Case(_))
        && result.multiplicity == terminal_psi::StructuralMultiplicity::Affine;
    if (result.multiplicity != terminal_psi::StructuralMultiplicity::Unrestricted
        && !owned_case_copy)
        || !result.claims.is_empty()
        || (!whole_bytes && !result.qualifications.is_empty())
        || !result.projected_qualifications.is_empty()
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
    if endpoint != result.structural_type && !byte_carrier {
        return Err(LegalizationError::custody());
    }
    if super::reference_custody::contains_reference(&plan.structural_types, copy_type) {
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
    let Some(declaration) = declarations.iter().find(|candidate| candidate.id == parent) else {
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
) -> Result<(), LegalizationError> {
    let AbstractOperation::StructuralCase { source, cases } = &node.operation else {
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
            || edge.trivial_affine_discards != case.trivial_affine_discards
            || !edge.residual_affine_discards.is_empty()
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
        let actions = case
            .trivial_affine_discards
            .iter()
            .copied()
            .map(terminal_psi::TerminalAffineCleanupAction::DiscardRoot)
            .collect::<Vec<_>>();
        if !super::aggregate_results::cleanup(function, &actions) {
            return Err(LegalizationError::custody());
        }
    }
    Ok(())
}
