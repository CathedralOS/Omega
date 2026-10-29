//! Exact input equivalence for source preparation.
//!
//! These keys describe content, not admission. Every child still validates its
//! package closure and generated-source execution/target custody before reuse.
//! Producer stamps do not alter imported bytes, but source scope and package
//! ownership do. These keys authorize neither Build execution nor target selection.

use crate::source_imports::EntryContractSource;
use crate::{PackageCompilationInputs, PackageCompilationSourceInputs};
use build_declarations::DependencyPurpose;
use build_output::PackageGeneratedSource;
use semantic_vocabulary::PackageKeyIdentity;
use std::sync::Arc;
use target::TargetProfile;

/// Input content observed by source assembly, resolution and typing within one
/// immutable parse checkpoint. Entry bindings partition this frontier only
/// when they select different source suppliers. Every target's contracts join
/// assembly, so target-specific nominations of one supplier must share it.
/// Complete binding records remain separate execution and admission inputs.
/// Supplier presence is only a source-routing observation: checkpoint reuse
/// still validates live custody, and source sealing rechecks the consumed bytes
/// before issuing a checked product. This key cannot attest filesystem contents.
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

/// Exact source inputs to a Product frontend. Target attachments and semantic
/// binding admissions do not partition source assembly, resolution, or typing;
/// entry contract source selection is already recorded in `SourceAssemblyInputs`,
/// and custody/semantic bindings are still admitted per occurrence.
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

/// The source projection used by preparation. Generated bundles
/// retain only the scope, package and bytes assembly consumes. Their producing
/// target, dependency closure and source commitment remain admission evidence
/// on PackageCompilationInputs, checked separately for each child.
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
                    source_owner: bundle.source_owner(),
                    sources: bundle.sources().to_vec(),
                    selected_product_sources: bundle
                        .provider_selections()
                        .iter()
                        .flat_map(|selection| [&selection.subject, &selection.provider])
                        .chain(
                            bundle
                                .native_bindings()
                                .iter()
                                .flat_map(|row| row.anchors()),
                        )
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
    source_owner: crate::BuildSourceOwner,
    sources: Vec<PackageGeneratedSource>,
    selected_product_sources: Vec<std::path::PathBuf>,
}
