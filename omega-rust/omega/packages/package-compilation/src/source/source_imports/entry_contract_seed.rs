// Multi-target `ProgramEntry` contract source seeding (`entry_contract_seeds`, `EntryContractSource`).
//
// Every catalogued target profile's `ProgramEntry` contract source joins the assembled
// `Product` source set so the parsed and checked program stays target-neutral and
// checks all target-scoped bodies as siblings (`wiki/spec/build/configuration.md#multi-target-compilation`).
// When the package graph contains a single supplier for a contract package, we seed
// that package's source root; when no package supplies it and no accepted binding
// overrides it, we fall back to the bundled `std` subtree and register closed
// toolchain-contract custody (`register_custody`). When multiple suppliers exist,
// we require an accepted semantic binding to disambiguate.

use crate::PackageCompilationInputs;
use crate::source::source_imports::SourceStorage;
use semantic_vocabulary::PackageKeyIdentity;
use std::path::PathBuf;

pub(crate) struct EntryContractSeed {
    source: PathBuf,
    root: PathBuf,
    closed_subtree: bool,
}

impl EntryContractSeed {
    pub(crate) fn register_custody(self, storage: &mut SourceStorage) -> PathBuf {
        if self.closed_subtree {
            storage.register_toolchain_contract_root(self.source.clone(), self.root.clone());
            storage.register_toolchain_contract_dir(self.root);
        }
        self.source
    }
}

pub(crate) fn entry_contract_seeds(
    package_inputs: Option<&PackageCompilationInputs>,
    source_storage: &SourceStorage,
) -> Result<Vec<EntryContractSeed>, Vec<diagnostics::Diagnostic>> {
    entry_contract_seeds_from_graphs(
        package_inputs,
        &package_inputs.into_iter().collect::<Vec<_>>(),
        source_storage,
    )
}

pub(super) fn entry_contract_seeds_from_graphs(
    original: Option<&PackageCompilationInputs>,
    graphs: &[&PackageCompilationInputs],
    source_storage: &SourceStorage,
) -> Result<Vec<EntryContractSeed>, Vec<diagnostics::Diagnostic>> {
    let bundled_root = crate::source::source_imports::bundled_omega_root()?.join("std");
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
        let selected = EntryContractSource::select_from_graphs(contract_package, original, graphs);
        let (root, closed_subtree) = match selected {
            EntryContractSource::Bundled => (bundled_root.as_path(), true),
            EntryContractSource::Package(package) => {
                let Some(root) = graphs.iter().find_map(|graph| graph.package_root(package)) else {
                    continue;
                };
                (root, false)
            }
            EntryContractSource::Unavailable => continue,
        };
        seeds.push(EntryContractSeed {
            source: root.join(relative_source),
            root: root.to_path_buf(),
            closed_subtree,
        });
    }
    Ok(seeds)
}

#[derive(Debug, Clone, PartialEq, Eq, compiler_storage::Record)]
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
        Self::select_from_graphs(
            contract,
            package_inputs,
            &package_inputs.into_iter().collect::<Vec<_>>(),
        )
    }

    fn select_from_graphs(
        contract: target::ProgramEntryPhysicalContractPackage,
        original: Option<&PackageCompilationInputs>,
        graphs: &[&PackageCompilationInputs],
    ) -> Self {
        let accepted_package = original
            .and_then(|inputs| {
                inputs
                    .accepted_semantic_binding(crate::program_entry_semantic_binding_role(contract))
            })
            .map(|binding| binding.package());
        let mut suppliers = graphs
            .iter()
            .flat_map(|inputs| {
                inputs
                    .packages()
                    .filter(|(_, root)| {
                        inputs
                            .source_filesystem()
                            .is_file(&root.join(contract.package_relative_source()))
                    })
                    .map(|(identity, _)| identity)
            })
            .collect::<Vec<_>>();
        suppliers.sort();
        suppliers.dedup();
        match suppliers.as_slice() {
            [single] => Self::Package(*single),
            [] if accepted_package.is_none() => Self::Bundled,
            _ => accepted_package
                .filter(|accepted| suppliers.contains(accepted))
                .map_or(Self::Unavailable, Self::Package),
        }
    }
}
