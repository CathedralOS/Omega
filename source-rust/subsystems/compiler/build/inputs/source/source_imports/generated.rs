//! Frozen generated files from the current activation and admitted dependency handoffs.
use diagnostics::Diagnostic;
use std::collections::HashSet;
use std::path::Path;

pub(crate) struct GeneratedFile<'source> {
    pub logical_path: std::path::PathBuf,
    pub source: &'source build_output::PackageGeneratedSource,
    pub stratum: source_admission::SourceResolutionStratum,
    pub package: semantic_vocabulary::PackageKeyIdentity,
    pub scope: source_provenance::DependencyScope,
}

impl<'source> GeneratedFile<'source> {
    pub fn authored_base(
        &self,
        package_inputs: &crate::PackageCompilationInputs,
        discovery: &mut impl super::LogicalSourceDiscovery,
    ) -> Result<
        Vec<(
            std::path::PathBuf,
            source_provenance::DependencyScope,
        )>,
        Vec<Diagnostic>,
    > {
        let filesystem = package_inputs.source_filesystem();
        let package_root = package_inputs
            .package_root(self.package)
            .expect("validated generated-source bundle retains its package root");
        let excluded = package_inputs
            .packages()
            .filter(|(owner, root)| *owner != self.package && root.starts_with(package_root))
            .map(|(_, root)| root.to_path_buf())
            .collect();
        discovery
            .select_owned(package_root, &[], filesystem, excluded)
            .map(|paths| paths.into_iter().map(|path| (path, self.scope)).collect())
    }

    pub fn source_text(&self) -> Result<&'source str, Vec<Diagnostic>> {
        std::str::from_utf8(self.source.bytes()).map_err(|_| {
            vec![Diagnostic::error(format!(
                "included generated source `{}` is not UTF-8 Omega source",
                self.logical_path.display(),
            ))]
        })
    }
}

pub(crate) fn collect_generated_files<'source>(
    package_inputs: &'source crate::PackageCompilationInputs,
    own_files: &'source [build_output::PackageGeneratedSource],
    source_scope: source_provenance::DependencyScope,
) -> Result<Vec<GeneratedFile<'source>>, Vec<Diagnostic>> {
    use source_admission::SourceResolutionStratum::{
        CurrentActivationExtension, DependencyActivationExtension,
    };
    use source_provenance::DependencyScope::{Build, Product};

    let filesystem = package_inputs.source_filesystem();
    let mut entries = Vec::new();
    let mut occupied = HashSet::new();
    let mut mount = |package,
                     scope,
                     stratum,
                     files: &'source [build_output::PackageGeneratedSource]|
     -> Result<(), Vec<Diagnostic>> {
        let root = package_inputs
            .package_root(package)
            .expect("admitted generated file owner");
        for source in files {
            let logical_path = generated_file_path(root, source)?;
            let filename = logical_path
                .strip_prefix(root.join(source_admission::GENERATED_SOURCE_DIRECTORY))
                .expect("generated coordinates are relative to their package");
            if super::files::captured_file(root, filename, filesystem)?.is_some()
                || filesystem.exists(&logical_path)
                || !occupied.insert((logical_path.clone(), scope))
            {
                return Err(vec![Diagnostic::error(format!(
                    "generated logical file `{}` collides with an existing source", filename.display(),
                ))]);
            }
            entries.push(GeneratedFile {
                logical_path,
                source,
                package,
                scope,
                stratum,
            });
        }
        Ok(())
    };
    mount(
        package_inputs.root(),
        Product,
        CurrentActivationExtension,
        own_files,
    )?;
    for (purpose, bundle) in package_inputs.dependency_build_handoff_instances() {
        let scope = match purpose {
            build_declarations::DependencyPurpose::Product => Product,
            build_declarations::DependencyPurpose::Build => Build,
        };
        if source_scope == Product || scope == Build {
            mount(
                bundle.package(),
                scope,
                DependencyActivationExtension,
                bundle.sources(),
            )?;
        }
    }
    Ok(entries)
}

pub(crate) fn generated_file_path(
    package_root: &Path,
    source: &build_output::PackageGeneratedSource,
) -> Result<std::path::PathBuf, Vec<Diagnostic>> {
    let filename = std::str::from_utf8(source.relative_path()).map_err(|_| {
        vec![Diagnostic::error(
            "generated source path is not canonical UTF-8",
        )]
    })?;
    Ok(package_root
        .join(source_admission::GENERATED_SOURCE_DIRECTORY)
        .join(filename))
}
