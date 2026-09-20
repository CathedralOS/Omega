//! Hosted physical entry contract seeding. The selected profile's physical
//! entry contract must join compilation even when no authored `use` names it;
//! this sibling derives which contract source and supplier root seed the
//! import queue from the selected target profile and reconciled package
//! closure — semantic derivation, not sequencing.

use crate::source::SourceStorage;
use package_compilation::PackageCompilationInputs;
use std::path::PathBuf;

/// One seeded hosted physical entry contract: the contract file, the root its
/// authored imports close inside, and whether that root is a closed toolchain
/// subtree. The bundled fallback owns an entire closed subtree so its
/// transitive `use` closure keeps toolchain custody. A reconciled supplier's
/// contract and ordinary imports retain that package's custody instead.
pub(crate) struct HostedEntryContractSeed {
    pub(crate) source: PathBuf,
    pub(crate) root: PathBuf,
    pub(crate) closed_subtree: bool,
}

/// The selected profile's physical entry contract must join compilation even
/// when no authored `use` names it. A reconciled supplier remains an ordinary
/// package subject to exact consumer acceptance; only the bundled fallback is
/// closed toolchain-owned source. The
/// seed defers until after authored imports resolve so an explicitly imported
/// contract copy keeps its own package custody and accepted-binding
/// requirement. Freestanding (`ProgramStorageApplication`) profiles keep
/// authored-import semantics and are never seeded.
pub(super) fn hosted_entry_contract_seed(
    target_name: Option<&str>,
    package_inputs: Option<&PackageCompilationInputs>,
    source_storage: &SourceStorage,
) -> Option<HostedEntryContractSeed> {
    // Targetless checking owns no exact profile: `from_omega_target_name`
    // would resolve `None` to the host and silently seed platform content.
    let target_name = target_name?;
    let profile = target::TargetProfile::from_omega_target_name(Some(target_name)).ok()?;
    let slot = profile.program_entry_slot();
    if slot.schema != target::ProgramEntrySchema::HostedApplication {
        return None;
    }
    let contract_package = slot.physical_contract_package?;
    let relative_source = contract_package.package_relative_source();
    let already_loaded = source_storage
        .files
        .iter()
        .any(|(_, file)| file.path.ends_with(relative_source));
    if already_loaded {
        return None;
    }
    match package_inputs {
        // Standalone custody is the byte-exact bundled toolchain contract.
        None => {
            let contract_root = crate::frontend::bundled_omega_root().join("std");
            Some(HostedEntryContractSeed {
                source: contract_root.join(relative_source),
                root: contract_root,
                closed_subtree: true,
            })
        }
        // Package-aware compilation loads the contract from inside the
        // reconciled dependency closure, preserving its package identity and
        // source frontier. Its location grants no toolchain provenance or
        // consumer acceptance. A consumer's accepted
        // binding names the supplying package when more than one qualifies.
        // When no package in the closure carries the contract and no accepted
        // binding claims a supplier, the bundled toolchain copy is the same
        // closed target definition the slot vocabulary already names.
        Some(inputs) => {
            let accepted_role =
                build_evaluation::program_entry_semantic_binding_role(contract_package);
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
                .map(|(_, root)| HostedEntryContractSeed {
                    source: root.join(relative_source),
                    root: root.to_path_buf(),
                    closed_subtree: false,
                })
                .or_else(|| {
                    (suppliers.is_empty() && accepted_package.is_none()).then(|| {
                        let contract_root = crate::frontend::bundled_omega_root().join("std");
                        HostedEntryContractSeed {
                            source: contract_root.join(relative_source),
                            root: contract_root,
                            closed_subtree: true,
                        }
                    })
                })
        }
    }
}
