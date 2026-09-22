//! Optimizer module role: stage group. Typed optimization units shared by the pass-family test suites.

mod control_flow_cleanup;
mod dead_scalar_elimination;
mod global_value_numbering;
mod proof_check_elision;
mod sparse_conditional_constant_propagation;

pub(crate) use control_flow_cleanup::*;
pub(crate) use dead_scalar_elimination::*;
pub(crate) use global_value_numbering::*;
pub(crate) use proof_check_elision::*;
pub(crate) use sparse_conditional_constant_propagation::*;

use super::{
    AcceptedObligationFact, BuiltInRuleRegistration, OptimizationFact, OrderedRuleRegistry,
    PsiOptimization, PsiOptimizationUnit, assemble_built_in_registry,
    attach_accepted_obligation_facts, built_in_rule_registrations,
};

pub(crate) fn shuffle_built_in_registrations(
    registrations: &mut [BuiltInRuleRegistration],
    mut state: u64,
) {
    for upper in (1..registrations.len()).rev() {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        let index =
            usize::try_from(state % u64::try_from(upper + 1).expect("registration count fits u64"))
                .expect("shuffle index fits usize");
        registrations.swap(upper, index);
    }
}

pub(crate) fn randomized_built_in_registries(
    optimization: PsiOptimization,
) -> Vec<OrderedRuleRegistry> {
    (1..=32)
        .map(|seed| {
            let mut registrations = built_in_rule_registrations(optimization);
            shuffle_built_in_registrations(&mut registrations, seed);
            assemble_built_in_registry(registrations)
                .expect("shuffling cannot alter a valid built-in schedule")
        })
        .collect()
}

pub(crate) fn id<T>(raw: u64, constructor: impl FnOnce(u64) -> Option<T>) -> T {
    constructor(raw).expect("nonzero test identity")
}

pub(crate) fn with_synthetic_accepted_obligations(
    unit: PsiOptimizationUnit,
) -> PsiOptimizationUnit {
    let facts = unit
        .functions
        .iter()
        .flat_map(|function| {
            function.facts.iter().filter_map(|fact| match fact {
                OptimizationFact::OperationObligationReference {
                    obligation,
                    support,
                } => Some(AcceptedObligationFact::new(
                    unit.psi,
                    [29; 32],
                    function.machine,
                    *support,
                    *obligation,
                    obligation.get().to_le_bytes().to_vec(),
                )),
                _ => None,
            })
        })
        .collect();
    attach_accepted_obligation_facts(unit, facts).unwrap()
}
