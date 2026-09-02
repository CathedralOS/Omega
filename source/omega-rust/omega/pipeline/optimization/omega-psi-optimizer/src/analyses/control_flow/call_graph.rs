//! Direct call graph, recursion, and call-component construction.

use std::collections::BTreeMap;

use omega_abstract_operations::AbstractOperation as O;
use omega_optimization_unit::PsiOptimizationUnit;

use super::{CallGraphAnalysis, components::strongly_connected_components};

pub(in crate::analyses) fn call_graph(unit: &PsiOptimizationUnit) -> CallGraphAnalysis {
    let mut graph = unit
        .functions
        .iter()
        .map(|function| (function.machine, Vec::new()))
        .collect::<BTreeMap<_, _>>();
    for function in &unit.functions {
        let callees = graph.get_mut(&function.machine).unwrap();
        for operation in function
            .blocks
            .iter()
            .flat_map(|block| block.nodes.iter().map(|node| &node.operation))
        {
            match operation {
                O::CallUnit { callee, .. }
                | O::CallStructuralScalar { callee, .. }
                | O::CallStructural { callee, .. }
                | O::Call { callee, .. } => callees.push(*callee),
                O::CallStructuralScalarWithDynamicArguments {
                    callee,
                    dynamic_arguments,
                    ..
                } => {
                    callees.push(*callee);
                    for argument in dynamic_arguments {
                        if let omega_abstract_operations::AbstractDynamicDescriptorSource::Rebound {
                            application,
                            ..
                        } = &argument.source
                        {
                            callees.extend(
                                application
                                    .realization_callables
                                    .iter()
                                    .map(|callable| callable.machine),
                            );
                        }
                    }
                }
                O::CallDynamicScalar {
                    dynamic_dispatch, ..
                } => callees.push(dynamic_dispatch.dispatch.realization),
                _ => {}
            }
        }
        callees.sort_unstable();
        callees.dedup();
    }
    let components = strongly_connected_components(&graph);
    let recursive_components = components
        .iter()
        .filter(|component| {
            component.len() > 1
                || component
                    .first()
                    .is_some_and(|machine| graph[machine].contains(machine))
        })
        .cloned()
        .collect();
    CallGraphAnalysis {
        callees: graph.into_iter().collect(),
        components,
        recursive_components,
    }
}
