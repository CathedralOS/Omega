//! Release a zero-cell run by moving its backing descriptor into the result.
//! The abstract call consumes custody; physical lowering reuses field transport.

use super::RowSite;
use crate::LoweringError;
use abstract_operations::{AbstractBoundaryResult, AbstractOperation};
use target_operations::{
    BoundaryExecutionBinding, BoundarySettlementBinding, CompilerBuiltinExecution,
};
use terminal_psi::{BoundaryMachineDeclaration, StructuralAccess, StructuralPathSegment};

pub(super) fn lower(
    operation: &AbstractOperation,
    site: &mut RowSite<'_>,
    declaration: &BoundaryMachineDeclaration,
    settlement: &BoundarySettlementBinding,
) -> Result<(), LoweringError> {
    let AbstractOperation::BoundaryCall {
        psi_operation,
        boundary,
        structural_arguments,
        arguments,
        result: AbstractBoundaryResult::Structural(result),
        ..
    } = operation
    else {
        return Err(LoweringError::unsupported_control_flow(
            site.function.machine,
        ));
    };
    let invalid = |reason| LoweringError::InvalidAccessLifecycle {
        machine: site.function.machine,
        operation: *psi_operation,
        boundary: *boundary,
        reason,
    };
    let contract = terminal_semantics::empty_run_release_contract(
        site.types.catalog().as_slice(),
        declaration,
    )
    .map_err(invalid)?;
    let [source] = structural_arguments.as_slice() else {
        return Err(invalid("empty-run release requires one owned source"));
    };
    if !arguments.is_empty()
        || source.access != StructuralAccess::Owned
        || settlement.execution
            != BoundaryExecutionBinding::CompilerBuiltin(CompilerBuiltinExecution::EmptyRunRelease)
        || result.structural_type != contract.result.structural_type
        || result.multiplicity != contract.result.multiplicity
    {
        return Err(invalid("empty-run release changed its admitted invocation"));
    }
    let (source_type, _) =
        super::aggregate_results::value_input(site.function, site.types, site.live, source)?;
    if source_type != declaration.structural_parameters[0].structural_type {
        return Err(invalid("empty-run release changed its owned carrier"));
    }
    let root_type = site
        .live
        .structural_observation_type(site.function, source.place)
        .ok_or_else(|| invalid("empty-run release lost its source root"))?;
    let mut path = source.path.clone();
    path.push(StructuralPathSegment::Field(contract.storage));
    super::leaf_copy::copy_extent(
        *psi_operation,
        result,
        source.place,
        &path.into(),
        root_type,
        site,
    )
}
