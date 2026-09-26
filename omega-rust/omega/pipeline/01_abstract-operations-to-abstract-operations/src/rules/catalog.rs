//! The complete ordered Psi pass catalog.
//!
//! This file is intentionally declarative. Each entry points to one pass
//! entrance, which visibly lists that pass's exact rule order.

use std::sync::Arc;

use optimization::PsiOptimization;

use crate::rules::{
    control_flow_cleanup, copy_propagation, dead_scalar_elimination, global_value_numbering,
    proof_check_elision, representation_specialization_rule_registrations,
    sparse_conditional_constant_propagation, state_specialization,
};
use crate::{OrderedRuleRegistry, PsiOptimizationRule, RuleRegistryError};

type RuleCatalog = fn() -> Vec<BuiltInRuleRegistration>;

#[derive(Clone, Copy)]
struct PsiPassCatalogPayload {
    rule_catalog: RuleCatalog,
}

/// One visible route from an exact Psi selection to its ordered rule leaf.
#[derive(Clone, Copy)]
pub struct PsiPassCatalogEntry {
    optimization: PsiOptimization,
    payload: PsiPassCatalogPayload,
}

impl PsiPassCatalogEntry {
    const fn new(optimization: PsiOptimization, rule_catalog: RuleCatalog) -> Self {
        Self {
            optimization,
            payload: PsiPassCatalogPayload { rule_catalog },
        }
    }

    pub const fn optimization(self) -> PsiOptimization {
        self.optimization
    }

    fn registrations(self) -> Vec<BuiltInRuleRegistration> {
        (self.payload.rule_catalog)()
    }
}

/// The single built-in Psi enable/disable and ordering table.
pub const PSI_PASS_CATALOG: [PsiPassCatalogEntry; 8] = [
    PsiPassCatalogEntry::new(
        PsiOptimization::SparseConditionalConstantPropagation,
        sparse_conditional_constant_propagation::built_in_registrations,
    ),
    PsiPassCatalogEntry::new(
        PsiOptimization::ControlFlowCleanup,
        control_flow_cleanup::built_in_registrations,
    ),
    PsiPassCatalogEntry::new(
        PsiOptimization::CopyPropagation,
        copy_propagation::built_in_registrations,
    ),
    PsiPassCatalogEntry::new(
        PsiOptimization::GlobalValueNumbering,
        global_value_numbering::built_in_registrations,
    ),
    PsiPassCatalogEntry::new(
        PsiOptimization::ProofCheckElision,
        proof_check_elision::built_in_registrations,
    ),
    PsiPassCatalogEntry::new(
        PsiOptimization::DeadPureScalarElimination,
        dead_scalar_elimination::built_in_registrations,
    ),
    PsiPassCatalogEntry::new(
        PsiOptimization::StateSpecialization,
        state_specialization::built_in_registrations,
    ),
    PsiPassCatalogEntry::new(
        PsiOptimization::RepresentationSpecialization,
        representation_specialization_rule_registrations,
    ),
];

pub(crate) fn registry_for_optimization(
    optimization: PsiOptimization,
) -> Result<OrderedRuleRegistry, RuleRegistryError> {
    let descriptor = PSI_PASS_CATALOG
        .iter()
        .copied()
        .find(|descriptor| descriptor.optimization() == optimization)
        .ok_or(RuleRegistryError::UnsupportedOptimization(optimization))?;
    assemble_built_in_registry(descriptor.registrations())
}

#[derive(Debug, Clone)]
pub(crate) struct BuiltInRuleRegistration {
    schedule_ordinal: u16,
    rule: Arc<dyn PsiOptimizationRule>,
}

impl BuiltInRuleRegistration {
    pub(crate) fn new(schedule_ordinal: u16, rule: impl PsiOptimizationRule + 'static) -> Self {
        Self {
            schedule_ordinal,
            rule: Arc::new(rule),
        }
    }
}

#[cfg(test)]
pub(crate) fn built_in_rule_registrations(
    optimization: PsiOptimization,
) -> Vec<BuiltInRuleRegistration> {
    PSI_PASS_CATALOG
        .iter()
        .copied()
        .find(|descriptor| descriptor.optimization() == optimization)
        .map(PsiPassCatalogEntry::registrations)
        .unwrap_or_default()
}

pub(crate) fn assemble_built_in_registry(
    mut registrations: Vec<BuiltInRuleRegistration>,
) -> Result<OrderedRuleRegistry, RuleRegistryError> {
    registrations.sort_by_key(|registration| registration.schedule_ordinal);
    for (expected, registration) in registrations.iter().enumerate() {
        let expected = u16::try_from(expected).expect("built-in rule schedule fits u16");
        assert_eq!(
            registration.schedule_ordinal, expected,
            "built-in rule schedule ordinals must be unique and contiguous"
        );
    }
    OrderedRuleRegistry::new(
        registrations
            .into_iter()
            .map(|registration| registration.rule),
    )
}
