//! Exact input equivalence for source preparation and unsponsored build reuse.
//!
//! These keys describe content, not admission. Every child still validates its
//! package closure and generated-source execution/target custody before reuse.
//! Producer stamps do not alter imported bytes, but source scope and package
//! ownership do. Build execution additionally observes accepted policy,
//! captured inputs and required outputs; a source key cannot authorize its reuse.

use crate::source_imports::EntryContractSource;
use crate::{
    AcceptedSemanticBinding, BuildSnapshotRequest, IndependentComponentDescription,
    PackageCompilationInputs, PackageCompilationSourceInputs,
};
use build_declarations::DependencyPurpose;
use build_output::PackageGeneratedSource;
use semantic_vocabulary::PackageKeyIdentity;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use target::TargetProfile;

/// Input content observed by source assembly, resolution and typing within one
/// immutable parse checkpoint. Entry bindings partition this frontier only
/// when they select different source suppliers. Every target's contracts join
/// assembly, so target-specific nominations of one supplier must share it.
/// Complete binding records still partition Build execution and admission.
/// Supplier presence is only a source-routing observation: checkpoint reuse
/// still validates live custody, and source sealing rechecks the consumed bytes
/// before issuing a checked product. This key cannot attest filesystem contents.
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// Complete inputs to an unsponsored build within one compilation's immutable
/// source checkpoint. The build runs on its execution profile, never on the
/// product target. Equal keys permit sharing its retained result, not skipping
/// per-child custody admission.
///
/// A captured build gets private staging from the filesystem owner, so its
/// caller's later publication directory is not observable. Without a snapshot,
/// the build can write the caller's live output directory and cannot reuse a
/// sibling directory's effects. Externally sponsored/consent-bound activations
/// must execute independently; this key deliberately has no constructor for them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildExecutionInputs {
    build_execution_profile: Option<TargetProfile>,
    package: Option<PackageSourceContent>,
    accepted_semantic_bindings: Vec<AcceptedSemanticBinding>,
    independent_component_descriptions: Vec<IndependentComponentDescription>,
    snapshot: Option<BuildSnapshotRequest>,
    live_output_directory: Option<PathBuf>,
}

impl BuildExecutionInputs {
    pub fn for_unsponsored(
        packages: Option<&PackageCompilationInputs>,
        build_execution_profile: Option<TargetProfile>,
        snapshot: Option<&BuildSnapshotRequest>,
        output_directory: Option<&Path>,
    ) -> Self {
        Self {
            build_execution_profile,
            package: packages.map(PackageSourceContent::new),
            accepted_semantic_bindings: packages
                .into_iter()
                .flat_map(PackageCompilationInputs::accepted_semantic_bindings)
                .cloned()
                .collect(),
            independent_component_descriptions: packages
                .into_iter()
                .flat_map(PackageCompilationInputs::independent_component_descriptions)
                .cloned()
                .collect(),
            snapshot: snapshot.cloned(),
            live_output_directory: output_directory
                .filter(|_| snapshot.is_none())
                .map(Path::to_owned),
        }
    }
}

/// The shared source projection used by both boundaries. Generated bundles
/// retain only the scope, package and bytes assembly consumes. Their producing
/// target, dependency closure and source commitment remain admission evidence
/// on PackageCompilationInputs, checked separately for each child.
#[derive(Debug, Clone, PartialEq, Eq)]
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
                .dependency_generated_source_instances()
                .map(|(scope, bundle)| GeneratedSourceContent {
                    scope,
                    package: bundle.package(),
                    sources: bundle.sources().to_vec(),
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GeneratedSourceContent {
    scope: DependencyPurpose,
    package: PackageKeyIdentity,
    sources: Vec<PackageGeneratedSource>,
}
