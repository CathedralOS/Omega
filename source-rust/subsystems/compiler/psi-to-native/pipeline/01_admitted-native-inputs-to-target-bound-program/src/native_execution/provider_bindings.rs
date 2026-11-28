//! Bind the reachable boundary requirements of an authorized program or export.
use super::PreparedNativeExecution;
use diagnostics::Diagnostic;
use provider_correspondence::CompilerIntrinsicPlanProvenance;
use std::collections::BTreeSet;
use target_bound_program::{NativeExecutionScope, ScopedProviderSelection};
use target_bound_program::{ScopedProviderBindings, builtins::SelectedCompilerBuiltin};
use verified_optimization_input::TerminalNativeRealizationProposal;

fn selected_intrinsic_provenance<'a>(
    proposal: &'a TerminalNativeRealizationProposal,
    selection: &'a build_providers::selections::SelectedProviderPlanFacts,
) -> Result<Vec<CompilerIntrinsicPlanProvenance<'a>>, Vec<Diagnostic>> {
    static NON_INTRINSIC_EXECUTION: [Option<
        native_intrinsics::CompilerIntrinsicExecutionIdentity,
    >; 1] = [None];
    selection
        .plans()
        .iter()
        .map(|plan| -> Result<_, Vec<Diagnostic>> {
            if let Some(position) = proposal
                .providers()
                .selection
                .plans()
                .iter()
                .position(|original| original == plan)
            {
                let row_executions = proposal
                    .providers()
                    .intrinsic_executions
                    .get(position)
                    .ok_or_else(|| {
                        vec![Diagnostic::error(
                            "native execution lost original intrinsic row custody",
                        )]
                    })?;
                return Ok(CompilerIntrinsicPlanProvenance {
                    plan,
                    semantic_schema: &plan.schema,
                    row_executions,
                });
            }
            let leaf = proposal
                .providers()
                .direct_boundary_leaves
                .iter()
                .find(|leaf| leaf.plan == *plan)
                .ok_or_else(|| {
                    vec![Diagnostic::error(
                        "native execution has an unowned direct plan",
                    )]
                })?;
            let row_executions: &[Option<native_intrinsics::CompilerIntrinsicExecutionIdentity>] =
                match &leaf.execution {
                    direct_boundary_bindings::DirectBoundaryExecution::Intrinsic(identity) => {
                        std::slice::from_ref(identity)
                    }
                    direct_boundary_bindings::DirectBoundaryExecution::Foreign(_) => {
                        &NON_INTRINSIC_EXECUTION
                    }
                };
            Ok(CompilerIntrinsicPlanProvenance {
                plan,
                semantic_schema: &leaf.satisfied_schema,
                row_executions,
            })
        })
        .collect::<Result<Vec<_>, _>>()
}
pub(super) fn callable_coverage(
    proposal: &TerminalNativeRealizationProposal,
    artifact: &terminal_codec::CanonicalTerminalArtifact,
    scope: &NativeExecutionScope,
    selection: &build_providers::selections::SelectedProviderPlanFacts,
) -> Result<boundary_applications::TerminalBoundaryApplicationCoverage, Vec<Diagnostic>> {
    let provenance = selected_intrinsic_provenance(proposal, selection)?;
    proposal.settle_scoped_boundary_applications(artifact, scope, selection, &provenance)
}

pub(super) fn bind(
    proposal: &TerminalNativeRealizationProposal,
    authorization: &psi_authorization::AuthorizedTerminalArtifact,
    scope: NativeExecutionScope,
) -> Result<PreparedNativeExecution, Vec<Diagnostic>> {
    let artifact = authorization.artifact();
    let module = artifact.semantic_module();
    provider_admission::validate_task_activations(
        artifact.view(),
        scope.reach(),
        &proposal.providers().selection,
        proposal.scope_source().root().target(),
    )?;
    let identity = |boundary: &terminal_psi::BoundaryMachineDeclaration| {
        module
            .identities
            .text(boundary.identity)
            .filter(|text| !text.is_empty())
            .ok_or_else(|| {
                vec![Diagnostic::error(
                    "native execution boundary has no exact identity",
                )]
            })
    };
    let mut demands = BTreeSet::new();
    let mut direct_demands = BTreeSet::new();
    for boundary in module
        .interfaces
        .boundary_machines
        .iter()
        .filter(|boundary| scope.reach().contains_boundary(boundary.id))
    {
        let name = identity(boundary)?;
        demands.insert(name.to_owned());
        if proposal
            .providers()
            .selection
            .selected_requirement_row(name)
            .is_some()
        {
            continue;
        }
        direct_demands.insert(name);
    }
    let mut direct = Vec::new();
    let mut external_bindings = proposal.providers().external_bindings.clone();
    for (index, leaf) in proposal
        .providers()
        .direct_boundary_leaves
        .iter()
        .enumerate()
    {
        if leaf.matches_any_requirement(&direct_demands) {
            direct.push(index);
            if let direct_boundary_bindings::DirectBoundaryExecution::Foreign(binding) =
                &leaf.execution
            {
                let binding = binding.clone()?;
                let mut prior = external_bindings.iter().filter(|row| {
                    row.target_name == binding.target_name
                        && row.requirement_identity == binding.requirement_identity
                });
                match (prior.next(), prior.next()) {
                    (Some(previous), None) if previous == &binding => {}
                    (None, None) => external_bindings.push(binding),
                    _ => {
                        return Err(vec![Diagnostic::error(
                            "direct boundary has competing retained external bindings",
                        )]);
                    }
                }
            }
        }
    }
    let verified = direct_boundary_bindings::VerifiedDirectProviderSelection::check(
        module,
        &proposal.providers().selection,
        &proposal.providers().direct_boundary_leaves,
        scope.reach().emitted_reach(),
        &direct,
    )?;
    let checked = ScopedProviderSelection::bind(
        authorization,
        proposal.scope_source(),
        scope,
        &proposal.providers().direct_boundary_leaves,
        verified,
    )?;
    let scope = checked.scope();
    let selection = checked.selection();
    let intrinsic_provenance = selected_intrinsic_provenance(proposal, selection)?;
    let boundary_application_coverage = proposal.settle_scoped_boundary_applications(
        artifact,
        scope,
        selection,
        &intrinsic_provenance,
    )?;
    let compiler_builtins = SelectedCompilerBuiltin::select(
        selection,
        intrinsic_provenance.into_iter(),
        &demands,
        artifact.view(),
        &proposal.providers().direct_boundary_leaves,
    )?;
    let demands = demands
        .into_iter()
        .map(|identity| {
            selection
                .selected_requirement_identity(&identity)
                .to_owned()
        })
        .collect();
    if std::env::var_os("OMEGA_TRACE_NATIVE_BOUNDARY").is_some() {
        // A missing settlement can originate before native admission. Show
        // the retained choices without changing which boundaries are used.
        let retained_boundary_count = module
            .interfaces
            .boundary_machines
            .iter()
            .filter(|boundary| scope.reach().contains_boundary(boundary.id))
            .count();
        let mut text = format!(
            "prepared native boundaries: entry={:?} total={} in_scope={} direct_leaves={} builtins={}\n",
            scope.reach().entry(),
            module.interfaces.boundary_machines.len(),
            retained_boundary_count,
            proposal.providers().direct_boundary_leaves.len(),
            compiler_builtins.len(),
        );
        for boundary in module.interfaces.boundary_machines.iter().take(16) {
            let retained = scope.reach().contains_boundary(boundary.id);
            let name = module.identities.text(boundary.identity);
            let selected =
                name.is_some_and(|name| selection.selected_requirement_row(name).is_some());
            let leaves = proposal
                .providers()
                .direct_boundary_leaves
                .iter()
                .filter(|leaf| {
                    leaf.plan.rows.iter().any(|row| {
                        name == Some(leaf.plan.identities.text(row.requirement_identity))
                    })
                })
                .count();
            let builtin = compiler_builtins
                .iter()
                .any(|row| row.boundary() == boundary.id);
            text.push_str(&format!(
                    "  id={:?} in_scope={retained} direct_leaves={leaves} selected={selected} builtin={builtin}\n",
                    boundary.id,
                ));
        }
        text.push_str(&format!(
            "prepared native boundaries omitted={}\n",
            module.interfaces.boundary_machines.len().saturating_sub(16),
        ));
        // Every field is a bounded numeric identity, count or boolean; also
        // cap the assembled record if an internal Debug spelling changes.
        let mut end = text.len().min(4096);
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        eprint!("{}", &text[..end]);
    }
    PreparedNativeExecution::from_scoped_providers(
        checked,
        ScopedProviderBindings {
            external_bindings,
            demands,
            compiler_builtins,
            boundary_application_coverage,
        },
    )
}
