//! Complete call-result backing before handing target operations to selection.
//!
//! The descriptor graph borrows this stage's existing optimized input; it does
//! not copy a previous program. Only backing alternatives travel forward.

use crate::{LoweringError, selected_boundary_calls};
use abstract_operations::{AbstractFunction, AbstractOperation, AbstractOperationPlan};
use semantic_vocabulary::{MachineId, PlaceId};
use std::borrow::Cow;
use target_operations::{
    TargetCallResult, TargetDescriptorOrigin, TargetOperationPlan, TargetUnitOperation,
};
use terminal_semantics::descriptor_results::{
    DescriptorAnalysis, DescriptorCallOrigin, DescriptorSlotKind,
};

pub(crate) fn retain<'a>(
    source: &AbstractOperationPlan,
    unit: &optimization_unit::PsiOptimizationUnit,
    selected: &selected_boundary_calls::SelectedCallsByCall,
    candidates: impl IntoIterator<Item = &'a AbstractFunction>,
    target: &mut TargetOperationPlan,
) -> Result<(), LoweringError> {
    let pending = target.functions.iter().find_map(|function| {
        function
            .graph
            .blocks
            .iter()
            .flat_map(|block| block.operations())
            .find_map(|operation| {
                let TargetUnitOperation::Call {
                    result: TargetCallResult::Structural { result, .. },
                    ..
                } = operation
                else {
                    return None;
                };
                (matches!(
                    result.multiplicity,
                    terminal_psi::nonlinear_multiplicity_pair_pattern!()
                ) && DescriptorSlotKind::Views
                    .owned_slots(
                        &source.structural_types,
                        result.place,
                        result.structural_type,
                    )
                    .is_none_or(|slots| !slots.is_empty()))
                .then_some((function.machine, result.place))
            })
    });
    let Some((caller, result_place)) = pending else {
        return Ok(());
    };
    let invalid = |machine, result, reason| LoweringError::DescriptorCallOrigins {
        machine,
        result,
        reason,
    };
    // Only selected boundary callees need this lookup. Reuse the coordinator's
    // selected family instead of allocating another whole-program index.
    let mut candidates = candidates.into_iter().collect::<Vec<_>>();
    candidates.sort_unstable_by_key(|candidate| candidate.machine);
    candidates.dedup_by_key(|candidate| candidate.machine);
    let graph = optimization_unit_semantics::descriptor_results::optimized_catalog_graph(
        unit.functions.iter(),
        &source.boundary_machines,
        &source.structural_types,
        &source.dynamic_dispatch,
        |function, node| {
            if let AbstractOperation::BoundaryCall {
                psi_operation,
                boundary,
                ..
            } = &node.operation
                && let Some(call) =
                    selected.get(&(function.machine, (*psi_operation).into(), *boundary))
            {
                return crate::control_flow::selected_boundary_calls::resolve(
                    &node.operation,
                    call,
                    candidates
                        .binary_search_by_key(&call.callee, |candidate| candidate.machine)
                        .ok()
                        .map(|position| candidates[position]),
                )
                .ok()
                .map(|(operation, _)| Cow::Owned(operation));
            }
            Some(Cow::Borrowed(&node.operation))
        },
    )
    .ok_or_else(|| {
        invalid(
            caller,
            result_place,
            "descriptor backing graph is unresolved",
        )
    })?;
    let mut analysis = DescriptorAnalysis::new(&source.structural_types, &graph);
    for function in &mut target.functions {
        for operation in function
            .graph
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.operations)
            .map(|instruction| &mut instruction.kind)
        {
            let TargetUnitOperation::Call {
                result:
                    TargetCallResult::Structural {
                        result,
                        descriptor_origins,
                        ..
                    },
                ..
            } = operation
            else {
                continue;
            };
            if !matches!(
                result.multiplicity,
                terminal_psi::nonlinear_multiplicity_pair_pattern!()
            ) {
                continue;
            }
            let slots = DescriptorSlotKind::Views
                .owned_slots(
                    &source.structural_types,
                    result.place,
                    result.structural_type,
                )
                .ok_or_else(|| {
                    invalid(
                        function.machine,
                        result.place,
                        "call result has unresolved descriptor slots",
                    )
                })?;
            if slots.is_empty() {
                continue;
            }
            *descriptor_origins = analysis
                .try_call_origins(function.machine, result.place)
                .map_err(|reason| invalid(function.machine, result.place, reason))?
                .into_iter()
                .map(|origin| project(function.machine, result.place, origin))
                .collect::<Result<_, _>>()?;
        }
    }
    Ok(())
}

fn project(
    machine: MachineId,
    result: PlaceId,
    origin: DescriptorCallOrigin,
) -> Result<TargetDescriptorOrigin, LoweringError> {
    Ok(match origin {
        DescriptorCallOrigin::Empty => TargetDescriptorOrigin::Empty,
        DescriptorCallOrigin::Literal { machine, place } => {
            TargetDescriptorOrigin::Literal { machine, place }
        }
        DescriptorCallOrigin::Argument { source, projection } => {
            let projection = projection.without_runtime_coordinates();
            TargetDescriptorOrigin::Argument {
                source: terminal_psi::StructuralArgument {
                    place: source,
                    path: projection.path,
                    access: terminal_psi::StructuralAccess::SharedBorrow,
                },
                exact: projection.exact,
                indirect: projection.indirect,
            }
        }
        DescriptorCallOrigin::Invocation { .. } | DescriptorCallOrigin::AccessFormation { .. } => {
            return Err(LoweringError::DescriptorCallOrigins {
                machine,
                result,
                reason: "call result backing has no supported storage origin",
            });
        }
    })
}
