//! Project package selection and target facts before source closure begins.
use source_compiler::compile_product::source_targets::{SourceEntryContract, SourceTarget};
use std::path::Path;

fn source_target(
    profile: target::TargetProfile,
    installed_root: &Path,
    package_owner: Option<semantic_vocabulary::PackageKeyIdentity>,
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
                package_owner,
                installed_root: installed_root.to_owned(),
                source_digest:
                    *target::entry_contract::exact_physical_contract_package_source_digest(contract)
                        .as_bytes(),
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

pub fn source_targets(
    packages: Option<&crate::PackageCompilationInputs>,
) -> Result<Vec<SourceTarget>, Vec<diagnostics::Diagnostic>> {
    let root = super::entry_contract_seed::entry_contract_root()?;
    let package_owner = if let Some(inputs) = packages {
        let mut mounted = inputs.packages().filter(|(_, path)| *path == root);
        match (mounted.next(), mounted.next()) {
            (None, None) => None,
            (Some((identity, _)), None)
                if inputs.package_name(identity)
                    == Some(super::entry_contract_seed::ENTRY_PACKAGE) =>
            {
                Some(identity)
            }
            _ => {
                return Err(vec![diagnostics::Diagnostic::error(
                    "installed entry contract package has a conflicting source owner",
                )]);
            }
        }
    } else {
        None
    };
    Ok(target::TargetProfile::ALL
        .into_iter()
        .map(|profile| source_target(profile, &root, package_owner))
        .collect())
}
