//! Optimizer module role: executable entrance. Built-in Psi optimization stage entrance.
//!
//! [`PSI_PASS_CATALOG`] is the only enable/disable and pass-order table. This
//! entrance filters exact selections through it and returns one ordered
//! registry per selected pass. Each pass is one file beside this one --
//! `<pass>.rs` holds that pass's visible local rule order and the contracts
//! its rules share -- and its rules are the files under `<pass>/`, one per
//! rule (or one table per rule family over a single proposal). `node_elision`
//! owns shared removal proposals and provenance/fuel accounting;
//! `accepted_obligations` rejoins the proof facts those proposals consume.
//! Constant, dominance, use-definition and cycle queries belong to `analyses`.
//! Independent acceptance remains in `optimization-unit-semantics`.

const SCCP_PASS_NAME: &[u8] = b"omega.psi-pass.sparse-conditional-constant-propagation.v5";
const CONTROL_FLOW_CLEANUP_PASS_NAME: &[u8] = b"omega.psi-pass.control-flow-cleanup.v13";
const COPY_PROPAGATION_PASS_NAME: &[u8] = b"omega.psi-pass.copy-propagation.v1";
const DEAD_PURE_SCALAR_PASS_NAME: &[u8] = b"omega.psi-pass.dead-pure-scalar-elimination.v2";
const PROOF_CHECK_ELISION_PASS_NAME: &[u8] = b"omega.psi-pass.proof-check-elision.v12";
const GLOBAL_VALUE_NUMBERING_PASS_NAME: &[u8] = b"omega.psi-pass.global-value-numbering.v14";
const STATE_SPECIALIZATION_PASS_NAME: &[u8] = b"omega.psi-pass.state-specialization.v1";
const REPRESENTATION_SPECIALIZATION_PASS_NAME: &[u8] =
    b"omega.psi-pass.representation-specialization.v1";

/// The contract shape every representation-specialization rule declares: the
/// representation pass at version 1 requiring the strongly-connected-
/// components product and invalidating use-definition, scalar-constant,
/// value-range and effect-summary products under structural-identity safety.
/// One built-in rule's contract: each rule declares its identity bytes,
/// pass, and rosters; the checked-constructor boilerplate stays here.
fn standard_rule_contract(
    identity: &'static [u8],
    pass: &'static [u8],
    version: u32,
    required_analyses: AnalysisSet,
    invalidated_analyses: AnalysisInvalidationSet,
    safety_class: OptimizationSafetyClass,
) -> OptimizationRuleContract {
    OptimizationRuleContract::new(
        OptimizationRuleIdentity::from_canonical_bytes(identity),
        OptimizationPassIdentity::from_canonical_bytes(pass),
        version,
        required_analyses,
        invalidated_analyses,
        safety_class,
    )
    .expect("built-in rule has nonzero version")
}

fn representation_specialization_contract(identity: &'static [u8]) -> OptimizationRuleContract {
    standard_rule_contract(
        identity,
        REPRESENTATION_SPECIALIZATION_PASS_NAME,
        1,
        AnalysisSet::new([AnalysisKind::StronglyConnectedComponents]),
        AnalysisInvalidationSet::new([
            AnalysisKind::UseDefinition,
            AnalysisKind::ScalarConstants,
            AnalysisKind::ValueRanges,
            AnalysisKind::EffectSummaries,
        ]),
        OptimizationSafetyClass::StructuralIdentity,
    )
}

/// The closed rule surface every classify-driven rule exposes: its
/// contract plus a proposal that forwards unit, analyses and the contract
/// into a shared propose helper with the rule's classify extras. One
/// declarative row per rule type expands that impl; rules whose proposal
/// does its own analysis plumbing keep their handwritten impl.
///
/// One rule's named type plus its static contract body: the unit-struct
/// declaration and the inherent `contract()` that handwritten impls
/// forward into. Rules expanding this keep only their propose path.
macro_rules! psi_rule_contract {
    (@adapter) => {
        fn contract(&self) -> OptimizationRuleContract {
            Self::contract()
        }
    };
    ($(#[$meta:meta])* $name:ident, $contract:expr $(,)?) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, Default)]
        pub(crate) struct $name;

        impl $name {
            pub fn contract() -> OptimizationRuleContract {
                $contract
            }
        }
    };
    ($(#[$meta:meta])* $name:ident, $contract:expr, propose = $propose:ident $(, $extra:expr)* $(,)?) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, Default)]
        pub(crate) struct $name;

        impl $name {
            pub fn contract() -> OptimizationRuleContract {
                $contract
            }
        }

        impl PsiOptimizationRule for $name {
            fn contract(&self) -> OptimizationRuleContract {
                Self::contract()
            }

            fn propose(
                &self,
                unit: &PsiOptimizationUnit,
                analyses: RuleAnalysisView<'_>,
            ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> {
                $propose(unit, analyses, Self::contract(), $($extra),*)
            }
        }
    };
}

/// The closed proposal surface every handwritten adapter rule exposes: the
/// contract forwarding plus a `propose` that reads the unit and analyses from
/// the registry call and runs the rule's own body. One declarative row per
/// rule replaces the impl header every adapter rule restated; rules whose
/// whole proposal is a shared helper keep the `propose = <fn>` arm above.
macro_rules! psi_rule_impl {
    ($name:ident, |$unit:ident, $analyses:ident| $body:block $(,)?) => {
        impl PsiOptimizationRule for $name {
            psi_rule_contract!(@adapter);

            fn propose(
                &self,
                $unit: &PsiOptimizationUnit,
                $analyses: RuleAnalysisView<'_>,
            ) -> Result<Vec<PsiRewriteCandidate>, RuleProposalError> $body
        }
    };
}

/// The shared missing-analysis requirement:
/// `required_analysis!(view, AnalysisKind::K => Some(AnalysisProduct::K(b)))`
/// reads product `K` through the view's first-match lookup or returns the
/// rule's `MissingAnalysis` error at the declared prerequisite order.
macro_rules! required_analysis {
    ($analyses:expr, $kind:expr => $($product:tt)*) => {
        let $($product)* = $analyses.get($kind) else {
            return Err(RuleProposalError::MissingAnalysis($kind));
        };
    };
}

mod accepted_obligations;
mod catalog;
mod node_elision;
pub(crate) mod registry;

mod case_membership_specialization;
mod control_flow_cleanup;
mod copy_propagation;
mod dead_scalar_elimination;
mod field_value_specialization;
mod global_value_numbering;
mod proof_check_elision;
mod sparse_conditional_constant_propagation;
mod state_specialization;

/// The representation-specialization pass runs both registered families in
/// schedule order: case-membership folds (ordinal 0), then field-value folds
/// (ordinal 1). Both are published under the same public selection.
fn representation_specialization_rule_registrations() -> Vec<catalog::BuiltInRuleRegistration> {
    let mut registrations = case_membership_specialization::built_in_registrations();
    registrations.extend(field_value_specialization::built_in_registrations());
    registrations
}

use optimization::PsiOptimizationSelections;
use optimization_core::{
    AnalysisInvalidationSet, AnalysisKind, AnalysisSet, OptimizationPassIdentity,
    OptimizationRuleContract, OptimizationRuleIdentity, OptimizationRuleSetIdentity,
    OptimizationSafetyClass,
};

use crate::{OrderedRuleRegistry, RuleRegistryError};

/// One scheduled Psi pass: either a fixed-point rewrite registry over
/// `PsiRewriteCandidate`s under analysis-manager custody, or a session-level
/// ranked-rewrite boundary whose candidates relocate authenticated cyclic
/// components through `VerifiedPsiOptimizationSession::from_transformed`.
/// Both steps publish the same decision, commit, manifest, and policy rows —
/// only the candidate declaration shape differs.
pub(crate) enum PsiPassStep {
    /// An ordered catalog rule registry iterating to a unit-local fixed point.
    RewriteRegistry(OrderedRuleRegistry),
    /// The unique-preheader loop-invariant scalar relocation boundary.
    LoopInvariantScalarMotion,
}

impl PsiPassStep {
    /// The named pass group this step publishes in the run manifest. An
    /// empty rewrite registry carries no pass row.
    pub(crate) fn pass(&self) -> Option<OptimizationPassIdentity> {
        match self {
            Self::RewriteRegistry(registry) => registry.pass(),
            Self::LoopInvariantScalarMotion => Some(
                OptimizationPassIdentity::from_canonical_bytes(
                    crate::ranked_rewrites::loop_invariant_scalar_motion::LOOP_INVARIANT_SCALAR_MOTION_PASS_NAME,
                ),
            )}
    }

    /// The ordered contract roster this step admits: the registry's rules, or
    /// the relocation boundary's single contract.
    pub(crate) fn contracts(&self) -> Vec<OptimizationRuleContract> {
        match self {
            Self::RewriteRegistry(registry) => registry.contracts().collect(),
            Self::LoopInvariantScalarMotion => {
                vec![crate::ranked_rewrites::loop_invariant_scalar_motion::contract()]
            }
        }
    }

    /// The step's ordered rule identities in manifest order.
    pub(crate) fn ordered_rules(&self) -> Vec<OptimizationRuleIdentity> {
        self.contracts()
            .iter()
            .map(|contract| contract.identity())
            .collect()
    }

    /// Canonical identity over the step's ordered rule identities.
    pub(crate) fn rule_set_identity(&self) -> OptimizationRuleSetIdentity {
        match self {
            Self::RewriteRegistry(registry) => registry.identity(),
            Self::LoopInvariantScalarMotion => OptimizationRuleSetIdentity::from_ordered_rules(&[
                crate::ranked_rewrites::loop_invariant_scalar_motion::rule_identity(),
            ])
            .expect("a single-rule schedule cannot duplicate"),
        }
    }
}
#[cfg(any(test, feature = "test-support"))]
pub use sparse_conditional_constant_propagation::WrappingIntegerAddConstantsRule;

/// Resolve exact Psi-owned selections to the ordered pass schedule: catalog
/// rewrite registries plus session-level ranked-rewrite steps.
pub(crate) fn built_in_psi_steps_for_selections(
    selections: &PsiOptimizationSelections,
) -> Result<Vec<PsiPassStep>, RuleRegistryError> {
    if let Some(unsupported) = selections.as_slice().iter().find(|optimization| {
        !catalog::PSI_PASS_CATALOG
            .iter()
            .any(|entry| entry.optimization() == **optimization)
    }) {
        return Err(RuleRegistryError::UnsupportedOptimization(*unsupported));
    }
    catalog::PSI_PASS_CATALOG
        .iter()
        .copied()
        .filter(|entry| selections.contains(entry.optimization()))
        .map(catalog::PsiPassCatalogEntry::step)
        .collect()
}
