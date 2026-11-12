// Pure source-level planning for `build.omg` dependency additions and replacements.
// Both planners operate on command-owned source strings without reading or writing the filesystem, binding any automatic edit or manual patch to the exact input SHA-256 digest and re-projecting the generated source to verify that the edit produced the exact expected row sequence.

use crate::declarations::dependencies::edit::layout::{BuildLayout, discover_build_layout};
use crate::declarations::dependencies::edit::rendering::{
    canonical_dependency_statement, source_digest,
};
use crate::declarations::dependencies::edit::{
    BuildDependencyEditError, BuildDependencyEditPlan, BuildDependencyManualPatch,
    BuildDependencyManualReason, BuildFileReplacement,
};
use crate::declarations::dependencies::read::{
    DependencyPurpose, DependencySourceRequest, extract_scoped_from_source,
};
use std::path::PathBuf;

pub(crate) fn plan_dependency_addition_from_source(
    build_path: PathBuf,
    source: String,
    request: &DependencySourceRequest,
) -> Result<BuildDependencyEditPlan, BuildDependencyEditError> {
    let requests =
        extract_scoped_from_source(&source).map_err(BuildDependencyEditError::InvalidBuild)?;
    if requests
        .iter()
        .any(|(purpose, existing)| purpose.is_product() && existing == request)
    {
        return Ok(BuildDependencyEditPlan::Unchanged);
    }
    let digest = source_digest(&source);
    let statement = canonical_dependency_statement(DependencyPurpose::Product, request, &source);
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
        (DependencyPurpose::Product, request.clone()),
        statement,
    )
}

fn validated_automatic_addition(
    build_path: PathBuf,
    digest: [u8; 32],
    replacement: String,
    mut expected: Vec<(DependencyPurpose, DependencySourceRequest)>,
    request: (DependencyPurpose, DependencySourceRequest),
    statement: String,
) -> Result<BuildDependencyEditPlan, BuildDependencyEditError> {
    expected.push(request);
    if extract_scoped_from_source(&replacement).ok().as_ref() != Some(&expected) {
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

pub(crate) fn plan_dependency_replacement_from_source(
    build_path: PathBuf,
    source: String,
    purpose: DependencyPurpose,
    accepted: &DependencySourceRequest,
    candidate: &DependencySourceRequest,
) -> Result<BuildDependencyEditPlan, BuildDependencyEditError> {
    let requests =
        extract_scoped_from_source(&source).map_err(BuildDependencyEditError::InvalidBuild)?;
    let scoped_contains = |request: &DependencySourceRequest| {
        requests
            .iter()
            .any(|(row_purpose, existing)| *row_purpose == purpose && existing == request)
    };
    if accepted == candidate {
        return Ok(if scoped_contains(candidate) {
            BuildDependencyEditPlan::Unchanged
        } else {
            manual_patch(
                build_path,
                source_digest(&source),
                BuildDependencyManualReason::AcceptedRequestMissing,
                Some(canonical_dependency_statement(purpose, accepted, &source)),
                canonical_dependency_statement(purpose, candidate, &source),
            )
        });
    }
    let digest = source_digest(&source);
    let current_statement = canonical_dependency_statement(purpose, accepted, &source);
    let proposed_statement = canonical_dependency_statement(purpose, candidate, &source);
    macro_rules! patch {
        ($reason:expr) => {
            manual_patch(
                build_path,
                digest,
                $reason,
                Some(current_statement),
                proposed_statement,
            )
        };
    }
    let accepted_indices = requests
        .iter()
        .enumerate()
        .filter_map(|(index, (row_purpose, request))| {
            (*row_purpose == purpose && request == accepted).then_some(index)
        })
        .collect::<Vec<_>>();
    let [accepted_index] = accepted_indices.as_slice() else {
        let reason = if accepted_indices.is_empty() {
            BuildDependencyManualReason::AcceptedRequestMissing
        } else {
            BuildDependencyManualReason::AcceptedRequestAmbiguous
        };
        return Ok(patch!(reason));
    };
    if scoped_contains(candidate) {
        return Ok(patch!(BuildDependencyManualReason::CandidateAlreadyPresent));
    }
    let Some(layout) = discover_build_layout(&source)? else {
        return Ok(patch!(
            BuildDependencyManualReason::NonCanonicalBuildBodyLayout
        ));
    };
    let Some(layout) = layout else {
        return Ok(patch!(
            BuildDependencyManualReason::NonCanonicalBuildSignature
        ));
    };
    if layout.dependency_rows().len() != requests.len() {
        return Ok(patch!(
            BuildDependencyManualReason::NonCanonicalDependencyRows
        ));
    }
    let row = &layout.dependency_rows()[*accepted_index];
    if row.contains_comment {
        return Ok(patch!(
            BuildDependencyManualReason::DependencyRowContainsComment
        ));
    }
    let replacement = BuildLayout::replace_row(&source, row, &proposed_statement);
    let mut expected = requests;
    expected[*accepted_index] = (purpose, candidate.clone());
    if extract_scoped_from_source(&replacement).ok().as_ref() != Some(&expected) {
        return Ok(patch!(BuildDependencyManualReason::GeneratedEditRejected));
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

#[cfg(test)]
mod tests {
    use super::{plan_dependency_addition_from_source, plan_dependency_replacement_from_source};
    use crate::declarations::AliasName;
    use crate::declarations::dependencies::edit::BuildDependencyEditPlan;
    use crate::declarations::dependencies::read::{
        DependencyPurpose, DependencySourceRequest, extract_scoped_from_source,
    };
    use std::path::PathBuf;

    const ROOT: &str = r#"machine build(builder: &mut Build) {
    Build::application(builder, "probe");
    let std_registration: DependencyRegistration = Build::dependency(builder, dependency::from_source(Source::Path { location: "../library/std" }));
    let mut kit_dependency: DependencyDescription = dependency::from_source(Source::Path { location: "kit" });
    dependency::with_alias(&mut kit_dependency, "kit");
    let kit_registration: DependencyRegistration = Build::build_dependency(builder, kit_dependency);
}
"#;

    fn path(alias: Option<&str>, location: &str) -> DependencySourceRequest {
        DependencySourceRequest::Path {
            explicit_alias: alias.map(|alias| AliasName::parse(alias).expect("alias")),
            location: location.to_owned(),
        }
    }

    fn replaced(plan: BuildDependencyEditPlan) -> String {
        let BuildDependencyEditPlan::Automatic(replacement) = plan else {
            panic!("expected an automatic edit, got {plan:?}")
        };
        replacement.replacement_source().to_owned()
    }

    #[test]
    fn both_registration_spellings_project_with_their_purpose_and_alias() {
        assert_eq!(
            extract_scoped_from_source(ROOT).expect("project"),
            [
                (DependencyPurpose::Product, path(None, "../library/std")),
                (DependencyPurpose::Build, path(Some("kit"), "kit")),
            ]
        );
    }

    #[test]
    fn an_added_aliased_dependency_is_the_three_statement_journey() {
        let edited = replaced(
            plan_dependency_addition_from_source(
                PathBuf::from("build.omg"),
                ROOT.to_owned(),
                &path(Some("other"), "../other"),
            )
            .expect("plan"),
        );
        assert!(
            edited.contains(
                "    let mut other_dependency: DependencyDescription = dependency::from_source(Source::Path { location: \"../other\" });\n    dependency::with_alias(&mut other_dependency, \"other\");\n    let other_registration: DependencyRegistration = Build::dependency(builder, other_dependency);\n}"
            ),
            "{edited}"
        );
        assert_eq!(
            extract_scoped_from_source(&edited)
                .expect("reproject")
                .len(),
            3
        );
    }

    #[test]
    fn an_added_unaliased_dependency_takes_an_unused_local_name() {
        let edited = replaced(
            plan_dependency_addition_from_source(
                PathBuf::from("build.omg"),
                ROOT.to_owned(),
                &path(None, "../vendor/std"),
            )
            .expect("plan"),
        );
        assert!(
            edited.contains("    let std_registration_2: DependencyRegistration = Build::dependency(builder, dependency::from_source(Source::Path { location: \"../vendor/std\" }));\n}"),
            "{edited}"
        );
    }

    #[test]
    fn a_replaced_aliased_row_keeps_its_place_and_indent() {
        let edited = replaced(
            plan_dependency_replacement_from_source(
                PathBuf::from("build.omg"),
                ROOT.to_owned(),
                DependencyPurpose::Build,
                &path(Some("kit"), "kit"),
                &path(Some("kit"), "../kit"),
            )
            .expect("plan"),
        );
        assert_eq!(
            extract_scoped_from_source(&edited).expect("reproject"),
            [
                (DependencyPurpose::Product, path(None, "../library/std")),
                (DependencyPurpose::Build, path(Some("kit"), "../kit")),
            ]
        );
        assert!(
            edited.contains("\n    dependency::with_alias(&mut kit_dependency_2, \"kit\");\n"),
            "{edited}"
        );
    }
}
