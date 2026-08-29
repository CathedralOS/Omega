use std::sync::Arc;

use omega_optimization_core::{Optimization, OptimizationExecutionPhase, OptimizationSelections};

use crate::{OrderedRuleRegistry, PsiOptimizationRule, RuleRegistryError};

use super::passes::*;

pub fn built_in_psi_registry(
    selections: &OptimizationSelections,
) -> Result<OrderedRuleRegistry, RuleRegistryError> {
    let mut registries = built_in_psi_registries(selections)?;
    if registries.len() > 1 {
        return Err(RuleRegistryError::UnsupportedOptimizationCombination);
    }
    Ok(registries
        .pop()
        .unwrap_or_else(|| OrderedRuleRegistry::new(Vec::new()).expect("empty registry is valid")))
}

/// Build the canonical pass-group schedule for an exact named selection set.
///
/// Selection declaration order is not pass order. The explicit schedule below
/// runs semantic constant propagation before CFG cleanup, structural copy
/// cleanup, local/global value numbering,
/// proof-certified check/work elision, and dead pure scalar
/// elimination. Each returned registry continues to own exactly one pass
/// identity.
pub fn built_in_psi_registries(
    selections: &OptimizationSelections,
) -> Result<Vec<OrderedRuleRegistry>, RuleRegistryError> {
    let psi_selections = selections.for_phase(OptimizationExecutionPhase::Psi);
    if let Some(unsupported) = psi_selections.as_slice().iter().find(|optimization| {
        !matches!(
            optimization,
            Optimization::SparseConditionalConstantPropagation
                | Optimization::ControlFlowCleanup
                | Optimization::CopyPropagation
                | Optimization::GlobalValueNumbering
                | Optimization::DeadPureScalarElimination
                | Optimization::ProofCheckElision
        )
    }) {
        return Err(RuleRegistryError::UnsupportedOptimization(*unsupported));
    }
    let mut registries = Vec::new();
    if psi_selections.contains(Optimization::SparseConditionalConstantPropagation) {
        registries.push(registry_for_optimization(
            Optimization::SparseConditionalConstantPropagation,
        )?);
    }
    if psi_selections.contains(Optimization::ControlFlowCleanup) {
        registries.push(registry_for_optimization(Optimization::ControlFlowCleanup)?);
    }
    if psi_selections.contains(Optimization::CopyPropagation) {
        registries.push(registry_for_optimization(Optimization::CopyPropagation)?);
    }
    if psi_selections.contains(Optimization::GlobalValueNumbering) {
        registries.push(registry_for_optimization(
            Optimization::GlobalValueNumbering,
        )?);
    }
    if psi_selections.contains(Optimization::ProofCheckElision) {
        registries.push(registry_for_optimization(Optimization::ProofCheckElision)?);
    }
    if psi_selections.contains(Optimization::DeadPureScalarElimination) {
        registries.push(registry_for_optimization(
            Optimization::DeadPureScalarElimination,
        )?);
    }
    Ok(registries)
}

pub(crate) fn registry_for_optimization(
    optimization: Optimization,
) -> Result<OrderedRuleRegistry, RuleRegistryError> {
    assemble_built_in_registry(built_in_rule_registrations(optimization))
}

#[derive(Debug, Clone)]
pub(crate) struct BuiltInRuleRegistration {
    schedule_ordinal: u16,
    rule: Arc<dyn PsiOptimizationRule>,
}

pub(crate) fn built_in_rule_registrations(
    optimization: Optimization,
) -> Vec<BuiltInRuleRegistration> {
    let mut registrations = Vec::new();
    macro_rules! register {
        ($ordinal:literal, $rule:expr) => {
            registrations.push(BuiltInRuleRegistration {
                schedule_ordinal: $ordinal,
                rule: Arc::new($rule),
            });
        };
    }
    if optimization == Optimization::SparseConditionalConstantPropagation {
        register!(0, ExactIntegerAddConstantsRule);
        register!(1, ExactIntegerSubtractConstantsRule);
        register!(2, ExactIntegerMultiplyConstantsRule);
        register!(3, WrappingIntegerAddConstantsRule);
        register!(4, WrappingIntegerSubtractConstantsRule);
        register!(5, WrappingIntegerMultiplyConstantsRule);
        register!(6, SaturatingIntegerAddConstantsRule);
        register!(7, SaturatingIntegerSubtractConstantsRule);
        register!(8, SaturatingIntegerMultiplyConstantsRule);
        register!(9, ExactIntegerDivideConstantsRule);
        register!(10, ExactIntegerRemainderConstantsRule);
        register!(11, WrappingIntegerDivideConstantsRule);
        register!(12, WrappingIntegerRemainderConstantsRule);
        register!(13, SaturatingIntegerDivideConstantsRule);
        register!(14, SaturatingIntegerRemainderConstantsRule);
        register!(15, ExactIntegerShiftLeftConstantsRule);
        register!(16, ExactIntegerShiftRightConstantsRule);
        register!(17, WrappingIntegerShiftLeftConstantsRule);
        register!(18, WrappingIntegerShiftRightConstantsRule);
        register!(19, ExactIntegerCastConstantsRule);
        register!(20, IntegerWidenConstantsRule);
        register!(21, IntegerBitwiseNotConstantsRule);
        register!(22, IntegerBitwiseAndConstantsRule);
        register!(23, IntegerBitwiseOrConstantsRule);
        register!(24, IntegerBitwiseXorConstantsRule);
        register!(25, BooleanNotConstantsRule);
        register!(26, BooleanEqualConstantsRule);
        register!(27, IntegerEqualConstantsRule);
        register!(28, IntegerLessThanConstantsRule);
        register!(29, IntegerLessOrEqualConstantsRule);
        register!(30, IntegerLessThanRangeConstantRule);
        register!(31, IntegerLessThanConstantRangeRule);
        register!(32, IntegerLessOrEqualRangeConstantRule);
        register!(33, IntegerLessOrEqualConstantRangeRule);
        register!(34, IntegerEqualRangeConstantRule);
        register!(35, IntegerEqualConstantRangeRule);
        register!(36, IntegerEqualRangeRangeRule);
        register!(37, IntegerLessThanRangeRangeRule);
        register!(38, IntegerLessOrEqualRangeRangeRule);
    }
    if optimization == Optimization::ControlFlowCleanup {
        register!(0, ConstantConditionalFoldRule);
        register!(1, LinearEmptyBlockThreadRule);
        register!(2, PathQualifiedEmptyBlockThreadRule);
        register!(3, AdjacentBlockMergeRule);
        register!(4, SharedJumpFusionRule);
        register!(5, UnreachablePrivateMachinePruneRule);
        register!(6, NonAdjacentBlockMergeRule);
    }
    if optimization == Optimization::CopyPropagation {
        register!(0, RedundantBlockParameterRule);
    }
    if optimization == Optimization::GlobalValueNumbering {
        register!(0, SameBlockTotalScalarCseRule);
        register!(1, SameBlockProofCertifiedScalarCseRule);
        register!(2, DominatorTotalScalarGvnRule);
        register!(3, DominatorProofCertifiedScalarGvnRule);
        register!(4, PhiTranslatedObligationFreeScalarGvnRule);
        register!(5, PhiTranslatedProofCertifiedScalarGvnRule);
        register!(6, SameBlockProofCertifiedCompatiblePolicyScalarCseRule);
        register!(7, DominatorProofCertifiedCompatiblePolicyScalarGvnRule);
        register!(8, PhiTranslatedProofCertifiedCompatiblePolicyScalarGvnRule);
    }
    if optimization == Optimization::DeadPureScalarElimination {
        register!(0, DeadScalarLiteralEliminationRule);
        register!(1, DeadUnconditionallyTotalScalarEliminationRule);
    }
    if optimization == Optimization::ProofCheckElision {
        register!(0, ProofCertifiedDeadScalarEliminationRule);
        register!(1, LiveProofCertifiedIntegerIdentityEliminationRule);
        register!(2, LiveProofCertifiedIntegerDivideByOneEliminationRule);
        register!(
            3,
            LiveProofCertifiedExactIntegerMultiplyByZeroEliminationRule
        );
        register!(4, LiveProofCertifiedIntegerZeroDividendEliminationRule);
        register!(
            5,
            LiveProofCertifiedExactIntegerZeroValueShiftEliminationRule
        );
        register!(6, LiveProofCertifiedExactIntegerSelfSubtractEliminationRule);
        register!(7, LiveProofCertifiedIntegerSelfRemainderEliminationRule);
        register!(8, LiveProofCertifiedIntegerSelfDivideEliminationRule);
        register!(9, LiveProofCertifiedIntegerRemainderByOneEliminationRule);
        register!(
            10,
            LiveProofCertifiedSignedIntegerRemainderByNegativeOneEliminationRule
        );
    }
    registrations
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
