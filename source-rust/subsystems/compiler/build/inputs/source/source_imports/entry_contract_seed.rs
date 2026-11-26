//! Admit the package installed for the target profiles before enqueuing their entry files.
use super::SourceStorage;
use diagnostics::Diagnostic;
use std::path::{Path, PathBuf};

pub(super) const ENTRY_PACKAGE: &str = "system";

pub(crate) fn entry_contract_root() -> Result<PathBuf, Vec<Diagnostic>> {
    Ok(super::bundled_omega_root()?.join(ENTRY_PACKAGE))
}

pub(crate) fn entry_contract_seeds(
    storage: &mut SourceStorage,
) -> Result<Vec<PathBuf>, Vec<Diagnostic>> {
    let root = register_contract_root(storage, &entry_contract_root()?)?;
    Ok(target::TargetProfile::ALL
        .into_iter()
        .filter_map(|profile| profile.program_entry_slot().physical_contract_package)
        .map(|package| root.join(package.package_relative_source()))
        .collect())
}

fn register_contract_root(
    storage: &mut SourceStorage,
    root: &Path,
) -> Result<PathBuf, Vec<Diagnostic>> {
    // A local dependency may already own the installation itself. Keep its reconciled
    // identity and check that its immutable source capture agrees with the installation.
    let package_owned = (storage.default_package_root() == root
        && storage.default_package_identity().is_some())
        || storage
            .package_roots
            .iter()
            .any(|package| package.root == root && package.identity.is_some());
    if package_owned {
        return storage.capture_installed_tree(root);
    }
    let root = storage.capture_toolchain_contract(root)?;
    storage.register_toolchain_contract_dir(root.clone());
    Ok(root)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_package() -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "omega-entry-contract-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir(&root).unwrap();
        root
    }

    #[test]
    fn package_owned_installation_must_retain_the_entire_designated_tree() {
        let root = temporary_package();
        let entry = root.join("entry.omg");
        std::fs::write(&entry, "public data Arrival {}").unwrap();
        let helper = root.join("helper.omg");
        std::fs::write(&helper, "public data Original {}").unwrap();
        let mut storage = SourceStorage::default();
        storage.register_reconciled_package_root(
            root.clone(),
            semantic_vocabulary::PackageKeyIdentity::from_digest([1; 32]).unwrap(),
        );
        assert_eq!(register_contract_root(&mut storage, &root).unwrap(), root);
        assert!(!storage.is_toolchain_contract_source(&entry));
        std::fs::write(&helper, "public data Changed {}").unwrap();
        assert!(register_contract_root(&mut storage, &root).is_err());
        assert_eq!(
            storage.filesystem.read_to_string(&helper).unwrap().as_ref(),
            "public data Original {}"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn repeated_entry_seeding_reuses_the_first_captured_namespace() {
        let root = temporary_package();
        let source = root.join("entry.omg");
        std::fs::write(&source, "public data Original {}").unwrap();
        let mut storage = SourceStorage::default();
        let seed = |storage: &mut SourceStorage| register_contract_root(storage, &root);
        let admitted = seed(&mut storage).unwrap().join("entry.omg");
        std::fs::write(&source, "public data Changed {}").unwrap();
        std::fs::write(root.join("late.omg"), "public data Late {}").unwrap();
        assert_eq!(seed(&mut storage).unwrap().join("entry.omg"), admitted);
        assert_eq!(
            storage
                .filesystem
                .read_to_string(&admitted)
                .unwrap()
                .as_ref(),
            "public data Original {}"
        );
        assert_eq!(
            storage
                .filesystem
                .captured_files(admitted.parent().unwrap())
                .unwrap(),
            vec![admitted]
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
