//! Project package selection and target facts before source closure begins.
use super::EntryContractSource;
use crate::PackageCompilationInputs;
use source_compiler::product::source_targets::{
    SourceEntryContract, SourceEntryContractOwner, SourceTarget,
};

pub fn source_target(
    profile: target::TargetProfile,
    packages: Option<&PackageCompilationInputs>,
) -> SourceTarget {
    let slot = profile.program_entry_slot();
    SourceTarget {
        name: profile.target_name(),
        entry: slot.source_contract(),
        boundary_schema: slot.boundary_schema,
        entry_contract: slot
            .physical_contract_package
            .map(|contract| SourceEntryContract {
                manifest_identity: contract.manifest_identity(),
                relative_source: contract.package_relative_source(),
                owner: match EntryContractSource::select(contract, packages) {
                    EntryContractSource::Bundled => SourceEntryContractOwner::Bundled,
                    EntryContractSource::Package(owner) => SourceEntryContractOwner::Package(owner),
                    EntryContractSource::Unavailable => SourceEntryContractOwner::Unavailable,
                },
                accepted_declaration: packages
                    .and_then(|inputs| {
                        inputs.accepted_semantic_binding(
                            crate::program_entry_semantic_binding_role(contract),
                        )
                    })
                    .map(|binding| binding.declaration_path().to_owned()),
            }),
        layout: profile.layout_target_facts(),
        address_model: target::TargetSemantics::close_address_model(profile).map(|model| {
            semantic_vocabulary::ScalarAddressModel {
                target: profile.target_name().to_owned(),
                carrier: model.carrier(),
                semantics_version: model.semantics_version(),
                commitment: model.commitment(),
            }
        }),
        numeric_support: profile.numeric_source_capabilities(),
    }
}

pub fn source_targets(packages: Option<&PackageCompilationInputs>) -> Vec<SourceTarget> {
    target::TargetProfile::ALL
        .into_iter()
        .map(|profile| source_target(profile, packages))
        .collect()
}
