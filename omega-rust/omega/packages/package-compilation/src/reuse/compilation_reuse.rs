// Exact input equivalence keys (`SourceAssemblyInputs`, `ProductSourceInputs`)
// for source assembly, resolution, and typing checkpoints. These keys project
// only the source content, package scopes, generated-source bytes, and
// entry-contract supplier selections that affect frontend preparation; every
// consumer still validates live package closure, target custody, and consumed
// source bytes before admitting a reused checkpoint.

use crate::source::source_imports::EntryContractSource;
use crate::{PackageCompilationInputs, PackageCompilationSourceInputs};
use build_declarations::DependencyPurpose;
use build_output::PackageGeneratedSource;
use semantic_vocabulary::PackageKeyIdentity;
use std::sync::Arc;
use target::TargetProfile;

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct SourceAssemblyInputs {
    build_execution_profile: Option<TargetProfile>,
    package: Option<PackageSourceContent>,
    entry_sources: Vec<(
        target::ProgramEntryPhysicalContractPackage,
        EntryContractSource,
    )>,
}

impl SourceAssemblyInputs {
    pub fn new(
        packages: Option<&PackageCompilationInputs>,
        build_execution_profile: Option<TargetProfile>,
    ) -> Self {
        Self {
            build_execution_profile,
            package: packages.map(PackageSourceContent::new),
            entry_sources: TargetProfile::ALL
                .into_iter()
                .filter_map(|profile| profile.program_entry_slot().physical_contract_package)
                .map(|contract| (contract, EntryContractSource::select(contract, packages)))
                .collect(),
        }
    }
}

#[derive(Clone, PartialEq, Eq, compiler_storage::Record)]
pub struct ProductSourceInputs {
    assembly: SourceAssemblyInputs,
}

impl ProductSourceInputs {
    pub fn new(
        packages: Option<&PackageCompilationInputs>,
        build_execution_profile: Option<TargetProfile>,
    ) -> Self {
        Self {
            assembly: SourceAssemblyInputs::new(packages, build_execution_profile),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
struct PackageSourceContent {
    purpose: DependencyPurpose,
    source: Arc<PackageCompilationSourceInputs>,
    generated: Vec<GeneratedSourceContent>,
}

impl PackageSourceContent {
    fn new(inputs: &PackageCompilationInputs) -> Self {
        Self {
            purpose: inputs.compilation_purpose(),
            source: inputs.source_inputs(),
            generated: inputs
                .dependency_build_handoff_instances()
                .map(|(scope, bundle)| GeneratedSourceContent {
                    scope,
                    package: bundle.package(),
                    sources: bundle.sources().to_vec(),
                    selected_product_sources: bundle
                        .provider_selections()
                        .iter()
                        .flat_map(|selection| [&selection.subject, &selection.provider])
                        .map(|anchor| anchor.source().path.clone())
                        .collect(),
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
struct GeneratedSourceContent {
    scope: DependencyPurpose,
    package: PackageKeyIdentity,
    sources: Vec<PackageGeneratedSource>,
    selected_product_sources: Vec<std::path::PathBuf>,
}
