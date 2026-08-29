use super::exact_identity::*;
use crate::model::*;
use omega_compiler::CheckedCompilation;
use psi_core::PackageKeyIdentity;
use psi_diagnostics::Diagnostic;
use psi_symbols::SymbolHandle;
use std::collections::BTreeSet;

pub(crate) fn validate_selected_provider_declaration_owner(
    declaration: &PackageReviewNominalIdentity,
    expected_package: Option<PackageKeyIdentity>,
    plan_name: &str,
    role: &str,
) -> Result<(), Vec<Diagnostic>> {
    let matches = match (expected_package, declaration.owner) {
        (Some(expected), PackageReviewNominalOwner::Package(actual)) => expected == actual,
        (None, PackageReviewNominalOwner::ToolchainSource(_)) => true,
        (Some(_), PackageReviewNominalOwner::ToolchainSource(_))
        | (None, PackageReviewNominalOwner::Package(_))
        | (_, PackageReviewNominalOwner::Unresolved) => false,
    };
    if matches {
        Ok(())
    } else {
        Err(vec![Diagnostic::error(format!(
            "selected provider plan `{plan_name}` {role} `{}` disagrees with its exact package/toolchain ownership",
            declaration.path,
        ))])
    }
}

pub(crate) fn project_semantic_dependencies(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
) -> Result<Vec<ProjectedSemanticDependencyRow>, Vec<Diagnostic>> {
    let derived = psi_typed_trees_to_checked_trees::derive_checked_semantic_dependencies(
        &compilation.typed,
        &compilation.facts,
    );
    if derived != compilation.facts.flow.semantic_dependencies {
        return Err(vec![Diagnostic::error(format!(
            "retained checked semantic-dependency evidence does not equal compiler rederivation (retained {} rows, derived {} rows)",
            compilation.facts.flow.semantic_dependencies.rows.len(),
            derived.rows.len(),
        ))]);
    }

    let mut projected: Vec<ProjectedSemanticDependencyRow> = Vec::new();
    for checked in &compilation.facts.flow.semantic_dependencies.rows {
        let consumer = nominal_identity(compilation, checked.consumer_machine)?;
        if !reviewed_package_owns(&consumer, package)? {
            continue;
        }
        let row = PackageReviewSemanticDependency {
            consumer,
            dependency: nominal_identity(compilation, checked.dependency)?,
            exposure: match checked.exposure {
                psi_checked_trees::CheckedSemanticDependencyExposure::PrivateImplementation => {
                    PackageReviewSemanticDependencyExposure::PrivateImplementation
                }
                psi_checked_trees::CheckedSemanticDependencyExposure::PublicInterface => {
                    PackageReviewSemanticDependencyExposure::PublicInterface
                }
            },
            kind: match checked.kind {
                psi_checked_trees::CheckedSemanticDependencyKind::NominalIdentity => {
                    PackageReviewSemanticDependencyKind::NominalIdentity
                }
                psi_checked_trees::CheckedSemanticDependencyKind::Layout => {
                    PackageReviewSemanticDependencyKind::Layout
                }
                psi_checked_trees::CheckedSemanticDependencyKind::OwnershipBehavior => {
                    PackageReviewSemanticDependencyKind::OwnershipBehavior
                }
                psi_checked_trees::CheckedSemanticDependencyKind::AutomaticCleanup => {
                    PackageReviewSemanticDependencyKind::AutomaticCleanup
                }
                psi_checked_trees::CheckedSemanticDependencyKind::AutomaticCleanupMachine => {
                    PackageReviewSemanticDependencyKind::AutomaticCleanupMachine
                }
            },
        };
        if let Some(existing) = projected.iter_mut().find(|existing| existing.row == row) {
            if !existing
                .consumer_declarations
                .contains(&checked.consumer_machine)
            {
                existing
                    .consumer_declarations
                    .push(checked.consumer_machine);
            }
            if !existing
                .dependency_declarations
                .contains(&checked.dependency)
            {
                existing.dependency_declarations.push(checked.dependency);
            }
        } else {
            projected.push(ProjectedSemanticDependencyRow {
                row,
                consumer_declarations: vec![checked.consumer_machine],
                dependency_declarations: vec![checked.dependency],
            });
        }
    }
    projected.sort_by(|left, right| left.row.cmp(&right.row));
    for row in &mut projected {
        row.consumer_declarations
            .sort_by_key(|symbol| symbol.arena_index());
        row.dependency_declarations
            .sort_by_key(|symbol| symbol.arena_index());
    }
    Ok(projected)
}

pub(crate) fn project_representation_tcb(
    compilation: &CheckedCompilation,
    package: PackageKeyIdentity,
) -> Result<Vec<ProjectedReviewRow<PackageReviewRepresentationTcb>>, Vec<Diagnostic>> {
    let mut rows = Vec::new();
    for definition in compilation.data_definitions().iter().filter(|definition| {
        definition.supply_mode == psi_language_semantics::DataSupplyMode::BoundaryOpaque
    }) {
        let declaration = nominal_identity(compilation, definition.symbol)?;
        if !reviewed_package_owns(&declaration, package)? {
            continue;
        }
        rows.push(ProjectedReviewRow {
            row: PackageReviewRepresentationTcb {
                declaration,
                abi: PackageReviewRepresentationAbiCommitment::Unbound,
                mechanism: PackageReviewRepresentationMechanism::Unbound,
            },
            declaration: definition.symbol,
            nested_source_locations: Vec::new(),
        });
    }
    rows.sort_by(|left, right| left.row.cmp(&right.row));
    rows.dedup_by(|left, right| left.row == right.row && left.declaration == right.declaration);
    Ok(rows)
}

pub(crate) fn project_dangerous_authorities(
    compilation: &CheckedCompilation,
    callables: &[ProjectedReviewRow<CheckedPackageCallableReview>],
) -> Result<Vec<ProjectedDangerousAuthorityRow>, Vec<Diagnostic>> {
    let mut exposed_services = BTreeSet::new();
    for callable in callables.iter().map(|projected| &projected.row) {
        if let Some(services) = callable.declared_service_reach() {
            exposed_services.extend(services.iter().cloned());
        }
        if let Some(services) = callable.checked_service_reach().realized() {
            exposed_services.extend(services.iter().cloned());
        }
        if let Some(services) = callable.checked_service_reach().concrete() {
            exposed_services.extend(services.iter().cloned());
        }
        for reach in callable.unresolved_installation_reaches() {
            exposed_services.extend(reach.upper_bound().iter().cloned());
        }
        if let Some(invocations) = callable.declared_synchronous_invocations() {
            exposed_services.extend(
                invocations
                    .iter()
                    .filter_map(PackageReviewSynchronousInvocation::service)
                    .cloned(),
            );
        }
        exposed_services.extend(
            callable
                .realized_synchronous_invocations()
                .iter()
                .filter_map(PackageReviewSynchronousInvocation::service)
                .cloned(),
        );
    }

    let mut rows = Vec::new();
    for definition in compilation.facts.service_reaches.services.definitions() {
        let service = nominal_identity(compilation, definition.symbol)?;
        if !exposed_services.contains(&service) {
            continue;
        }
        let Some(class) = dangerous_authority_class(compilation, definition) else {
            continue;
        };
        let exposures = callables
            .iter()
            .filter(|callable| callable_exposes_service(&callable.row, &service))
            .map(|callable| callable.declaration)
            .collect();
        rows.push(ProjectedDangerousAuthorityRow {
            row: PackageReviewDangerousAuthority { class, service },
            declaration: definition.symbol,
            exposures,
        });
    }
    rows.sort_by(|left, right| left.row.cmp(&right.row));
    rows.dedup_by(|left, right| {
        left.row == right.row
            && left.declaration == right.declaration
            && left.exposures == right.exposures
    });
    Ok(rows)
}

pub(crate) fn project_dangerous_authority_slack(
    compilation: &CheckedCompilation,
    callables: &[ProjectedReviewRow<CheckedPackageCallableReview>],
) -> Result<Vec<ProjectedDangerousAuthoritySlackRow>, Vec<Diagnostic>> {
    let mut catalog = Vec::new();
    for definition in compilation.facts.service_reaches.services.definitions() {
        let Some(class) = dangerous_authority_class(compilation, definition) else {
            continue;
        };
        catalog.push((
            nominal_identity(compilation, definition.symbol)?,
            class,
            definition.symbol,
        ));
    }
    catalog.sort_by(|left, right| left.0.cmp(&right.0));

    let mut rows = Vec::new();
    for callable in callables {
        let Some(realized) = callable.row.checked_service_reach().realized() else {
            continue;
        };
        let Some(declared) = callable.row.declared_service_reach() else {
            continue;
        };
        for service in declared {
            if realized.contains(service) {
                continue;
            }
            let Ok(index) = catalog.binary_search_by(|entry| entry.0.cmp(service)) else {
                continue;
            };
            let (_, class, authority_declaration) = &catalog[index];
            rows.push(ProjectedDangerousAuthoritySlackRow {
                row: PackageReviewDangerousAuthoritySlack {
                    class: *class,
                    callable: callable.row.identity.clone(),
                    service: service.clone(),
                },
                authority_declaration: *authority_declaration,
                callable_declaration: callable.declaration,
            });
        }
    }
    rows.sort_by(|left, right| left.row.cmp(&right.row));
    rows.dedup_by(|left, right| {
        left.row == right.row
            && left.authority_declaration == right.authority_declaration
            && left.callable_declaration == right.callable_declaration
    });
    Ok(rows)
}

pub(crate) fn finalize_projected_rows<Row>(
    compilation: &CheckedCompilation,
    projected: Vec<ProjectedReviewRow<Row>>,
    role: PackageReviewSourceLocationRole,
) -> Result<(Vec<Row>, Vec<PackageReviewCanonicalRowSource>), Vec<Diagnostic>> {
    let mut rows = Vec::with_capacity(projected.len());
    let mut sources = Vec::with_capacity(projected.len());
    for projected in projected {
        let mut locations = vec![canonical_source_location(
            compilation,
            projected.declaration,
            role,
        )?];
        for nested in projected.nested_source_locations {
            locations.push(canonical_source_span_location(
                compilation,
                nested.source_span,
                nested.role,
            )?);
        }
        locations.sort();
        locations.dedup();
        sources.push(PackageReviewCanonicalRowSource::authored(locations));
        rows.push(projected.row);
    }
    Ok((rows, sources))
}

pub(crate) fn finalize_semantic_dependency_rows(
    compilation: &CheckedCompilation,
    projected: Vec<ProjectedSemanticDependencyRow>,
) -> Result<
    (
        Vec<PackageReviewSemanticDependency>,
        Vec<PackageReviewCanonicalRowSource>,
    ),
    Vec<Diagnostic>,
> {
    let mut rows = Vec::with_capacity(projected.len());
    let mut sources = Vec::with_capacity(projected.len());
    for projected in projected {
        let mut locations = Vec::new();
        for declaration in projected.consumer_declarations {
            locations.push(canonical_source_location(
                compilation,
                declaration,
                PackageReviewSourceLocationRole::SemanticDependencyConsumer,
            )?);
        }
        for declaration in projected.dependency_declarations {
            locations.push(canonical_source_location(
                compilation,
                declaration,
                PackageReviewSourceLocationRole::SemanticDependencyDeclaration,
            )?);
        }
        locations.sort();
        locations.dedup();
        sources.push(PackageReviewCanonicalRowSource::authored(locations));
        rows.push(projected.row);
    }
    Ok((rows, sources))
}

pub(crate) fn finalize_dangerous_authority_rows(
    compilation: &CheckedCompilation,
    projected: Vec<ProjectedDangerousAuthorityRow>,
) -> Result<
    (
        Vec<PackageReviewDangerousAuthority>,
        Vec<PackageReviewCanonicalRowSource>,
    ),
    Vec<Diagnostic>,
> {
    let mut rows = Vec::with_capacity(projected.len());
    let mut sources = Vec::with_capacity(projected.len());
    for projected in projected {
        let mut locations = vec![canonical_source_location(
            compilation,
            projected.declaration,
            PackageReviewSourceLocationRole::AuthorityDeclaration,
        )?];
        for exposure in projected.exposures {
            locations.push(canonical_source_location(
                compilation,
                exposure,
                PackageReviewSourceLocationRole::AuthorityExposure,
            )?);
        }
        locations.sort();
        locations.dedup();
        sources.push(PackageReviewCanonicalRowSource::authored(locations));
        rows.push(projected.row);
    }
    Ok((rows, sources))
}

pub(crate) fn finalize_dangerous_authority_slack_rows(
    compilation: &CheckedCompilation,
    projected: Vec<ProjectedDangerousAuthoritySlackRow>,
) -> Result<
    (
        Vec<PackageReviewDangerousAuthoritySlack>,
        Vec<PackageReviewCanonicalRowSource>,
    ),
    Vec<Diagnostic>,
> {
    let mut rows = Vec::with_capacity(projected.len());
    let mut sources = Vec::with_capacity(projected.len());
    for projected in projected {
        let mut locations = vec![
            canonical_source_location(
                compilation,
                projected.authority_declaration,
                PackageReviewSourceLocationRole::AuthorityDeclaration,
            )?,
            canonical_source_location(
                compilation,
                projected.callable_declaration,
                PackageReviewSourceLocationRole::AuthorityExposure,
            )?,
        ];
        locations.sort();
        locations.dedup();
        sources.push(PackageReviewCanonicalRowSource::authored(locations));
        rows.push(projected.row);
    }
    Ok((rows, sources))
}

pub(crate) fn selected_provider_row_source(
    compilation: &CheckedCompilation,
    selected_providers: &[CheckedPackageProviderReview],
) -> Result<PackageReviewCanonicalRowSource, Vec<Diagnostic>> {
    let selected_plans = compilation.selected_provider_plans().plans();
    let provenance = compilation.selected_provider_provenance();
    if selected_plans.len() != selected_providers.len() || selected_plans.len() != provenance.len()
    {
        return Err(vec![Diagnostic::error(
            "selected-provider review provenance is not aligned with the canonical selected plan set",
        )]);
    }
    if selected_plans.is_empty() {
        return Ok(PackageReviewCanonicalRowSource::compiler_derived(
            PackageReviewSyntheticSourceKind::EmptySelectedProviderSet,
        ));
    }

    let mut locations = Vec::new();
    let mut compiler_derivations = Vec::new();
    for (index, plan) in selected_plans.iter().enumerate() {
        let retained = &provenance[index];
        if retained.plan != *plan {
            return Err(vec![Diagnostic::error(format!(
                "selected provider plan `{}` is not aligned with its retained provenance",
                plan.name,
            ))]);
        }

        match &retained.selected_by {
            omega_provider_planning::plans::ProviderSelectionProvenance::BuildOverride(declarations)
            | omega_provider_planning::plans::ProviderSelectionProvenance::TargetDefault(declarations) => {
                for declaration in declarations {
                    locations.push(canonical_source_span_location(
                        compilation,
                        declaration.source_span,
                        PackageReviewSourceLocationRole::ProviderSelection,
                    )?);
                }
            }
            omega_provider_planning::plans::ProviderSelectionProvenance::UniqueCoveringCandidate => {
                compiler_derivations
                    .push(PackageReviewSyntheticSourceKind::UniqueCoveringProviderSelection);
            }
        }

        locations.push(canonical_source_location(
            compilation,
            retained.provider.schema.symbol(),
            PackageReviewSourceLocationRole::ProviderSchemaDeclaration,
        )?);

        if let Some(provider_type) = retained.provider.provider_type {
            locations.push(canonical_source_location(
                compilation,
                provider_type,
                PackageReviewSourceLocationRole::ProviderTypeDeclaration,
            )?);
        } else {
            compiler_derivations.push(PackageReviewSyntheticSourceKind::FreeExternalProviderType);
        }

        for requirement in &retained.provider.row_requirements {
            locations.push(canonical_source_location(
                compilation,
                *requirement,
                PackageReviewSourceLocationRole::ProviderRequirementDeclaration,
            )?);
        }

        for realization in &retained.provider.row_realizations {
            locations.push(canonical_source_location(
                compilation,
                *realization,
                PackageReviewSourceLocationRole::ProviderRealization,
            )?);
        }
    }
    locations.sort();
    locations.dedup();
    compiler_derivations.sort();
    compiler_derivations.dedup();
    Ok(PackageReviewCanonicalRowSource::mixed(
        locations,
        compiler_derivations,
    ))
}

pub(crate) const MAX_PACKAGE_REVIEW_SOURCE_LOCATIONS: usize = 262_144;
pub(crate) const MAX_PACKAGE_REVIEW_SOURCE_LOCATION_PATH_BYTES: usize = 16 * 1024 * 1024;

pub(crate) fn validate_canonical_row_source_limits(
    sources: &PackageReviewCanonicalRowSources,
) -> Result<(), Vec<Diagnostic>> {
    let all = sources
        .public_traits
        .iter()
        .chain(&sources.public_conformances)
        .chain(&sources.public_domains)
        .chain(&sources.public_propositions)
        .chain(&sources.public_consts)
        .chain(&sources.public_operators)
        .chain(&sources.public_data)
        .chain(&sources.representation_tcb)
        .chain(&sources.semantic_dependencies)
        .chain(&sources.callables)
        .chain(&sources.external_executable_supply)
        .chain(&sources.dangerous_authorities)
        .chain(&sources.dangerous_authority_slack)
        .chain(std::iter::once(&sources.selected_provider_set));
    let mut count = 0usize;
    let mut path_bytes = 0usize;
    for source in all {
        let locations = &source.authored_locations;
        let derivations = &source.compiler_derivations;
        if locations.is_empty() && derivations.is_empty() {
            return Err(vec![Diagnostic::error(
                "package review row has neither authored source locations nor a compiler derivation",
            )]);
        }
        if locations.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(vec![Diagnostic::error(
                "authored package review source locations are not strictly canonical",
            )]);
        }
        if derivations.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(vec![Diagnostic::error(
                "package review compiler derivations are not strictly canonical",
            )]);
        }
        count = count.checked_add(locations.len()).ok_or_else(|| {
            vec![Diagnostic::error(
                "package review source-location count overflow",
            )]
        })?;
        for location in locations {
            path_bytes = path_bytes
                .checked_add(location.relative_path.len())
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "package review source-location path-byte count overflow",
                    )]
                })?;
        }
    }
    if count > MAX_PACKAGE_REVIEW_SOURCE_LOCATIONS {
        return Err(vec![Diagnostic::error(
            "package review exceeds the source-location count ceiling",
        )]);
    }
    if path_bytes > MAX_PACKAGE_REVIEW_SOURCE_LOCATION_PATH_BYTES {
        return Err(vec![Diagnostic::error(
            "package review exceeds the source-location path-byte ceiling",
        )]);
    }
    Ok(())
}

pub(crate) fn canonical_source_location(
    compilation: &CheckedCompilation,
    symbol: SymbolHandle,
    mut role: PackageReviewSourceLocationRole,
) -> Result<PackageReviewSourceLocation, Vec<Diagnostic>> {
    if compilation
        .typed
        .symbols
        .symbol_source_span(symbol)
        .is_none()
    {
        role = PackageReviewSourceLocationRole::DerivationOrigin;
    }
    let span = compilation
        .typed
        .symbols
        .symbol_provenance_source_span(symbol)
        .ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "reviewed declaration `{}` has no authored source span",
                compilation.typed.symbols.display_path(symbol, "::")
            ))]
        })?;
    canonical_source_span_location(compilation, span, role)
}

pub(crate) fn project_nested_declaration_source_location(
    compilation: &CheckedCompilation,
    symbol: SymbolHandle,
    authored_role: PackageReviewSourceLocationRole,
    subject: &str,
) -> Result<ProjectedNestedSourceLocation, Vec<Diagnostic>> {
    let (source_span, role) = match compilation.typed.symbols.symbol_source_span(symbol) {
        Some(source_span) => (source_span, authored_role),
        None => (
            compilation
                .typed
                .symbols
                .symbol_provenance_source_span(symbol)
                .ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "{subject} has neither an authored declaration span nor a derivation origin"
                    ))]
                })?,
            PackageReviewSourceLocationRole::DerivationOrigin,
        ),
    };
    Ok(ProjectedNestedSourceLocation { source_span, role })
}

pub(crate) fn canonical_source_span_location(
    compilation: &CheckedCompilation,
    span: psi_source::SourceSpan,
    role: PackageReviewSourceLocationRole,
) -> Result<PackageReviewSourceLocation, Vec<Diagnostic>> {
    let source_file = compilation.typed.symbols.source_file(span).ok_or_else(|| {
        vec![Diagnostic::error(
            "reviewed declaration source span has no retained source file",
        )]
    })?;
    if span.span.start >= span.span.end
        || span.span.end > source_file.source.len()
        || !source_file.source.is_char_boundary(span.span.start)
        || !source_file.source.is_char_boundary(span.span.end)
    {
        return Err(vec![Diagnostic::error(format!(
            "reviewed declaration source span is outside `{}`",
            source_file.path.display()
        ))]);
    }
    let owner = match source_file.origin {
        psi_source::SourceOrigin::User => PackageReviewSourceLocationOwner::Package(
            source_file.package_identity.ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "reviewed package source `{}` has no reconciled package identity",
                    source_file.path.display()
                ))]
            })?,
        ),
        psi_source::SourceOrigin::Toolchain => {
            PackageReviewSourceLocationOwner::Toolchain(toolchain_source_identity(source_file)?)
        }
    };
    let relative_path = canonical_review_relative_path(source_file)?;
    Ok(PackageReviewSourceLocation {
        owner,
        relative_path,
        start_byte: u64::try_from(span.span.start).expect("retained source byte offset fits u64"),
        end_byte: u64::try_from(span.span.end).expect("retained source byte offset fits u64"),
        role,
    })
}

pub(crate) fn canonical_review_relative_path(
    source_file: &psi_source::SourceFile,
) -> Result<String, Vec<Diagnostic>> {
    let relative = match source_file.path.strip_prefix(&source_file.package_root) {
        Ok(relative) => relative,
        Err(_)
            if source_file.origin == psi_source::SourceOrigin::Toolchain
                && is_canonical_virtual_toolchain_path(&source_file.path) =>
        {
            source_file.path.as_path()
        }
        Err(_) => {
            return Err(vec![Diagnostic::error(format!(
                "reviewed source `{}` is outside its retained package root `{}`",
                source_file.path.display(),
                source_file.package_root.display()
            ))]);
        }
    };
    let mut path = String::new();
    for component in relative.components() {
        let std::path::Component::Normal(component) = component else {
            return Err(vec![Diagnostic::error(format!(
                "reviewed source `{}` has a non-canonical relative path",
                source_file.path.display()
            ))]);
        };
        let component = component.to_str().ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "reviewed source `{}` has a non-UTF-8 relative path component",
                source_file.path.display()
            ))]
        })?;
        if !path.is_empty() {
            path.push('/');
        }
        path.push_str(component);
    }
    if path.is_empty() {
        return Err(vec![Diagnostic::error(
            "reviewed source location requires a non-empty relative path",
        )]);
    }
    Ok(path)
}

pub(crate) fn callable_exposes_service(
    callable: &CheckedPackageCallableReview,
    service: &PackageReviewNominalIdentity,
) -> bool {
    callable
        .declared_service_reach()
        .is_some_and(|services| services.contains(service))
        || callable
            .checked_service_reach()
            .realized()
            .is_some_and(|services| services.contains(service))
        || callable
            .checked_service_reach()
            .concrete()
            .is_some_and(|services| services.contains(service))
        || callable
            .unresolved_installation_reaches()
            .iter()
            .any(|reach| reach.upper_bound().contains(service))
        || callable
            .declared_synchronous_invocations()
            .is_some_and(|invocations| {
                invocations
                    .iter()
                    .any(|invocation| invocation.service() == Some(service))
            })
        || callable
            .realized_synchronous_invocations()
            .iter()
            .any(|invocation| invocation.service() == Some(service))
}

/// Compiler-owned intrinsic metadata for the standard authority catalog.
/// Both the declaration path and immutable toolchain source coordinate must
/// match. A package-authored lookalike therefore cannot acquire or suppress a
/// risk class by choosing a declaration name.
pub(crate) fn dangerous_authority_class(
    compilation: &CheckedCompilation,
    definition: &psi_language_semantics::ServiceReachDefinition,
) -> Option<PackageReviewDangerousAuthorityClass> {
    let source_file = compilation
        .typed
        .symbols
        .symbol_source_span(definition.symbol)
        .and_then(|span| compilation.typed.symbols.source_file(span))?;
    if source_file.origin != psi_source::SourceOrigin::Toolchain {
        return None;
    }
    let relative_source = source_file
        .path
        .strip_prefix(&source_file.package_root)
        .ok()?;
    match (
        relative_source,
        compilation
            .typed
            .symbols
            .display_path(definition.symbol, "::")
            .as_str(),
    ) {
        (path, "FilesystemHost") if path == std::path::Path::new("filesystem_host.omg") => {
            Some(PackageReviewDangerousAuthorityClass::Filesystem)
        }
        (path, "MachineControl") if path == std::path::Path::new("assembly.omg") => {
            Some(PackageReviewDangerousAuthorityClass::MachineControl)
        }
        (path, "PortIo") if path == std::path::Path::new("assembly.omg") => {
            Some(PackageReviewDangerousAuthorityClass::PortIo)
        }
        (path, "InterruptMaskControl") if path == std::path::Path::new("interrupt.omg") => {
            Some(PackageReviewDangerousAuthorityClass::InterruptControl)
        }
        (path, "InterruptEntry") if path == std::path::Path::new("interrupt.omg") => {
            Some(PackageReviewDangerousAuthorityClass::InterruptEntry)
        }
        (path, "ExtentRootProvider") if path == std::path::Path::new("extent.omg") => {
            Some(PackageReviewDangerousAuthorityClass::RootMemory)
        }
        (path, "Console") if path == std::path::Path::new("console.omg") => {
            Some(PackageReviewDangerousAuthorityClass::Process)
        }
        _ => None,
    }
}
