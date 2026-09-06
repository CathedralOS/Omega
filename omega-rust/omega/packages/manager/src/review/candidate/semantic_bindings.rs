use super::{CompileResolvedPackageReviewsError, CompilerIssuedPackageReviewSet};
use crate::declarations::PackageKey;
use crate::resolution::graph::ResolvedPackageSourceClosure;
use compiler::CheckedCompilation;
use effects::provider_plan::ServiceSchema;
use package_compilation::{AcceptedSemanticBinding, AcceptedSemanticBindingRole};
use package_evidence::record::{
    CheckedPackageCallableReview, CheckedPackageProviderReview, CheckedPackageReviewProjection,
    PackageReviewNominalIdentity, PackageReviewNominalOwner,
};
use std::collections::{BTreeMap, BTreeSet};

/// One compiler-issued semantic-binding proposal together with the exact
/// checked service schema from which a consumer may author permission rows.
///
/// The schema is review material, not policy: this carrier never assigns an
/// authority class from a service path or readable method name. A caller must
/// explicitly choose exact requirement identities and submit the resulting
/// binding for a complete checked recompilation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticBindingReviewCandidate {
    binding: AcceptedSemanticBinding,
    service_schema: ServiceSchema,
}

impl SemanticBindingReviewCandidate {
    fn new(binding: AcceptedSemanticBinding, service_schema: ServiceSchema) -> Self {
        debug_assert_eq!(
            binding.normalized_schema_digest(),
            package_compilation::accepted_service_schema_digest(binding.role(), &service_schema,),
        );
        Self {
            binding,
            service_schema,
        }
    }

    pub const fn binding(&self) -> &AcceptedSemanticBinding {
        &self.binding
    }

    pub const fn service_schema(&self) -> &ServiceSchema {
        &self.service_schema
    }
}

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
        inputs.extend(review.semantic_binding_candidates.iter().map(|candidate| {
            ConsumerScopedSemanticBindingReviewInput::new(
                review.key().clone(),
                candidate.binding().clone(),
            )
        }));
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

/// Nominate exact package-owned requirement surfaces that the consumer's
/// checked API actually reaches. Readable names guide this confined candidate
/// pass only; the returned row binds exact package ownership, nominal path, and
/// normalized schema and must be consumed by the compiler on the final pass.
pub(super) fn candidate_service_bindings(
    checked: &CheckedCompilation,
    projection: &CheckedPackageReviewProjection,
    consumer: &PackageKey,
) -> Result<Vec<SemanticBindingReviewCandidate>, CompileResolvedPackageReviewsError> {
    let referenced = projection
        .callables()
        .iter()
        .flat_map(callable_service_references)
        .filter(|service| {
            service.path() == "FilesystemHost"
                && matches!(service.owner(), PackageReviewNominalOwner::Package(_))
        })
        .cloned()
        .collect::<BTreeSet<_>>();
    let candidates = referenced
        .iter()
        .filter_map(|service| match service.owner() {
            PackageReviewNominalOwner::Package(package) => {
                Some((package, service.path().to_owned()))
            }
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    let candidates = candidates.iter().collect::<Vec<_>>();
    let [(package, path)] = candidates.as_slice() else {
        if candidates.is_empty() {
            return Ok(Vec::new());
        }
        return Err(
            CompileResolvedPackageReviewsError::AmbiguousCandidateSemanticBinding {
                consumer: consumer.clone(),
                role: AcceptedSemanticBindingRole::FilesystemHostService,
                candidate_count: candidates.len(),
            },
        );
    };
    let binding = checked
        .candidate_service_binding(
            AcceptedSemanticBindingRole::FilesystemHostService,
            *package,
            path,
        )
        .map_err(
            |_| CompileResolvedPackageReviewsError::InvalidCandidateSemanticBinding {
                consumer: consumer.clone(),
                role: AcceptedSemanticBindingRole::FilesystemHostService,
            },
        )?;
    let definitions = checked
        .traits()
        .iter()
        .filter(|definition| {
            checked
                .typed
                .symbols
                .symbol_package_identity(definition.symbol)
                == Some(*package)
                && checked.typed.symbols.display_path(definition.symbol, "::") == *path
        })
        .collect::<Vec<_>>();
    let [definition] = definitions.as_slice() else {
        return Err(
            CompileResolvedPackageReviewsError::InvalidCandidateSemanticBinding {
                consumer: consumer.clone(),
                role: AcceptedSemanticBindingRole::FilesystemHostService,
            },
        );
    };
    let Some(service_schema) =
        provider_planning::service_schema::from_typed(&checked.typed, definition)
    else {
        return Err(
            CompileResolvedPackageReviewsError::InvalidCandidateSemanticBinding {
                consumer: consumer.clone(),
                role: AcceptedSemanticBindingRole::FilesystemHostService,
            },
        );
    };
    if package_compilation::accepted_service_schema_digest(binding.role(), &service_schema)
        != binding.normalized_schema_digest()
    {
        return Err(
            CompileResolvedPackageReviewsError::InvalidCandidateSemanticBinding {
                consumer: consumer.clone(),
                role: AcceptedSemanticBindingRole::FilesystemHostService,
            },
        );
    }
    Ok(vec![SemanticBindingReviewCandidate::new(
        binding,
        service_schema,
    )])
}

fn callable_service_references(
    callable: &CheckedPackageCallableReview,
) -> Vec<&PackageReviewNominalIdentity> {
    let mut services = Vec::new();
    if let Some(reach) = callable.declared_service_reach() {
        services.extend(reach);
    }
    if let Some(reach) = callable.checked_service_reach().realized() {
        services.extend(reach);
    }
    if let Some(reach) = callable.checked_service_reach().concrete() {
        services.extend(reach);
    }
    for reach in callable.unresolved_installation_reaches() {
        services.extend(reach.upper_bound());
    }
    if let Some(invocations) = callable.declared_synchronous_invocations() {
        services.extend(
            invocations
                .iter()
                .filter_map(|invocation| invocation.service()),
        );
    }
    services.extend(
        callable
            .realized_synchronous_invocations()
            .iter()
            .filter_map(|invocation| invocation.service()),
    );
    services
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
