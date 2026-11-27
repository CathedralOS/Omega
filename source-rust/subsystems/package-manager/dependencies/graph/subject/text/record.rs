//! The canonical package graph records a purpose on every edge. Recovery checks
//! field limits, total allocation and byte-for-byte canonical re-emission.

use super::super::CatalogObserver;
use super::super::{
    CanonicalDependencySourceSelection, CanonicalSourceClosureSubject,
    CanonicalSourceClosureSubjectError as Error, CanonicalSourceClosureSubjectLimits as Limits,
};
use super::super::{CanonicalSourceClosureSubjectRecoveryUsage as Usage, usage::Budget};
use super::framing::{Reader, Writer};
use super::requests::{
    read_navigation, read_request, read_root, write_navigation, write_request, write_root,
};
use super::source::{
    read_key, read_lineage, read_resolution, read_source, write_key, write_lineage,
    write_resolution, write_source,
};
use crate::declarations::dependencies::read::{
    DependencyProjections, DependencyPurpose, DependencyUse, ProjectedDependencies,
};
use semantic_vocabulary::dependency_reach::DependencyReach;
use target::TargetProfile;

const TEXT_VERSION: usize = 8;

impl CanonicalSourceClosureSubject {
    pub fn canonical_text(&self, limits: Limits) -> Result<String, Error> {
        let limits = limits.compiler_bounded();
        self.validate_recovery_limits(limits)?;
        super::super::encoding::encode_subject(
            self.target_profile,
            &self.root,
            &self.packages,
            &self.package_navigations,
            &self.package_dependency_projections,
            &self.dependency_requests,
            &self.catalog_dependencies,
            limits,
            &mut Budget::new(usize::MAX),
        )?;
        let mut writer = Writer::new(limits.maximum_record_bytes, Budget::new(usize::MAX));
        self.write_text(&mut writer)?;
        Ok(writer.finish()?.0)
    }

    fn write_text(&self, writer: &mut Writer) -> Result<(), Error> {
        writer.number("omega-source-closure", TEXT_VERSION)?;
        writer.row(
            "target",
            &[self.target_profile().identity().as_str().as_bytes()],
        )?;
        write_root(writer, self.root())?;
        writer.number("packages", self.packages().len())?;
        for ((package, navigation), projection) in self
            .packages
            .iter()
            .zip(&self.package_navigations)
            .zip(&self.package_dependency_projections)
        {
            writer.row("package", &[])?;
            write_source(writer, package)?;
            write_navigation(writer, navigation)?;
            for (label, scope) in [
                ("authored", projection.product()),
                ("authored-build", projection.build()),
            ] {
                writer.number(label, scope.authored_dependencies().len())?;
                for request in scope.authored_dependencies() {
                    write_request(writer, &request.source)?;
                    write_reach(writer, &request.max_reach)?;
                }
            }
        }
        writer.number("edges", self.dependency_requests().len())?;
        for edge in self.dependency_requests() {
            writer.row("edge", &[])?;
            writer.row("requester", &[])?;
            write_key(writer, edge.requester())?;
            writer.row("purpose", &[edge.purpose().name().as_bytes()])?;
            writer.number("ordinal", edge.dependency_index())?;
            write_request(writer, edge.request())?;
            writer.row("resolved-alias", &[edge.alias().as_str().as_bytes()])?;
            writer.row("selected", &[])?;
            write_source(writer, edge.selected())?;
        }

        writer.number("catalog-dependencies", self.catalog_dependencies.len())?;
        for dependency in &self.catalog_dependencies {
            writer.row("catalog-dependency", &[])?;
            match &dependency.observer {
                CatalogObserver::Member { source, navigation } => {
                    writer.row("observer member", &[])?;
                    write_source(writer, source)?;
                    write_navigation(writer, navigation)?;
                }
                CatalogObserver::Workspace {
                    lineage,
                    resolution,
                } => {
                    writer.row("observer workspace", &[])?;
                    write_lineage(writer, lineage)?;
                    write_resolution(writer, resolution)?;
                }
            }
            writer.number("preparing", usize::from(dependency.preparation.is_some()))?;
            if let Some(stage) = &dependency.preparation {
                writer.number("stage", stage.ordinal)?;
                writer.row("successor", &[stage.successor.as_str().as_bytes()])?;
            }
            writer.number("ordinal", dependency.ordinal)?;
            writer.row("purpose", &[dependency.purpose.name().as_bytes()])?;
            write_request(writer, &dependency.request)?;
            write_reach(writer, &dependency.max_reach)?;
            dependency.provider.write_text(writer)?;
        }

        writer.row("end", &[])?;
        Ok(())
    }

    pub fn recover_text(text: &str, limits: Limits) -> Result<Self, Error> {
        Self::recover_text_with_usage(text, limits, usize::MAX).map(|(subject, _)| subject)
    }

    pub fn recover_text_with_usage(
        text: &str,
        limits: Limits,
        maximum_owned_bytes: usize,
    ) -> Result<(Self, Usage), Error> {
        let limits = limits.compiler_bounded();
        let mut reader = Reader::new(
            text,
            limits.maximum_record_bytes,
            Budget::new(maximum_owned_bytes),
        )?;
        let subject = Self::read_text(&mut reader, limits, true)?;
        reader.finish()?;
        let mut writer = Writer::verifying(limits.maximum_record_bytes, text, reader.budget);
        subject.write_text(&mut writer)?;
        let (_, budget) = writer.finish()?;
        Ok((subject, budget.usage))
    }

    fn read_text(
        reader: &mut Reader<'_>,
        limits: Limits,
        allow_catalog: bool,
    ) -> Result<Self, Error> {
        reader.expect("omega-source-closure")?;
        if reader.number(usize::MAX)? != TEXT_VERSION {
            return Err(Error::new(
                "unsupported source-closure version; archive omega.lock and run omega packages update for a fresh review",
            ));
        }
        reader.expect("target")?;
        let target = reader.string(limits.maximum_identity_bytes)?;
        let target = TargetProfile::ALL
            .into_iter()
            .find(|profile| profile.identity().as_str() == target)
            .ok_or_else(|| Error::new("unknown text target-profile identity"))?;
        let root = read_root(reader, limits)?;
        reader.expect("packages")?;
        let count = reader.count(limits.maximum_packages)?;
        let mut packages = reader.budget.reserve(count)?;
        let mut navigations = reader.budget.reserve(count)?;
        let mut projections = reader.budget.reserve(count)?;
        let mut total_authored = 0usize;
        for _ in 0..count {
            reader.expect("package")?;
            packages.push(read_source(reader, limits)?);
            navigations.push(read_navigation(reader, limits)?);
            let product = read_authored_scope(reader, "authored", limits, &mut total_authored)?;
            let build = read_authored_scope(reader, "authored-build", limits, &mut total_authored)?;
            projections.push(DependencyProjections::new(product, build));
        }
        reader.expect("edges")?;
        let count = reader.count(limits.maximum_dependency_requests)?;
        let mut edges = reader.budget.reserve(count)?;
        for _ in 0..count {
            reader.expect("edge")?;
            reader.expect("requester")?;
            let requester = read_key(reader, limits)?;
            let purpose = read_purpose(reader)?;
            reader.expect("ordinal")?;
            let dependency_index = reader.number(u32::MAX as usize)?;
            let request = read_request(reader, limits)?;
            reader.expect("resolved-alias")?;
            let alias = reader.alias(limits.maximum_identity_bytes)?;
            reader.expect("selected")?;
            let selected = read_source(reader, limits)?;
            edges.push(CanonicalDependencySourceSelection {
                requester,
                purpose,
                dependency_index,
                request,
                alias,
                selected,
            });
        }

        reader.expect("catalog-dependencies")?;
        let count = reader.count(if allow_catalog {
            limits.maximum_dependency_requests
        } else {
            0
        })?;
        let mut catalog_dependencies = reader.budget.reserve(count)?;
        for _ in 0..count {
            reader.expect("catalog-dependency")?;
            reader.expect("observer")?;
            let observer = match reader.atom()? {
                "member" => CatalogObserver::Member {
                    source: read_source(reader, limits)?,
                    navigation: read_navigation(reader, limits)?,
                },
                "workspace" => CatalogObserver::Workspace {
                    lineage: read_lineage(reader, limits)?,
                    resolution: read_resolution(reader)?,
                },
                _ => return Err(Error::new("unknown catalog observer role")),
            };
            reader.expect("preparing")?;
            let preparation = if reader.number(1)? == 1 {
                reader.expect("stage")?;
                let ordinal = reader.number(u32::MAX as usize)?;
                reader.expect("successor")?;
                let successor = package_compilation::declarations::BuildEntryPath::parse(
                    reader.string(limits.maximum_request_bytes)?,
                )
                .map_err(Error::new)?;
                Some(crate::resolution::source::CatalogPreparation { ordinal, successor })
            } else {
                None
            };
            reader.expect("ordinal")?;
            let ordinal = reader.number(u32::MAX as usize)?;
            let purpose = read_purpose(reader)?;
            let request = read_request(reader, limits)?;
            let max_reach = read_reach(reader, limits)?;
            let provider = Self::read_text(reader, limits, false)?;
            catalog_dependencies.push(super::super::CanonicalCatalogDependency {
                observer,
                preparation,
                purpose,
                ordinal,
                request,
                max_reach,
                provider,
            });
        }

        reader.expect("end")?;
        reader.budget.usage.packages += packages.len() + catalog_dependencies.len();
        reader.budget.usage.authored_dependency_requests +=
            total_authored + catalog_dependencies.len();
        reader.budget.usage.dependency_requests += edges.len() + catalog_dependencies.len();
        let subject = Self::finish(
            target,
            root,
            packages,
            navigations,
            projections,
            edges,
            limits,
            &mut reader.budget,
        )?;
        let subject =
            subject.with_catalog_dependencies(catalog_dependencies, limits, &mut reader.budget)?;
        Ok(subject)
    }
}

fn read_authored_scope(
    reader: &mut Reader<'_>,
    label: &str,
    limits: Limits,
    total_authored: &mut usize,
) -> Result<ProjectedDependencies, Error> {
    reader.expect(label)?;
    let authored_count = reader.count(limits.maximum_dependency_requests - *total_authored)?;
    *total_authored = total_authored
        .checked_add(authored_count)
        .ok_or_else(|| Error::new("text authored-request count overflow"))?;
    if *total_authored > limits.maximum_dependency_requests {
        return Err(Error::new("text authored-request count exceeds its limit"));
    }
    let mut authored = reader.budget.reserve(authored_count)?;
    for _ in 0..authored_count {
        let source = read_request(reader, limits)?;
        authored.push(DependencyUse {
            source,
            max_reach: read_reach(reader, limits)?,
        });
    }
    Ok(ProjectedDependencies::issued(authored))
}

fn write_reach(writer: &mut Writer, reach: &DependencyReach) -> Result<(), Error> {
    writer.number("max-reach", reach.services().count())?;
    for service in reach.services() {
        writer.row("service", &[service.as_bytes()])?;
    }
    Ok(())
}

fn read_reach(reader: &mut Reader<'_>, limits: Limits) -> Result<DependencyReach, Error> {
    reader.expect("max-reach")?;
    let count = reader.count(limits.maximum_record_bytes)?;
    let mut identities = reader.budget.reserve::<String>(count)?;
    for _ in 0..count {
        reader.expect("service")?;
        let identity = reader.string(limits.maximum_identity_bytes)?;
        if identity.is_empty() || identities.last().is_some_and(|prior| prior >= &identity) {
            return Err(Error::new(
                "ceiling services require distinct, ordered application identities",
            ));
        }
        identities.push(identity);
    }
    Ok(DependencyReach::from_checked_services(identities))
}

fn read_purpose(reader: &mut Reader<'_>) -> Result<DependencyPurpose, Error> {
    reader.expect("purpose")?;
    match reader.string(64)?.as_str() {
        "product" => Ok(DependencyPurpose::Product),
        "build" => Ok(DependencyPurpose::Build),
        _ => Err(Error::new("unknown text dependency purpose")),
    }
}
