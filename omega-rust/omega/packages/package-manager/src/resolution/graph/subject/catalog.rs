//! Catalog execution sources are recorded beside, never inside, Product routing.

use super::{
    CanonicalCatalogDependency, CanonicalDependencySourceRequest,
    CanonicalDependencySourceSelection, CanonicalSourceClosureSubject,
    CanonicalSourceClosureSubjectError as Error, CanonicalSourceClosureSubjectLimits as Limits,
};
use super::{request_view::Request, usage::Budget};
use crate::declarations::BuildDeclarationKind;
#[cfg(test)]
use crate::declarations::dependencies::read::DependencyPurpose;
use crate::resolution::graph::{ExactTargetPackageSourceClosure, ResolvedSourceIdentity};

pub(super) fn project(
    target: &ExactTargetPackageSourceClosure<'_>,
    limits: Limits,
) -> Result<Vec<CanonicalCatalogDependency>, Error> {
    let mut pending = vec![target.source_closure()];
    let mut dependencies: Vec<CanonicalCatalogDependency> = Vec::new();
    let mut position = 0;
    while let Some(closure) = pending.get(position).copied() {
        position += 1;
        for custody in closure.custodies() {
            for observation in custody.selection_evidence().observations() {
                for (selection, graph) in observation
                    .selections
                    .iter()
                    .zip(observation.dependency_graphs())
                {
                    let observer = &observation.source;
                    let request = observer
                        .dependency_requests(selection.purpose)
                        .get(selection.request_ordinal)
                        .ok_or_else(|| Error::new("catalog provider has no issued request"))?;
                    let provider = CanonicalSourceClosureSubject::from_resolved_without_catalog(
                        &graph.closure.for_exact_target(target.target_profile()),
                        limits,
                    )?;
                    let dependency = CanonicalCatalogDependency {
                        observer: ResolvedSourceIdentity::from_validated_parts(
                            observer.key().clone(),
                            observer.resolution().clone(),
                        ),
                        navigation: observer.navigation().clone(),
                        purpose: selection.purpose,
                        ordinal: selection.request_ordinal,
                        request: CanonicalDependencySourceRequest::from(request),
                        provider,
                    };
                    if let Some(previous) = dependencies.iter().find(|previous| {
                        previous.observer.key() == dependency.observer.key()
                            && previous.purpose == dependency.purpose
                            && previous.ordinal == dependency.ordinal
                    }) {
                        if previous != &dependency {
                            return Err(Error::new(
                                "catalog observations disagree on executed source custody",
                            ));
                        }
                        continue;
                    }
                    if dependencies.len() >= limits.maximum_dependency_requests {
                        return Err(Error::new("catalog execution exceeds its dependency limit"));
                    }
                    dependencies.push(dependency);
                    pending.push(&graph.closure);
                }
            }
        }
    }
    dependencies.sort_by(|left, right| {
        left.observer
            .key()
            .cmp(right.observer.key())
            .then(left.purpose.cmp(&right.purpose))
            .then(left.ordinal.cmp(&right.ordinal))
    });
    Ok(dependencies)
}

pub(super) fn validate(
    subject: &CanonicalSourceClosureSubject,
    dependencies: &[CanonicalCatalogDependency],
    limits: Limits,
    budget: &mut Budget,
) -> Result<(), Error> {
    let mut packages = subject.packages.len();
    let mut requests = subject.dependency_requests.len();
    let mut previous: Option<&CanonicalCatalogDependency> = None;
    for dependency in dependencies {
        if !dependency.provider.catalog_dependencies.is_empty()
            || dependency.provider.target_profile != subject.target_profile
            || dependency.provider.root_role() != BuildDeclarationKind::Package
        {
            return Err(Error::new(
                "catalog provider must be a flat closed package source graph at the same target",
            ));
        }
        packages = packages
            .checked_add(dependency.provider.packages.len() + 1)
            .filter(|count| *count <= limits.maximum_packages)
            .ok_or_else(|| Error::new("catalog execution exceeds its package limit"))?;
        requests = requests
            .checked_add(dependency.provider.dependency_requests.len() + 1)
            .filter(|count| *count <= limits.maximum_dependency_requests)
            .ok_or_else(|| Error::new("catalog execution exceeds its dependency limit"))?;
        if previous.is_some_and(|previous| {
            (previous.observer.key(), previous.purpose, previous.ordinal)
                >= (
                    dependency.observer.key(),
                    dependency.purpose,
                    dependency.ordinal,
                )
        }) {
            return Err(Error::new(
                "catalog dependencies are not in strict canonical order",
            ));
        }
        previous = Some(dependency);
        super::validation::validate_source_identity(
            &dependency.observer,
            limits.maximum_identity_bytes,
        )?;
        super::validation::validate_package_navigation(
            &dependency.observer,
            &dependency.navigation,
        )?;
        super::validation::validate_dependency_request(
            Request::from(&dependency.request),
            limits.maximum_request_bytes,
        )?;
        let selected = dependency.provider.root().selected();
        let alias = match dependency.request.explicit_alias() {
            Some(alias) => alias.clone(),
            None => selected.key().name().default_alias(),
        };
        let edge = CanonicalDependencySourceSelection {
            requester: dependency.observer.key().clone(),
            purpose: dependency.purpose,
            dependency_index: dependency.ordinal,
            request: dependency.request.clone(),
            alias,
            selected: selected.clone(),
        };
        for graph in
            std::iter::once(subject).chain(dependencies.iter().map(|entry| &entry.provider))
        {
            if graph
                .packages
                .binary_search_by(|source| source.key().cmp(dependency.observer.key()))
                .is_ok()
                && !graph
                    .dependency_requests
                    .iter()
                    .any(|recorded| recorded == &edge)
            {
                return Err(Error::new(
                    "catalog execution disagrees with the same package's complete dependency ledger",
                ));
            }
        }
        let navigation = dependency
            .provider
            .package_navigation(selected.key())
            .ok_or_else(|| Error::new("catalog provider root has no navigation"))?;
        super::validation::validate_dependency_selection_kind(
            &edge,
            &dependency.observer,
            &dependency.navigation,
            navigation,
            budget,
        )?;
        let provider = &dependency.provider;
        super::validation::validate_subject_with_budget(
            &provider.root,
            &provider.packages,
            &provider.package_navigations,
            &provider.package_dependency_projections,
            &provider.dependency_requests,
            limits,
            budget,
        )?;
    }
    let mut sources = budget.reserve(packages)?;
    sources.extend(subject.packages.iter().zip(&subject.package_navigations));
    for dependency in dependencies {
        sources.push((&dependency.observer, &dependency.navigation));
        sources.extend(
            dependency
                .provider
                .packages
                .iter()
                .zip(&dependency.provider.package_navigations),
        );
    }
    sources.sort_by(|left, right| left.0.key().cmp(right.0.key()));
    if sources
        .windows(2)
        .any(|pair| pair[0].0.key() == pair[1].0.key() && pair[0] != pair[1])
    {
        return Err(Error::new(
            "catalog and Product sources disagree on immutable custody",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::{CanonicalRootSourceRequest, CanonicalRootSourceSelection};
    use super::*;
    use crate::declarations::dependencies::read::{DependencyProjections, PackageSelection};
    use crate::declarations::{PackageKey, PackageName};
    use crate::resolution::source::PackageSourceNavigation;
    use package_source::{GitCommitId, GitSourceRequest, GitTreeId, ImmutableSourceResolution};

    fn source_graph(name: &str, locator: &str) -> CanonicalSourceClosureSubject {
        let request = GitSourceRequest::new(locator, Some("main".into())).unwrap();
        let source = ResolvedSourceIdentity::new(
            PackageKey::new(PackageName::parse(name).unwrap(), request.lineage().clone()),
            ImmutableSourceResolution::git(
                GitCommitId::parse_hex(&"1".repeat(40)).unwrap(),
                GitTreeId::parse_hex(&"2".repeat(40)).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        CanonicalSourceClosureSubject::finish_with_projections(
            target::TargetProfile::from_canonical_target_name("linux_x86_64").unwrap(),
            CanonicalRootSourceSelection {
                request: CanonicalRootSourceRequest::Git {
                    requested_locator: locator.to_owned(),
                    requested_revision: "main".into(),
                    selection: PackageSelection::Root,
                },
                role: BuildDeclarationKind::Package,
                selected: source.clone(),
            },
            vec![source],
            vec![PackageSourceNavigation::Root],
            vec![DependencyProjections::from_executed_requests(std::iter::empty()).unwrap()],
            Vec::new(),
            Limits::default(),
        )
        .unwrap()
    }

    fn catalog_graph() -> CanonicalSourceClosureSubject {
        let root = source_graph("selected", "https://catalog.invalid/workspace.git");
        let provider = source_graph("helper", "https://catalog.invalid/helper.git");
        let dependency = CanonicalCatalogDependency {
            observer: ResolvedSourceIdentity::new(
                PackageKey::new(
                    PackageName::parse("unselected").unwrap(),
                    root.root.selected.key().source_lineage().clone(),
                ),
                root.root.selected.resolution().clone(),
            )
            .unwrap(),
            navigation: PackageSourceNavigation::Member(
                package_source::SourceRelativePath::parse("unselected").unwrap(),
            ),
            purpose: DependencyPurpose::Build,
            ordinal: 0,
            request: CanonicalDependencySourceRequest::Git {
                explicit_alias: None,
                repository: "https://catalog.invalid/helper.git".into(),
                revision: "main".into(),
                selection: PackageSelection::Root,
            },
            provider,
        };
        root.with_catalog_dependencies(
            vec![dependency],
            Limits::default(),
            &mut Budget::new(usize::MAX),
        )
        .unwrap()
    }

    #[test]
    fn catalog_query_roundtrip_keeps_purpose_local_ordinals_distinct() {
        let previous = catalog_graph();
        let mut dependencies = previous.catalog_dependencies.clone();
        let mut queried = dependencies[0].clone();
        queried.purpose = DependencyPurpose::Product;
        dependencies.push(queried);
        dependencies.sort_by_key(|dependency| dependency.purpose);
        let subject = source_graph("selected", "https://catalog.invalid/workspace.git")
            .with_catalog_dependencies(
                dependencies,
                Limits::default(),
                &mut Budget::new(usize::MAX),
            )
            .unwrap();
        let text = subject.canonical_text(Limits::default()).unwrap();
        assert!(text.starts_with("omega-source-closure 4\n"));
        assert!(text.contains("ordinal 0\npurpose \"product\"\n"));
        let recovered =
            CanonicalSourceClosureSubject::recover_text(&text, Limits::default()).unwrap();
        assert_eq!(recovered, subject);
        assert_eq!(recovered.packages().len(), 1);
        assert_eq!(recovered.catalog_dependencies.len(), 2);
        assert!(!previous.same_source_graph(&subject));
        let mut changed = previous.clone();
        changed.catalog_dependencies[0].purpose = DependencyPurpose::Product;
        assert!(
            !previous.same_source_graph(&changed),
            "purpose is a source reuse input"
        );
        assert!(
            CanonicalSourceClosureSubject::recover_text(
                &text.replace("omega-source-closure 4", "omega-source-closure 3"),
                Limits::default()
            )
            .is_err()
        );
    }

    #[test]
    fn catalog_execution_roundtrip_preserves_product_roster_and_helper_pin() {
        let subject = catalog_graph();
        let text = subject.canonical_text(Limits::default()).unwrap();
        assert!(text.starts_with("omega-source-closure 3\n"));
        let recovered =
            CanonicalSourceClosureSubject::recover_text(&text, Limits::default()).unwrap();
        assert_eq!(recovered, subject);
        assert_eq!(
            recovered.packages().len(),
            1,
            "helper is not a Product package"
        );
        let pins = crate::resolution::graph::GitDependencyPins::new(
            &recovered,
            &[],
            package_source::git::resolution::GitExactRevisionAcquisition::Offline,
        )
        .unwrap();
        let options = crate::resolution::graph::GitResolutionOptions {
            pins: Some(pins),
            offline: true,
        };
        let request =
            GitSourceRequest::new("https://catalog.invalid/helper.git", Some("main".into()))
                .unwrap();
        let recorded = options.recorded(&request).unwrap().unwrap();
        assert_eq!(
            recorded.resolution,
            subject.catalog_dependencies[0]
                .provider
                .root
                .selected
                .resolution()
        );
    }

    #[test]
    fn catalog_execution_refuses_nested_records_duplicates_and_excess_storage() {
        let subject = catalog_graph();
        let text = subject.canonical_text(Limits::default()).unwrap();
        let narrow = Limits {
            maximum_packages: 2,
            ..Limits::default()
        };
        assert!(CanonicalSourceClosureSubject::recover_text(&text, narrow).is_err());
        assert!(
            CanonicalSourceClosureSubject::recover_text_with_usage(&text, Limits::default(), 1)
                .is_err()
        );
        let mut duplicate = subject.catalog_dependencies.clone();
        duplicate.push(duplicate[0].clone());
        let base = source_graph("selected", "https://catalog.invalid/workspace.git");
        assert!(
            base.clone()
                .with_catalog_dependencies(
                    duplicate,
                    Limits::default(),
                    &mut Budget::new(usize::MAX)
                )
                .is_err()
        );
        let mut nested = subject.catalog_dependencies.clone();
        nested[0].provider = subject;
        assert!(
            base.with_catalog_dependencies(nested, Limits::default(), &mut Budget::new(usize::MAX))
                .is_err()
        );
    }

    #[test]
    fn catalog_free_subject_retains_version_seven_identity() {
        let subject = source_graph("selected", "https://catalog.invalid/workspace.git");
        let text = subject.canonical_text(Limits::default()).unwrap();
        assert!(text.starts_with("omega-source-closure 2\n"));
        let version =
            &subject.canonical_bytes()[super::super::SOURCE_CLOSURE_SUBJECT_MAGIC.len()..][..2];
        assert_eq!(version, 7u16.to_le_bytes());
        assert_eq!(
            CanonicalSourceClosureSubject::recover_text(&text, Limits::default()).unwrap(),
            subject
        );
    }
}
