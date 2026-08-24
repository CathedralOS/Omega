use psi_checked_trees::CheckedTrees;
use psi_diagnostics::Diagnostic;
use psi_source::{SourceFile, SourceOrigin};
use sha2::{Digest, Sha256};
use std::path::{Component, Path};

/// Compiler-owned commitment to the reconciled package graph and exact
/// authored/toolchain source bytes retained by one package-aware frontend run.
///
/// This binds review output to what the compiler consumed. It is not an
/// accepted package instance, a whole-compiler identity, or protection against
/// a hostile process that can race every filesystem observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackageSourceConsumptionCommitment {
    digest: [u8; 32],
}

impl PackageSourceConsumptionCommitment {
    pub const fn digest(self) -> [u8; 32] {
        self.digest
    }
}

pub(super) fn derive(
    program: &CheckedTrees,
    inputs: &super::PackageCompilationInputs,
) -> Result<PackageSourceConsumptionCommitment, Vec<Diagnostic>> {
    let mut entries = program
        .typed
        .symbols
        .source_files()
        .map(canonical_source_entry)
        .collect::<Result<Vec<_>, _>>()?;
    if entries.is_empty() {
        return Err(vec![Diagnostic::error(
            "package source consumption commitment requires retained frontend source metadata",
        )]);
    }
    entries.sort();

    let mut digest = Sha256::new();
    digest.update(b"OMEGA-PACKAGE-SOURCE-CONSUMPTION\0");
    hash_field(&mut digest, &inputs.root().digest());
    let packages = inputs
        .packages()
        .map(|(identity, _)| identity)
        .collect::<Vec<_>>();
    digest.update(
        u64::try_from(packages.len())
            .expect("compiler package count fits u64")
            .to_le_bytes(),
    );
    for package in packages {
        hash_field(&mut digest, &package.digest());
    }
    let dependencies = inputs.dependencies().collect::<Vec<_>>();
    digest.update(
        u64::try_from(dependencies.len())
            .expect("compiler dependency count fits u64")
            .to_le_bytes(),
    );
    for (requester, alias, target) in dependencies {
        hash_field(&mut digest, &requester.digest());
        hash_field(&mut digest, alias.as_bytes());
        hash_field(&mut digest, &target.digest());
    }
    digest.update(
        u64::try_from(entries.len())
            .expect("loaded source count fits u64")
            .to_le_bytes(),
    );
    for entry in entries {
        hash_field(&mut digest, &entry);
    }
    Ok(PackageSourceConsumptionCommitment {
        digest: digest.finalize().into(),
    })
}

pub(super) fn verify_current_files(program: &CheckedTrees) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    for source in program.typed.symbols.source_files() {
        if source.origin == SourceOrigin::Toolchain && is_virtual_toolchain_path(&source.path) {
            continue;
        }
        match std::fs::read(&source.path) {
            Ok(current) if current == source.source.as_bytes() => {}
            Ok(_) => diagnostics.push(Diagnostic::error(format!(
                "compiler-consumed source `{}` changed after frontend loading",
                source.path.display()
            ))),
            Err(error) => diagnostics.push(Diagnostic::error(format!(
                "compiler-consumed source `{}` cannot be re-read: {error}",
                source.path.display()
            ))),
        }
    }
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn canonical_source_entry(source: &SourceFile) -> Result<Vec<u8>, Vec<Diagnostic>> {
    let mut entry = Vec::new();
    match source.origin {
        SourceOrigin::User => {
            entry.push(0);
            let package = source.package_identity.ok_or_else(|| {
                vec![Diagnostic::error(format!(
                    "package-aware source `{}` has no reconciled package identity",
                    source.path.display()
                ))]
            })?;
            append_field(&mut entry, &package.digest());
            let relative = source
                .path
                .strip_prefix(&source.package_root)
                .map_err(|_| {
                    vec![Diagnostic::error(format!(
                        "package-aware source `{}` is outside its reconciled root `{}`",
                        source.path.display(),
                        source.package_root.display()
                    ))]
                })?;
            append_path(&mut entry, relative, &source.path)?;
        }
        SourceOrigin::Toolchain if is_virtual_toolchain_path(&source.path) => {
            entry.push(1);
            append_path(&mut entry, &source.path, &source.path)?;
        }
        SourceOrigin::Toolchain => {
            entry.push(2);
            let namespace = source
                .package_root
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| {
                    vec![Diagnostic::error(format!(
                        "toolchain source root `{}` has no canonical UTF-8 namespace",
                        source.package_root.display()
                    ))]
                })?;
            append_field(&mut entry, namespace.as_bytes());
            let relative = source
                .path
                .strip_prefix(&source.package_root)
                .map_err(|_| {
                    vec![Diagnostic::error(format!(
                        "toolchain source `{}` is outside its canonical root `{}`",
                        source.path.display(),
                        source.package_root.display()
                    ))]
                })?;
            append_path(&mut entry, relative, &source.path)?;
        }
    }
    append_field(&mut entry, source.source.as_bytes());
    Ok(entry)
}

fn append_path(
    output: &mut Vec<u8>,
    relative: &Path,
    diagnostic_path: &Path,
) -> Result<(), Vec<Diagnostic>> {
    let components = relative.components().collect::<Vec<_>>();
    if components.is_empty() {
        return Err(vec![Diagnostic::error(format!(
            "compiler-consumed source `{}` has an empty canonical path",
            diagnostic_path.display()
        ))]);
    }
    output.extend_from_slice(
        &u64::try_from(components.len())
            .expect("source path component count fits u64")
            .to_le_bytes(),
    );
    for component in components {
        let Component::Normal(component) = component else {
            return Err(vec![Diagnostic::error(format!(
                "compiler-consumed source `{}` has a non-canonical relative path",
                diagnostic_path.display()
            ))]);
        };
        let component = component.to_str().ok_or_else(|| {
            vec![Diagnostic::error(format!(
                "compiler-consumed source `{}` has a non-UTF-8 path component",
                diagnostic_path.display()
            ))]
        })?;
        append_field(output, component.as_bytes());
    }
    Ok(())
}

fn append_field(output: &mut Vec<u8>, bytes: &[u8]) {
    output.extend_from_slice(
        &u64::try_from(bytes.len())
            .expect("source commitment field length fits u64")
            .to_le_bytes(),
    );
    output.extend_from_slice(bytes);
}

fn hash_field(digest: &mut Sha256, bytes: &[u8]) {
    digest.update(
        u64::try_from(bytes.len())
            .expect("source commitment entry length fits u64")
            .to_le_bytes(),
    );
    digest.update(bytes);
}

fn is_virtual_toolchain_path(path: &Path) -> bool {
    let mut components = path.components();
    let Some(Component::Normal(component)) = components.next() else {
        return false;
    };
    components.next().is_none()
        && component
            .to_str()
            .is_some_and(|component| component.starts_with('<') && component.ends_with('>'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use psi_core::PackageKeyIdentity;
    use psi_source::SourceId;
    use std::path::PathBuf;
    use std::sync::Arc;

    fn source(
        path: &str,
        root: &str,
        package: Option<PackageKeyIdentity>,
        origin: SourceOrigin,
        text: &str,
    ) -> SourceFile {
        SourceFile {
            source_id: SourceId(0),
            path: PathBuf::from(path),
            package_root: PathBuf::from(root),
            package_identity: package,
            origin,
            source: Arc::from(text),
        }
    }

    #[test]
    fn canonical_entries_ignore_absolute_package_location() {
        let package = PackageKeyIdentity::from_digest([7; 32]).expect("package identity");
        let first = source(
            "/cache/one/pkg/main.omg",
            "/cache/one/pkg",
            Some(package),
            SourceOrigin::User,
            "machine main {}",
        );
        let second = source(
            "/different/cache/pkg/main.omg",
            "/different/cache/pkg",
            Some(package),
            SourceOrigin::User,
            "machine main {}",
        );
        assert_eq!(
            canonical_source_entry(&first).expect("first canonical entry"),
            canonical_source_entry(&second).expect("second canonical entry")
        );
    }

    #[test]
    fn canonical_entries_bind_package_path_and_bytes() {
        let first_package = PackageKeyIdentity::from_digest([7; 32]).expect("first package");
        let second_package = PackageKeyIdentity::from_digest([8; 32]).expect("second package");
        let baseline = source(
            "/cache/pkg/main.omg",
            "/cache/pkg",
            Some(first_package),
            SourceOrigin::User,
            "machine main {}",
        );
        let other_package = source(
            "/cache/pkg/main.omg",
            "/cache/pkg",
            Some(second_package),
            SourceOrigin::User,
            "machine main {}",
        );
        let other_path = source(
            "/cache/pkg/lib.omg",
            "/cache/pkg",
            Some(first_package),
            SourceOrigin::User,
            "machine main {}",
        );
        let other_bytes = source(
            "/cache/pkg/main.omg",
            "/cache/pkg",
            Some(first_package),
            SourceOrigin::User,
            "machine changed {}",
        );
        let baseline = canonical_source_entry(&baseline).expect("baseline entry");
        assert_ne!(baseline, canonical_source_entry(&other_package).unwrap());
        assert_ne!(baseline, canonical_source_entry(&other_path).unwrap());
        assert_ne!(baseline, canonical_source_entry(&other_bytes).unwrap());
    }
}
