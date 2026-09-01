use super::{CompileResolvedPackageReviewsError, CompilerIssuedPackageReviewSet};
use crate::declarations::PackageKey;
use crate::resolution::graph::ResolvedPackageSourceClosure;
use omega_package_compilation::{AcceptedSemanticBinding, AcceptedSemanticBindingRole};
use omega_package_evidence::record::{CheckedPackageProviderReview, PackageReviewNominalOwner};
use std::collections::{BTreeMap, BTreeSet};

/// One consumer-scoped semantic-binding policy input for candidate review.
///
/// The consumer is an exact package key in the resolver-owned closure. The
/// binding is policy authority supplied to that consumer's compilation; this
/// input is not proof of an audit, an audit receipt, or package admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsumerScopedSemanticBindingReviewInput {
    consumer: PackageKey,
    binding: AcceptedSemanticBinding,
}

impl ConsumerScopedSemanticBindingReviewInput {
    pub fn new(consumer: PackageKey, binding: AcceptedSemanticBinding) -> Self {
        Self { consumer, binding }
    }

    pub fn consumer(&self) -> &PackageKey {
        &self.consumer
    }

    pub fn binding(&self) -> &AcceptedSemanticBinding {
        &self.binding
    }
}

pub(super) fn semantic_bindings_by_consumer(
    closure: &ResolvedPackageSourceClosure,
    inputs: &[ConsumerScopedSemanticBindingReviewInput],
) -> Result<BTreeMap<PackageKey, Vec<AcceptedSemanticBinding>>, CompileResolvedPackageReviewsError>
{
    let mut seen_roles = BTreeSet::<(PackageKey, AcceptedSemanticBindingRole)>::new();
    let mut bindings_by_consumer = BTreeMap::<PackageKey, Vec<AcceptedSemanticBinding>>::new();
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
        if !seen_roles.insert((consumer.clone(), role)) {
            return Err(
                CompileResolvedPackageReviewsError::DuplicateConsumerSemanticBindingRole {
                    consumer: consumer.clone(),
                    role,
                },
            );
        }
        bindings_by_consumer
            .entry(consumer.clone())
            .or_default()
            .push(input.binding().clone());
    }
    Ok(bindings_by_consumer)
}

/// Derive non-authoritative semantic-binding proposals from a preliminary
/// compiler review. The bound recompilation remains the exact validator, and
/// its resulting provider and authority rows still require root policy.
pub(super) fn candidate_semantic_binding_inputs(
    preliminary: &CompilerIssuedPackageReviewSet,
) -> Result<Vec<ConsumerScopedSemanticBindingReviewInput>, CompileResolvedPackageReviewsError> {
    let mut inputs = Vec::new();
    for review in preliminary.reviews() {
        let candidates = review
            .projection()
            .selected_providers()
            .iter()
            .filter(|provider| is_package_console_intrinsic_candidate(provider))
            .collect::<Vec<_>>();
        let provider = match candidates.as_slice() {
            [] => continue,
            [provider] => *provider,
            _ => {
                return Err(
                    CompileResolvedPackageReviewsError::AmbiguousCandidateSemanticBinding {
                        consumer: review.key().clone(),
                        role: AcceptedSemanticBindingRole::ConsoleExitProcessI32,
                        candidate_count: candidates.len(),
                    },
                );
            }
        };
        let PackageReviewNominalOwner::Package(package) = provider.schema_declaration().owner()
        else {
            unreachable!("candidate predicate admits only package-owned schemas")
        };
        let binding = AcceptedSemanticBinding::new(
            AcceptedSemanticBindingRole::ConsoleExitProcessI32,
            package,
            provider.schema_declaration().path(),
            provider.schema().identity_digest(),
            provider.selected_plan_digest(),
        )
        .map_err(|_| {
            CompileResolvedPackageReviewsError::InvalidCandidateSemanticBinding {
                consumer: review.key().clone(),
                role: AcceptedSemanticBindingRole::ConsoleExitProcessI32,
            }
        })?;
        inputs.push(ConsumerScopedSemanticBindingReviewInput::new(
            review.key().clone(),
            binding,
        ));
    }
    Ok(inputs)
}

fn is_package_console_intrinsic_candidate(provider: &CheckedPackageProviderReview) -> bool {
    let PackageReviewNominalOwner::Package(package) = provider.schema_declaration().owner() else {
        return false;
    };
    provider.service_schema() == "Console"
        && provider.provider_type_package() == Some(package)
        && provider.realizing_package() == Some(package)
        && provider
            .compiler_intrinsic_methods()
            .any(|method| method.name == "exit_process")
}
