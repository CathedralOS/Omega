//! Close the source-level provider choices and publish precisely their native inputs.
use super::ProductCodeSource;
use backend_plan::{ProviderExecutionOrigins, PublishedProviderDemand};
use diagnostics::Diagnostic;
use semantic_vocabulary::{BoundaryMachineId, MachineId};
use source_compiler::{AcceptedTerminalSource, PortableTerminalProgram, PsiCompilationObserver};
use std::sync::Arc;

fn capture_origins(
    accepted: &AcceptedTerminalSource,
    program: &PortableTerminalProgram,
    sources: &package_compilation::CheckedSourceCustody,
    native_supplies: &[provider_choices::NativeBuildSupply],
    profile: target::TargetProfile,
    reached: &[BoundaryMachineId],
) -> ProviderExecutionOrigins {
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
            )
            .map(|()| (boundary, supply.materialization()))
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
    ProviderExecutionOrigins {
        artifact: Arc::clone(artifact),
        bindings,
        schema_custody,
        package_permissions: sources
            .dependency_closure()
            .map(|graph| graph.declaration_permissions()),
        authority_permissions: package_authority_permissions(sources),
    }
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

impl ProductCodeSource<'_> {
    pub fn publish_provider_demand(
        &self,
        profile: Option<target::TargetProfile>,
        entry: Option<MachineId>,
        selections: &[source_compiler::provider_choice_inputs::ProviderSelection],
        native_supplies: &[provider_choices::NativeBuildSupply],
        observer: &mut impl PsiCompilationObserver,
    ) -> Result<PublishedProviderDemand, Vec<Diagnostic>> {
        let accepted = self.publication.accepted()?;
        let declared = accepted
            .boundary_requirements_declared()
            .map_err(|error| vec![error])?
            .into_iter()
            .map(
                |row| source_compiler::provider_supply::ReachedBoundaryRequirement {
                    boundary: row.boundary,
                    requirement: row.requirement,
                    slot: row.slot,
                    name: row.name,
                },
            )
            .collect::<Vec<_>>();
        let selected_requirements = source_compiler::provider_supply::explicit_build_requirements(
            &declared,
            selections,
            profile.map(|profile| profile.target_name()),
        );
        let Some(target) = profile else {
            if !selected_requirements.is_empty() {
                return Err(declared
                    .iter()
                    .filter(|row| selected_requirements.binary_search(&row.boundary).is_ok())
                    .map(|row| {
                        Diagnostic::error(format!(
                            "boundary requirement `{}` requires an explicit deployment target",
                            row.name,
                        ))
                    })
                    .collect());
            }
            let roots = self
                .publication
                .roster()?
                .entries
                .iter()
                .map(|symbol| {
                    accepted.machine_of(*symbol).ok_or_else(|| {
                        vec![Diagnostic::error(
                            "unscoped source lost its actual invocation root",
                        )]
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let program = self.publish(self.publication.optimizations(), observer)?;
            return Ok(PublishedProviderDemand::Unscoped {
                artifact: Arc::clone(program.artifact()),
                roots,
            });
        };
        let closed = accepted.close_logical_provider_reach(
            target.target_name(),
            entry,
            &selected_requirements,
        )?;
        let program = self.publish(self.publication.optimizations(), observer)?;
        let origins = capture_origins(
            accepted,
            &program,
            self.sources,
            native_supplies,
            target,
            closed.requirements(),
        );
        let review =
            (self.sources.package_identity().is_some() && !accepted.is_invocation()).then(|| {
                crate::package_review_requirements(
                    accepted,
                    target.target_name(),
                    native_supplies,
                    closed.bindings().iter().map(|row| row.machine),
                )
            });
        let (logical, bodies) = closed.into_parts();
        // Non-executing and named inspection products do not require a program entry.
        let folded_entry = backend_plan::FoldedEntryReach::capture(
            program.artifact().view(), target.target_name(), &bodies, !program.applications().is_empty(),
        );
        let mut labels: Vec<_> = declared
            .into_iter()
            .map(|row| (row.boundary, row.name))
            .collect();
        labels.sort_unstable_by_key(|(boundary, _)| *boundary);
        Ok(PublishedProviderDemand::Target {
            profile: target,
            logical,
            folded_entry,
            bodies,
            labels,
            origins,
            review,
        })
    }

    pub fn publish_provider_origins(
        &self,
        native_supplies: &[provider_choices::NativeBuildSupply],
        profile: target::TargetProfile,
        reached: &[BoundaryMachineId],
        observer: &mut impl PsiCompilationObserver,
    ) -> Result<ProviderExecutionOrigins, Vec<Diagnostic>> {
        let program = self.publish(self.publication.optimizations(), observer)?;
        Ok(capture_origins(
            self.publication.accepted()?,
            &program,
            self.sources,
            native_supplies,
            profile,
            reached,
        ))
    }
}
