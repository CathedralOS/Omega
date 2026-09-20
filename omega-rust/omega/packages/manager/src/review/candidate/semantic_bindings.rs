use super::{CompileResolvedPackageReviewsError, CompilerIssuedPackageReviewSet};
use crate::declarations::{DependencyPurpose, PackageKey};
use crate::lock::{PackageCheckedContext, PackageOccurrenceRoster};
use crate::resolution::graph::ExactTargetPackageSourceClosure;
use compiler::CheckedCompilation;
use effects::provider_plan::ServiceSchema;
use effects::{
    ServiceTerminalAuthorityPermission, TerminalAuthorityClass, TerminalAuthorityDisposition,
};
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
/// The consumer and checked context name an exact occurrence in the resolver-owned
/// closure. The binding is policy authority supplied only to that compilation; this
/// input is not proof of an audit, an audit receipt, or package admission.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsumerScopedSemanticBindingReviewInput {
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
/// The Console exit proposal for the closure root also carries the
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
/// A discovered `FilesystemHost` service binding at the root likewise carries
/// the toolchain-settled cohort permission table for its exact checked schema:
/// every method row the canonical boundary declares resolves its own settled
/// disposition, so the proposal remains exact per requirement and the accepted
/// policy projected for realization covers the demanded leaves. The table
/// refuses to fabricate rows for requirements outside the settled cohorts, and
/// the accepted binding still rejoins each row to its exact schema digest.
pub(super) fn candidate_semantic_binding_inputs(
    preliminary: &CompilerIssuedPackageReviewSet,
    root: &PackageKey,
) -> Result<Vec<ConsumerScopedSemanticBindingReviewInput>, CompileResolvedPackageReviewsError> {
    let mut inputs = Vec::new();
    for review in preliminary.reviews() {
        for candidate in &review.semantic_binding_candidates {
            let mut binding = candidate.binding().clone();
            if review.key() == root
                && review.checked_context().purpose() == DependencyPurpose::Product
                && binding.role() == AcceptedSemanticBindingRole::FilesystemHostService
            {
                binding = binding
                    .with_terminal_authority_permissions(
                        native_realization::filesystem_host_permission_rows(
                            candidate.service_schema(),
                        )
                        .map_err(|_| {
                            CompileResolvedPackageReviewsError::InvalidCandidateSemanticBinding {
                                consumer: review.key().clone(),
                                role: AcceptedSemanticBindingRole::FilesystemHostService,
                            }
                        })?,
                    )
                    .map_err(|_| {
                        CompileResolvedPackageReviewsError::InvalidCandidateSemanticBinding {
                            consumer: review.key().clone(),
                            role: AcceptedSemanticBindingRole::FilesystemHostService,
                        }
                    })?;
            }
            inputs.push(ConsumerScopedSemanticBindingReviewInput::new(
                review.key().clone(),
                review.checked_context(),
                binding,
            ));
        }
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
        let invalid = || CompileResolvedPackageReviewsError::InvalidCandidateSemanticBinding {
            consumer: review.key().clone(),
            role: AcceptedSemanticBindingRole::ConsoleExitProcessI32,
        };
        let mut binding = AcceptedSemanticBinding::new(
            AcceptedSemanticBindingRole::ConsoleExitProcessI32,
            package,
            provider.schema_declaration().path(),
            provider.schema().identity_digest(),
            provider.selected_plan_digest(),
        )
        .map_err(|_| invalid())?;
        if review.key() == root && review.checked_context().purpose() == DependencyPurpose::Product
        {
            binding = binding
                .with_terminal_authority_permissions(
                    candidate_console_permissions(provider).ok_or_else(invalid)?,
                )
                .map_err(|_| invalid())?;
        }
        inputs.push(ConsumerScopedSemanticBindingReviewInput::new(
            review.key().clone(),
            review.checked_context(),
            binding,
        ));
    }
    Ok(inputs)
}

/// The terminal-authority permission rows for the compiler-intrinsic Console
/// leaves of one nominated provider schema: process termination for the
/// exact `exit_process` requirement, process output for `write_byte` and
/// process input for `read_byte`. Each class is the one the compiler's own
/// mechanism classification assigns to that hosted leaf. The readable names
/// only select the schema methods, as the binding candidate itself does; each
/// row binds the schema digest and requirement identity, so a lookalike
/// schema cannot reuse an accepted row. The exit leaf is the nomination
/// itself and must be present exactly once; the byte leaves are proposed only
/// when the provider declares them as compiler-intrinsic rows, so a provider
/// without console I/O proposes no output or input row.
fn candidate_console_permissions(
    provider: &CheckedPackageProviderReview,
) -> Option<Vec<ServiceTerminalAuthorityPermission>> {
    const LEAVES: [(&str, TerminalAuthorityClass, bool); 3] = [
        (
            "exit_process",
            TerminalAuthorityClass::ProcessTermination,
            true,
        ),
        ("write_byte", TerminalAuthorityClass::ProcessOutput, false),
        ("read_byte", TerminalAuthorityClass::ProcessInput, false),
    ];
    let mut permissions = Vec::with_capacity(LEAVES.len());
    for (name, class, required) in LEAVES {
        let mut methods = provider
            .compiler_intrinsic_methods()
            .filter(|method| method.name == name);
        let Some(method) = methods.next() else {
            if required {
                return None;
            }
            continue;
        };
        if methods.next().is_some() {
            return None;
        }
        permissions.push(ServiceTerminalAuthorityPermission::new(
            provider.schema().identity_digest(),
            method.requirement_identity.clone(),
            TerminalAuthorityDisposition::from_classes([class]),
        ));
    }
    Some(permissions)
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

/// A checked dependency can nominate its target entry schema before an
/// application is checked. The proposal grants no entry authority: the
/// application's bound compilation must replay the exact package/schema and
/// calling pair, and the resulting review still requires consumer acceptance.
pub(super) fn candidate_target_entry_binding(
    checked: &CheckedCompilation,
    owner: &PackageKey,
    target: target::TargetProfile,
) -> Result<Option<SemanticBindingReviewCandidate>, CompileResolvedPackageReviewsError> {
    let slot = target.program_entry_slot();
    let (Some(package), Some(schema_name)) = (slot.physical_contract_package, slot.boundary_schema)
    else {
        return Ok(None);
    };
    let role = match package {
        target::ProgramEntryPhysicalContractPackage::UefiX64 => {
            AcceptedSemanticBindingRole::UefiX64ProgramEntry
        }
        target::ProgramEntryPhysicalContractPackage::MacosArm64 => {
            AcceptedSemanticBindingRole::MacosArm64ProgramEntry
        }
        target::ProgramEntryPhysicalContractPackage::LinuxX86_64 => {
            AcceptedSemanticBindingRole::LinuxX86_64ProgramEntry
        }
        target::ProgramEntryPhysicalContractPackage::LinuxArm64 => {
            AcceptedSemanticBindingRole::LinuxArm64ProgramEntry
        }
        target::ProgramEntryPhysicalContractPackage::WindowsX64 => {
            AcceptedSemanticBindingRole::WindowsX64ProgramEntry
        }
    };
    let mut definitions = checked.traits().iter().filter(|definition| {
        definition.is_boundary
            && definition.name.as_str() == schema_name
            && checked
                .typed
                .symbols
                .symbol_package_identity(definition.symbol)
                == Some(owner.identity())
    });
    let Some(definition) = definitions.next() else {
        return Ok(None);
    };
    if definitions.next().is_some() {
        return Err(
            CompileResolvedPackageReviewsError::AmbiguousCandidateSemanticBinding {
                consumer: owner.clone(),
                role,
                candidate_count: 2,
            },
        );
    }
    let path = checked.typed.symbols.display_path(definition.symbol, "::");
    let binding = checked
        .candidate_service_binding(role, owner.identity(), &path)
        .map_err(
            |_| CompileResolvedPackageReviewsError::InvalidCandidateSemanticBinding {
                consumer: owner.clone(),
                role,
            },
        )?;
    let service_schema = provider_planning::service_schema::from_typed(&checked.typed, definition)
        .ok_or_else(
            || CompileResolvedPackageReviewsError::InvalidCandidateSemanticBinding {
                consumer: owner.clone(),
                role,
            },
        )?;
    Ok(Some(SemanticBindingReviewCandidate::new(
        binding,
        service_schema,
    )))
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

#[cfg(test)]
mod tests {
    use super::{
        AcceptedSemanticBinding, AcceptedSemanticBindingRole, CompileResolvedPackageReviewsError,
        ConsumerScopedSemanticBindingReviewInput, DependencyPurpose,
        ExactTargetPackageSourceClosure, PackageCheckedContext, PackageKey,
        PackageOccurrenceRoster, semantic_bindings_by_consumer,
    };
    use crate::resolution::graph::{
        CanonicalSourceClosureSubject, CanonicalSourceClosureSubjectLimits,
        PackageSourceClosureLimits, ResolvedPackageSourceClosure,
        resolve_external_local_package_closure,
    };
    use package_source::{
        ExternalSourceContext, LocalSourceLimits, PrimaryGitChoices, SourceResolverStorage,
    };
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use target::TargetProfile;

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "omega-binding-context-{}-{stamp}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            for (name, dependencies) in [
                (
                    "root",
                    "builder.depend_as(\"product_shared\", Source::Path { location: \"../shared\" }); builder.build_depend_as(\"build_shared\", Source::Path { location: \"../shared\" });",
                ),
                ("shared", ""),
            ] {
                let package = root.join(name);
                std::fs::create_dir_all(&package).unwrap();
                std::fs::write(package.join("build.omg"), format!("machine build(builder: &mut Build) {{ builder.package(\"{name}\"); {dependencies} }}\n")).unwrap();
                std::fs::write(package.join("main.omg"), "pub const VALUE: u64 = 1;\n").unwrap();
            }
            Self(root)
        }

        fn closure(&self) -> ResolvedPackageSourceClosure {
            let storage = SourceResolverStorage::for_hardened_base(
                self.0.join("cache"),
                PrimaryGitChoices::default(),
            )
            .unwrap();
            resolve_external_local_package_closure(
                self.0.join("root"),
                ExternalSourceContext::derive(b"binding-context-tests"),
                &storage,
                LocalSourceLimits::default(),
                PackageSourceClosureLimits::default(),
            )
            .unwrap()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn binding(package: &PackageKey, marker: u8) -> AcceptedSemanticBinding {
        AcceptedSemanticBinding::new_service(
            AcceptedSemanticBindingRole::FilesystemHostService,
            package.identity(),
            "FilesystemHost",
            effects::provider_plan::ServiceSchemaDigest::from_digest([marker; 32]),
        )
        .unwrap()
    }

    fn roster(target: &ExactTargetPackageSourceClosure<'_>) -> PackageOccurrenceRoster {
        let source = CanonicalSourceClosureSubject::from_resolved(
            target,
            CanonicalSourceClosureSubjectLimits::default(),
        )
        .unwrap();
        PackageOccurrenceRoster::derive(&source).unwrap()
    }

    #[test]
    fn same_package_binding_roles_remain_separate_across_checked_contexts() {
        let fixture = Fixture::new();
        let closure = fixture.closure();
        let target = closure.for_exact_target(TargetProfile::WindowsX64);
        let roster = roster(&target);
        let package = roster
            .coverages()
            .iter()
            .find(|coverage| coverage.package().name().as_str() == "shared")
            .unwrap()
            .package();
        let execution = Some(TargetProfile::LinuxX64);
        let product = ConsumerScopedSemanticBindingReviewInput::new(
            package.clone(),
            PackageCheckedContext::new(
                DependencyPurpose::Product,
                TargetProfile::WindowsX64,
                execution,
            ),
            binding(package, 1),
        );
        let build = ConsumerScopedSemanticBindingReviewInput::new(
            package.clone(),
            PackageCheckedContext::new(
                DependencyPurpose::Build,
                TargetProfile::LinuxX64,
                execution,
            ),
            binding(package, 2),
        );
        let grouped = semantic_bindings_by_consumer(
            &target,
            execution,
            &roster,
            &[product.clone(), build.clone()],
        )
        .unwrap();
        assert_eq!(grouped.len(), 2);
        assert_eq!(
            grouped[&(package.clone(), DependencyPurpose::Product)],
            vec![product.binding().clone()]
        );
        assert_eq!(
            grouped[&(package.clone(), DependencyPurpose::Build)],
            vec![build.binding().clone()]
        );
        assert!(matches!(
            semantic_bindings_by_consumer(&target, execution, &roster, &[product.clone(), product]),
            Err(CompileResolvedPackageReviewsError::DuplicateConsumerSemanticBindingRole { .. })
        ));
    }

    #[test]
    fn bindings_reject_wrong_target_execution_profile_and_unauthorized_purpose() {
        let fixture = Fixture::new();
        let closure = fixture.closure();
        let target = closure.for_exact_target(TargetProfile::WindowsX64);
        let roster = roster(&target);
        let execution = Some(TargetProfile::LinuxX64);
        let shared = roster
            .coverages()
            .iter()
            .find(|coverage| coverage.package().name().as_str() == "shared")
            .unwrap()
            .package();
        for (package, context) in [
            (
                shared,
                PackageCheckedContext::new(
                    DependencyPurpose::Product,
                    TargetProfile::LinuxX64,
                    execution,
                ),
            ),
            (
                shared,
                PackageCheckedContext::new(
                    DependencyPurpose::Build,
                    TargetProfile::WindowsX64,
                    execution,
                ),
            ),
            (
                shared,
                PackageCheckedContext::new(
                    DependencyPurpose::Product,
                    TargetProfile::WindowsX64,
                    None,
                ),
            ),
            (
                shared,
                PackageCheckedContext::new(
                    DependencyPurpose::Product,
                    TargetProfile::WindowsX64,
                    Some(TargetProfile::MacosArm64),
                ),
            ),
            (
                closure.graph().root(),
                PackageCheckedContext::new(
                    DependencyPurpose::Build,
                    TargetProfile::LinuxX64,
                    execution,
                ),
            ),
        ] {
            let input = ConsumerScopedSemanticBindingReviewInput::new(
                package.clone(),
                context,
                binding(package, 1),
            );
            assert!(matches!(
                semantic_bindings_by_consumer(&target, execution, &roster, &[input]),
                Err(CompileResolvedPackageReviewsError::SemanticBindingContextMismatch { .. })
            ));
        }
        let input = ConsumerScopedSemanticBindingReviewInput::new(
            shared.clone(),
            PackageCheckedContext::new(DependencyPurpose::Build, TargetProfile::LinuxX64, None),
            binding(shared, 1),
        );
        assert!(matches!(
            semantic_bindings_by_consumer(&target, None, &roster, &[input]),
            Err(CompileResolvedPackageReviewsError::SemanticBindingContextMismatch { .. })
        ));
    }
}
