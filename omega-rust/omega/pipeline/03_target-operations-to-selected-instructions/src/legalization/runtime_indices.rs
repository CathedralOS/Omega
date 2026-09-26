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
use semantic_vocabulary::{OperationId, ScalarType, ValueId};

/// Join each runtime element of `operation`'s projection, in path order, to
/// its selector's exact definition and to the accepted certificate for the
/// element's obligation at that operation of this machine. A selector must
/// be an integer carrier the 64-bit address model can extend.
pub(crate) fn operands(
    optimized: &PsiOptimizationFunction,
    unit: &PsiOptimizationUnit,
    operation: OperationId,
    elements: &[RuntimeElement],
) -> Result<Vec<LegalizedRuntimeIndexOperand>, LegalizationError> {
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
                        && fact.operation == operation
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
        .chain(optimized.blocks.iter().flat_map(|block| {
            block
                .parameters
                .iter()
                .chain(block.nodes.iter().flat_map(|node| &node.definitions))
        }))
        .find(|definition| definition.value == value)
        .map(|definition| definition.scalar_type)
}
