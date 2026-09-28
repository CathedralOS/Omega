use super::SourceStorage;
use diagnostics::Diagnostic;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRoots {
    pub sources: Vec<std::path::PathBuf>,
    pub build: Option<std::path::PathBuf>,
}

pub fn project_roots(root_path: &std::path::Path) -> ProjectRoots {
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
