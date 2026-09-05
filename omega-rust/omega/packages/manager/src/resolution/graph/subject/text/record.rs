//! Complete text record rendering and recovery through existing graph validation.

use super::super::validation::validate_subject;
use super::super::{
    CanonicalDependencySourceSelection, CanonicalSourceClosureSubject,
    CanonicalSourceClosureSubjectError as Error, CanonicalSourceClosureSubjectLimits as Limits,
};
use super::super::{
    CanonicalSourceClosureSubjectRecoveryUsage as Usage, request_view::Request, usage::Budget,
};
use super::framing::{Reader, Writer};
use super::requests::{
    into_authored, read_navigation, read_request, read_root, write_navigation, write_request,
    write_root,
};
use super::source::{read_key, read_source, write_key, write_source};
use crate::declarations::dependencies::read::ProjectedDependencies;
use omega_target::TargetProfile;

impl CanonicalSourceClosureSubject {
    /// Render the exact source graph as bounded canonical text, without
    /// granting acceptance or changing its binary identity.
    pub fn canonical_text(&self, limits: Limits) -> Result<String, Error> {
        let limits = limits.compiler_bounded();
        if self.canonical_bytes().len() > limits.maximum_record_bytes {
            return Err(Error::new(
                "source-closure subject exceeds its record-byte limit",
            ));
        }
        validate_subject(
            &self.root,
            &self.packages,
            &self.package_navigations,
            &self.package_dependency_projections,
            &self.dependency_requests,
            limits,
        )?;
        // The binary field encoder owns the existing identity/request limits
        // (including target, navigation, aliases, and named selections). Reuse
        // those checks under this caller's possibly tighter ceilings.
        let encoded_length = super::super::encoding::encode_subject(
            self.target_profile,
            &self.root,
            &self.packages,
            &self.package_navigations,
            &self.package_dependency_projections,
            &self.dependency_requests,
            limits,
        )?
        .len();
        if encoded_length > limits.maximum_record_bytes {
            return Err(Error::new(
                "source-closure subject exceeds its record-byte limit",
            ));
        }
        let mut writer = Writer::new(limits.maximum_record_bytes, Budget::new(usize::MAX));
        self.write_text(&mut writer)?;
        Ok(writer.finish()?.0)
    }

    fn write_text(&self, writer: &mut Writer) -> Result<(), Error> {
        writer.row("omega-source-closure 1", &[])?;
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
            writer.number("authored", projection.authored_dependencies().len())?;
            for request in projection.authored_dependencies() {
                write_request(writer, Request::from(request))?;
            }
        }
        writer.number("edges", self.dependency_requests().len())?;
        for edge in self.dependency_requests() {
            writer.row("edge", &[])?;
            writer.row("requester", &[])?;
            write_key(writer, edge.requester())?;
            writer.number("ordinal", edge.dependency_index())?;
            write_request(writer, Request::from(edge.request()))?;
            writer.row("resolved-alias", &[edge.alias().as_str().as_bytes()])?;
            writer.row("selected", &[])?;
            write_source(writer, edge.selected())?;
        }
        writer.row("end", &[])?;
        Ok(())
    }

    /// Recover exact source identity and requests only. No filesystem access,
    /// selector update, compiler evidence, or project decision is implied.
    pub fn recover_text(text: &str, limits: Limits) -> Result<Self, Error> {
        Self::recover_text_with_usage(text, limits, usize::MAX).map(|(subject, _)| subject)
    }

    /// Recover under one monotone allowance covering parsed/retained values and
    /// validation, constructor, binary-encoding, and text-verification scratch.
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
        reader.expect("omega-source-closure")?;
        reader.expect("1")?;
        reader.expect("target")?;
        let target = reader.string(limits.maximum_identity_bytes)?;
        let target = TargetProfile::ALL
            .into_iter()
            .find(|profile| profile.identity().as_str() == target)
            .ok_or_else(|| Error::new("unknown text target-profile identity"))?;
        let root = read_root(&mut reader, limits)?;
        reader.expect("packages")?;
        let count = reader.count(limits.maximum_packages)?;
        let mut packages = reader.budget.reserve(count)?;
        let mut navigations = reader.budget.reserve(count)?;
        let mut projections = reader.budget.reserve(count)?;
        let mut total_authored = 0usize;
        for _ in 0..count {
            reader.expect("package")?;
            packages.push(read_source(&mut reader, limits)?);
            navigations.push(read_navigation(&mut reader, limits)?);
            reader.expect("authored")?;
            let authored_count =
                reader.count(limits.maximum_dependency_requests - total_authored)?;
            total_authored = total_authored
                .checked_add(authored_count)
                .ok_or_else(|| Error::new("text authored-request count overflow"))?;
            if total_authored > limits.maximum_dependency_requests {
                return Err(Error::new("text authored-request count exceeds its limit"));
            }
            let mut authored = reader.budget.reserve(authored_count)?;
            for _ in 0..authored_count {
                authored.push(into_authored(read_request(&mut reader, limits)?));
            }
            projections.push(ProjectedDependencies::from(authored));
        }
        reader.expect("edges")?;
        let count = reader.count(limits.maximum_dependency_requests)?;
        let mut edges = reader.budget.reserve(count)?;
        for _ in 0..count {
            reader.expect("edge")?;
            reader.expect("requester")?;
            let requester = read_key(&mut reader, limits)?;
            reader.expect("ordinal")?;
            let dependency_index = reader.number(u32::MAX as usize)?;
            let request = read_request(&mut reader, limits)?;
            reader.expect("resolved-alias")?;
            let alias = reader.alias(limits.maximum_identity_bytes)?;
            reader.expect("selected")?;
            let selected = read_source(&mut reader, limits)?;
            edges.push(CanonicalDependencySourceSelection {
                requester,
                dependency_index,
                request,
                alias,
                selected,
            });
        }
        reader.expect("end")?;
        reader.finish()?;
        reader.budget.usage.packages = packages.len();
        reader.budget.usage.authored_dependency_requests = total_authored;
        reader.budget.usage.dependency_requests = edges.len();
        let mut budget = reader.budget;
        let subject = Self::finish_with_budget(
            target,
            root,
            packages,
            navigations,
            projections,
            edges,
            limits,
            &mut budget,
        )?;
        let mut writer = Writer::verifying(limits.maximum_record_bytes, text, budget);
        subject.write_text(&mut writer)?;
        let (_, budget) = writer.finish()?;
        Ok((subject, budget.usage))
    }
}
