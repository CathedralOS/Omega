/*
    We validate consumer-scoped semantic-binding policy inputs
    (`semantic_bindings_by_consumer`) and derive non-authoritative semantic
    binding proposals (`candidate_semantic_binding_inputs`) from the preliminary
    discovery review pass for the bound recompilation pass.

    Only the closure's Product root occurrence may carry terminal-authority
    permission proposals (for `ProcessExit` / `Console` exit-binding leaves or
    settled `FilesystemHost` / `TimeHost` cohort permission tables) into root
    policy, because permissions are the consuming project's decision and every
    package's open permissions propagate into one root policy where duplicate
    rows would be rejected. Non-root or Build occurrences have their proposed
    terminal-authority permissions cleared before recompilation.
*/

use super::{CompileResolvedPackageReviewsError, CompilerIssuedPackageReviewSet};
use crate::declarations::{DependencyPurpose, PackageKey};
use crate::lock::{PackageCheckedContext, PackageOccurrenceRoster};
use crate::resolution::graph::ExactTargetPackageSourceClosure;
use package_compilation::{AcceptedSemanticBinding, AcceptedSemanticBindingRole};
use package_evidence::SemanticBindingProjectionError;
use provider_admission::{filesystem_host_permission_rows, time_host_permission_rows};
use std::collections::{BTreeMap, BTreeSet};
use terminal_psi_to_abstract_operations::provider_admission;

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
            return Err(CompileResolvedPackageReviewsError::new(
                crate::review::CompileResolvedPackageReviewCause::SemanticBindingConsumerAbsent {
                    consumer: consumer.clone(),
                    role,
                },
            ));
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
            return Err(CompileResolvedPackageReviewsError::new(
                crate::review::CompileResolvedPackageReviewCause::SemanticBindingContextMismatch {
                    consumer: consumer.clone(),
                    role,
                },
            ));
        }
        if !seen_roles.insert((consumer.clone(), context.purpose(), role)) {
            return Err(
                CompileResolvedPackageReviewsError::new(crate::review::CompileResolvedPackageReviewCause::DuplicateConsumerSemanticBindingRole {
                    consumer: consumer.clone(),
                    role,
                }),
            );
        }
        bindings_by_consumer
            .entry((consumer.clone(), context.purpose()))
            .or_default()
            .push(input.binding().clone());
    }
    Ok(bindings_by_consumer)
}

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
                        filesystem_host_permission_rows(
                            candidate.service_schema(),
                            &mut permission_identities,
                        )
                        .ok()
                    }
                    AcceptedSemanticBindingRole::TimeHostService => time_host_permission_rows(
                        candidate.service_schema(),
                        &mut permission_identities,
                    )
                    .ok(),
                    _ => None,
                };
                if matches!(
                    role,
                    AcceptedSemanticBindingRole::FilesystemHostService
                        | AcceptedSemanticBindingRole::TimeHostService
                ) {
                    let permissions = settled_permission_rows.ok_or_else(|| {
                        CompileResolvedPackageReviewsError::new(crate::review::CompileResolvedPackageReviewCause::InvalidCandidateSemanticBinding {
                            consumer: review.key().clone(),
                            role,
                        })
                    })?;
                    binding = binding
                        .with_terminal_authority_permissions(
                            permissions,
                            &permission_identities.snapshot(),
                        )
                        .map_err(|_| {
                            CompileResolvedPackageReviewsError::new(crate::review::CompileResolvedPackageReviewCause::InvalidCandidateSemanticBinding {
                                consumer: review.key().clone(),
                                role,
                            })
                        })?;
                }
            } else {
                binding = binding
                    .with_terminal_authority_permissions(
                        Vec::new(),
                        &terminal_psi::TerminalIdentityTable::from(vec![String::new()]),
                    )
                    .map_err(|_| {
                        CompileResolvedPackageReviewsError::new(crate::review::CompileResolvedPackageReviewCause::InvalidCandidateSemanticBinding {
                            consumer: review.key().clone(),
                            role,
                        })
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

pub(super) fn projection_error(
    consumer: &PackageKey,
    error: SemanticBindingProjectionError,
) -> CompileResolvedPackageReviewsError {
    match error {
        SemanticBindingProjectionError::Ambiguous {
            role,
            candidate_count,
        } => CompileResolvedPackageReviewsError::new(
            crate::review::CompileResolvedPackageReviewCause::AmbiguousCandidateSemanticBinding {
                consumer: consumer.clone(),
                role,
                candidate_count,
            },
        ),
        SemanticBindingProjectionError::MissingCallableFacts { diagnostics } => {
            CompileResolvedPackageReviewsError::new(
                crate::review::CompileResolvedPackageReviewCause::Compilation {
                    package: consumer.clone(),
                    failure: compilation_products::CompileFailure::Diagnostics(diagnostics),
                },
            )
        }
    }
}
