use super::{AbstractOperationPlan, Error};
use crate::LegalizationError;
use crate::legalization::scalar_graph_input;
use abstract_operations::AbstractOperation;
use legalized_operations::{
    LegalizedScalarReturn, LegalizedScalarReturnValue, LegalizedScalarSuccessor,
    LegalizedScalarTerminator,
};
pub(super) fn project(
    node: &optimization_unit::OptimizationNode,
    function: &optimization_unit::PsiOptimizationFunction,
    plan: &AbstractOperationPlan,
    native: &target_operations::TargetOperationPlan,
    unit: &optimization_unit::PsiOptimizationUnit,
) -> Result<LegalizedScalarTerminator, LegalizationError> {
    let cleanup_calls =
        scalar_graph_input::cleanup_calls::expected(&node.operation, function, native, plan, unit)?;
    let mut result = match &node.operation {
        AbstractOperation::ExternalCompletion {
            psi_edge,
            boundary,
            kind,
            structural_arguments,
            requirement_obligations,
            frontier_lower_bound,
            ..
        } => {
            let mut matches = native
                .functions
                .iter()
                .filter(|row| row.machine == function.machine)
                .flat_map(|row| &row.graph.blocks)
                .filter_map(|block| match &block.terminator {
                    target_operations::TargetControlTerminator::ExternalCompletion {
                        psi_edge: edge,
                        arguments,
                        binding,
                        ..
                    } if edge == psi_edge => Some((arguments, binding)),
                    _ => None,
                });
            let (arguments, binding) = matches.next().ok_or(Error::custody())?;
            if matches.next().is_some() {
                return Err(Error::custody());
            }
            Ok(LegalizedScalarTerminator::ExternalCompletion {
                psi_edge: *psi_edge,
                boundary: *boundary,
                kind: *kind,
                arguments: arguments.clone(),
                binding: binding.clone(),
                structural_arguments: structural_arguments.clone(),
                requirement_obligations: requirement_obligations.clone(),
                frontier_lower_bound: frontier_lower_bound.clone(),
                fuel: node.fuel.clone(),
                effect: node.effect,
                ownership: node.ownership.clone(),
            })
        }
        AbstractOperation::Crash {
            psi_edge,
            cause,
            site_guard,
            frontier_lower_bound,
        } => Ok(LegalizedScalarTerminator::Crash {
            psi_edge: *psi_edge,
            cause: *cause,
            site_guard: site_guard.clone(),
            frontier_lower_bound: frontier_lower_bound.clone(),
            fuel: node.fuel.clone(),
            effect: node.effect,
            ownership: node.ownership.clone(),
        }),
        AbstractOperation::ReturnStructural {
            psi_edge, source, ..
        } => {
            let value = if scalar_graph_input::byte_views::descriptor_return(
                function,
                *source,
                &plan.structural_types,
            ) {
                LegalizedScalarReturnValue::SharedDescriptor { place: *source }
            } else if function.structural_parameter(*source).is_some() {
                LegalizedScalarReturnValue::StructuralParameter { place: *source }
            } else {
                LegalizedScalarReturnValue::Structural {
                    source: scalar_graph_input::structural_case::source_owner(function, *source)?,
                }
            };
            Ok(LegalizedScalarTerminator::Return(LegalizedScalarReturn {
                edge: *psi_edge,
                value,
                fuel: node.fuel.clone(),
                effect: node.effect,
                ownership: node.ownership.clone(),
            }))
        }
        AbstractOperation::StructuralCase { .. } => {
            super::structural_case::project(node, function, plan)
        }
        AbstractOperation::ReturnUnit { psi_edge, .. } => {
            Ok(LegalizedScalarTerminator::Return(LegalizedScalarReturn {
                edge: *psi_edge,
                value: LegalizedScalarReturnValue::Unit,
                fuel: node.fuel.clone(),
                effect: node.effect,
                ownership: node.ownership.clone(),
            }))
        }
        AbstractOperation::Return {
            psi_edge,
            value,
            scalar_type,
            ..
        } if scalar_graph_input::scalar_shape(*scalar_type).is_some() => {
            Ok(LegalizedScalarTerminator::Return(LegalizedScalarReturn {
                edge: *psi_edge,
                value: LegalizedScalarReturnValue::Value {
                    value: *value,
                    scalar_type: *scalar_type,
                },
                fuel: node.fuel.clone(),
                effect: node.effect,
                ownership: node.ownership.clone(),
            }))
        }
        AbstractOperation::Jump { .. } => {
            let [edge] = node.successors.as_slice() else {
                return Err(Error::custody());
            };
            Ok(LegalizedScalarTerminator::Jump {
                successor: successor(edge, function, plan, unit)?,
                effect: node.effect,
                ownership: node.ownership.clone(),
            })
        }
        AbstractOperation::Conditional { condition, .. } => {
            let [when_true, when_false] = node.successors.as_slice() else {
                return Err(Error::custody());
            };
            Ok(LegalizedScalarTerminator::Conditional {
                condition: *condition,
                when_true: successor(when_true, function, plan, unit)?,
                when_false: successor(when_false, function, plan, unit)?,
                effect: node.effect,
                ownership: node.ownership.clone(),
            })
        }
        _ => Err(Error::custody()),
    }?;
    if cleanup_calls.is_empty() {
        return Ok(result);
    }
    if let LegalizedScalarTerminator::Return(returned) = result {
        return Ok(LegalizedScalarTerminator::ReturnCleanup {
            returned,
            calls: cleanup_calls,
        });
    }
    for (owner, call) in cleanup_calls {
        let calls = match &mut result {
            LegalizedScalarTerminator::Jump { successor, .. }
                if owner.edge() == Some(successor.edge) =>
            {
                &mut successor.cleanup_calls
            }
            LegalizedScalarTerminator::Conditional {
                when_true,
                when_false,
                ..
            } => {
                if owner.edge() == Some(when_true.edge) {
                    &mut when_true.cleanup_calls
                } else if owner.edge() == Some(when_false.edge) {
                    &mut when_false.cleanup_calls
                } else {
                    return Err(Error::custody());
                }
            }
            LegalizedScalarTerminator::StructuralCase { cases, .. } => {
                &mut cases
                    .iter_mut()
                    .find(|case| owner.edge() == Some(case.edge))
                    .ok_or(Error::custody())?
                    .cleanup_calls
            }
            _ => return Err(Error::custody()),
        };
        calls.push((owner, call));
    }
    Ok(result)
}
fn successor(
    edge: &optimization_unit::OptimizationEdge,
    function: &optimization_unit::PsiOptimizationFunction,
    plan: &AbstractOperationPlan,
    unit: &optimization_unit::PsiOptimizationUnit,
) -> Result<LegalizedScalarSuccessor, LegalizationError> {
    let structural_bindings = edge.structural_bindings.iter().map(|binding| {
        let indices = if binding.argument.path.iter().any(|segment| segment.runtime_index().is_some()) {
            let root = scalar_graph_input::primitive_locals::read_root_type(function, binding.argument.place)
                .ok_or(Error::custody())?;
            // Preserve selector regions even when the endpoint is an inline
            // field without a standalone nominal declaration. This geometry
            // grants no custody; every index joins its real edge certificate.
            let geometry = crate::structural_inputs::structural_reference_input::reference_formation_geometry(
                root, &binding.argument.path, &plan.structural_types,
            ).ok_or(Error::custody())?;
            crate::legalization::runtime_indices::operands(
                function, unit, optimization_unit::PsiProvenance::Edge(edge.psi_edge), &geometry.elements,
            )?
        } else { Vec::new() };
        Ok(legalized_operations::LegalizedStructuralBinding { semantic: binding.clone(), indices })
    }).collect::<Result<Vec<_>, LegalizationError>>()?;
    Ok(LegalizedScalarSuccessor {
        edge: edge.psi_edge,
        target: edge.target,
        bindings: edge.bindings.clone(),
        structural_bindings,
        fuel: edge.fuel.clone(),
        cleanup_calls: Vec::new(),
    })
}
