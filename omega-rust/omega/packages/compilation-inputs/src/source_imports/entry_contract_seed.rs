//! Every target's program-entry contract source joins the assembled source
//! set, not only the selected target's: the set is target-neutral and the
//! other targets' target-scoped bodies are checked as siblings
//! (`wiki/spec/build/configuration.md#multi-target-compilation`). Which
//! contract realizes the program entry remains a per-target selection made
//! later, from the target catalog's slot.

use crate::PackageCompilationInputs;
use crate::source_imports::SourceStorage;
use semantic_vocabulary::PackageKeyIdentity;
use std::path::PathBuf;

pub(crate) struct EntryContractSeed {
    source: PathBuf,
    root: PathBuf,
    closed_subtree: bool,
}

impl EntryContractSeed {
    /// Register only bundled fallback custody, then hand the selected source
    /// to the loading stage. Ordinary suppliers keep their package identity.
    pub(crate) fn register_custody(self, storage: &mut SourceStorage) -> PathBuf {
        if self.closed_subtree {
            storage.register_toolchain_contract_root(self.source.clone(), self.root.clone());
            storage.register_toolchain_contract_dir(self.root);
        }
        self.source
    }
}

/// One seed per catalogued profile that owns a physical entry contract
/// package, skipping any contract source the storage already holds.
pub(crate) fn entry_contract_seeds(
    package_inputs: Option<&PackageCompilationInputs>,
    source_storage: &SourceStorage,
) -> Result<Vec<EntryContractSeed>, Vec<diagnostics::Diagnostic>> {
    let bundled_root = crate::source_imports::bundled_omega_root()?.join("std");
    let mut seeds = Vec::new();
    for profile in target::TargetProfile::ALL {
        let slot = profile.program_entry_slot();
        let Some(contract_package) = slot.physical_contract_package else {
            continue;
        };
        let relative_source = contract_package.package_relative_source();
        let already_loaded = source_storage
            .sources
            .selected_files()
            .any(|file| file.path.ends_with(relative_source));
        if already_loaded
            || seeds
                .iter()
                .any(|seed: &EntryContractSeed| seed.source.ends_with(relative_source))
        {
            continue;
        }
        if let Some(seed) = contract_seed(
            contract_package,
            relative_source,
            package_inputs,
            &bundled_root,
        ) {
            seeds.push(seed);
        }
    }
    Ok(seeds)
}

/// The source choice a binding can affect, separate from its later authority.
/// With one supplier, nominating another target or changing a schema record
/// does not change the source forest. Multiple suppliers still require the
/// exact accepted package; an unavailable selection must not reuse a fallback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EntryContractSource {
    Bundled,
    Package(PackageKeyIdentity),
    Unavailable,
}

impl EntryContractSource {
    pub(crate) fn select(
        contract: target::ProgramEntryPhysicalContractPackage,
        package_inputs: Option<&PackageCompilationInputs>,
    ) -> Self {
        let Some(inputs) = package_inputs else {
            return Self::Bundled;
        };
        let accepted_package = inputs
            .accepted_semantic_binding(crate::program_entry_semantic_binding_role(contract))
            .map(|binding| binding.package());
        let filesystem = inputs.source_filesystem();
        let mut suppliers = inputs
            .packages()
            .filter(|(_, root)| filesystem.is_file(&root.join(contract.package_relative_source())))
            .map(|(identity, _)| identity)
            .collect::<Vec<_>>();
        suppliers.sort();
        match suppliers.as_slice() {
            [single] => Self::Package(*single),
            [] if accepted_package.is_none() => Self::Bundled,
            _ => accepted_package
                .filter(|accepted| suppliers.contains(accepted))
                .map_or(Self::Unavailable, Self::Package),
        }
    }
}

fn contract_seed(
    contract_package: target::ProgramEntryPhysicalContractPackage,
    relative_source: &str,
    package_inputs: Option<&PackageCompilationInputs>,
    bundled_root: &std::path::Path,
) -> Option<EntryContractSeed> {
    let (root, closed_subtree) = match EntryContractSource::select(contract_package, package_inputs)
    {
        EntryContractSource::Bundled => (bundled_root, true),
        EntryContractSource::Package(package) => (package_inputs?.package_root(package)?, false),
        EntryContractSource::Unavailable => return None,
    };
    Some(EntryContractSeed {
        source: root.join(relative_source),
        root: root.to_path_buf(),
        closed_subtree,
    })
}
