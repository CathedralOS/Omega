use super::SourceStorage;
use diagnostics::Diagnostic;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRoots {
    pub sources: Vec<std::path::PathBuf>,
    pub build: Option<std::path::PathBuf>,
}

pub fn project_roots(root_path: &std::path::Path) -> ProjectRoots {
    if root_path.is_dir() {
        return directory_project_roots(root_path);
    }
    let mut roots = vec![root_path.to_path_buf()];
    let mut build = (root_path.file_name().and_then(|name| name.to_str()) == Some("build.omg"))
        .then(|| root_path.to_path_buf());
    let Some(parent) = root_path.parent() else {
        return ProjectRoots {
            sources: roots,
            build,
        };
    };

    for companion_name in companion_root_names(root_path.file_name().and_then(|name| name.to_str()))
    {
        let companion = parent.join(companion_name);
        if companion != root_path && companion.is_file() {
            if *companion_name == "build.omg" {
                build = Some(companion.clone());
            }
            roots.push(companion);
        }
    }

    ProjectRoots {
        sources: roots,
        build,
    }
}

/// A directory names its declared project sources directly: the directory
/// itself is not a source file, so seeding it would load a directory as
/// source text. A directory holding neither root still names itself — the
/// loader's directory diagnostic stays honest rather than silently skipping.
fn directory_project_roots(directory: &std::path::Path) -> ProjectRoots {
    let mut roots = Vec::new();
    let mut build = None;
    for name in ["build.omg", "main.omg"] {
        let candidate = directory.join(name);
        if candidate.is_file() {
            if name == "build.omg" {
                build = Some(candidate.clone());
            }
            roots.push(candidate);
        }
    }
    if roots.is_empty() {
        roots.push(directory.to_path_buf());
    }
    ProjectRoots {
        sources: roots,
        build,
    }
}

fn companion_root_names(root_name: Option<&str>) -> &'static [&'static str] {
    match root_name {
        Some("main.omg") => &["build.omg"],
        Some("build.omg") => &["main.omg"],
        _ => &["build.omg"],
    }
}

/// Require the exact selected free build root to declare its project role
/// through the same compiler-neutral grammar used by package orchestration.
/// An application declaration is retained whole for admission and
/// publication: its one authored name supplies the `.app` basename and
/// executable leaf, and its `artifact_only` modifier narrows the admitted
/// route (wiki/spec/build/macos_application.md).
pub fn validate_selected_build_role(
    source_storage: &SourceStorage,
    build_source_id: Option<source::SourceId>,
) -> Result<Option<build_declarations::ApplicationDeclaration>, Vec<Diagnostic>> {
    let Some(build_source_id) = build_source_id else {
        return Ok(None);
    };
    let source = source_storage.sources.get(build_source_id).ok_or_else(|| {
        vec![Diagnostic::error(
            "selected build source has no retained source text",
        )]
    })?;
    build_declarations::project_build_declaration_from_source(&source.source)
        .map(|declaration| match declaration {
            build_declarations::BuildDeclaration::Application(application) => Some(application),
            _ => None,
        })
        .map_err(|error| {
            vec![Diagnostic::error(format!(
                "{}: invalid project build declaration: {error}",
                source.path.display()
            ))]
        })
}

/// Exact project-root scopes and the selected canonical Build source.
/// Loading and parsing consume the scoped paths; only this selection rejoins
/// the resulting source id, so a Product instance cannot impersonate Build.
pub struct ProjectSourceSelection {
    sources: Vec<(std::path::PathBuf, source::DependencyScope)>,
    build_path: Option<std::path::PathBuf>,
}

impl ProjectSourceSelection {
    pub fn prepare(root_path: &std::path::Path) -> Result<Self, Vec<Diagnostic>> {
        let roots = project_roots(root_path);
        let build_path = roots
            .build
            .as_deref()
            .map(|path| {
                path.canonicalize().map_err(|error| {
                    vec![Diagnostic::error(format!(
                        "failed to establish exact build source {}: {error}",
                        path.display()
                    ))]
                })
            })
            .transpose()?;
        let sources = roots
            .sources
            .into_iter()
            .map(|path| {
                let canonical = path.canonicalize().ok();
                let scope = if canonical.is_some() && canonical == build_path {
                    source::DependencyScope::Build
                } else {
                    source::DependencyScope::Product
                };
                (path, scope)
            })
            .collect();
        Ok(Self {
            sources,
            build_path,
        })
    }

    pub fn sources(&self) -> &[(std::path::PathBuf, source::DependencyScope)] {
        &self.sources
    }

    pub fn build_source_id(
        &self,
        storage: &SourceStorage,
    ) -> Result<Option<source::SourceId>, Vec<Diagnostic>> {
        self.build_path
            .as_deref()
            .map(|selected| {
                storage
                    .files
                    .iter()
                    .find(|(_, file)| {
                        file.path
                            .canonicalize()
                            .is_ok_and(|loaded| loaded == selected)
                            && storage.sources.get(file.source_id).is_some_and(|source| {
                                source.dependency_scope == source::DependencyScope::Build
                            })
                    })
                    .map(|(_, file)| file.source_id)
                    .ok_or_else(|| {
                        vec![Diagnostic::error(format!(
                            "selected build source {} disappeared from the loaded frontier",
                            selected.display()
                        ))]
                    })
            })
            .transpose()
    }
}
