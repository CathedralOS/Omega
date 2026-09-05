//! Checked-Psi observations emitted by the production compiler route.
//!
//! Native backend diagrams belonged to the retired StateGraph compatibility
//! harness. Target realization owns its own retained artifacts and reports.

use artifacts::ArtifactWriter;
use diagnostics::Diagnostic;

pub(super) fn write_checked_snapshots(
    writer: &ArtifactWriter,
    checked: &checked_trees::CheckedTrees,
    selected_entry_machine: Option<&str>,
    selected_provider_plans: &effects::SelectedProviderPlanFacts,
    task_activations: &task_plans::TaskActivationPlanSet,
    component_progress: Option<&effects::ComponentProgressManifest>,
) -> Result<(), Vec<Diagnostic>> {
    write_text(
        writer,
        "05_capability_manifest.json",
        &visualizations::capability_manifest_json_with_composition(
            checked,
            selected_entry_machine,
            Some(selected_provider_plans),
            component_progress,
        ),
    )?;
    write_text(
        writer,
        "05_machine_contracts.json",
        &visualizations::machine_contract_manifest_json(checked),
    )?;
    write_text(
        writer,
        "05_qualification_evidence.json",
        &visualizations::qualification_evidence_manifest_json(checked, selected_provider_plans),
    )?;
    write_text(
        writer,
        "05_index_compatibility.json",
        &visualizations::index_compatibility_manifest_json(checked),
    )?;
    write_text(
        writer,
        "05_claim_outcomes.json",
        &visualizations::claim_outcome_manifest_json(checked),
    )?;
    write_text(
        writer,
        "05_carry_manifest.json",
        &visualizations::carry_manifest_json(checked),
    )?;
    write_text(
        writer,
        "05_task_activations.json",
        &visualizations::task_activation_manifest_json(checked, task_activations),
    )?;
    write_text(
        writer,
        "05_executable_tcb_manifest.json",
        &visualizations::executable_tcb_manifest_json(selected_provider_plans),
    )
}

fn write_text(
    writer: &ArtifactWriter,
    file_name: &str,
    contents: &str,
) -> Result<(), Vec<Diagnostic>> {
    writer
        .write_text(file_name, contents)
        .map_err(|error| vec![error])
}
