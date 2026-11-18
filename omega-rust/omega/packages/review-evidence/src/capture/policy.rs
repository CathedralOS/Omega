//! Assemble a policy only after every inventory it claims is accounted for.
//! Empty rows below are justified by the accepted source/module inventories;
//! an unsupported nonempty inventory refuses before a baseline is returned.

use super::PackageReviewInput;
use super::declarations::unavailable;
use super::input::PolicySource;
use super::provider_rows::{self, ProviderChoice};
use crate::record::{
    PackagePolicyBaseline, PackagePolicyBoundaryApplications, PackagePolicyCallables,
    PackagePolicyPublicApi, PackagePolicyRepresentation, PackagePolicyRestrictedBuildRequest,
    PackagePolicySelectedProviders, PackagePolicyTerminalPermissions, PackageReviewNominalIdentity,
    PackageReviewRepresentationArchitecture, PackageReviewRepresentationObjectFormat,
    PackageReviewRepresentationTarget, PackageReviewRepresentationTargetProfile,
};
use diagnostics::Diagnostic;
use semantic_vocabulary::PackageKeyIdentity;
use source_compiler::evaluation::product_declarations::ServiceAuditRegistration;
use std::path::Path;
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
    let module = input.module()?;
    let choices = provider_rows::from_selected(checked.execution().plans().plans(), module);
    prepare(&input.source, target, package, choices.clone())?.finish(checked, || choices)
}

pub(super) struct PreparedPolicy {
    package: PackageKeyIdentity,
    target: TargetProfile,
    native: super::native_bindings::PreparedNativeBindings,
    providers: Result<super::providers::PreparedProviders, Vec<Diagnostic>>,
    inventory: Result<(), Vec<Diagnostic>>,
    declarations: Result<PolicyDeclarations, Vec<Diagnostic>>,
}

struct PolicyDeclarations {
    public_api: PackagePolicyPublicApi,
    callables: Vec<crate::record::PackagePolicyCallable>,
    representation: PackageReviewRepresentationTarget,
    restricted_build_requests: Vec<PackagePolicyRestrictedBuildRequest>,
    audits: Vec<PackageReviewNominalIdentity>,
}

pub(super) fn prepare(
    declarations: &PolicySource<'_>,
    target: TargetProfile,
    package: PackageKeyIdentity,
    choices: Result<Vec<ProviderChoice>, Vec<Diagnostic>>,
) -> Result<PreparedPolicy, Vec<Diagnostic>> {
    let accepted = declarations.accepted();
    let source = accepted.source();
    let origins = super::callables::service_origins(
        accepted,
        declaration_contracts::source_identity::DependencyScope::Product,
    )?;
    let services = super::callables::service_identities(
        source,
        declarations.sources().exact_toolchain_sources(),
        &origins,
    )?;
    let accepted_declarations = super::providers::AcceptedDeclarations::new(
        accepted,
        &origins,
        &services,
        choices
            .as_ref()
            .is_ok_and(|providers| !providers.is_empty()),
    );
    let native =
        super::native_bindings::prepare(declarations, package, target, &accepted_declarations)?;
    let providers = choices.and_then(|choices| {
        declarations.prepare_providers(
            package,
            target,
            &accepted_declarations,
            native.declared(),
            choices,
        )
    });
    let inventory = providers
        .as_ref()
        .map_err(Clone::clone)
        .and_then(|providers| {
            require_supported_inventory(declarations, package, &providers.review.slots)
        });
    let source_declarations = inventory
        .as_ref()
        .map_err(Clone::clone)
        .and_then(|()| declaration_policy(declarations, target, package, &services));
    Ok(PreparedPolicy {
        package,
        target,
        native,
        providers,
        inventory,
        declarations: source_declarations,
    })
}

fn declaration_policy(
    declarations: &PolicySource<'_>,
    target: TargetProfile,
    package: PackageKeyIdentity,
    services: &[(semantic_vocabulary::ServiceId, PackageReviewNominalIdentity)],
) -> Result<PolicyDeclarations, Vec<Diagnostic>> {
    // The owned declaration census above refuses unserved admission claims,
    // including unused private claims. The provider inventory's separate
    // root-grant census also contains unrelated imported toolchain claims;
    // it cannot substitute for this package's own review subject.
    let config = &declarations.build().config;
    if !config.default_provider_selections.is_empty()
        || !config.assumed_bindings.is_empty()
        || !config.opaque_representation_selections.is_empty()
        || !config.wire_compatibility_demands.is_empty()
        || !config.grants.is_empty()
        || !config.behavior_exclusions.is_empty()
        || declarations
            .sources()
            .accepted_semantic_bindings()
            .iter()
            .any(|binding| !binding.terminal_authority_permissions().is_empty())
    {
        return Err(unavailable(
            "selected provider, representation or authority contracts",
        ));
    }
    let module = declarations.accepted().module();
    if !module.external_providers.is_empty()
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
    let callables = declarations.callables(package, services)?;
    let representation = representation_target(target)?;
    let restricted_build_requests = declarations
        .admission
        .restricted_build_requests
        .iter()
        .map(PackagePolicyRestrictedBuildRequest::from)
        .collect();
    let audits = service_audits(declarations, &config.service_audits)?;
    Ok(PolicyDeclarations {
        public_api,
        callables,
        representation,
        restricted_build_requests,
        audits,
    })
}

impl PreparedPolicy {
    pub(super) fn finish(
        &self,
        checked: &products::CheckedCompilation,
        choices: impl FnOnce() -> Result<Vec<ProviderChoice>, Vec<Diagnostic>>,
    ) -> Result<PackagePolicyBaseline, Vec<Diagnostic>> {
        let package = self.package;
        let target = self.target;
        if checked.sources().package_identity() != Some(package)
            || checked.execution().target_profile() != Some(target)
            || checked.execution().native_target() != Some(target.native_target())
        {
            return Err(unavailable(
                "a policy for a different package or target activation",
            ));
        }
        let native_bindings = self.native.finish(checked, target)?;
        let providers =
            self.providers
                .as_ref()
                .map_err(Clone::clone)?
                .finish(checked, target, choices()?)?;
        self.inventory.as_ref().map_err(Clone::clone)?;
        if !checked.execution().plans().assumed_bindings().is_empty() {
            return Err(unavailable(
                "selected provider, representation or authority contracts",
            ));
        }
        let source = self.declarations.as_ref().map_err(Clone::clone)?;
        let mut public_api = source.public_api.clone();
        public_api.traits = providers.public_traits;
        let mut callables = source.callables.clone();
        callables.extend(providers.callables);
        callables.sort_by(|left, right| left.identity.cmp(&right.identity));
        let mut policy = PackagePolicyBaseline {
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
            terminal_permissions: PackagePolicyTerminalPermissions {
                package,
                target,
                services: Vec::new(),
            },
            representation: PackagePolicyRepresentation {
                package,
                target: source.representation.clone(),
                declarations: Vec::new(),
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
            semantic_dependencies: providers.semantic_dependencies,
            boundary_applications: PackagePolicyBoundaryApplications {
                demands: Vec::new(),
                realizations: providers.applications,
            },
            assumed_bindings: Vec::new(),
            service_audits: Vec::new(),
            restricted_build_requests: source.restricted_build_requests.clone(),
            component_assumptions: Vec::new(),
        };
        policy.apply_service_audit_policy(&source.audits);
        policy.validate_canonical_structure().map_err(unavailable)?;
        Ok(policy)
    }
}

fn service_audits(
    declarations: &PolicySource<'_>,
    registrations: &[ServiceAuditRegistration],
) -> Result<Vec<PackageReviewNominalIdentity>, Vec<Diagnostic>> {
    if registrations.is_empty() {
        return Ok(Vec::new());
    }

    // An audit may name a boundary with no methods or realized reach. Index
    // the admitted source declarations and let rejoin check exact custody
    // within each matching source coordinate.
    let typed = declarations.typed();
    let mut boundaries = Vec::new();
    for declaration in typed
        .traits()
        .iter()
        .filter(|declaration| declaration.is_boundary)
    {
        let symbol = declaration.symbol;
        if !typed.symbols.symbols().nodes().is_valid(symbol) {
            return Err(unavailable(
                "a stale boundary declaration in the audit inventory",
            ));
        }
        if let Some(span) = typed.symbols.symbol_source_span(symbol)
            && let Some(source) = typed.symbols.source_file(span)
        {
            boundaries.push((declaration_coordinate(source, span.span), symbol));
        }
    }
    boundaries.sort_unstable_by_key(|&(coordinate, _)| coordinate);

    registrations
        .iter()
        .map(|registration| {
            let declaration = &registration.declaration;
            let coordinate = declaration_coordinate(declaration.source(), declaration.span());
            let first = boundaries.partition_point(|&(candidate, _)| candidate < coordinate);
            let end = boundaries.partition_point(|&(candidate, _)| candidate <= coordinate);
            let symbol = source_compiler::source_symbols::rejoin_declaration(
                declaration,
                &typed.symbols,
                boundaries[first..end].iter().map(|&(_, symbol)| symbol),
            )
            .map_err(unavailable)?;
            declarations.nominal(symbol)
        })
        .collect()
}

fn declaration_coordinate(
    source: &source_admission::SourceFile,
    span: source_text::Span,
) -> (
    &Path,
    &Path,
    declaration_contracts::source_identity::DependencyScope,
    usize,
    usize,
) {
    (
        &source.path,
        &source.package_root,
        source.dependency_scope,
        span.start,
        span.end,
    )
}

fn require_supported_inventory(
    declarations: &PolicySource<'_>,
    package: PackageKeyIdentity,
    reviewed_boundary_slots: &[symbols::SymbolHandle],
) -> Result<(), Vec<Diagnostic>> {
    let typed = declarations.typed();
    let owned = |symbol| {
        for purpose in [
            declaration_contracts::source_identity::DependencyScope::Product,
            declaration_contracts::source_identity::DependencyScope::Build,
        ] {
            if source_compiler::source_state::source_subjects::contains(
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
    for declaration in typed.data_definitions() {
        if declaration.supply_mode != declaration_contracts::DataSupplyMode::CheckedShape
            && owned(declaration.symbol)?
        {
            return Err(unavailable(
                "opaque declaration and representation contracts",
            ));
        }
        if declaration.is_public
            && source_compiler::source_state::source_subjects::contains(
                typed,
                declaration.symbol,
                &[Some(package)],
                declaration_contracts::source_identity::DependencyScope::Build,
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
        if source_compiler::source_state::source_subjects::contains(
            typed,
            declaration.symbol,
            &[Some(package)],
            declaration_contracts::source_identity::DependencyScope::Build,
        )
        .map_err(unavailable)?
        {
            return Err(unavailable("Build-scope public domain declarations"));
        }
    }
    for declaration in typed.traits() {
        if (declaration.is_public || declaration.is_boundary)
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
    for declaration in typed.conformances().iter().filter(|row| row.is_public) {
        if owned(declaration.symbol)? {
            return Err(unavailable("public conformance contracts"));
        }
    }
    for declaration in typed.propositions().iter().filter(|row| {
        row.is_public
            || matches!(
                row.body,
                source_compiler::source_state::proposition::PropositionBody::Primitive
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
                source_compiler::source_state::mathematical::MathematicalBody::Assumption
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
        let selected_build = declarations.build().selected_build_machine_symbol;
        if Some(declaration.symbol) != selected_build
            && !declarations.owns(declaration.symbol, package)?
            && (declaration.is_public || !declaration.supplies_checked_body())
        {
            return Err(unavailable(&format!(
                "the Build-scope public or bodyless callable contract of `{}`",
                declaration.symbol_spelling()
            )));
        }
    }
    Ok(())
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
