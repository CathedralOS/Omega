//! Open exact catalog declarations without charging unrelated repository payloads.

use crate::git::cache::repository::VerifiedGitRepository;
use crate::git::executable::executor::GitExecutor;
use crate::git::objects::{AuthenticatedGitTreeInspection, GitTreeEntry, GitTreeEntryKind};
use crate::git::workspace::{GitWorkspaceDeclaration, GitWorkspaceDeclarationLimits};
use crate::{LocalSourceLimits, SourceRelativePath, SourceResolveError};
use std::collections::BTreeSet;

pub(in crate::git::resolution) fn open_root(
    inspection: &AuthenticatedGitTreeInspection,
    executor: &GitExecutor,
    repository: &VerifiedGitRepository,
    limits: LocalSourceLimits,
    declaration_limits: GitWorkspaceDeclarationLimits,
) -> Result<GitWorkspaceDeclaration, SourceResolveError> {
    validate_workspace_declaration_limits(limits, declaration_limits)?;
    let root_path = b"build.omg".to_vec();
    let mut root_entries = inspection.open_regular_files(
        executor,
        repository,
        [root_path.clone()],
        LocalSourceLimits {
            max_entries: 1,
            max_bytes: declaration_limits
                .maximum_declaration_bytes()
                .min(limits.max_bytes),
            max_depth: limits.max_depth,
        },
    )?;
    let root_entry = root_entries
        .pop()
        .expect("one exact root declaration path yields one authenticated entry");
    let root_declaration = GitWorkspaceDeclaration::root(
        "build.omg".to_owned(),
        root_entry.oid.clone(),
        git_file_bytes(&root_entry)?.to_vec(),
    );
    Ok(root_declaration)
}

pub(in crate::git::resolution) fn open_members(
    inspection: &AuthenticatedGitTreeInspection,
    executor: &GitExecutor,
    repository: &VerifiedGitRepository,
    root_declaration: &GitWorkspaceDeclaration,
    member_paths: &[SourceRelativePath],
    limits: LocalSourceLimits,
    declaration_limits: GitWorkspaceDeclarationLimits,
) -> Result<Vec<GitWorkspaceDeclaration>, SourceResolveError> {
    validate_workspace_declaration_limits(limits, declaration_limits)?;
    let maximum_members = declaration_limits.maximum_members().min(limits.max_entries);
    let maximum_declaration_bytes = declaration_limits
        .maximum_declaration_bytes()
        .min(limits.max_bytes);
    let maximum_total_declaration_bytes = declaration_limits
        .maximum_total_declaration_bytes()
        .min(limits.max_bytes);
    if member_paths.len() > maximum_members {
        return Err(SourceResolveError::TooManyFiles {
            limit: maximum_members,
        });
    }

    let mut unique_members = BTreeSet::new();
    let mut member_declaration_paths = Vec::with_capacity(member_paths.len());
    for member_path in member_paths {
        if !unique_members.insert(member_path.clone()) {
            return Err(SourceResolveError::GitTreeInvalid {
                path: member_path.as_str().as_bytes().to_vec(),
                message: "workspace planner returned one member path more than once".to_owned(),
            });
        }
        member_declaration_paths.push(format!("{}/build.omg", member_path.as_str()).into_bytes());
    }

    let root_bytes = u64::try_from(root_declaration.bytes().len()).map_err(|_| {
        SourceResolveError::TooManyBytes {
            limit: maximum_total_declaration_bytes,
        }
    })?;
    let remaining_declaration_bytes = maximum_total_declaration_bytes
        .checked_sub(root_bytes)
        .ok_or(SourceResolveError::TooManyBytes {
            limit: maximum_total_declaration_bytes,
        })?;
    let member_entries = inspection.open_regular_files(
        executor,
        repository,
        member_declaration_paths.clone(),
        LocalSourceLimits {
            max_entries: maximum_members,
            max_bytes: remaining_declaration_bytes,
            max_depth: limits.max_depth,
        },
    )?;
    let mut member_declarations = Vec::with_capacity(member_entries.len());
    for ((member_path, repository_path), entry) in member_paths
        .iter()
        .cloned()
        .zip(member_declaration_paths.iter())
        .zip(member_entries)
    {
        if entry.size > maximum_declaration_bytes {
            return Err(SourceResolveError::TooManyBytes {
                limit: maximum_declaration_bytes,
            });
        }
        let repository_path = std::str::from_utf8(repository_path)
            .expect("validated workspace member paths are UTF-8")
            .to_owned();
        member_declarations.push(GitWorkspaceDeclaration::member(
            member_path,
            repository_path,
            entry.oid.clone(),
            git_file_bytes(&entry)?.to_vec(),
        ));
    }

    Ok(member_declarations)
}

fn validate_workspace_declaration_limits(
    source_limits: LocalSourceLimits,
    declaration_limits: GitWorkspaceDeclarationLimits,
) -> Result<(), SourceResolveError> {
    if source_limits.max_entries == 0
        || source_limits.max_bytes == 0
        || declaration_limits.maximum_members() == 0
        || declaration_limits.maximum_declaration_bytes() == 0
        || declaration_limits.maximum_total_declaration_bytes()
            < declaration_limits.maximum_declaration_bytes()
    {
        return Err(SourceResolveError::GitExecutionBoundaryInvalid {
            message: "workspace declaration and source limits must be nonzero and coherent"
                .to_owned(),
        });
    }
    Ok(())
}

fn git_file_bytes(entry: &GitTreeEntry) -> Result<&[u8], SourceResolveError> {
    match &entry.kind {
        GitTreeEntryKind::File { bytes, .. } => Ok(bytes.as_slice()),
        _ => Err(SourceResolveError::GitTreeInvalid {
            path: entry.relative_bytes.clone(),
            message: "authenticated declaration is not a regular file".to_owned(),
        }),
    }
}
