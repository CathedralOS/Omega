use super::{CompileResolvedPackageReviewsError, CompilerIssuedPackageReviewSet};
use crate::declarations::{DependencyPurpose, PackageKey};
use crate::lock::{PackageCheckedContext, PackageOccurrenceRoster};
use crate::resolution::graph::ExactTargetPackageSourceClosure;
use package_compilation::{AcceptedSemanticBinding, AcceptedSemanticBindingRole};
use package_evidence::SemanticBindingProjectionError;
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi_to_abstract_operations::provider_admission;

/// One consumer-scoped semantic-binding policy input for candidate review.
///
/// The consumer and checked context name an exact occurrence in the resolver-owned
/// closure. The binding is policy authority supplied only to that compilation; this
/// input is not proof of an audit, an audit receipt, or package admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConsumerScopedSemanticBindingReviewInput {
    consumer: PackageKey,
    checked_context: PackageCheckedContext,
    binding: AcceptedSemanticBinding,
}

impl ConsumerScopedSemanticBindingReviewInput {
    pub fn new(
        consumer: PackageKey,
        checked_context: PackageCheckedContext,
        binding: AcceptedSemanticBinding,
    ) -> Self {
        Self {
            consumer,
            checked_context,
            binding,
        }
    }

    pub fn consumer(&self) -> &PackageKey {
        &self.consumer
    }

    pub const fn checked_context(&self) -> PackageCheckedContext {
        self.checked_context
    }

    pub fn binding(&self) -> &AcceptedSemanticBinding {
        &self.binding
    }
}

pub(super) fn semantic_bindings_by_consumer(
    target_closure: &ExactTargetPackageSourceClosure<'_>,
    execution_profile: Option<target::TargetProfile>,
    roster: &PackageOccurrenceRoster,
    inputs: &[ConsumerScopedSemanticBindingReviewInput],
) -> Result<
    BTreeMap<(PackageKey, DependencyPurpose), Vec<AcceptedSemanticBinding>>,
    CompileResolvedPackageReviewsError,
> {
    let mut seen_roles = BTreeSet::new();
    let mut bindings_by_consumer =
        BTreeMap::<(PackageKey, DependencyPurpose), Vec<AcceptedSemanticBinding>>::new();
    if inputs.is_empty() {
        return Ok(bindings_by_consumer);
    }
    let closure = target_closure.source_closure();
    for input in inputs {
        let consumer = input.consumer();
        let role = input.binding().role();
        if closure.custody(consumer).is_none() {
            return Err(
                CompileResolvedPackageReviewsError::SemanticBindingConsumerAbsent {
                    consumer: consumer.clone(),
                    role,
                },
            );
        }
        let context = input.checked_context();
        let expected_target = match context.purpose() {
            DependencyPurpose::Product => Some(target_closure.target_profile()),
            DependencyPurpose::Build => execution_profile,
        };
        if !roster.is_occurrence(consumer, context.purpose())
            || Some(context.target()) != expected_target
            || context.build_execution_profile() != execution_profile
        {
            return Err(
                CompileResolvedPackageReviewsError::SemanticBindingContextMismatch {
                    consumer: consumer.clone(),
                    role,
                },
            );
        }
        if !seen_roles.insert((consumer.clone(), context.purpose(), role)) {
            return Err(
                CompileResolvedPackageReviewsError::DuplicateConsumerSemanticBindingRole {
                    consumer: consumer.clone(),
                    role,
                },
            );
        }
        bindings_by_consumer
            .entry((consumer.clone(), context.purpose()))
            .or_default()
            .push(input.binding().clone());
    }
    Ok(bindings_by_consumer)
}

/// Derive non-authoritative semantic-binding proposals from a preliminary
/// compiler review. The bound recompilation remains the exact validator, and
/// its resulting provider and authority rows still require root policy.
///
/// Console and core ProcessExit use the same exit-binding proposal flow. Console
/// owns its schema in the provider package; ProcessExit owns its schema in core
/// and binds the provider package instead. Both retain the exact selected plan
/// and require the bound recompilation to validate that ownership relationship.
/// Intrinsic and imported exit leaves go through the same review: target linkage
/// changes realization, not the consumer's obligation to approve termination.
///
/// The exit proposal for the closure root also carries the
/// terminal-authority permissions its recognized compiler-intrinsic leaves
/// need at native realization: process termination for `exit_process`, and
/// process output and input for the `write_byte` and `read_byte` rows the
/// nominated provider declares. Without those rows the review has nothing to
/// decide, the lock retains no permission, and the accepted policy projected
/// for realization is empty, so the ordinary CLI route can never realize a
/// program that writes, reads or exits. Each row is a proposal, not a grant:
/// it surfaces in the review as its own blocking `terminal_permission`
/// decision, the final pass rejoins it to exactly one schema method, and
/// realization still checks the selected mechanism's exercised classes
/// against it. Only the root consumer receives them because permissions are
/// the consuming project's decision and every package's open permissions
/// propagate into one root policy, where an identical row accepted twice
/// would be a duplicate rather than a grant.
///
/// A discovered `FilesystemHost` or `TimeHost` service binding at the root
/// likewise carries the toolchain-settled cohort permission table for its
/// exact checked schema: every method row the canonical boundary declares
/// resolves its own settled disposition, so the proposal remains exact per
/// requirement and the accepted policy projected for realization covers the
/// demanded leaves. The table refuses to fabricate rows for requirements
/// outside the settled cohorts, and the accepted binding still rejoins each
/// row to its exact schema digest.
pub(super) fn candidate_semantic_binding_inputs(
    preliminary: &CompilerIssuedPackageReviewSet,
    root: &PackageKey,
) -> Result<Vec<ConsumerScopedSemanticBindingReviewInput>, CompileResolvedPackageReviewsError> {
    let mut inputs = Vec::new();
    for review in preliminary.reviews() {
        for candidate in &review.semantic_binding_candidates {
            let mut binding = candidate.binding().clone();
            let role = binding.role();
            if review.key() == root
                && review.checked_context().purpose() == DependencyPurpose::Product
            {
                let mut permission_identities =
                    terminal_psi::TerminalIdentityTableBuilder::default();
                let settled_permission_rows = match role {
                    AcceptedSemanticBindingRole::FilesystemHostService => {
                        provider_admission::filesystem_host_permission_rows(
                            candidate.service_schema(),
                            &mut permission_identities,
                        )
                        .ok()
                    }
                    AcceptedSemanticBindingRole::TimeHostService => {
                        provider_admission::time_host_permission_rows(
                            candidate.service_schema(),
                            &mut permission_identities,
                        )
                        .ok()
                    }
                    _ => None,
                };
                if matches!(
                    role,
                    AcceptedSemanticBindingRole::FilesystemHostService
                        | AcceptedSemanticBindingRole::TimeHostService
                ) {
                    let permissions = settled_permission_rows.ok_or_else(|| {
                        CompileResolvedPackageReviewsError::InvalidCandidateSemanticBinding {
                            consumer: review.key().clone(),
                            role,
                        }
                    })?;
                    binding = binding
                        .with_terminal_authority_permissions(
                            permissions,
                            &permission_identities.snapshot(),
                        )
                        .map_err(|_| {
                            CompileResolvedPackageReviewsError::InvalidCandidateSemanticBinding {
                                consumer: review.key().clone(),
                                role,
                            }
                        })?;
                }
            } else {
                // Cached nominations are package-local, not root-local.
                // A hosted candidate may propose permissions, but only this
                // invocation's product root may carry them into policy.
                binding = binding
                    .with_terminal_authority_permissions(
                        Vec::new(),
                        &terminal_psi::TerminalIdentityTable::from(vec![String::new()]),
                    )
                    .map_err(|_| {
                        CompileResolvedPackageReviewsError::InvalidCandidateSemanticBinding {
                            consumer: review.key().clone(),
                            role,
                        }
                    })?;
            }
            inputs.push(ConsumerScopedSemanticBindingReviewInput::new(
                review.key().clone(),
                review.checked_context(),
                binding,
            ));
        }
    }
    Ok(inputs)
}

/// Attach the resolver's occurrence identity without interpreting compiler facts.
pub(super) fn projection_error(
    consumer: &PackageKey,
    error: SemanticBindingProjectionError,
) -> CompileResolvedPackageReviewsError {
    match error {
        SemanticBindingProjectionError::Ambiguous {
            role,
            candidate_count,
        } => CompileResolvedPackageReviewsError::AmbiguousCandidateSemanticBinding {
            consumer: consumer.clone(),
            role,
            candidate_count,
        },
        SemanticBindingProjectionError::Invalid { role } => {
            CompileResolvedPackageReviewsError::InvalidCandidateSemanticBinding {
                consumer: consumer.clone(),
                role,
            }
        }
        SemanticBindingProjectionError::MissingCallableFacts { diagnostics } => {
            CompileResolvedPackageReviewsError::Compilation {
                package: consumer.clone(),
                diagnostics,
            }
        }
    }
}
