//! Runtime-selected projection elements joined to their selectors and to the
//! verifier's certificates.
//!
//! A primitive read or store, a scalar field store's carrier, and a leaf
//! copy all spell a runtime-selected element as the Terminal path segment
//! `RuntimeIndex { index, obligation }`. Legalization turns each into one
//! `(index, stride)` run of the address model and carries the segment's
//! accepted certificate beside it, so selection scales an index the verified
//! Terminal already bounded — no stage re-derives the bound or trusts a
//! byte offset in its place. Source projection and independent replay both
//! call this one join.

use crate::LegalizationError;
use crate::structural_inputs::structural_reference_input::RuntimeElement;
use legalized_operations::LegalizedRuntimeIndexOperand;
use optimization_unit::{PsiOptimizationFunction, PsiOptimizationUnit};
use semantic_vocabulary::{OperationId, ScalarType, StructuralTypeId, ValueId};

/// Join each runtime element of the owner's projection, in path order, to
/// its selector's exact definition and to the accepted certificate for the
/// element's obligation at that operation or edge of this machine. A selector must
/// be an integer carrier the 64-bit address model can extend.
pub(crate) fn operands(
    optimized: &PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
    owner: impl Into<optimization_unit::PsiProvenance>,
    elements: &[RuntimeElement],
) -> Result<Vec<LegalizedRuntimeIndexOperand>, LegalizationError> {
    let owner = owner.into();
    elements
        .iter()
        .map(|element| {
            let scalar_type = defined_type(optimized, element.index)
                .filter(
                    |scalar| matches!(scalar, ScalarType::Integer(integer) if integer.bits() <= 64),
                )
                .ok_or(LegalizationError::custody())?;
            let accepted_fact = unit
                .accepted_obligation_facts
                .iter()
                .find(|fact| {
                    fact.machine == optimized.machine
                        && fact.owner == owner
                        && fact.obligation == element.obligation
                })
                .map(|fact| fact.identity)
                .ok_or(LegalizationError::custody())?;
            Ok(LegalizedRuntimeIndexOperand {
                operand: abstract_operations::AbstractResult {
                    value: element.index,
                    scalar_type,
                },
                stride: element.stride,
                extent: element.extent,
                obligation: element.obligation,
                accepted_fact,
            })
        })
        .collect()
}

/// The declared type of one value of this function: a function or block
/// parameter, or a node's definition.
pub(in crate::legalization) fn defined_type(
    optimized: &PsiOptimizationFunction,
    value: ValueId,
) -> Option<ScalarType> {
    optimized
        .parameters
        .iter()
        .chain(
            optimized
                .blocks
                .iter()
                .flat_map(|block| block.parameters.iter().chain(block.definitions())),
        )
        .find(|definition| definition.value == value)
        .map(|definition| definition.scalar_type)
}

/// Join a projection's runtime elements without demanding a declaration for
/// its final inline leaf. Call placement and storage operations own endpoint
/// presentation and access; selector transport keeps the actual occurrence's
/// strides and obligations. Explicit reference crossings still require their
/// pointer regions. Access belongs to the operation's source contract, not
/// this selector roster. A static path mints no indices.
pub(crate) fn projection(
    optimized: &PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
    owner: impl Into<optimization_unit::PsiProvenance>,
    root: StructuralTypeId,
    path: &[terminal_psi::StructuralPathSegment],
    declarations: &[terminal_psi::StructuralTypeDeclaration],
) -> Result<Vec<LegalizedRuntimeIndexOperand>, LegalizationError> {
    if !path.iter().any(|segment| {
        matches!(
            segment,
            terminal_psi::StructuralPathSegment::RuntimeIndex { .. }
        )
    }) {
        return Ok(Vec::new());
    }
    let elements = if path.contains(&terminal_psi::StructuralPathSegment::Referent) {
        crate::structural_inputs::structural_reference_input::reference_formation_geometry(
            root,
            path,
            declarations,
        )
        .ok_or(LegalizationError::custody())?
        .elements
    } else {
        crate::structural_inputs::structural_reference_input::runtime_projection_elements(
            root,
            path,
            declarations,
        )
        .ok_or(LegalizationError::custody())?
    };
    operands(optimized, unit, owner, &elements)
}

pub(crate) fn reference_source(
    optimized: &PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
    operation: OperationId,
    source: &terminal_psi::StructuralArgument,
) -> Result<Vec<LegalizedRuntimeIndexOperand>, LegalizationError> {
    let root = super::scalar_graph_input::primitive_locals::read_root_type(optimized, source.place)
        .ok_or(LegalizationError::custody())?;
    // Formation's final marker is interpreted by the endpoint: it loads an
    // actual Reference shell, but creates no crossing at an ordinary leaf.
    // Both endpoints retain the same selectors before that final marker.
    let path = match source.path.split_last() {
        Some((terminal_psi::StructuralPathSegment::Referent, prefix)) => prefix,
        _ => source.path.as_slice(),
    };
    // Selector transport needs the actual storage regions, not a standalone
    // declaration for an inline endpoint. Formation owns its presentation.
    let geometry =
        crate::structural_inputs::structural_reference_input::reference_formation_geometry(
            root,
            path,
            &unit.structural_types,
        )
        .ok_or(LegalizationError::custody())?;
    operands(optimized, unit, operation, &geometry.elements)
}

/// A collection-view formation retains the backing projection's runtime
/// selectors independently of its final optional fixed byte window.
pub(crate) fn collection_view(
    optimized: &PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
    operation: OperationId,
    source: &terminal_psi::StructuralArgument,
) -> Result<Vec<LegalizedRuntimeIndexOperand>, LegalizationError> {
    if !source
        .path
        .iter()
        .any(|segment| segment.runtime_index().is_some())
    {
        return Ok(Vec::new());
    }
    let root = super::scalar_graph_input::primitive_locals::read_root_type(optimized, source.place)
        .ok_or(LegalizationError::custody())?;
    let path = match source.path.last() {
        Some(terminal_psi::StructuralPathSegment::FixedByteRange { .. }) => {
            &source.path[..source.path.len() - 1]
        }
        _ => &source.path,
    };
    let (_, _, elements) =
        crate::structural_inputs::structural_reference_input::collection_projection(
            root,
            path,
            &unit.structural_types,
        )
        .ok_or(LegalizationError::custody())?;
    operands(optimized, unit, operation, &elements)
}
