//! A leaf copy observes an `Unrestricted` subtree without consuming or
//! refining its owner. Declaration lookup supplies nominal identity, not
//! liveness: control-flow availability and the ownership frontier
//! independently check the occurrence before it executes. The result's
//! declared `Unrestricted` multiplicity is the copyability evidence — an
//! affine leaf would ride the move/restoration contract instead.

use super::{
    BTreeSet, ModuleError, OperationKind, PlaceId, StructuralAccess, StructuralMultiplicity,
    StructuralPathSegment, TerminalMachine, TerminalModule, resolve_structural_path,
};

pub(super) fn validate(
    module: &TerminalModule,
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    source: PlaceId,
    path: &[StructuralPathSegment],
) -> Result<(), ModuleError> {
    let invalid = || ModuleError::InvalidStructuralLeafCopy {
        operation: operation.id,
        source,
    };
    let Some(result) = operation.result.structural() else {
        return Err(invalid());
    };
    if result.multiplicity != StructuralMultiplicity::Unrestricted
        || !result.qualifications.is_empty()
        || !result.claims.is_empty()
    {
        return Err(invalid());
    }
    let parameter = machine
        .structural_parameters
        .iter()
        .find(|parameter| parameter.place == source)
        .or_else(|| super::block_views::parameter(machine, source));
    if parameter.is_some_and(|parameter| parameter.access == StructuralAccess::WriteOnlyBorrow) {
        return Err(ModuleError::StructuralObservationRequiresReadableAccess {
            operation: operation.id,
            source,
        });
    }
    let signature = super::structural_result_contracts::source_signature(machine, source)
        .ok_or_else(invalid)?;
    let selected_type =
        resolve_structural_path(module, signature.structural_type, path).ok_or_else(invalid)?;
    if selected_type != result.structural_type {
        return Err(invalid());
    }
    Ok(())
}

/// A copied leaf's result is owned the way an `EstablishScalarCase` result
/// is: the copy is fresh storage, so an `Unrestricted` machine result may
/// publish it without the plain-shape proviso that payloadless returns need.
/// Control-flow availability and frontier accounting still apply.
pub(super) fn copied_return_source(machine: &TerminalMachine, source: PlaceId) -> bool {
    machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .any(|operation| {
            matches!(operation.kind, OperationKind::StructuralLeafCopy { .. })
                && operation.result.structural().is_some_and(|result| {
                    result.place == source
                        && result.multiplicity == StructuralMultiplicity::Unrestricted
                        && result.qualifications.is_empty()
                        && result.projected_qualifications.is_empty()
                        && result.claims.is_empty()
                })
        })
}

pub(super) fn validate_available(
    machine: &TerminalMachine,
    operation: &terminal_psi::Operation,
    available: &BTreeSet<PlaceId>,
) -> Result<(), ModuleError> {
    let OperationKind::StructuralLeafCopy { source, .. } = operation.kind else {
        return Ok(());
    };
    if !machine
        .structural_parameters
        .iter()
        .any(|parameter| parameter.place == source)
        && !available.contains(&source)
    {
        return Err(ModuleError::InvalidStructuralLeafCopy {
            operation: operation.id,
            source,
        });
    }
    Ok(())
}
