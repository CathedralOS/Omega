use crate::pipeline::source::SourceStorage;
use psi_diagnostics::Diagnostic;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ProjectRoots {
    pub(super) sources: Vec<std::path::PathBuf>,
    pub(super) build: Option<std::path::PathBuf>,
}

pub(super) fn project_roots(root_path: &std::path::Path) -> ProjectRoots {
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

pub(super) fn validate_selected_target(
    source_storage: &SourceStorage,
    selected_target_name: Option<&str>,
) -> Result<(), Vec<Diagnostic>> {
    let Some(target_name) = selected_target_name else {
        return Ok(());
    };

    let target_found = source_storage.files.iter().any(|(_, file)| {
        file.root_items.iter().any(|root_item| {
            let item = source_storage.syntax_trees.root_item(*root_item);
            matches!(
                item,
                psi_syntax_trees::item::Item::Target(target) if target.name.as_str() == target_name
            )
        })
    });

    if target_found {
        return Ok(());
    }

    Err(vec![Diagnostic::error(format!(
        "target `{target_name}` was not found in discovered source frontier"
    ))])
}
