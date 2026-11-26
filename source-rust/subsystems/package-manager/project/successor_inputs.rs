//! Join one completed activation's ordinary dependency registrations to the next Build.
use super::services::SourceServices;
use super::{PreparedRequest, SourceHost};
use crate::PackageCommandError;
use crate::resolution::source::AcquiredBuildSource;
use build_declarations::{BuildRequest, DependencyPurpose};
use build_execution::ReturnedBuild;
use package_compilation::source_imports::SuppliedBuildLibrary;
use std::collections::{BTreeMap, BTreeSet};

impl SourceHost<'_, '_> {
    pub(super) fn successor_libraries(
        &mut self,
        acquired: &AcquiredBuildSource,
        returned: &ReturnedBuild,
        mut prepared: BTreeMap<(DependencyPurpose, usize), PreparedRequest>,
    ) -> Result<Vec<SuppliedBuildLibrary>, PackageCommandError> {
        returned.require_source_acquisition(acquired.source_capture())?;
        // A suspended query may have already seen some of these occurrences. A demand
        // must agree with exactly that preparation, even when another alias names the same source.
        for demand in returned.demanded_sources() {
            let request = prepared
                .get(&(demand.purpose, demand.request_ordinal))
                .ok_or(
                    "the staged Build demanded a dependency that was never prepared".to_owned(),
                )?;
            if self.settled[request.settled].declaration.identity() != demand.package {
                return Err("the staged Build demanded another dependency instance"
                    .to_owned()
                    .into());
            }
        }
        let mut services = SourceServices {
            host: self,
            acquired,
            requests: &mut prepared,
        };
        let mut libraries = Vec::new();
        let mut ordinals = BTreeMap::new();
        let mut aliases = BTreeSet::new();
        let mut issued_count = 0;
        for issued in returned.requests() {
            let BuildRequest::Dependency {
                purpose,
                alias,
                source,
            } = issued.request()
            else {
                continue;
            };
            let ordinal = ordinals.entry(*purpose).or_insert(0);
            let index = services.dependency(returned.requests(), *purpose, *ordinal)?;
            *ordinal += 1;
            issued_count += 1;
            let settled = &services.host.settled[index];
            let name = alias.as_ref().map_or_else(
                || settled.declaration.package_name(),
                |alias| alias.as_str(),
            );
            if !aliases.insert((*purpose, name.to_owned())) {
                return Err(format!(
                    "duplicate {} dependency alias `{name}` in staged Build",
                    purpose.name()
                )
                .into());
            }
            if let Some(binding) = issued.forwarded() {
                libraries.push(binding.library().clone());
            } else if purpose.is_product() {
                let input = services.host.library_input(
                    settled,
                    DependencyPurpose::Build,
                    name.to_owned(),
                )?;
                libraries.push(SuppliedBuildLibrary::new(source.clone(), input)?);
            }
        }
        if issued_count != prepared.len() {
            return Err(
                "a prepared dependency has no registration in the completed Build"
                    .to_owned()
                    .into(),
            );
        }
        Ok(libraries)
    }
}
