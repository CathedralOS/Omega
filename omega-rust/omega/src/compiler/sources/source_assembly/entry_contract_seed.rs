//! Every target's program-entry contract source joins the assembled source
//! set, not only the selected target's: the set is target-neutral and the
//! other targets' target-scoped bodies are checked as siblings
//! (`wiki/spec/build/configuration.md#multi-target-compilation`). Which
//! contract realizes the program entry remains a per-target selection made
//! later, from the target catalog's slot.

use crate::compiler::sources::source::SourceStorage;
use crate::package_compilation::PackageCompilationInputs;
use std::path::PathBuf;

pub struct EntryContractSeed {
    pub source: PathBuf,
    pub root: PathBuf,
    pub closed_subtree: bool,
}

/// One seed per catalogued profile that owns a physical entry contract
/// package, skipping any contract source the storage already holds.
pub(super) fn entry_contract_seeds(
    package_inputs: Option<&PackageCompilationInputs>,
    source_storage: &SourceStorage,
) -> Vec<EntryContractSeed> {
    let mut seeds = Vec::new();
    for profile in target::TargetProfile::ALL {
        let slot = profile.program_entry_slot();
        let Some(contract_package) = slot.physical_contract_package else {
            continue;
        };
        let relative_source = contract_package.package_relative_source();
        let already_loaded = source_storage
            .files
            .iter()
            .any(|(_, file)| file.path.ends_with(relative_source));
        if already_loaded
            || seeds
                .iter()
                .any(|seed: &EntryContractSeed| seed.source.ends_with(relative_source))
        {
            continue;
        }
        if let Some(seed) = contract_seed(contract_package, relative_source, package_inputs) {
            seeds.push(seed);
        }
    }
    seeds
}

fn contract_seed(
    contract_package: target::ProgramEntryPhysicalContractPackage,
    relative_source: &str,
    package_inputs: Option<&PackageCompilationInputs>,
) -> Option<EntryContractSeed> {
    let bundled = || {
        let contract_root = crate::compiler::sources::frontend::bundled_omega_root().join("std");
        EntryContractSeed {
            source: contract_root.join(relative_source),
            root: contract_root,
            closed_subtree: true,
        }
    };
    let Some(inputs) = package_inputs else {
        return Some(bundled());
    };
    let accepted_role =
        crate::build_evaluation::program_entry_semantic_binding_role(contract_package);
    let accepted_package = inputs
        .accepted_semantic_binding(accepted_role)
        .map(|binding| binding.package());
    let mut suppliers = inputs
        .packages()
        .filter(|(_, root)| root.join(relative_source).is_file())
        .collect::<Vec<_>>();
    suppliers.sort_by_key(|(identity, _)| *identity);
    let supplier = match suppliers.as_slice() {
        [single] => Some(*single),
        _ => suppliers
            .iter()
            .find(|(identity, _)| Some(*identity) == accepted_package)
            .copied(),
    };
    supplier
        .map(|(_, root)| EntryContractSeed {
            source: root.join(relative_source),
            root: root.to_path_buf(),
            closed_subtree: false,
        })
        .or_else(|| (suppliers.is_empty() && accepted_package.is_none()).then(bundled))
}
