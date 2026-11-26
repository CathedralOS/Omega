// Pure source-level planning for `build.omg` dependency additions and replacements.
// Both planners operate on command-owned source strings without reading or writing the filesystem, binding any automatic edit or manual patch to the exact input SHA-256 digest and re-projecting the generated source to verify that the edit produced the exact expected row sequence.

use crate::declarations::dependencies::edit::layout::discover_build_layout;
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
use crate::declarations::editable_build::DependencyImport;
use std::path::PathBuf;

pub(crate) fn plan_dependency_addition_from_source(
    build_path: PathBuf,
    source: String,
    request: &DependencySourceRequest,
) -> Result<BuildDependencyEditPlan, BuildDependencyEditError> {
    let EditableSource {
        parsed,
        entry,
        rows,
    } = project_source_for_edit(&source).map_err(BuildDependencyEditError::InvalidBuild)?;
    let requests: Vec<_> = rows
        .into_iter()
        .map(|row| (row.purpose, row.request))
        .collect();
    let receiver = entry.receiver_name(&parsed.syntax);
    let import = DependencyImport::for_edit(&parsed.syntax, request.explicit_alias().is_some());
    if requests
        .iter()
        .any(|(purpose, existing)| purpose.is_product() && existing == request)
    {
        return Ok(BuildDependencyEditPlan::Unchanged);
    }
    let digest = source_digest(&source);
    let statement = canonical_dependency_statement(
        DependencyPurpose::Product,
        request,
        &source,
        &import.alias,
        receiver,
    );
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

/// The explicit version-4 update operation changes one selected Git revision.
/// Use its parsed string operand so every other authored byte keeps its meaning.
pub(crate) fn plan_git_revision_from_source(
    build_path: PathBuf,
    source: String,
    purpose: DependencyPurpose,
    accepted: &DependencySourceRequest,
    revision: &str,
) -> Result<BuildDependencyEditPlan, BuildDependencyEditError> {
    let EditableSource { parsed, rows, .. } =
        project_source_for_edit(&source).map_err(BuildDependencyEditError::InvalidBuild)?;
    let mut candidate = accepted.clone();
    let DependencySourceRequest::Git {
        revision: requested,
        repository,
        explicit_alias,
        ..
    } = &mut candidate
    else {
        return Err(BuildDependencyEditError::ExpectedGitDependency);
    };
    let digest = source_digest(&source);
    let current = source_text::display_literal_bytes(requested.as_bytes());
    let updated = source_text::display_literal_bytes(revision.as_bytes());
    let repository = source_text::display_literal_bytes(repository.as_bytes());
    let alias = explicit_alias
        .as_ref()
        .map(|name| format!(" as `{}`", name.as_str()))
        .unwrap_or_default();
    let proposal = format!(
        "In the `Build::{}` registration for {repository}{alias}, change revision {current} to {updated}.",
        purpose.operation().name()
    );
    let patch = |reason| {
        manual_patch(
            build_path.clone(),
            digest,
            reason,
            Some(format!("revision: {current}")),
            proposal.clone(),
        )
    };
    let mut matches = rows
        .iter()
        .enumerate()
        .filter(|(_, row)| row.purpose == purpose && &row.request == accepted);
    let Some((index, row)) = matches.next() else {
        return Ok(patch(BuildDependencyManualReason::AcceptedRequestMissing));
    };
    if requested == revision {
        return Ok(BuildDependencyEditPlan::Unchanged);
    }
    if matches.next().is_some() {
        return Ok(patch(BuildDependencyManualReason::AcceptedRequestAmbiguous));
    }
    requested.clear();
    requested.push_str(revision);
    if rows
        .iter()
        .any(|row| row.purpose == purpose && row.request == candidate)
    {
        return Ok(patch(BuildDependencyManualReason::CandidateAlreadyPresent));
    }
    let expressions = &parsed.syntax.expressions;
    let syntax_trees::expression::ExpressionNode::StructLiteral(literal) =
        expressions.expression(row.source)
    else {
        return Ok(patch(BuildDependencyManualReason::GeneratedEditRejected));
    };
    let Some(field) = expressions
        .struct_fields(literal.fields)
        .iter()
        .find(|field| field.name.as_str(&parsed.syntax) == "revision")
    else {
        return Ok(patch(BuildDependencyManualReason::GeneratedEditRejected));
    };
    let span = expressions.source_span(field.value).span;
    if source.get(span.start..span.end).is_none() {
        return Ok(patch(BuildDependencyManualReason::GeneratedEditRejected));
    }
    let mut expected: Vec<_> = rows
        .into_iter()
        .map(|row| (row.purpose, row.request))
        .collect();
    expected[index].1 = candidate;
    let mut edited = source;
    edited.replace_range(span.start..span.end, &updated);
    if extract_scoped_from_source(&edited).ok().as_ref() != Some(&expected) {
        return Ok(patch(BuildDependencyManualReason::GeneratedEditRejected));
    }
    Ok(BuildDependencyEditPlan::Automatic(
        BuildFileReplacement::new(build_path, digest, edited),
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
    use super::{plan_dependency_addition_from_source, plan_git_revision_from_source};
    use crate::declarations::AliasName;
    use crate::declarations::dependencies::edit::BuildDependencyEditPlan;
    use crate::declarations::dependencies::read::{
        DependencyPurpose, DependencySourceRequest, extract_scoped_from_source,
    };
    use std::path::PathBuf;

    const ROOT: &str = r#"use {omega}/build/dependency.omg;
machine build(builder: @mutable BuildPlan) {
    builder.product = BuildProduct::Application { name: "probe" };
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
    fn dependency_edits_preserve_the_helper_which_selects_the_product() {
        let source = "machine build(plan: @mutable BuildPlan) {\n    choose(plan);\n}\n";
        let helper = "machine choose(plan: @mutable BuildPlan) { plan.product = BuildProduct::Package { name: \"kit\" }; }\n";
        let proposed = path(Some("tools"), "../tools");
        let edit = plan_dependency_addition_from_source(
            PathBuf::from("build.omg"),
            format!("{source}{helper}"),
            &proposed,
        )
        .unwrap();
        let updated = replaced(edit);
        assert!(updated.contains("    choose(plan);"));
        assert!(updated.ends_with(helper));
        assert_eq!(
            extract_scoped_from_source(&updated).unwrap(),
            [(DependencyPurpose::Product, proposed)]
        );
    }

    #[test]
    fn additions_use_the_parameter_bound_by_the_entry() {
        let initial = "machine build(plan: @mutable BuildPlan) {\n    plan.product = BuildProduct::Package { name: \"client\" };\n}\n";
        let first = path(Some("selected"), "../first");
        let file = PathBuf::from("build.omg");
        let BuildDependencyEditPlan::Automatic(added) =
            plan_dependency_addition_from_source(file.clone(), initial.to_owned(), &first).unwrap()
        else {
            panic!("expected an edit for a renamed Build parameter");
        };
        let added = added.replacement_source();
        assert!(added.contains("Build::dependency(plan, selected_dependency)"));
    }

    #[test]
    fn adds_the_installed_file_once_and_reuses_its_actual_alias() {
        let plain = "machine build(builder: @mutable BuildPlan) {\n    builder.product = BuildProduct::Application { name: \"probe\" };\n}\n";
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
            let spoof = source.replace(
                "{omega}/build/dependency.omg",
                "{vendor}/wrong/dependency.omg",
            );
            assert!(extract_scoped_from_source(&spoof).is_err());
        }
    }

    #[test]
    fn direct_registrations_need_a_helper_import_only_when_an_alias_is_added() {
        let initial = r#"machine build(plan: @mutable BuildPlan) {
    _ = Build::dependency(plan, Source::Path { location: "../first" });
    plan.product = BuildProduct::Package { name: "demo" };
}"#;
        let file = PathBuf::from("build.omg");
        let first = path(None, "../first");
        let second = path(None, "../second");
        let added = replaced(
            plan_dependency_addition_from_source(file.clone(), initial.to_owned(), &second)
                .unwrap(),
        );
        assert!(!added.contains("use "));
        assert!(
            added.contains(r#"Build::dependency(plan, Source::Path { location: "../second" })"#)
        );
        let third = path(Some("selected"), "../third");
        let updated = replaced(plan_dependency_addition_from_source(file, added, &third).unwrap());
        assert_eq!(
            updated.matches("use {omega}/build/dependency.omg;").count(),
            1
        );
        assert_eq!(
            extract_scoped_from_source(&updated).unwrap(),
            [
                (DependencyPurpose::Product, first),
                (DependencyPurpose::Product, second),
                (DependencyPurpose::Product, third),
            ]
        );
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
            edited.contains("    let system_registration_2: DependencyRegistration = Build::dependency(builder, Source::Path { location: \"../vendor/system\" });\n}"),
            "{edited}"
        );
    }

    #[test]
    fn repinning_changes_only_the_requested_revision_literal() {
        let original = r#"use {omega}/build/dependency.omg as dep;
machine build(plan: @mutable BuildPlan) {
    let mutable request: DependencyDescription = |dep|::from_source(Source::Git {
        selection: PackageSelection::Root {},
        revision: /* provenance */ "old",
        repository: "ssh://git@test.invalid/old"
    });
    |dep|::with_alias(@mutable request, "old");
    let chosen: DependencyRegistration = Build::build_dependency(plan, request);
    inspect(@chosen);
}
machine inspect(registered: @DependencyRegistration) {}
"#;
        let [(purpose, accepted)]: [_; 1] = extract_scoped_from_source(original)
            .unwrap()
            .try_into()
            .unwrap();
        let edit = plan_git_revision_from_source(
            "build.omg".into(),
            original.to_owned(),
            purpose,
            &accepted,
            "new",
        )
        .unwrap();
        assert_eq!(
            replaced(edit),
            original.replace("/* provenance */ \"old\"", "/* provenance */ \"new\"")
        );
    }
}
