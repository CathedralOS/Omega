//! Source-authored alternatives challenge receiving admission after the source
//! session is dropped. Changed selections never execute; their exact cycle
//! diagnostic is the oracle, independently of source-only checking.

use backend_plan::{TerminalTargetProviderSelection, admit_selected_checked_providers};
use effects::SelectedProviderPlanFacts;
use effects::provider_plan::ProviderBinding;
use terminal_codec::CanonicalTerminalArtifact;
use terminal_psi_to_abstract_operations::RetainedTerminalArtifact;

pub(super) fn inspect(retained: &RetainedTerminalArtifact, target: &str) -> Result<(), String> {
    let loaded = CanonicalTerminalArtifact::from_bytes(&retained.artifact().to_bytes())
        .map_err(|error| format!("provider transport: {error:?}"))?;
    let original = retained
        .native_realization_proposal()
        .ok_or("provider controls need a native proposal")?
        .target_provider_proposals()
        .iter()
        .find(|proposal| proposal.target == target)
        .ok_or("provider controls lost the target proposal")?
        .selection
        .as_ref()
        .map_err(|errors| format!("provider selection: {errors:?}"))?;
    original.admit(loaded.view(), target)?;
    let module = loaded.semantic_module();
    let candidate_named = |name: &str| {
        module
            .provider_candidates
            .iter()
            .find(|candidate| {
                module
                    .identities
                    .text(candidate.provider_declaration.identity())
                    .is_some_and(|identity| identity.contains(name))
            })
            .ok_or_else(|| format!("source fixture must retain provider {name}"))
    };
    let alternative = candidate_named("AlternativeBeta")?;
    let quiet = candidate_named("SelectedBeta")?.candidate;
    let requirement = module
        .identities
        .text(alternative.source_requirement_identity)
        .ok_or("alternative requirement identity is absent")?;
    let mut plans = original.plans.clone();
    let selected = plans
        .iter_mut()
        .find(|selected| {
            selected
                .plan
                .rows
                .iter()
                .any(|row| selected.plan.identity_text(row.requirement_identity) == requirement)
        })
        .ok_or("selected Beta slot is absent")?;
    selected.provider_declaration = Some(
        module
            .identities
            .text(alternative.provider_declaration.identity())
            .ok_or("alternative declaration is absent")?
            .into(),
    );
    selected.plan.provider_type = module
        .identities
        .text(alternative.provider_identity)
        .ok_or("alternative provider identity is absent")?
        .into();
    let machine_identity = selected.plan.identities.intern(
        module
            .identities
            .text(alternative.source_candidate_identity)
            .ok_or("alternative callable identity is absent")?,
    );
    selected.plan.rows[0].binding = ProviderBinding::CheckedAdapter {
        machine_identity,
        machine_package_identity: alternative.source_candidate_package_identity,
    };
    let changed = TerminalTargetProviderSelection::from_plans(plans, loaded.view())?;
    require_cycle(changed.admit(loaded.view(), target), "checked alternative")?;
    let facts = SelectedProviderPlanFacts::from_selected_plans(
        changed
            .plans
            .iter()
            .map(|selected| selected.plan.clone())
            .collect(),
    )?;
    require_cycle(
        admit_selected_checked_providers(loaded.view(), target, &facts, &changed.plans),
        "native checked alternative",
    )?;

    // Changing the default to an independent, quiet body cannot hide a cycle
    // among the other installed closed instances in this same artifact.
    let mut uncalled = module.clone();
    uncalled.entry = Some(terminal_psi::TerminalCallable::Machine(quiet));
    let optimization = terminal_codec::build_identity_optimization_execution_record(&uncalled)
        .map_err(|error| format!("uncalled optimization: {error:?}"))?;
    let uncalled = CanonicalTerminalArtifact::from_parts(uncalled, optimization, None)
        .map_err(|error| format!("uncalled artifact: {error:?}"))?;
    let uncalled = CanonicalTerminalArtifact::from_bytes(&uncalled.to_bytes())
        .map_err(|error| format!("uncalled transport: {error:?}"))?;
    let calls = terminal_installation::SynchronousCallGraph::new(uncalled.semantic_module(), None)?;
    if !calls.from_root(quiet)?.boundaries.is_empty() {
        return Err("quiet entry unexpectedly calls a provider".into());
    }
    require_cycle(
        changed.admit(uncalled.view(), target),
        "uncalled installed cycle",
    )?;

    // Opaque bindings use these same source-authored ceilings, not a summary
    // asserted for a checked body. Both public and native admission must check
    // the resulting cycle, even when native facts carry toolchain markers.
    let mut opaque = original
        .plans
        .iter()
        .filter(|selected| {
            selected
                .provider_declaration
                .as_deref()
                .is_some_and(|identity| {
                    identity.contains("SelectedAlpha") || identity.contains("SelectedBeta")
                })
        })
        .cloned()
        .collect::<Vec<_>>();
    if opaque.len() != 2 {
        return Err("opaque controls need both source-authored provider schemas".into());
    }
    for selected in &mut opaque {
        selected.provider_declaration = None;
        selected.plan.provider_type = "OpaqueTable".into();
        for row in &mut selected.plan.rows {
            row.binding = ProviderBinding::TableFunction {
                table: "OpaqueTable".into(),
                field: "read".into(),
            };
        }
    }
    let mut opaque = TerminalTargetProviderSelection::from_plans(opaque, loaded.view())?;
    require_cycle(opaque.admit(loaded.view(), target), "opaque ceilings")?;
    let facts = SelectedProviderPlanFacts::from_selected_plans(
        opaque
            .plans
            .iter()
            .map(|selected| selected.plan.clone())
            .collect(),
    )?;
    require_cycle(
        admit_selected_checked_providers(loaded.view(), target, &facts, &opaque.plans),
        "native opaque ceilings",
    )?;
    let mut marked = SelectedProviderPlanFacts::default();
    for selected in &opaque.plans {
        marked = marked.with_toolchain_settled_plan(selected.plan.clone())?;
    }
    require_cycle(
        admit_selected_checked_providers(loaded.view(), target, &marked, &opaque.plans),
        "marked opaque ceilings",
    )?;
    for method in &mut opaque.plans[0].plan.schema.methods {
        method.synchronous_invocations.clear();
    }
    opaque.admit(loaded.view(), target)?;
    Ok(())
}

fn require_cycle<T>(result: Result<T, String>, control: &str) -> Result<(), String> {
    match result {
        Err(reason) if reason.contains("cyclic synchronous `invokes` graph") => Ok(()),
        Err(reason) => Err(format!("{control} refused for another reason: {reason}")),
        Ok(_) => Err(format!("{control} accepted a synchronous provider cycle")),
    }
}
