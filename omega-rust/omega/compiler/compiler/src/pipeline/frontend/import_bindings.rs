//! Join authored import paths to the exact parsed source frontier.

use super::*;

pub(in crate::pipeline) fn retain_module_import_bindings(
    storage: &SourceStorage,
    root_path: &Path,
    packages: Option<&PackageCompilationInputs>,
) -> Result<Vec<symbols::SourceScopedTopLevelBinding>, Vec<Diagnostic>> {
    let root_dir = root_path.parent().unwrap_or_else(|| Path::new("."));
    let mut bindings = Vec::new();
    for (_, requesting_source) in storage.files.iter() {
        for item in &requesting_source.root_items {
            let Item::Use(import) = storage.syntax_trees.root_item(*item) else {
                continue;
            };
            let members = storage
                .syntax_trees
                .items
                .identifier_path_members(import.path);
            let requester = storage
                .sources
                .get(requesting_source.source_id)
                .and_then(|source| source.package_identity);
            let (resolved, source_root, relative_path, package_prefix_members, exact_generated) =
                match packages {
                    Some(packages) => match reconciled_package_import(
                        &requesting_source.path,
                        members,
                        requester,
                        packages,
                    )? {
                        ReconciledPackageImport::Toolchain(resolved) => (
                            resolved,
                            bundled_omega_root(),
                            identifier_relative_path(&members[2..]),
                            2,
                            None,
                        ),
                        ReconciledPackageImport::Package(request) => {
                            let prefix_count =
                                members.len() - request.relative_path.components().count();
                            let exact_generated = packages
                                .generated_source_import_path(
                                    request.package,
                                    &source_path_candidates(&request.relative_path),
                                )
                                .map_err(|error| vec![Diagnostic::error(error)])?;
                            (
                                request.resolve_for_exact_target(packages)?,
                                request.expected_root,
                                request.relative_path,
                                prefix_count,
                                exact_generated,
                            )
                        }
                    },
                    None => {
                        let prefix_count = if is_bundled_omega_path(members) { 2 } else { 0 };
                        let source_root = if prefix_count == 2 {
                            bundled_omega_root()
                        } else {
                            standalone_source_root(root_dir, &requesting_source.path)
                        };
                        let relative_path = identifier_relative_path(&members[prefix_count..]);
                        (
                            normalize_path(&resolve_source_path(&source_root, members))?,
                            source_root,
                            relative_path,
                            prefix_count,
                            None,
                        )
                    }
                };
            let Some((_, declaration_source)) = storage
                .files
                .iter()
                .find(|(_, source)| source.path == resolved)
            else {
                return Err(vec![Diagnostic::error(format!(
                    "import `{}` no longer resolves to its parsed source frontier",
                    identifier_path_text(members),
                ))]);
            };
            let declares_module = declaration_source
                .root_items
                .iter()
                .any(|item| matches!(storage.syntax_trees.root_item(*item), Item::Module(_)));
            if !declares_module
                && exact_generated.as_ref() != Some(&resolved)
                && !source_path_candidates(&relative_path)
                    .into_iter()
                    .any(|path| {
                        let path = source_root.join(path);
                        path == resolved || path.canonicalize().is_ok_and(|path| path == resolved)
                    })
            {
                return Err(vec![Diagnostic::error(format!(
                    "import `{}` requires a declared module in {}",
                    identifier_path_text(members),
                    resolved.display(),
                ))]);
            }
            bindings.push(symbols::SourceScopedTopLevelBinding::module_import(
                requesting_source.source_id,
                declaration_source.source_id,
                &identifier_path_text(members),
                package_prefix_members,
            ));
        }
    }
    Ok(bindings)
}

fn identifier_relative_path(members: &[Identifier]) -> PathBuf {
    members.iter().fold(PathBuf::new(), |mut path, member| {
        path.push(member.as_str());
        path
    })
}
