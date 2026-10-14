//! A placed boundary remains an observable event on its current original backing.
//! Catalog geometry, live descriptor custody and scalar operands are independent joins.
use super::LiveDefinitions;
use crate::{LoweringError, lowering::structural_type_lookup::StructuralTypeLookup};
use abstract_operations::{AbstractBoundaryResult, AbstractFunction, AbstractOperation};
use semantic_vocabulary::{BoundaryMachineId, ScalarType};
use std::collections::BTreeMap;
use target_operations::{
    BoundaryRealization, BoundarySettlementBinding, BoundarySettlementRealization,
    TargetPlacedInterpretation, TargetUnitOperation, TargetUnitScalarHomeRequirement,
    TerminalPsiProvenance,
};
use terminal_psi::{StructuralArgument, StructuralPathSegment, TerminalPlacedAccessorKind};

pub(super) fn lower(
    operation: &AbstractOperation,
    function: &AbstractFunction,
    types: &StructuralTypeLookup<'_>,
    settlements: &BTreeMap<BoundaryMachineId, BoundarySettlementBinding>,
    live: &mut LiveDefinitions,
    operations: &mut Vec<TargetUnitOperation>,
    provenance: &mut TerminalPsiProvenance,
) -> Result<(), LoweringError> {
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
    let (Some(StructuralPathSegment::Field(field)), parent) = (
        argument.path.last(),
        &argument.path[..argument.path.len().saturating_sub(1)],
    ) else {
        return Err(invalid!());
    };
    if *field != demand.view_field
        || argument.access != demand.access
        || !completion_claim_sources.is_empty()
        || !completion_receipts.is_empty()
    {
        return Err(invalid!());
    }
    let location = StructuralArgument {
        place: argument.place,
        path: parent.to_vec(),
        access: argument.access,
    };
    let interpretation =
        super::references::placed_source(live, &location).ok_or_else(|| invalid!())?;
    match &interpretation {
        TargetPlacedInterpretation::Captured(captured) => {
            if captured.carrier != demand.carrier || captured.schema != demand.schema {
                return Err(invalid!());
            }
            let catalog = types.catalog();
            let plan = catalog
                .placed_views()
                .iter()
                .find(|plan| plan.carrier == demand.carrier)
                .ok_or_else(|| invalid!())?;
            let expected = terminal_semantics::placed_ram_plan_correspondence(
                catalog.as_slice(),
                catalog.placed_views(),
                plan,
                demand.schema,
            )
            .map_err(|_| invalid!())?;
            if captured != &expected {
                return Err(invalid!());
            }
        }
        TargetPlacedInterpretation::Formal(formal) => {
            if formal.placement_commitment != demand.placement_commitment
                || !demand.access.admits(formal.access)
            {
                return Err(invalid!());
            }
        }
    }
    // Current def-use resolution must succeed independently of the catalog row.
    // Formal sources remain activation obligations answered by every caller.
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
        interpretation,
        operand,
        result_home,
    });
    provenance.operations.push(*psi_operation);
    Ok(())
}
