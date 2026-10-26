//! A placed boundary remains an observable event on its current original backing.
//! Catalog geometry, live descriptor custody and scalar operands are independent joins.
use super::RowSite;
use crate::LoweringError;
use abstract_operations::{AbstractBoundaryResult, AbstractOperation};
use semantic_vocabulary::{BoundaryMachineId, ScalarType};
use std::collections::BTreeMap;
use target_operations::{
    BoundaryRealization, BoundarySettlementBinding, BoundarySettlementRealization,
    TargetUnitOperation, TargetUnitScalarHomeRequirement,
};
use terminal_psi::TerminalPlacedAccessorKind;

pub(super) fn lower(
    operation: &AbstractOperation,
    site: &mut RowSite<'_>,
    settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
) -> Result<(), LoweringError> {
    let (function, types, live, operations, provenance) = site.parts();
    macro_rules! invalid {
        () => {
            LoweringError::unsupported_control_flow(function.machine)
        };
    }
    let AbstractOperation::BoundaryCall {
        psi_operation,
        boundary,
        arguments,
        structural_arguments,
        result,
        completion_claim_sources,
        completion_receipts,
    } = operation
    else {
        return Err(invalid!());
    };
    let binding = settlements.get(boundary).ok_or_else(|| invalid!())?;
    let BoundarySettlementRealization::Builtin(BoundaryRealization::PlacedScalarAccess(
        realization,
    )) = binding.realization
    else {
        return Err(invalid!());
    };
    let demand = realization.demand;
    let [argument] = structural_arguments.as_slice() else {
        return Err(invalid!());
    };
    if argument.access != demand.access
        || super::references::reference_source_type(function, live, types, argument)
            != Some(demand.accessor)
        || !completion_claim_sources.is_empty()
        || !completion_receipts.is_empty()
    {
        return Err(invalid!());
    }
    // The accessor's authored storage starts at its transfer container. Keep
    // this actual reference, including captured projections and call results;
    // recovering a whole-view parent would select a historical source instead
    // of the pointer currently carried by the argument. Stage 00 admitted the
    // exact boundary against every reachable placed origin. This stage joins
    // its exact accessor type to current descriptor storage, not a new grant.
    let location = argument.clone();
    super::references::normalized_source(function, live, types, &location)?;
    let operand = match (demand.operation, arguments.as_slice()) {
        (TerminalPlacedAccessorKind::Read, []) => None,
        (TerminalPlacedAccessorKind::Write, [value]) => {
            let source = super::scalar_sources::source(*value, function, live)?;
            if source.scalar_type() != demand.scalar {
                return Err(invalid!());
            }
            Some(source)
        }
        _ => return Err(invalid!()),
    };
    let result_home = match (demand.operation, result) {
        (TerminalPlacedAccessorKind::Read, AbstractBoundaryResult::Scalar(result))
            if result.scalar_type == demand.scalar =>
        {
            let home = TargetUnitScalarHomeRequirement {
                defining_operation: *psi_operation,
                source_value: result.value,
                scalar_type: demand.scalar,
                shape: crate::lowering::scalar_abi::fixed_native_scalar_shape(demand.scalar)
                    .ok_or_else(|| invalid!())?,
            };
            match demand.scalar {
                ScalarType::Integer(_) => {
                    crate::lowering::unit::known_integers::insert_known_unit_integer(
                        &mut live.integers,
                        home.source_value,
                        super::KnownUnitInteger::Home(home),
                    )?
                }
                _ => {
                    if live.scalar_homes.insert(home.source_value, home).is_some() {
                        return Err(LoweringError::DuplicateValue(home.source_value));
                    }
                }
            }
            Some(home)
        }
        (TerminalPlacedAccessorKind::Write, AbstractBoundaryResult::Unit) => None,
        _ => return Err(invalid!()),
    };
    operations.push(TargetUnitOperation::PlacedScalarAccess {
        psi_operation: *psi_operation,
        boundary: *boundary,
        realization,
        location,
        operand,
        result_home,
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}
