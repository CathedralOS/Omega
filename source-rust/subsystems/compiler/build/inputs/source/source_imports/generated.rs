//! Frozen generated files from the current activation and admitted dependency handoffs.
use diagnostics::Diagnostic;
use std::collections::HashSet;
use std::path::Path;

pub(crate) struct GeneratedFile<'source> {
    pub logical_path: std::path::PathBuf,
    pub source: &'source build_output::PackageGeneratedSource,
    pub stratum: source_admission::SourceResolutionStratum,
    pub scope: source_provenance::DependencyScope,
}

impl<'source> GeneratedFile<'source> {
    pub fn source_text(&self) -> Result<&'source str, Vec<Diagnostic>> {
        std::str::from_utf8(self.source.bytes()).map_err(|_| {
            vec![Diagnostic::error(format!(
                "included generated source `{}` is not UTF-8 Omega source",
                self.logical_path.display(),
            ))]
        })
    }
}

#[derive(Default)]
pub(crate) struct GeneratedFiles<'source> {
    pub entries: Vec<GeneratedFile<'source>>,
    occupied: HashSet<(std::path::PathBuf, source_provenance::DependencyScope)>,
}

impl<'source> GeneratedFiles<'source> {
    fn mount(
        &mut self,
        inputs: &crate::PackageCompilationInputs,
        package: semantic_vocabulary::PackageKeyIdentity,
        scope: source_provenance::DependencyScope,
        stratum: source_admission::SourceResolutionStratum,
        sources: &'source [build_output::PackageGeneratedSource],
    ) -> Result<(), Vec<Diagnostic>> {
        let root = inputs.package_root(package).ok_or_else(|| {
            vec![Diagnostic::error(
                "generated files have no admitted package root",
            )]
        })?;
        let filesystem = inputs.source_filesystem();
        for source in sources {
            let logical_path = generated_file_path(root, source)?;
            let filename = logical_path
                .strip_prefix(root.join(source_admission::GENERATED_SOURCE_DIRECTORY))
                .expect("generated coordinates are relative to their package");
            if super::files::captured_file(root, filename, filesystem)?.is_some()
                || filesystem.exists(&logical_path)
                || !self.occupied.insert((logical_path.clone(), scope))
            {
                return Err(vec![Diagnostic::error(format!(
                    "generated logical file `{}` collides with an existing source",
                    filename.display(),
                ))]);
            }
            self.entries.push(GeneratedFile {
                logical_path,
                source,
                scope,
                stratum,
            });
        }
        Ok(())
    }

    pub fn for_package(
        inputs: &'source crate::PackageCompilationInputs,
        own: &'source [build_output::PackageGeneratedSource],
        scope: source_provenance::DependencyScope,
    ) -> Result<Self, Vec<Diagnostic>> {
        use source_admission::SourceResolutionStratum::{
            CurrentActivationExtension, DependencyActivationExtension,
        };
        use source_provenance::DependencyScope::{Build, Product};

        let mut files = Self::default();
        files.mount(
            inputs,
            inputs.root(),
            Product,
            CurrentActivationExtension,
            own,
        )?;
        for (purpose, handoff) in inputs.dependency_build_handoff_instances() {
            let dependency_scope = match purpose {
                build_declarations::DependencyPurpose::Product => Product,
                build_declarations::DependencyPurpose::Build => Build,
            };
            if scope == Product || dependency_scope == Build {
                files.mount(
                    inputs,
                    handoff.package(),
                    dependency_scope,
                    DependencyActivationExtension,
                    handoff.sources(),
                )?;
            }
        }
        Ok(files)
    }

    pub fn for_build_libraries(
        inputs: &'source super::ProductQueryInputs,
    ) -> Result<Self, Vec<Diagnostic>> {
        let mut files = Self::default();
        let mut mounted = HashSet::new();
        for dependency in inputs.build_dependencies() {
            if let Some(handoff) = &dependency.generated
                && !handoff.sources().is_empty()
                && mounted.insert(dependency.inputs.root())
            {
                files.mount(
                    &dependency.inputs,
                    dependency.inputs.root(),
                    source_provenance::DependencyScope::Build,
                    source_admission::SourceResolutionStratum::DependencyActivationExtension,
                    handoff.sources(),
                )?;
            }
        }
        Ok(files)
    }
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
