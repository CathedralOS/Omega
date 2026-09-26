//! Crash route buckets and crash plans.

use crate::checked_trees::facts::contract_plans::{
    CheckedCrashCallSite, CheckedCrashSite, CrashCause, CrashInterface, CrashRouteBucketId,
    CrashRouteGuard, CrashSiteLocation,
};
use std::hash::Hash;
use symbols::SymbolHandle;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CrashRouteBucket {
    pub(super) cause: CrashCause,
    /// Canonical nonempty set. `Truth` is always the sole entry when present.
    pub(super) alternative_guards: Vec<CrashRouteGuard>,
}

impl CrashRouteBucket {
    pub fn new(cause: CrashCause, mut alternative_guards: Vec<CrashRouteGuard>) -> Option<Self> {
        if alternative_guards.contains(&CrashRouteGuard::Truth) {
            alternative_guards = vec![CrashRouteGuard::Truth];
        } else {
            alternative_guards.sort();
            alternative_guards.dedup();
        }
        (!alternative_guards.is_empty()).then_some(Self {
            cause,
            alternative_guards,
        })
    }

    pub fn unconditional(cause: CrashCause) -> Self {
        Self::new(cause, vec![CrashRouteGuard::Truth])
            .expect("the unconditional crash bucket has one canonical guard")
    }

    pub fn cause(&self) -> CrashCause {
        self.cause
    }

    pub fn alternative_guards(&self) -> &[CrashRouteGuard] {
        &self.alternative_guards
    }

    pub fn is_unconditional(&self) -> bool {
        self.alternative_guards == [CrashRouteGuard::Truth]
    }
}

/// The published and body-derived halves of CRASH-CONTRACT remain independent:
/// published route buckets are contract identity, while checked sites are
/// implementation evidence and never enter that fingerprint. Path guards,
/// complete covering buckets, and frontier lower bounds enrich
/// the site layer without changing the published interface.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CrashPlan {
    pub(crate) interface: CrashInterface,
    pub(crate) published: Vec<CrashRouteBucket>,
    /// Complete source-independent lowering of the machine and entry-state
    /// `requires` package into the bounded structural scalar vocabulary.
    /// `None` means at least one authored requirement is outside that
    /// vocabulary; consumers must not publish a partial terminal contract.
    structural_runtime_requirements: Option<Vec<crate::CheckedBooleanExpression>>,
    checked_sites: Vec<CheckedCrashSite>,
    checked_calls: Vec<CheckedCrashCallSite>,
    checked_operators:
        Vec<crate::checked_trees::facts::checked_crash_operator_site::CheckedCrashOperatorSite>,
    /// Body statements whose planned scalar value executes a Trapping
    /// primitive. Each such operation is its own `Trap` site; like an
    /// explicit crash it is unconditional implementation evidence for an
    /// inferred body, never part of the published contract identity.
    trapping_sites: Vec<CrashSiteLocation>,
}

impl CrashPlan {
    pub fn published_ceiling(mut published: Vec<CrashRouteBucket>) -> Self {
        published.sort();
        published.dedup();
        Self {
            interface: CrashInterface::PublishedCeiling,
            published,
            structural_runtime_requirements: None,
            checked_sites: Vec::new(),
            checked_calls: Vec::new(),
            checked_operators: Vec::new(),
            trapping_sites: Vec::new(),
        }
    }

    /// Attach the canonical (state, statement) roster of Trapping sites.
    pub fn with_trapping_sites(mut self, mut sites: Vec<CrashSiteLocation>) -> Self {
        sites.sort_by_key(|site| {
            (
                site.state.arena_index(),
                site.state.generation(),
                site.statement_ordinal,
            )
        });
        sites.dedup();
        self.trapping_sites = sites;
        self
    }

    pub fn trapping_sites(&self) -> &[CrashSiteLocation] {
        &self.trapping_sites
    }

    pub fn with_structural_runtime_requirements(
        mut self,
        requirements: Option<Vec<crate::CheckedBooleanExpression>>,
    ) -> Self {
        self.structural_runtime_requirements = requirements;
        self
    }

    pub fn structural_runtime_requirements(&self) -> Option<&[crate::CheckedBooleanExpression]> {
        self.structural_runtime_requirements.as_deref()
    }

    /// Whether a published structural crash predicate contains proof-gated
    /// arithmetic whose safety may depend on the complete retained runtime
    /// requirement package rather than self-proving literals alone.
    pub fn uses_structural_proof_gated_arithmetic(&self) -> bool {
        fn scalar_uses_proof_gated_arithmetic(expression: &crate::CheckedScalarExpression) -> bool {
            match expression {
                crate::CheckedScalarExpression::StructuralParameterIndexedRead {
                    index, ..
                } => scalar_uses_proof_gated_arithmetic(index),
                crate::CheckedScalarExpression::IntegerBinary {
                    kind, left, right, ..
                } => {
                    let exact = matches!(
                        kind,
                        crate::CheckedIntegerBinaryKind::ExactDivide
                            | crate::CheckedIntegerBinaryKind::ExactRemainder
                    );
                    let policy = matches!(
                        kind,
                        crate::CheckedIntegerBinaryKind::WrappingDivide
                            | crate::CheckedIntegerBinaryKind::WrappingRemainder
                            | crate::CheckedIntegerBinaryKind::SaturatingDivide
                            | crate::CheckedIntegerBinaryKind::SaturatingRemainder
                    );
                    let runtime_divisor = matches!(
                        kind,
                        crate::CheckedIntegerBinaryKind::ExactDivide
                            | crate::CheckedIntegerBinaryKind::ExactRemainder
                            | crate::CheckedIntegerBinaryKind::WrappingDivide
                            | crate::CheckedIntegerBinaryKind::WrappingRemainder
                            | crate::CheckedIntegerBinaryKind::SaturatingDivide
                            | crate::CheckedIntegerBinaryKind::SaturatingRemainder
                    ) && !matches!(
                        right.as_ref(),
                        crate::CheckedScalarExpression::IntegerLiteral { literal }
                            if literal.landing().is_some_and(|landing| {
                                if landing.landed_type.is_signed() {
                                    literal.value_i64().is_some_and(|value| {
                                        value != 0 && (policy || (exact && value != -1))
                                    })
                                } else {
                                    literal.value_u64().is_some_and(|value| value != 0)
                                }
                            })
                    );
                    let exact_shift = matches!(
                        kind,
                        crate::CheckedIntegerBinaryKind::ExactShiftLeft
                            | crate::CheckedIntegerBinaryKind::ExactShiftRight
                    );
                    runtime_divisor
                        || exact_shift
                        || scalar_uses_proof_gated_arithmetic(left)
                        || scalar_uses_proof_gated_arithmetic(right)
                }
                crate::CheckedScalarExpression::IntegerBitwiseNot { operand, .. }
                | crate::CheckedScalarExpression::IntegerWiden { operand, .. }
                | crate::CheckedScalarExpression::IntegerExactCast { operand, .. }
                | crate::CheckedScalarExpression::IntegerWrappingCast { operand, .. }
                | crate::CheckedScalarExpression::IntegerSaturatingCast { operand, .. } => {
                    scalar_uses_proof_gated_arithmetic(operand)
                }
                crate::CheckedScalarExpression::Boolean(expression) => {
                    boolean_uses_proof_gated_arithmetic(expression)
                }
                _ => false,
            }
        }

        fn boolean_uses_proof_gated_arithmetic(
            expression: &crate::CheckedBooleanExpression,
        ) -> bool {
            match expression {
                crate::CheckedBooleanExpression::Not(operand) => {
                    boolean_uses_proof_gated_arithmetic(operand)
                }
                crate::CheckedBooleanExpression::Equal { left, right }
                | crate::CheckedBooleanExpression::And { left, right }
                | crate::CheckedBooleanExpression::Or { left, right } => {
                    boolean_uses_proof_gated_arithmetic(left)
                        || boolean_uses_proof_gated_arithmetic(right)
                }
                crate::CheckedBooleanExpression::IntegerComparison { left, right, .. } => {
                    scalar_uses_proof_gated_arithmetic(left)
                        || scalar_uses_proof_gated_arithmetic(right)
                }
                _ => false,
            }
        }

        self.published.iter().any(|bucket| {
            bucket.alternative_guards().iter().any(|guard| {
                matches!(guard, CrashRouteGuard::Predicate(predicate)
                    if predicate.scalar_expression().is_some_and(boolean_uses_proof_gated_arithmetic))
            })
        })
    }

    pub fn with_checked_sites(mut self, mut checked_sites: Vec<CheckedCrashSite>) -> Option<Self> {
        checked_sites.sort_by_key(|site| {
            (
                site.location.state.arena_index(),
                site.location.state.generation(),
                site.location.statement_ordinal,
                site.cause,
            )
        });
        checked_sites.dedup();
        if checked_sites.windows(2).any(|sites| {
            sites[0].location.state == sites[1].location.state
                && sites[0].location.statement_ordinal == sites[1].location.statement_ordinal
        }) {
            return None;
        }
        if checked_sites.iter().any(|site| {
            site.guard_covering_buckets.iter().any(|bucket| {
                self.published_bucket(*bucket)
                    .is_none_or(|published| published.cause != site.cause)
            }) || site
                .frontier_lower_bound
                .contains(&language_semantics::PermissionClaimIdentity::Unknown)
        }) {
            return None;
        }
        self.checked_sites = checked_sites;
        Some(self)
    }

    pub fn interface(&self) -> CrashInterface {
        self.interface
    }

    pub fn published(&self) -> &[CrashRouteBucket] {
        &self.published
    }

    pub fn published_with_ids(
        &self,
    ) -> impl Iterator<Item = (CrashRouteBucketId, &CrashRouteBucket)> {
        self.published
            .iter()
            .enumerate()
            .map(|(index, bucket)| (CrashRouteBucketId::from_index(index), bucket))
    }

    pub fn published_bucket(&self, id: CrashRouteBucketId) -> Option<&CrashRouteBucket> {
        self.published.get(id.index()?)
    }

    pub fn checked_sites(&self) -> &[CheckedCrashSite] {
        &self.checked_sites
    }

    pub fn with_checked_calls(
        mut self,
        mut checked_calls: Vec<CheckedCrashCallSite>,
    ) -> Option<Self> {
        checked_calls.sort_by_key(|call| {
            (
                call.location.state.arena_index(),
                call.location.state.generation(),
                call.location.statement_ordinal,
                call.location.call_ordinal,
            )
        });
        checked_calls.dedup();
        if checked_calls
            .windows(2)
            .any(|calls| calls[0].location == calls[1].location)
        {
            return None;
        }
        self.checked_calls = checked_calls;
        Some(self)
    }

    pub fn checked_calls(&self) -> &[CheckedCrashCallSite] {
        &self.checked_calls
    }

    pub fn with_checked_operators(
        mut self,
        mut operators: Vec<
            crate::checked_trees::facts::checked_crash_operator_site::CheckedCrashOperatorSite,
        >,
    ) -> Option<Self> {
        // Exactly one identity form is valid on a site: a spelled use carries
        // its `operator_use`/`invocation` pair, a named call its `named_use`.
        let identity = |site: &crate::checked_trees::facts::checked_crash_operator_site::CheckedCrashOperatorSite| {
            if site.named_use.is_valid() {
                (1, site.named_use.arena_index(), site.named_use.generation())
            } else {
                (
                    0,
                    site.operator_use.arena_index(),
                    site.operator_use.generation(),
                )
            }
        };
        operators.sort_by_key(identity);
        if operators.iter().any(|site| {
            !site.selected_operator.is_valid()
                || site.operator_use.is_valid() == site.named_use.is_valid()
                || (site.named_use.is_valid() && site.invocation.is_valid())
                || (!site.named_use.is_valid() && !site.invocation.is_valid())
        }) || operators
            .windows(2)
            .any(|sites| identity(&sites[0]) == identity(&sites[1]))
        {
            return None;
        }
        self.checked_operators = operators;
        Some(self)
    }

    pub fn checked_operators(
        &self,
    ) -> &[crate::checked_trees::facts::checked_crash_operator_site::CheckedCrashOperatorSite] {
        &self.checked_operators
    }

    pub fn checked_call_at(
        &self,
        state: SymbolHandle,
        statement_ordinal: u32,
        call_ordinal: u32,
    ) -> Option<&CheckedCrashCallSite> {
        self.checked_calls.iter().find(|call| {
            call.location.state == state
                && call.location.statement_ordinal == statement_ordinal
                && call.location.call_ordinal == call_ordinal
        })
    }

    /// Published buckets whose guards cover this checked body site.
    pub fn covering_buckets_for_site<'plan>(
        &'plan self,
        site: &'plan CheckedCrashSite,
    ) -> impl Iterator<Item = (CrashRouteBucketId, &'plan CrashRouteBucket)> + 'plan {
        site.guard_covering_buckets.iter().filter_map(move |id| {
            self.published_bucket(*id)
                .and_then(|bucket| (bucket.cause == site.cause).then_some((*id, bucket)))
        })
    }

    pub fn checked_site_at(
        &self,
        state: SymbolHandle,
        statement_ordinal: u32,
    ) -> Option<&CheckedCrashSite> {
        self.checked_sites.iter().find(|site| {
            site.location.state == state && site.location.statement_ordinal == statement_ordinal
        })
    }
}
