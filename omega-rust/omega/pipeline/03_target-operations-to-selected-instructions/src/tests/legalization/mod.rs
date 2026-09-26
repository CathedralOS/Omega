//! Optimizer module role: stage group.
//! Common graph legalization and independent corruption controls.

mod borrowed_windows;
pub(crate) mod byte_input;
mod byte_output;
mod call_structural;
mod dynamic_parameter_call;
mod ieee_literal_sources;
mod leaf_copy;
mod normalized_foreign;
mod ownership_frontier_facts;
mod plain_unit;
mod primitive_stores;
mod process_exit;
mod saturating_arithmetic;
mod scalar_arrays;
mod scalar_call_unit;
mod scalar_transfers;
mod scalar_unit_calls;
mod shared_type_catalog;
pub(crate) mod structural_case;
mod trivial_affine_locals;
mod unit_graph;
mod unit_view_graph;
mod widening;
mod wrapping_add;
mod wrapping_division;

/// Accept every obligation reference the unit's functions record, as the
/// verified-Terminal projection does for an artifact whose certificates the
/// verifier accepted. A runtime-selected path element's bound reaches
/// legalization only through such a fact.
pub(crate) fn accept_referenced_obligations(
    unit: terminal_psi_to_abstract_operations::optimization_unit::PsiOptimizationUnit,
) -> terminal_psi_to_abstract_operations::optimization_unit::PsiOptimizationUnit {
    let facts = unit
        .functions
        .iter()
        .flat_map(|function| {
            function.facts.iter().filter_map(|fact| match fact {
                terminal_psi_to_abstract_operations::optimization_unit::OptimizationFact::OperationObligationReference {
                    obligation,
                    support,
                } => Some(terminal_psi_to_abstract_operations::optimization_unit::AcceptedObligationFact::new(
                    unit.psi,
                    [4; 32],
                    function.machine,
                    *support,
                    *obligation,
                    vec![1],
                )),
                _ => None,
            })
        })
        .collect();
    terminal_psi_to_abstract_operations::optimization_unit::attach_accepted_obligation_facts(
        unit, facts,
    )
    .expect("each referenced obligation has one owner")
}

/// The legalized runtime traversal of `value` at `stride` inside an array of
/// `extent` elements, carrying the unit's accepted certificate for
/// `obligation` at `operation`.
pub(crate) fn runtime_operand(
    unit: &terminal_psi_to_abstract_operations::optimization_unit::PsiOptimizationUnit,
    operation: semantic_vocabulary::OperationId,
    operand: terminal_psi_to_abstract_operations::abstract_operations::AbstractResult,
    stride: u32,
    extent: u64,
    obligation: semantic_vocabulary::ObligationId,
) -> crate::legalized_operations::LegalizedRuntimeIndexOperand {
    crate::legalized_operations::LegalizedRuntimeIndexOperand {
        operand,
        stride,
        extent,
        obligation,
        accepted_fact: unit
            .accepted_obligation_facts
            .iter()
            .find(|fact| fact.operation == operation && fact.obligation == obligation)
            .expect("the runtime element's certificate is accepted")
            .identity,
    }
}
