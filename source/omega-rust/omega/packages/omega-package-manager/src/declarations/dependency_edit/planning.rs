use crate::declarations::dependency_edit::BUILD_FILE_NAME;
use crate::declarations::dependency_edit::layout::discover_build_layout;
use crate::declarations::dependency_edit::model::{
    BuildDependencyEditError, BuildDependencyEditPlan, BuildDependencyManualPatch,
    BuildDependencyManualReason, BuildFileReplacement,
};
use crate::declarations::dependency_edit::rendering::{
    canonical_dependency_statement, source_digest,
};
use crate::declarations::dependency_projection::{DependencySourceRequest, extract_from_source};
use std::fs;
use std::path::{Path, PathBuf};

pub fn plan_dependency_addition(
    package_root: impl AsRef<Path>,
    request: &DependencySourceRequest,
) -> Result<BuildDependencyEditPlan, BuildDependencyEditError> {
    let (build_path, source) = read_build_source(package_root.as_ref())?;
    plan_addition_from_source(build_path, source, request)
}

pub fn plan_dependency_replacement(
    package_root: impl AsRef<Path>,
    accepted: &DependencySourceRequest,
    candidate: &DependencySourceRequest,
) -> Result<BuildDependencyEditPlan, BuildDependencyEditError> {
    let (build_path, source) = read_build_source(package_root.as_ref())?;
    plan_replacement_from_source(build_path, source, accepted, candidate)
}

fn read_build_source(package_root: &Path) -> Result<(PathBuf, String), BuildDependencyEditError> {
    let build_path = package_root.join(BUILD_FILE_NAME);
    let bytes = fs::read(&build_path).map_err(|error| BuildDependencyEditError::ReadBuildFile {
        path: build_path.clone(),
        message: error.to_string(),
    })?;
    let source = String::from_utf8(bytes).map_err(|_| {
        BuildDependencyEditError::InvalidBuildFileEncoding {
            path: build_path.clone(),
        }
    })?;
    Ok((build_path, source))
}

pub(super) fn plan_addition_from_source(
    build_path: PathBuf,
    source: String,
    request: &DependencySourceRequest,
) -> Result<BuildDependencyEditPlan, BuildDependencyEditError> {
    let requests = extract_from_source(&source).map_err(BuildDependencyEditError::InvalidBuild)?;
    if requests.contains(request) {
        return Ok(BuildDependencyEditPlan::Unchanged);
    }
    let digest = source_digest(&source);
    let statement = canonical_dependency_statement(request);
    let Some(layout) = discover_build_layout(&source)? else {
        return Ok(manual_patch(
            build_path,
            digest,
            BuildDependencyManualReason::NonCanonicalBuildBodyLayout,
            None,
            statement,
        ));
    };
    let Some(layout) = layout else {
        return Ok(manual_patch(
            build_path,
            digest,
            BuildDependencyManualReason::NonCanonicalBuildSignature,
            None,
            statement,
        ));
    };
    let Some(replacement) = layout.insert_statement(&source, &statement) else {
        return Ok(manual_patch(
            build_path,
            digest,
            BuildDependencyManualReason::NonCanonicalBuildBodyLayout,
            None,
            statement,
        ));
    };
    validated_automatic_addition(
        build_path,
        digest,
        replacement,
        requests,
        request,
        statement,
    )
}

fn validated_automatic_addition(
    build_path: PathBuf,
    digest: [u8; 32],
    replacement: String,
    mut expected: Vec<DependencySourceRequest>,
    request: &DependencySourceRequest,
    statement: String,
) -> Result<BuildDependencyEditPlan, BuildDependencyEditError> {
    expected.push(request.clone());
    if extract_from_source(&replacement).ok().as_ref() != Some(&expected) {
        return Ok(manual_patch(
            build_path,
            digest,
            BuildDependencyManualReason::GeneratedEditRejected,
            None,
            statement,
        ));
    }
    Ok(BuildDependencyEditPlan::Automatic(
        BuildFileReplacement::new(build_path, digest, replacement),
    ))
}

pub(super) fn plan_replacement_from_source(
    build_path: PathBuf,
    source: String,
    accepted: &DependencySourceRequest,
    candidate: &DependencySourceRequest,
) -> Result<BuildDependencyEditPlan, BuildDependencyEditError> {
    let requests = extract_from_source(&source).map_err(BuildDependencyEditError::InvalidBuild)?;
    if accepted == candidate {
        return Ok(if requests.contains(candidate) {
            BuildDependencyEditPlan::Unchanged
        } else {
            manual_patch(
                build_path,
                source_digest(&source),
                BuildDependencyManualReason::AcceptedRequestMissing,
                Some(canonical_dependency_statement(accepted)),
                canonical_dependency_statement(candidate),
            )
        });
    }
    let digest = source_digest(&source);
    let current_statement = canonical_dependency_statement(accepted);
    let proposed_statement = canonical_dependency_statement(candidate);
    let accepted_indices = requests
        .iter()
        .enumerate()
        .filter_map(|(index, request)| (request == accepted).then_some(index))
        .collect::<Vec<_>>();
    let [accepted_index] = accepted_indices.as_slice() else {
        let reason = if accepted_indices.is_empty() {
            BuildDependencyManualReason::AcceptedRequestMissing
        } else {
            BuildDependencyManualReason::AcceptedRequestAmbiguous
        };
        return Ok(manual_patch(
            build_path,
            digest,
            reason,
            Some(current_statement),
            proposed_statement,
        ));
    };
    if requests.iter().any(|request| request == candidate) {
        return Ok(manual_patch(
            build_path,
            digest,
            BuildDependencyManualReason::CandidateAlreadyPresent,
            Some(current_statement),
            proposed_statement,
        ));
    }
    let Some(layout) = discover_build_layout(&source)? else {
        return Ok(manual_patch(
            build_path,
            digest,
            BuildDependencyManualReason::NonCanonicalBuildBodyLayout,
            Some(current_statement),
            proposed_statement,
        ));
    };
    let Some(layout) = layout else {
        return Ok(manual_patch(
            build_path,
            digest,
            BuildDependencyManualReason::NonCanonicalBuildSignature,
            Some(current_statement),
            proposed_statement,
        ));
    };
    if layout.dependency_rows().len() != requests.len() {
        return Ok(manual_patch(
            build_path,
            digest,
            BuildDependencyManualReason::NonCanonicalDependencyRows,
            Some(current_statement),
            proposed_statement,
        ));
    }
    let row = &layout.dependency_rows()[*accepted_index];
    if row.contains_comment {
        return Ok(manual_patch(
            build_path,
            digest,
            BuildDependencyManualReason::DependencyRowContainsComment,
            Some(current_statement),
            proposed_statement,
        ));
    }
    let mut replacement = source.clone();
    replacement.replace_range(row.start..row.end, &proposed_statement);
    let mut expected = requests;
    expected[*accepted_index] = candidate.clone();
    if extract_from_source(&replacement).ok().as_ref() != Some(&expected) {
        return Ok(manual_patch(
            build_path,
            digest,
            BuildDependencyManualReason::GeneratedEditRejected,
            Some(current_statement),
            proposed_statement,
        ));
    }
    Ok(BuildDependencyEditPlan::Automatic(
        BuildFileReplacement::new(build_path, digest, replacement),
    ))
}

fn manual_patch(
    build_path: PathBuf,
    expected_sha256: [u8; 32],
    reason: BuildDependencyManualReason,
    current_statement: Option<String>,
    proposed_statement: String,
) -> BuildDependencyEditPlan {
    BuildDependencyEditPlan::Manual(BuildDependencyManualPatch::new(
        build_path,
        expected_sha256,
        reason,
        current_statement,
        proposed_statement,
    ))
}
