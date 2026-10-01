//! Independent checks for `ScalarRepresentationView`.
//!
//! The operation carries only its operand and an optional landing
//! obligation. The carrier pair is rederived here from the operand's and the
//! result's declarations through the closed table in
//! `terminal_semantics::ScalarRepresentationViewPair`: equal width and a
//! family change (IEEE float and integer, Boolean and byte), nothing else. A
//! producer cannot name the pair, so it cannot widen it.
//!
//! The landing obligation must be present exactly when the pair lands an
//! integer on `bool`. Its proposition is not encoded either: reconstruction
//! (`operation_facts.rs`) rebuilds `0 <= operand <= 1` from the operand's
//! declared carrier, and the proof bundle must discharge that goal from the
//! path's facts (a bounded field, a data `where` row, a guard) or the module
//! fails verification. An omitted obligation on a landing, or a supplied one
//! on a total pair, rejects before reconstruction.

use std::collections::{BTreeMap, BTreeSet};

use semantic_vocabulary::{ScalarType, ValueId};
use terminal_psi::{Operation, OperationKind};
use terminal_semantics::ScalarRepresentationViewPair;

use super::ModuleError;
use super::operations::require_defined;

/// The admitted pair of one well-formed re-view. Validation has already
/// registered the operand and result types this reads.
pub(crate) fn scalar_representation_view_pair(
    operation: &Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
) -> Result<ScalarRepresentationViewPair, ModuleError> {
    let OperationKind::ScalarRepresentationView {
        operand,
        landing_obligation,
    } = operation.kind
    else {
        unreachable!("dispatched a scalar representation view")
    };
    let source = value_types
        .get(&operand)
        .copied()
        .ok_or(ModuleError::UnknownValue(operand))?;
    let target = operation
        .result
        .scalar_ref()
        .map(|result| result.scalar_type)
        .ok_or(ModuleError::ScalarOperationHasUnitResult(operation.id))?;
    let pair = ScalarRepresentationViewPair::classify(source, target).ok_or(
        ModuleError::ScalarRepresentationViewPairMismatch {
            operation: operation.id,
            source,
            target,
        },
    )?;
    if pair.requires_landing() != landing_obligation.is_some() {
        return Err(ModuleError::ScalarRepresentationViewLandingMismatch {
            operation: operation.id,
            source,
            target,
        });
    }
    Ok(pair)
}

pub(super) fn validate_operands(
    operation: &Operation,
    value_types: &BTreeMap<ValueId, ScalarType>,
    defined: &BTreeSet<ValueId>,
) -> Result<(), ModuleError> {
    let OperationKind::ScalarRepresentationView { operand, .. } = operation.kind else {
        unreachable!("dispatched a scalar representation view")
    };
    require_defined(operand, value_types, defined)?;
    scalar_representation_view_pair(operation, value_types).map(|_| ())
}
