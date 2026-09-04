//! The complete ordered Psi pass catalog.
//!
//! This file is intentionally declarative. Each entry points to one pass
//! entrance, which visibly lists that pass's exact rule order.

use std::sync::Arc;

use psi_optimization::PsiOptimization;

use crate::{OrderedRuleRegistry, PsiOptimizationRule, RuleRegistryError};

use super::passes::{
    control_flow_cleanup_rule_registrations, copy_propagation_rule_registrations,
    dead_scalar_elimination_rule_registrations, global_value_numbering_rule_registrations,
    proof_check_elision_rule_registrations,
    sparse_conditional_constant_propagation_rule_registrations,
};

type RuleCatalog = fn() -> Vec<BuiltInRuleRegistration>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PsiPassTargetApplicability {
    TargetIndependent,
}

#[derive(Clone, Copy)]
struct PsiPassCatalogPayload {
    target: PsiPassTargetApplicability,
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
            payload: PsiPassCatalogPayload {
                target: PsiPassTargetApplicability::TargetIndependent,
                rule_catalog,
            },
        }
    }

    pub const fn optimization(self) -> PsiOptimization {
        self.optimization
    }

    pub const fn target_applicability(self) -> PsiPassTargetApplicability {
        self.payload.target
    }

    fn registrations(self) -> Vec<BuiltInRuleRegistration> {
        (self.payload.rule_catalog)()
    }
}

/// The single built-in Psi enable/disable and ordering table.
pub const PSI_PASS_CATALOG: [PsiPassCatalogEntry; 6] = [
    PsiPassCatalogEntry::new(
        PsiOptimization::SparseConditionalConstantPropagation,
        sparse_conditional_constant_propagation_rule_registrations,
    ),
    PsiPassCatalogEntry::new(
        PsiOptimization::ControlFlowCleanup,
        control_flow_cleanup_rule_registrations,
    ),
    PsiPassCatalogEntry::new(
        PsiOptimization::CopyPropagation,
        copy_propagation_rule_registrations,
    ),
    PsiPassCatalogEntry::new(
        PsiOptimization::GlobalValueNumbering,
        global_value_numbering_rule_registrations,
    ),
    PsiPassCatalogEntry::new(
        PsiOptimization::ProofCheckElision,
        proof_check_elision_rule_registrations,
    ),
    PsiPassCatalogEntry::new(
        PsiOptimization::DeadPureScalarElimination,
        dead_scalar_elimination_rule_registrations,
    ),
];

/// Compatibility view derived from [`PSI_PASS_CATALOG`], never a second table.
pub const ORDERED_PSI_PASSES: [PsiOptimization; 6] = [
    PSI_PASS_CATALOG[0].optimization(),
    PSI_PASS_CATALOG[1].optimization(),
    PSI_PASS_CATALOG[2].optimization(),
    PSI_PASS_CATALOG[3].optimization(),
    PSI_PASS_CATALOG[4].optimization(),
    PSI_PASS_CATALOG[5].optimization(),
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
