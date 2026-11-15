//! Assemble a policy only after every inventory it claims is accounted for.
//! Empty rows below are justified by the accepted source/module inventories;
//! an unsupported nonempty inventory refuses before a baseline is returned.

use super::PackageReviewInput;
use super::declarations::unavailable;
use crate::record::{
    PackagePolicyBaseline, PackagePolicyBoundaryApplications, PackagePolicyCallables,
    PackagePolicyPublicApi, PackagePolicyRepresentation, PackagePolicyRestrictedBuildRequest,
    PackagePolicySelectedProviders, PackagePolicyTerminalPermissions,
    PackageReviewRepresentationArchitecture, PackageReviewRepresentationObjectFormat,
    PackageReviewRepresentationTarget, PackageReviewRepresentationTargetProfile,
};
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
use target::TargetProfile;

pub(super) fn project(
    input: PackageReviewInput<'_>,
    target: TargetProfile,
    package: PackageKeyIdentity,
) -> Result<PackagePolicyBaseline, Vec<Diagnostic>> {
    let checked = input.compilation();
    if checked.sources().package_identity() != Some(package)
        || checked.execution().target_profile() != Some(target)
        || checked.execution().native_target() != Some(target.native_target())
    {
        return Err(unavailable(
            "a policy for a different package or target activation",
        ));
    }
    let declarations = input;
    require_supported_inventory(&declarations, package)?;
    // The owned declaration census above refuses unserved admission claims,
    // including unused private claims. The provider inventory's separate
    // root-grant census also contains unrelated imported toolchain claims;
    // it cannot substitute for this package's own review subject.
    let config = &checked.build().config;
    if !checked.execution().plans().plans().is_empty()
        || !checked.execution().plans().assumed_bindings().is_empty()
        || !config.provider_selections.is_empty()
        || !config.default_provider_selections.is_empty()
        || !config.assumed_bindings.is_empty()
        || !config.native_bindings.is_empty()
        || !config.opaque_representation_selections.is_empty()
        || !config.wire_compatibility_demands.is_empty()
        || !config.grants.is_empty()
        || !config.service_audits.is_empty()
        || !config.behavior_exclusions.is_empty()
        || checked
            .sources()
            .accepted_semantic_bindings()
            .iter()
            .any(|binding| !binding.terminal_authority_permissions().is_empty())
    {
        return Err(unavailable(
            "selected provider, representation or authority contracts",
        ));
    }
    let module = input.module()?;
    if !module.services.is_empty()
        || !module.boundary_machines.is_empty()
        || !module.external_providers.is_empty()
        || !module.provider_candidates.is_empty()
        || !module.provider_instance_requirements.is_empty()
        || !module.boundary_application_candidates.is_empty()
        || !module.boundary_application_demands.is_empty()
        || !module.callback_applications.is_empty()
        || !module.installed_interrupt_registrations.is_empty()
        || !module.opaque_representations.is_empty()
        || !module.dynamic_dispatch.interfaces.is_empty()
        || !module.dynamic_dispatch.tables.is_empty()
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
        traits: Vec::new(),
        conformances: Vec::new(),
        domains: declarations.public_domains(package)?,
        propositions: Vec::new(),
        consts: Vec::new(),
        operators: Vec::new(),
        data: declarations.public_data(package)?,
    };
    let policy = PackagePolicyBaseline {
        package,
        target,
        public_api,
        callables: PackagePolicyCallables {
            package,
            target,
            callables: declarations.callables(package)?,
        },
        selected_providers: PackagePolicySelectedProviders {
            package,
            target,
            plans: Vec::new(),
            families: Vec::new(),
        },
        terminal_permissions: PackagePolicyTerminalPermissions {
            package,
            target,
            services: Vec::new(),
        },
        representation: PackagePolicyRepresentation {
            package,
            target: representation_target(target)?,
            declarations: Vec::new(),
            producer_availability: Vec::new(),
            selected_availability: Vec::new(),
            demands: Vec::new(),
        },
        native_bindings: Vec::new(),
        external_supplies: Vec::new(),
        dangerous_capabilities: Vec::new(),
        slack_uses: Vec::new(),
        semantic_dependencies: Vec::new(),
        boundary_applications: PackagePolicyBoundaryApplications {
            demands: Vec::new(),
            realizations: Vec::new(),
        },
        assumed_bindings: Vec::new(),
        service_audits: Vec::new(),
        restricted_build_requests: checked
            .build_admission()
            .restricted_build_requests
            .iter()
            .map(PackagePolicyRestrictedBuildRequest::from)
            .collect(),
        component_assumptions: Vec::new(),
    };
    policy.validate_canonical_structure().map_err(unavailable)?;
    Ok(policy)
}

fn require_supported_inventory(
    declarations: &PackageReviewInput<'_>,
    package: PackageKeyIdentity,
) -> Result<(), Vec<Diagnostic>> {
    let typed = declarations.typed();
    let owned = |symbol| {
        for purpose in [
            source::DependencyScope::Product,
            source::DependencyScope::Build,
        ] {
            if typed_trees::source_subjects::contains(typed, symbol, &[Some(package)], purpose)
                .map_err(unavailable)?
            {
                return Ok(true);
            }
        }
        Ok::<_, Vec<Diagnostic>>(false)
    };
    for declaration in typed.data_definitions() {
        if declaration.supply_mode != language_semantics::DataSupplyMode::CheckedShape
            && owned(declaration.symbol)?
        {
            return Err(unavailable(
                "opaque declaration and representation contracts",
            ));
        }
        if declaration.is_public
            && typed_trees::source_subjects::contains(
                typed,
                declaration.symbol,
                &[Some(package)],
                source::DependencyScope::Build,
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
        if typed_trees::source_subjects::contains(
            typed,
            declaration.symbol,
            &[Some(package)],
            source::DependencyScope::Build,
        )
        .map_err(unavailable)?
        {
            return Err(unavailable("Build-scope public domain declarations"));
        }
    }
    for declaration in typed.traits() {
        if owned(declaration.symbol)? && (declaration.is_public || declaration.is_boundary) {
            return Err(unavailable("trait and boundary declaration contracts"));
        }
    }
    for declaration in typed.conformances().iter().filter(|row| row.is_public) {
        if owned(declaration.symbol)? {
            return Err(unavailable("public conformance contracts"));
        }
    }
    for declaration in typed.propositions().iter().filter(|row| {
        row.is_public
            || matches!(
                row.body,
                typed_trees::proposition::PropositionBody::Primitive
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
                typed_trees::mathematical::MathematicalBody::Assumption
            )
    }) {
        if owned(declaration.symbol)? {
            return Err(unavailable("public mathematical declaration contracts"));
        }
    }
    for declaration in typed
        .const_declarations()
        .iter()
        .filter(|row| row.is_public)
    {
        if owned(declaration.symbol)? {
            return Err(unavailable("public constant contracts"));
        }
    }
    for declaration in typed.all_operators().filter(|row| row.is_public) {
        if owned(declaration.symbol)? {
            return Err(unavailable("public operator contracts"));
        }
    }
    for declaration in typed.type_aliases().iter().filter(|row| row.is_public) {
        if owned(declaration.symbol)? {
            return Err(unavailable("public type-alias contracts"));
        }
    }
    for declaration in typed.wire_schemas().iter().filter(|row| row.is_public) {
        if owned(declaration.symbol)? {
            return Err(unavailable("public wire-schema contracts"));
        }
    }
    for declaration in typed.machines() {
        if !owned(declaration.symbol)? {
            continue;
        }
        let selected_build = declarations
            .compilation()
            .build()
            .selected_build_machine_symbol;
        if Some(declaration.symbol) != selected_build
            && !declarations.owns(declaration.symbol, package)?
            && (declaration.is_public || !declaration.supplies_checked_body())
        {
            return Err(unavailable(
                "Build-scope public or bodyless callable contracts",
            ));
        }
    }
    Ok(())
}

fn representation_target(
    target: TargetProfile,
) -> Result<PackageReviewRepresentationTarget, Vec<Diagnostic>> {
    use PackageReviewRepresentationTargetProfile as Profile;
    let profile = match target {
        TargetProfile::LinuxArm64 => Profile::LinuxArm64,
        TargetProfile::LinuxX64 => Profile::LinuxX64,
        TargetProfile::MacosArm64 => Profile::MacosArm64,
        TargetProfile::MacosX64 => Profile::MacosX64,
        TargetProfile::WindowsX64 => Profile::WindowsX64,
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
