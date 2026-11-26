//! Assemble a policy only after every inventory it claims is accounted for.
//! Empty rows below are justified by the accepted source/module inventories;
//! an unsupported nonempty inventory refuses before a baseline is returned.

use super::declarations::unavailable;
use super::input::PolicySource;
use super::provider_rows;
use diagnostics::Diagnostic;
use language_core::core_vocabulary::CoreVocabulary;
use review_evidence::record::{
    PackagePolicyBaseline, PackagePolicyBoundaryApplications, PackagePolicyCallables,
    PackagePolicyPublicApi, PackagePolicyRepresentation, PackagePolicyRestrictedBuildRequest,
    PackagePolicySelectedProviders, PackageReviewNominalIdentity,
    PackageReviewRepresentationArchitecture, PackageReviewRepresentationObjectFormat,
    PackageReviewRepresentationTarget, PackageReviewRepresentationTargetProfile,
    PackageReviewServiceIdentity,
};
use semantic_vocabulary::PackageKeyIdentity;
use target::TargetProfile;

pub(super) fn project(
    services: &[super::services::ReviewedService],
    input: &PolicySource<'_>,
    checked: &compiler::SelectedProviderResult,
    target: TargetProfile,
    package: PackageKeyIdentity,
    component_assumptions: Vec<review_evidence::record::PackagePolicyComponentAssumption>,
) -> Result<PackagePolicyBaseline, Vec<Diagnostic>> {
    if input.sources().package_identity() != Some(package)
        || input.declarations.published_program() != checked.original_program()
        || checked.target_profile() != Some(target)
        || checked.native_target() != Some(target.native_target())
    {
        return Err(unavailable(
            "a policy for a different package or target activation",
        ));
    }
    let accepted = input.accepted();
    let choices = provider_rows::from_review(checked)?;
    let census =
        super::providers::AcceptedDeclarations::new(accepted, services, !choices.is_empty());
    let native = super::native_bindings::prepare(input, package, target, &census)?;
    let providers = input.providers(
        checked,
        package,
        target,
        &census,
        native.declared(),
        choices,
    )?;
    let opaque_declarations = check_declaration_inventory(input, package, &providers.slots)?;
    if !checked.plans().assumed_bindings().is_empty() {
        return Err(unavailable("selected provider assumptions"));
    }
    let source = declaration_policy(
        input,
        target,
        package,
        services,
        input.public_traits(package)?,
    )?;
    providers.validate_service_inventory(&census, &source.callables, &source.public_api.traits)?;
    let native_bindings = native.finish(checked, target)?;
    let mut public_api = source.public_api;
    public_api.traits.extend(providers.public_traits);
    public_api
        .traits
        .sort_by(|left, right| left.identity.cmp(&right.identity));
    let mut callables = source.callables;
    callables.extend(providers.callables);
    callables.sort_by(|left, right| left.identity.cmp(&right.identity));
    let terminal_permissions =
        super::terminal_permissions::project(input, checked, target, package)?;
    let mut semantic_dependencies = providers.semantic_dependencies;
    semantic_dependencies.extend(input.carried_dependencies(package)?);
    semantic_dependencies.sort_unstable();
    semantic_dependencies.dedup();
    let mut policy = PackagePolicyBaseline {
        schema_version: review_evidence::encoding::PACKAGE_POLICY_BASELINE_VERSION,
        package,
        target,
        public_api,
        callables: PackagePolicyCallables {
            package,
            target,
            callables,
        },
        selected_providers: PackagePolicySelectedProviders {
            package,
            target,
            plans: providers.plans,
            families: Vec::new(),
        },
        terminal_permissions,
        representation: PackagePolicyRepresentation {
            package,
            target: source.representation,
            declarations: opaque_declarations,
            producer_availability: Vec::new(),
            selected_availability: Vec::new(),
            demands: Vec::new(),
        },
        native_bindings: native_bindings
            .into_iter()
            .map(|(binding, _)| binding)
            .collect(),
        external_supplies: Vec::new(),
        dangerous_capabilities: Vec::new(),
        slack_uses: Vec::new(),
        semantic_dependencies,
        boundary_applications: PackagePolicyBoundaryApplications {
            demands: Vec::new(),
            realizations: providers.applications,
        },
        assumed_bindings: Vec::new(),
        service_audits: Vec::new(),
        restricted_build_requests: source.restricted_build_requests,
        component_assumptions,
    };
    let classes = super::authority::critical_services(input, checked, &policy)?;
    policy.apply_service_classes(&classes);
    policy.apply_service_audit_policy(&source.audits);
    policy.validate_canonical_structure().map_err(unavailable)?;
    Ok(policy)
}

struct PolicyDeclarations {
    public_api: PackagePolicyPublicApi,
    callables: Vec<review_evidence::record::PackagePolicyCallable>,
    representation: PackageReviewRepresentationTarget,
    restricted_build_requests: Vec<PackagePolicyRestrictedBuildRequest>,
    audits: Vec<PackageReviewServiceIdentity>,
}

fn declaration_policy(
    declarations: &PolicySource<'_>,
    target: TargetProfile,
    package: PackageKeyIdentity,
    services: &[super::services::ReviewedService],
    ordinary_traits: Vec<review_evidence::record::PackagePolicyTraitShape>,
) -> Result<PolicyDeclarations, Vec<Diagnostic>> {
    // The owned declaration census above refuses unserved admission claims,
    // including unused private claims. The provider inventory's separate
    // root-grant census also contains unrelated imported toolchain claims;
    // it cannot substitute for this package's own review subject.
    let config = &declarations.build().config;
    if !declarations
        .typed()
        .opaque_representation_selections
        .is_empty()
        || declarations.declarations.has_assumed_bindings()
        || !config.default_provider_selections.is_empty()
        || !config.assumed_bindings.is_empty()
        || !config.opaque_representation_selections.is_empty()
        || !config.wire_compatibility_demands.is_empty()
        || !config.grants.is_empty()
        || !config.behavior_exclusions.is_empty()
    {
        return Err(unavailable(
            "selected provider, representation or authority contracts",
        ));
    }
    let module = declarations.accepted().module();
    if !module.interfaces.external_providers.is_empty()
        || !module.interfaces.provider_instance_requirements.is_empty()
        || !module.interfaces.boundary_application_candidates.is_empty()
        || !module.interfaces.boundary_application_demands.is_empty()
        || !module.interfaces.callback_applications.is_empty()
        || !module
            .interfaces
            .installed_interrupt_registrations
            .is_empty()
        || !module.types.opaque_representations.is_empty()
        || !module.execution.dynamic_dispatch.interfaces.is_empty()
        || !module.execution.dynamic_dispatch.tables.is_empty()
    {
        return Err(unavailable(
            "the accepted module's boundary and physical authority inventory",
        ));
    }
    // Register operations with no service coordinate still require an audit;
    // they cannot evade the nonempty service inventory check above.
    if module
        .machines
        .iter()
        .flat_map(|machine| machine.operations())
        .any(|operation| {
            matches!(
                operation.kind,
                terminal_psi::OperationKind::RegisterAccess { .. }
            )
        })
    {
        return Err(unavailable("register-operation authority"));
    }
    let public_api = PackagePolicyPublicApi {
        traits: ordinary_traits,
        conformances: declarations.public_conformances(package)?,
        domains: declarations.public_domains(package)?,
        propositions: Vec::new(),
        consts: declarations.public_consts(package)?,
        operators: Vec::new(),
        data: declarations.public_data(package)?,
        type_aliases: Some(declarations.public_type_aliases(package)?),
    };
    let callables = declarations.callables(package, services)?;
    let representation = representation_target(target)?;
    let restricted_build_requests = declarations
        .restricted_build_requests
        .iter()
        .map(PackagePolicyRestrictedBuildRequest::from)
        .collect();
    let audits = declarations
        .declarations
        .service_audits()
        .iter()
        .map(|symbol| declarations.unapplied_service(*symbol))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(PolicyDeclarations {
        public_api,
        callables,
        representation,
        restricted_build_requests,
        audits,
    })
}

fn check_declaration_inventory(
    declarations: &PolicySource<'_>,
    package: PackageKeyIdentity,
    reviewed_boundary_slots: &[symbols::SymbolHandle],
) -> Result<Vec<PackageReviewNominalIdentity>, Vec<Diagnostic>> {
    let typed = declarations.typed();
    let owned = |symbol| {
        for purpose in [
            source_provenance::DependencyScope::Product,
            source_provenance::DependencyScope::Build,
        ] {
            if source_compiler::representations::source_state::source_subjects::contains(
                typed,
                symbol,
                &[Some(package)],
                purpose,
            )
            .map_err(unavailable)?
            {
                return Ok(true);
            }
        }
        Ok::<_, Vec<Diagnostic>>(false)
    };
    let mut opaque_declarations = Vec::new();
    for declaration in typed.data_definitions() {
        if declaration.supply_mode == language_core::DataSupplyMode::BoundaryOpaque
            && owned(declaration.symbol)?
        {
            opaque_declarations.push(declarations.nominal(declaration.symbol)?);
        }
        if declaration.is_public
            && source_compiler::representations::source_state::source_subjects::contains(
                typed,
                declaration.symbol,
                &[Some(package)],
                source_provenance::DependencyScope::Build,
            )
            .map_err(unavailable)?
        {
            return Err(unavailable("Build-scope public data declarations"));
        }
    }
    for declaration in typed
        .domain_definitions()
        .iter()
        .filter(|row| row.is_public)
    {
        if source_compiler::representations::source_state::source_subjects::contains(
            typed,
            declaration.symbol,
            &[Some(package)],
            source_provenance::DependencyScope::Build,
        )
        .map_err(unavailable)?
        {
            return Err(unavailable("Build-scope public domain declarations"));
        }
    }
    for declaration in typed.traits() {
        if declaration.is_boundary
            && owned(declaration.symbol)?
            && reviewed_boundary_slots
                .binary_search_by_key(
                    &super::providers::symbol_key(declaration.symbol),
                    |symbol| super::providers::symbol_key(*symbol),
                )
                .is_err()
        {
            return Err(unavailable("trait and boundary declaration contracts"));
        }
    }
    for declaration in typed.propositions().iter().filter(|row| {
        row.is_public
            || matches!(
                row.body,
                source_compiler::representations::source_state::proposition::PropositionBody::Primitive
            )
    }) {
        if owned(declaration.symbol)? {
            return Err(unavailable("public proposition contracts"));
        }
    }
    for declaration in typed.mathematical_definitions().iter().filter(|row| {
        row.is_public
            || matches!(
                row.body,
                source_compiler::representations::source_state::mathematical::MathematicalBody::Assumption
            )
    }) {
        if owned(declaration.symbol)? {
            return Err(unavailable("public mathematical declaration contracts"));
        }
    }
    for declaration in typed.all_operators().filter(|row| row.is_public) {
        if owned(declaration.symbol)? {
            return Err(unavailable("public operator contracts"));
        }
    }
    for declaration in typed.wire_schemas().iter().filter(|row| row.is_public) {
        if owned(declaration.symbol)? {
            return Err(unavailable("public wire-schema contracts"));
        }
    }
    // A public representation producer discloses a carrier even when no Build
    // selects it. The opaque name alone cannot account for that availability.
    for conformance in typed.conformances() {
        if conformance.is_public
            && typed.core_vocabulary(conformance.trait_symbol)
                == Some(CoreVocabulary::OpaqueRepresentation)
            && owned(conformance.symbol)?
        {
            return Err(unavailable("opaque representation producer availability"));
        }
    }
    let selected_build = declarations.selected_build_symbol()?;
    for declaration in typed.machines() {
        if !owned(declaration.symbol)? {
            continue;
        }
        if Some(declaration.symbol) != selected_build
            && !declarations.owns(declaration.symbol, package)?
            && (declaration.is_public || !declaration.supplies_checked_body())
        {
            return Err(unavailable(format!(
                "the Build-scope public or bodyless callable contract of `{}`",
                declaration.symbol_spelling()
            )));
        }
    }
    opaque_declarations.sort();
    Ok(opaque_declarations)
}

pub(super) fn representation_target(
    target: TargetProfile,
) -> Result<PackageReviewRepresentationTarget, Vec<Diagnostic>> {
    use PackageReviewRepresentationTargetProfile as Profile;
    let profile = match target {
        TargetProfile::LinuxArm64 => Profile::LinuxArm64,
        TargetProfile::LinuxX64 => Profile::LinuxX64,
        TargetProfile::MacosArm64 => Profile::MacosArm64,
        TargetProfile::MacosX64 => Profile::MacosX64,
        TargetProfile::WindowsX64 => Profile::WindowsX64,
        TargetProfile::WindowsArm64 => Profile::WindowsArm64,
        TargetProfile::UefiX64 => Profile::UefiX64,
        TargetProfile::CrossPlatformCli => Profile::CrossPlatformCli,
        TargetProfile::LocalUnchecked => Profile::LocalUnchecked,
        TargetProfile::AlphaBootstrap => {
            return Err(unavailable("a native review target for Alpha"));
        }
    };
    let native = target.native_target();
    Ok(PackageReviewRepresentationTarget {
        profile,
        architecture: match native.architecture {
            target::Architecture::Aarch64 => PackageReviewRepresentationArchitecture::Aarch64,
            target::Architecture::X86_64 => PackageReviewRepresentationArchitecture::X86_64,
        },
        object_format: match native.object_format {
            target::ObjectFormat::Elf => PackageReviewRepresentationObjectFormat::Elf,
            target::ObjectFormat::MachO => PackageReviewRepresentationObjectFormat::MachO,
            target::ObjectFormat::Coff => PackageReviewRepresentationObjectFormat::Coff,
        },
        pointer_size: u16::try_from(native.pointer_size)
            .map_err(|_| unavailable("target pointer size"))?,
        pointer_alignment: u16::try_from(native.pointer_alignment)
            .map_err(|_| unavailable("target pointer alignment"))?,
    })
}
