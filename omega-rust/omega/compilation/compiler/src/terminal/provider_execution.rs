//! Evaluate only the calling policies an actual provider demand activates.
//! The checked program and explicit package permissions supply every input;
//! retaining a declaration alone does not activate it.

use super::ProductCodeSource;
use diagnostics::Diagnostic;
use native_program_planning::target_supply::TargetSupplyExecutionInputs;
use native_program_planning::{ProviderExecutionOrigins, plan_provider_execution_inputs};
use semantic_vocabulary::BoundaryMachineId;
use source_compiler::SourcePublication;
use std::sync::Arc;

pub fn prepare_provider_execution_inputs(
    publication: &SourcePublication,
    sources: &package_compilation::CheckedSourceCustody,
    native_supplies: &[provider_choices::NativeBuildSupply],
    profile: target::TargetProfile,
    reached: &[BoundaryMachineId],
    observer: &mut impl source_compiler::PsiCompilationObserver,
) -> Result<TargetSupplyExecutionInputs, Vec<Diagnostic>> {
    let program = ProductCodeSource {
        publication,
        sources,
    }
    .publish(publication.optimizations(), observer)?;
    let accepted = publication.accepted()?;
    let mut bindings = Vec::new();
    for supply in native_supplies {
        if supply.selection.target.canonical_name() != profile.target_name() {
            continue;
        }
        let mut actuals =
            accepted
                .closure()
                .elaboration
                .boundaries
                .iter()
                .filter(|(boundary, source)| {
                    **source == supply.selection.requirement && reached.contains(boundary)
                });
        let Some((boundary, _)) = actuals.next() else {
            continue;
        };
        let row = if actuals.next().is_some() {
            Err("Build native binding has competing exact boundary occurrences".to_owned())
        } else {
            source_compiler::evaluation::native_bindings::require_canonical_requirement(
                &supply.selection,
                accepted.source(),
            ).map(|()| (boundary, supply.materialization()))
        };
        bindings.push(row);
    }
    let artifact = program.artifact();
    let schema_custody = program
        .source_selections()
        .ok_or_else(|| "provider policies lost their source selection custody".to_owned())
        .and_then(|selections| {
            let questions = selections
                .questions_for(artifact.view().into())
                .ok_or_else(|| {
                    "source declaration custody belongs to another program or invocation".to_owned()
                })?;
            Ok(
                terminal_installation::selection_questions::ProductSchemaCustody::from_selections(
                    artifact.view().into(),
                    questions,
                ),
            )
        });
    let origins = ProviderExecutionOrigins {
        artifact: Arc::clone(artifact),
        bindings,
        schema_custody,
        package_permissions: sources
            .dependency_closure()
            .map(|graph| graph.declaration_permissions()),
        authority_permissions: package_authority_permissions(sources),
    };
    plan_provider_execution_inputs(origins, profile, reached)
}

fn package_authority_permissions(
    sources: &package_compilation::CheckedSourceCustody,
) -> Result<
    (
        terminal_psi::TerminalIdentityTable,
        Vec<effects::ServiceTerminalAuthorityPermission>,
    ),
    String,
> {
    let mut identities = terminal_psi::TerminalIdentityTableBuilder::default();
    let mut permissions = Vec::new();
    for binding in sources.accepted_semantic_bindings() {
        for permission in binding.terminal_authority_permissions() {
            let requirement = binding
                .terminal_authority_permission_identities()
                .text(permission.requirement_identity())
                .filter(|text| !text.is_empty())
                .ok_or_else(|| {
                    "accepted package permission lost its requirement identity".to_owned()
                })?;
            let mut permission = permission.clone();
            permission.rekey_requirement_identity(identities.intern(requirement));
            if !permissions.contains(&permission) {
                permissions.push(permission);
            }
        }
    }
    Ok((identities.build(), permissions))
}
