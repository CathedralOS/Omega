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
    DependencyPurpose, DependencySourceRequest, EditableSource, extract_scoped_from_source,
    project_source_for_edit,
};
use std::path::PathBuf;

pub(crate) fn plan_dependency_addition_from_source(
    build_path: PathBuf,
    source: String,
    request: &DependencySourceRequest,
) -> Result<BuildDependencyEditPlan, BuildDependencyEditError> {
    let EditableSource {
        parsed,
        entry,
        requests,
        import,
    } = project_source_for_edit(&source).map_err(BuildDependencyEditError::InvalidBuild)?;
    let receiver = entry.receiver_name(&parsed.syntax);
    if requests
        .iter()
        .any(|(purpose, existing)| purpose.is_product() && existing == request)
    {
        return Ok(BuildDependencyEditPlan::Unchanged);
    }
    let digest = source_digest(&source);
    let statement =
        canonical_dependency_statement(DependencyPurpose::Product, request, &source, &import.alias, receiver);
    let Some(layout) = discover_build_layout(&parsed, receiver) else {
        return Ok(manual_patch(
            build_path,
            digest,
            BuildDependencyManualReason::NonCanonicalBuildBodyLayout,
            None,
            import.manual_proposal(statement),
        ));
    };
    let Some(layout) = layout else {
        return Ok(manual_patch(
            build_path,
            digest,
            BuildDependencyManualReason::NonCanonicalBuildSignature,
            None,
            import.manual_proposal(statement),
        ));
    };
    let Some(replacement) = layout.insert_statement(&source, &statement) else {
        return Ok(manual_patch(
            build_path,
            digest,
            BuildDependencyManualReason::NonCanonicalBuildBodyLayout,
            None,
            import.manual_proposal(statement),
        ));
    };
    validated_automatic_addition(
        build_path,
        digest,
        import.include_in(replacement),
        requests,
        (DependencyPurpose::Product, request.clone()),
        import.manual_proposal(statement),
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
    let EditableSource {
        parsed,
        entry,
        requests,
        import,
    } = project_source_for_edit(&source).map_err(BuildDependencyEditError::InvalidBuild)?;
    let receiver = entry.receiver_name(&parsed.syntax);
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
                Some(canonical_dependency_statement(
                    purpose,
                    accepted,
                    &source,
                    &import.alias,
                    receiver,
                )),
                import.manual_proposal(canonical_dependency_statement(
                    purpose,
                    candidate,
                    &source,
                    &import.alias,
                    receiver,
                )),
            )
        });
    }
    let digest = source_digest(&source);
    let current_statement =
        canonical_dependency_statement(purpose, accepted, &source, &import.alias, receiver);
    let proposed_statement =
        canonical_dependency_statement(purpose, candidate, &source, &import.alias, receiver);
    let patch = |reason| {
        manual_patch(
            build_path.clone(),
            digest,
            reason,
            Some(current_statement),
            import.manual_proposal(proposed_statement.clone()),
        )
    };
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
        return Ok(patch(reason));
    };
    if scoped_contains(candidate) {
        return Ok(patch(BuildDependencyManualReason::CandidateAlreadyPresent));
    }
    let Some(layout) = discover_build_layout(&parsed, receiver) else {
        return Ok(patch(
            BuildDependencyManualReason::NonCanonicalBuildBodyLayout
        ));
    };
    let Some(layout) = layout else {
        return Ok(patch(
            BuildDependencyManualReason::NonCanonicalBuildSignature
        ));
    };
    if layout.dependency_rows().len() != requests.len() {
        return Ok(patch(
            BuildDependencyManualReason::NonCanonicalDependencyRows
        ));
    }
    let row = &layout.dependency_rows()[*accepted_index];
    if row.contains_comment {
        return Ok(patch(
            BuildDependencyManualReason::DependencyRowContainsComment
        ));
    }
    let replacement = BuildLayout::replace_row(&source, row, &proposed_statement);
    let mut expected = requests;
    expected[*accepted_index] = (purpose, candidate.clone());
    if extract_scoped_from_source(&replacement).ok().as_ref() != Some(&expected) {
        return Ok(patch(BuildDependencyManualReason::GeneratedEditRejected));
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

    const ROOT: &str = r#"use {omega}/build/dependency.omg;
machine build(builder: @mutable Build) {
    Build::application(builder, "probe");
    let system_registration: DependencyRegistration = Build::dependency(builder, |dependency|::from_source(Source::Path { location: "../library/system" }));
    let mutable kit_dependency: DependencyDescription = |dependency|::from_source(Source::Path { location: "kit" });
    |dependency|::with_alias(@mutable kit_dependency, "kit");
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
    fn add_and_replace_use_the_parameter_bound_by_the_entry() {
        let initial = "machine build(plan: @mutable Build) {\n    Build::package(plan, \"client\");\n}\n";
        let first = path(Some("selected"), "../first");
        let second = path(Some("selected"), "../second");
        let file = PathBuf::from("build.omg");
        let BuildDependencyEditPlan::Automatic(added) =
            plan_dependency_addition_from_source(file.clone(), initial.to_owned(), &first).unwrap()
        else {
            panic!("expected an edit for a renamed Build parameter");
        };
        let added = added.replacement_source();
        assert!(added.contains("Build::dependency(plan, selected_dependency)"));
        let BuildDependencyEditPlan::Automatic(replaced) = plan_dependency_replacement_from_source(
            file, added.to_owned(), DependencyPurpose::Product, &first, &second,
        ).unwrap() else {
            panic!("expected an update for the renamed Build parameter");
        };
        let updated = replaced.replacement_source();
        assert_eq!(extract_scoped_from_source(updated).unwrap(), [(DependencyPurpose::Product, second)]);
        let wrong_receiver = updated.replace("Build::dependency(plan,", "Build::dependency(builder,");
        assert!(extract_scoped_from_source(&wrong_receiver).is_err());
    }

    #[test]
    fn adds_the_installed_file_once_and_reuses_its_actual_alias() {
        let plain = "machine build(builder: @mutable Build) {\n    Build::application(builder, \"probe\");\n}\n";
        for (imports, expected, inserted) in [
            ("", "dependency", true),
            ("use {vendor}/build/dependency.omg;\n", "dependency_2", true),
            ("use {omega}/build/dependency.omg as pkg;\n", "pkg", false),
        ] {
            let original = format!("{imports}{plain}").replace('\n', "\r\n");
            let request = path(Some("kit"), "../kit");
            let edit = plan_dependency_addition_from_source(
                "build.omg".into(),
                original.clone(),
                &request,
            )
            .unwrap();
            let BuildDependencyEditPlan::Automatic(edit) = edit else {
                panic!("missing edit for {original}")
            };
            let source = edit.replacement_source();
            assert_eq!(source.matches("{omega}/build/dependency.omg").count(), 1);
            assert_eq!(
                source.matches("use ").count(),
                imports.lines().count() + usize::from(inserted)
            );
            assert!(source.contains(&format!("|{expected}|::from_source(")));
            assert!(source.contains(&format!("|{expected}|::with_alias(")));
            assert!(!source.replace("\r\n", "").contains('\n'));
            assert_eq!(
                extract_scoped_from_source(source).unwrap(),
                [(DependencyPurpose::Product, request.clone())]
            );
            let candidate = path(Some("kit"), "../next");
            let change = plan_dependency_replacement_from_source(
                "build.omg".into(),
                source.to_owned(),
                DependencyPurpose::Product,
                &request,
                &candidate,
            )
            .unwrap();
            let BuildDependencyEditPlan::Automatic(change) = change else {
                panic!("missing update")
            };
            assert_eq!(
                extract_scoped_from_source(change.replacement_source()).unwrap(),
                [(DependencyPurpose::Product, candidate)]
            );
            let spoof = source.replace(
                "{omega}/build/dependency.omg",
                "{vendor}/wrong/dependency.omg",
            );
            assert!(extract_scoped_from_source(&spoof).is_err());
        }
    }

    #[test]
    fn both_registration_spellings_project_with_their_purpose_and_alias() {
        assert_eq!(
            extract_scoped_from_source(ROOT).expect("project"),
            [
                (DependencyPurpose::Product, path(None, "../library/system")),
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
                "    let mutable other_dependency: DependencyDescription = |dependency|::from_source(Source::Path { location: \"../other\" });\n    |dependency|::with_alias(@mutable other_dependency, \"other\");\n    let other_registration: DependencyRegistration = Build::dependency(builder, other_dependency);\n}"
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
                &path(None, "../vendor/system"),
            )
            .expect("plan"),
        );
        assert!(
            edited.contains("    let system_registration_2: DependencyRegistration = Build::dependency(builder, |dependency|::from_source(Source::Path { location: \"../vendor/system\" }));\n}"),
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
                (DependencyPurpose::Product, path(None, "../library/system")),
                (DependencyPurpose::Build, path(Some("kit"), "../kit")),
            ]
        );
        assert!(
            edited
                .contains("\n    |dependency|::with_alias(@mutable kit_dependency_2, \"kit\");\n"),
            "{edited}"
        );
    }
}
