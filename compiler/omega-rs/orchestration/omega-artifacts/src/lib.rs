use std::path::{Path, PathBuf};

use omega_calling_conventions::{
    BoundaryEntryPlan, CallingPolicy, EntryControl, EntryStack, IndirectPointerLocation,
    MachineRegime, MachineRegister, Preemption, RegisterSet, SystemVEightbyteClass, ValueClass,
    ValueLocation, ValuePlacement, ValueShape,
};
use omega_core::allocations::AllocationDelta;
use omega_external_roots::{InstalledRootLedger, InstalledRootRecord};
use omega_image::{EmittedImageOutput, ImageOutputKind};
use omega_target::NativeTarget;
use psi_arena::Arena;
use psi_checked_trees::{CheckedTrees, machine::Machine};
use psi_diagnostics::Diagnostic;

mod artifact_writer;
mod boundary_report;
mod frontend_reports;
mod timing_report;
mod wire_report;

pub use artifact_writer::ArtifactWriter;

// Foundational report/plan data types live in `omega-backend-report-types` so the
// backend report passes can depend on them downward. Re-exported here so existing
// `omega_artifacts::{EmissionPlan, BackendSurfaceReport, ...}` paths keep working.
pub use omega_backend_report_types::{
    BackendEntryPoint, BackendMachineSurface, BackendSurfaceReport, EmissionBlocker, EmissionPlan,
    emission_blocker,
};

impl ArtifactWriter {
    pub fn write_emission_plan(&self, emission_plan: &EmissionPlan) -> Result<(), Diagnostic> {
        let mut output = String::new();

        output.push_str("# Omega Emission Plan\n\n");
        output.push_str(&format!("image format: {:?}\n", emission_plan.image_format));
        output.push_str(&format!("entry symbol: {}\n", emission_plan.entry_symbol));
        output.push_str(&format!("sections: {}\n", emission_plan.sections));
        output.push_str(&format!("symbols: {}\n", emission_plan.symbols));
        output.push_str(&format!("host bindings: {}\n", emission_plan.host_bindings));
        output.push_str(&format!("host calls: {}\n", emission_plan.host_calls));
        output.push_str(&format!("data bytes: {}\n", emission_plan.data_bytes));
        output.push_str(&format!(
            "selected instructions: {}\n",
            emission_plan.selected_instructions
        ));
        output.push_str(&format!(
            "instruction operands: {}\n",
            emission_plan.instruction_operands
        ));
        output.push_str(&format!(
            "machine code bytes: {}\n",
            emission_plan.machine_code_bytes
        ));
        output.push_str(&format!(
            "encoded machine bytes: {}\n",
            emission_plan.encoded_machine_bytes
        ));
        output.push_str(&format!("relocations: {}\n", emission_plan.relocations));
        output.push_str(&format!("blockers: {}\n\n", emission_plan.blockers.len()));

        if emission_plan.blockers.is_empty() {
            output.push_str("status: ready to emit\n");
        } else {
            output.push_str("status: blocked before byte emission\n\n");
            output.push_str("## Blockers\n");

            for (_, blocker) in emission_plan.blockers.iter() {
                output.push_str(&format!("- {}: {}\n", blocker.stage, blocker.reason));
            }
        }

        self.write_text("12_emission.txt", &output)
    }

    pub fn write_emitted_native_output(
        &self,
        emitted_output: &EmittedImageOutput,
    ) -> Result<PathBuf, Diagnostic> {
        let output_path = self.write_bytes(&emitted_output.file_name, &emitted_output.bytes)?;

        let mut output = String::new();
        output.push_str("# Omega Emitted Native Output\n\n");
        output.push_str(&format!("path: {}\n", output_path.display()));
        output.push_str(&format!("format: {}\n", emitted_output.format));
        output.push_str(&format!("kind: {:?}\n", emitted_output.kind));
        output.push_str(&format!("bytes: {}\n", emitted_output.bytes.len()));
        output.push_str(&format!("text bytes: {}\n", emitted_output.text_bytes));
        output.push_str(&format!("data bytes: {}\n", emitted_output.data_bytes));
        output.push_str(&format!("bss bytes: {}\n", emitted_output.bss_bytes));
        output.push_str(&format!("symbols: {}\n", emitted_output.symbols));
        output.push_str(&format!("relocations: {}\n", emitted_output.relocations));
        output.push_str(&format!(
            "final image symbols: {}\n",
            emitted_output.final_image_symbols
        ));
        output.push_str(&format!(
            "final image imports: {}\n",
            emitted_output.final_image_imports
        ));
        output.push_str(&format!(
            "final image relocations: {}\n",
            emitted_output.final_image_relocations
        ));

        self.write_html_report("13_emitted_output.html", "emitted_output", &output)?;

        Ok(output_path)
    }

    pub fn remove_stale_native_output(&self) -> Result<(), Diagnostic> {
        self.remove_files([
            "omega-program",
            "12_emitted_output.txt",
            "12_emitted_output.html",
            "13_emitted_output.txt",
            "13_emitted_output.html",
            "13_finalization.txt",
            "13_finalization.html",
            "14_finalization.txt",
            "14_finalization.html",
        ])
    }

    pub fn write_executable_finalization_report(
        &self,
        finalization: &ExecutableFinalization,
    ) -> Result<(), Diagnostic> {
        let mut output = String::new();

        output.push_str("# Omega Executable Finalization\n\n");
        output.push_str(&format!("status: {:?}\n", finalization.status));
        output.push_str(&format!(
            "output: {}\n",
            finalization.executable_path.display()
        ));
        if finalization.command.is_empty() {
            output.push_str("command: none\n");
        } else {
            output.push_str(&format!("command: {}\n", finalization.command.join(" ")));
        }
        if !finalization.stdout.is_empty() {
            output.push_str("\n## stdout\n");
            output.push_str(&finalization.stdout);
            output.push('\n');
        }
        if !finalization.stderr.is_empty() {
            output.push_str("\n## stderr\n");
            output.push_str(&finalization.stderr);
            output.push('\n');
        }
        if finalization.status == ExecutableFinalizationStatus::AlreadyExecutable {
            output.push_str(
                "\nno finalization command was needed; the backend emitted an executable image directly.\n",
            );
        }

        self.write_html_report("14_finalization.html", "finalization", &output)
    }

    /// GR5: the chapter-10 trust report -- the proof-tier surface the
    /// boundary report does not carry. Written even when empty (an empty
    /// report is the honest "no semantic commitments admitted" statement).
    pub fn write_trust_report(&self, trust_report: &TrustReport) -> Result<(), Diagnostic> {
        let mut output = String::new();
        output.push_str("# Omega Trust\n\n");
        output.push_str(&format!(
            "selected provider closure: {:016x}\n\n",
            trust_report.selected_provider_closure_fingerprint
        ));
        output.push_str(&format!(
            "admitted commitments: {}\n\n",
            trust_report.rows.len()
        ));
        for row in &trust_report.rows {
            output.push_str(&format!("- {} -- {}", row.commitment, row.provenance));
            if let Some(fingerprint) = row.machine_contract_fingerprint {
                output.push_str(&format!(" -- machine contract: {fingerprint:016x}"));
            }
            if let Some(fingerprint) = row.machine_template_fingerprint {
                output.push_str(&format!(" -- accepted template: {fingerprint:016x}"));
            }
            if let Some(service_reach) = &row.machine_service_reach {
                output.push_str(" -- service reach: ");
                if service_reach.is_empty() {
                    output.push_str("none");
                } else {
                    output.push_str(&service_reach.join(", "));
                }
            }
            if let Some(invocations) = &row.machine_synchronous_invocations {
                output.push_str(" -- synchronous invocations: ");
                if invocations.is_empty() {
                    output.push_str("none");
                } else {
                    output.push_str(&invocations.join(", "));
                }
            }
            if let Some(may_suspend) = row.machine_may_suspend {
                output.push_str(&format!(
                    " -- may suspend: {}",
                    if may_suspend { "yes" } else { "no" }
                ));
            }
            if let Some(may_block) = row.machine_may_block {
                output.push_str(&format!(
                    " -- may block: {}",
                    if may_block { "yes" } else { "no" }
                ));
            }
            if let Some(terminates) = row.machine_terminates_guarantee {
                output.push_str(&format!(
                    " -- termination guarantee: {}",
                    if terminates { "yes" } else { "no" }
                ));
            }
            if let Some(routes) = &row.machine_crash_routes {
                output.push_str(" -- crash routes: ");
                if routes.is_empty() {
                    output.push_str("none");
                } else {
                    for (route_index, route) in routes.iter().enumerate() {
                        if route_index > 0 {
                            output.push_str(", ");
                        }
                        output.push_str(route.cause.as_str());
                        output.push('[');
                        for (guard_index, guard) in route.alternative_guards.iter().enumerate() {
                            if guard_index > 0 {
                                output.push_str(" | ");
                            }
                            output.push_str(&guard.report_text());
                        }
                        output.push(']');
                    }
                }
            }
            if row.standing_warning {
                output.push_str(" [STANDING WARNING: dev-active until the final build grants it (`b.accept_boundary<..>();`)]");
            }
            output.push('\n');
        }
        output.push_str("\n## Generic accepted instances\n\n");
        output.push_str(&format!(
            "generic accepted instances: {}\n\n",
            trust_report.generic_accepted_instances.len()
        ));
        for row in &trust_report.generic_accepted_instances {
            output.push_str(&format!(
                "- accepted template: {} [{:016x}] -- instance: {:016x} -- instance contract: {:016x} -- type argument identities: {} -- const argument identities: {} -- machine argument contracts: {} -- conformance arguments: {}\n",
                row.template_commitment,
                row.template_fingerprint,
                row.instance_fingerprint,
                row.instance_contract_fingerprint,
                if row.type_argument_identities.is_empty() {
                    "none".to_owned()
                } else {
                    row.type_argument_identities.join(", ")
                },
                if row.const_argument_identities.is_empty() {
                    "none".to_owned()
                } else {
                    row.const_argument_identities.join(", ")
                },
                if row.machine_argument_contract_fingerprints.is_empty() {
                    "none".to_owned()
                } else {
                    row.machine_argument_contract_fingerprints
                        .iter()
                        .map(|fingerprint| format!("{fingerprint:016x}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                },
                if row.conformance_argument_fingerprints.is_empty() {
                    "none".to_owned()
                } else {
                    row.conformance_argument_fingerprints
                        .iter()
                        .map(|fingerprint| format!("{fingerprint:016x}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                }
            ));
        }
        output.push_str("\n## Provider requirements\n\n");
        output.push_str(&format!(
            "provider requirements: {}\n\n",
            trust_report.provider_requirements.len()
        ));
        for row in &trust_report.provider_requirements {
            output.push_str(&format!(
                "- provider plan: {} [{:016x}] -- provider type: {} -- target: {} -- provider origin package: {} -- service schema: {} -- calling plan: {} -- selected: {} -- requirement owner: {} -- requirement identity: {} -- method: {} -- parameter types: {} -- result type: {} -- service reach: {} -- synchronous invocations: {} -- may suspend: {} -- may block: {} -- termination guarantee: {} -- realization: {} -- {} -- grant selectors: {}",
                row.provider_plan,
                row.provider_plan_fingerprint,
                if row.provider_type.is_empty() {
                    "<free external>"
                } else {
                    row.provider_type.as_str()
                },
                if row.target.is_empty() {
                    "<all>"
                } else {
                    row.target.as_str()
                },
                if row.provider_origin_package.is_empty() {
                    "<none>"
                } else {
                    row.provider_origin_package.as_str()
                },
                row.service_schema,
                row.calling_plan_fingerprint
                    .map_or_else(|| "<none>".to_owned(), |value| format!("{value:016x}")),
                if row.selected { "yes" } else { "no" },
                row.requirement_owner,
                row.requirement_identity,
                row.method,
                if row.parameter_type_identities.is_empty() {
                    "<none>".to_owned()
                } else {
                    row.parameter_type_identities.join(", ")
                },
                row.result_type_identity.as_deref().unwrap_or("<none>"),
                if row.service_reach.is_empty() {
                    "none".to_owned()
                } else {
                    row.service_reach.join(", ")
                },
                if row.synchronous_invocations.is_empty() {
                    "none".to_owned()
                } else {
                    row.synchronous_invocations.join(", ")
                },
                if row.may_suspend { "yes" } else { "no" },
                if row.may_block { "yes" } else { "no" },
                if row.terminates_guarantee { "yes" } else { "no" },
                row.realization.report_text(),
                row.provenance,
                if row.grant_selectors.is_empty() {
                    "none".to_owned()
                } else {
                    row.grant_selectors.join(", ")
                },
            ));
            if row.standing_warning {
                output.push_str(" [STANDING WARNING: dev-active until the final build grants its provider plan]");
            }
            output.push('\n');
        }
        output.push_str("\n## Routed qualifications\n\n");
        output.push_str(&format!(
            "routed qualifications: {}\n\n",
            trust_report.qualifications.len()
        ));
        for row in &trust_report.qualifications {
            output.push_str(&format!(
                "- provider plan: {} [{:016x}] -- provider type: {} -- target: {} -- provider origin package: {} -- service schema: {} -- calling plan: {} -- selected: {} -- requirement owner: {} -- requirement identity: {} -- method: {} -- subject: {} -- flow: {} -- domain: {} -- carry: {} -- predicate discharge: {} -- {} -- grant selectors: {}",
                row.provider_plan,
                row.provider_plan_fingerprint,
                if row.provider_type.is_empty() {
                    "<free external>"
                } else {
                    row.provider_type.as_str()
                },
                if row.target.is_empty() {
                    "<all>"
                } else {
                    row.target.as_str()
                },
                if row.provider_origin_package.is_empty() {
                    "<none>"
                } else {
                    row.provider_origin_package.as_str()
                },
                row.service_schema,
                row.calling_plan_fingerprint
                    .map_or_else(|| "<none>".to_owned(), |value| format!("{value:016x}")),
                if row.selected { "yes" } else { "no" },
                row.requirement_owner,
                row.requirement_identity,
                row.method,
                row.subject,
                row.authority_flow,
                row.domain,
                row.effective_carry,
                if row.predicate_discharge_required {
                    "required"
                } else {
                    "none"
                },
                row.provenance,
                if row.grant_selectors.is_empty() {
                    "none".to_owned()
                } else {
                    row.grant_selectors.join(", ")
                },
            ));
            if row.standing_warning {
                output.push_str(" [STANDING WARNING: dev-active until the final build grants its provider plan]");
            }
            output.push('\n');
        }
        self.write_text("trust_report.md", &output)
    }

    /// Write the provider/runtime-owned external-root manifest.
    ///
    /// This deliberately has no numbered compiler-pipeline stage: roots become
    /// live when a slot owner installs them, which may happen after the image
    /// was built. The manifest is nevertheless a normal artifact with complete
    /// normalized identities and no numeric entry address.
    pub fn write_external_root_report(
        &self,
        ledger: &InstalledRootLedger,
    ) -> Result<(), Diagnostic> {
        self.write_text("external_roots.json", &external_root_manifest_json(ledger))
    }
}

/// Canonical JSON projection of the live external-root ledger. The ledger's
/// `BTreeMap` ordering and every normalized set keep this output independent of
/// insertion order. Friendly source names and numeric code addresses are not
/// part of the report identity and do not appear here.
pub fn external_root_manifest_json(ledger: &InstalledRootLedger) -> String {
    let records = ledger.records().collect::<Vec<_>>();
    external_root_records_manifest_json(ledger.report_fingerprint(), &records)
}

fn external_root_records_manifest_json(
    report_fingerprint: u64,
    records: &[&InstalledRootRecord],
) -> String {
    let mut output = String::new();
    output.push_str("{\n  \"ledger_fingerprint\": ");
    push_hex_identity(&mut output, report_fingerprint);
    output.push_str(",\n  \"root_count\": ");
    output.push_str(&records.len().to_string());
    output.push_str(",\n  \"roots\": [");
    for (index, record) in records.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        output.push_str("\n    ");
        push_external_root_json(&mut output, record);
    }
    if !records.is_empty() {
        output.push('\n');
        output.push_str("  ");
    }
    output.push_str("]\n}\n");
    output
}

fn push_external_root_json(output: &mut String, record: &InstalledRootRecord) {
    output.push_str("{\"root\": ");
    push_hex_identity(output, record.root.normalized_identity());
    output.push_str(", \"normalized_root_identity\": ");
    push_hex_identity(output, record.normalized_root_identity);
    output.push_str(", \"entry\": ");
    push_hex_identity(output, record.entry.normalized_identity());
    output.push_str(", \"installed_code\": ");
    push_hex_identity(output, record.installed_code.normalized_identity());
    output.push_str(", \"artifact\": ");
    push_hex_identity(output, record.artifact.normalized_identity());
    output.push_str(", \"slot\": ");
    push_hex_identity(output, record.slot.normalized_identity());
    output.push_str(", \"slot_owner\": ");
    push_hex_identity(output, record.owner.normalized_identity());
    output.push_str(", \"admission\": ");
    push_hex_identity(output, record.admission.normalized_identity());
    output.push_str(", \"provider_execution\": ");
    push_hex_identity(output, record.provider_execution.normalized_identity());
    output.push_str(", \"provider_execution_fingerprint\": ");
    push_hex_identity(output, record.provider_execution_fingerprint);
    output.push_str(", \"provider_plan\": ");
    push_hex_identity(output, record.provider_plan.normalized_identity());
    output.push_str(", \"boundary_contract\": ");
    push_hex_identity(output, record.boundary_contract_fingerprint);
    output.push_str(", \"boundary_plan\": ");
    push_boundary_plan_json(output, &record.boundary);
    output.push_str(", \"provider\": ");
    push_hex_identity(output, record.provider.normalized_identity());
    output.push_str(", \"effects\": [");
    push_identity_set(
        output,
        record
            .effects
            .iter()
            .map(|identity| identity.normalized_identity()),
    );
    output.push_str("], \"trust_receipts\": [");
    push_identity_set(
        output,
        record
            .trust_receipts
            .iter()
            .map(|identity| identity.normalized_identity()),
    );
    output.push_str("], \"nesting_relation\": ");
    push_hex_identity(output, record.nesting_relation.normalized_identity());
    output.push_str(", \"acknowledgement_policy\": ");
    if let Some(identity) = record.acknowledgement_policy {
        push_hex_identity(output, identity.normalized_identity());
    } else {
        output.push_str("null");
    }
    output.push_str(", \"resources\": {\"stack\": {\"ceiling_bytes\": ");
    output.push_str(&record.stack.ceiling_bytes.to_string());
    output.push_str(", \"domain\": ");
    push_entry_stack_json(output, record.boundary.state.stack);
    output.push_str(", \"local_wcsu_bytes\": ");
    output.push_str(&record.stack.realization.local_wcsu_bytes().to_string());
    output.push_str(", \"composed_wcsu_bytes\": ");
    output.push_str(&record.stack.realization.composed_wcsu_bytes().to_string());
    output.push_str(", \"alignment\": ");
    output.push_str(&record.stack.realization.wcsu_alignment().to_string());
    output.push_str(", \"composition_fingerprint\": ");
    push_hex_identity(output, record.stack.realization.composition_fingerprint());
    output.push_str(", \"artifact_composition_fingerprint\": ");
    push_hex_identity(
        output,
        record.stack.realization.artifact_composition_fingerprint(),
    );
    output.push_str(", \"contributing_roots\": [");
    push_identity_set(
        output,
        record
            .stack
            .realization
            .contributing_roots()
            .iter()
            .map(|identity| identity.normalized_identity()),
    );
    output.push_str("], \"provider_validation_receipts\": [");
    push_identity_set(
        output,
        record
            .stack
            .realization
            .validation_receipts()
            .iter()
            .map(|identity| identity.normalized_identity()),
    );
    output.push_str("], \"summary_evidence\": [");
    for (index, (root, summary)) in record.stack.realization.summary_evidence().enumerate() {
        if index != 0 {
            output.push_str(", ");
        }
        output.push_str("{\"root\": ");
        push_hex_identity(output, root.normalized_identity());
        output.push_str(", \"provider\": ");
        push_hex_identity(output, summary.provider.normalized_identity());
        output.push_str(", \"local_wcsu_bytes\": ");
        output.push_str(&summary.local_wcsu_bytes().to_string());
        output.push_str(", \"alignment\": ");
        output.push_str(&summary.wcsu_alignment().to_string());
        match &summary.local_evidence {
            omega_external_roots::StackLocalEvidence::TerminalEntry(binding) => {
                output
                    .push_str(", \"origin\": \"terminal_entry\", \"terminal_vocabulary_marker\": ");
                output.push_str(&binding.terminal_psi().vocabulary_marker.get().to_string());
                output.push_str(", \"terminal_fingerprint\": \"");
                output.push_str(&binding.terminal_psi().program_fingerprint.to_string());
                output.push_str("\", \"terminal_entry\": ");
                push_hex_identity(output, binding.terminal_entry().get());
                output.push_str(", \"installed_code\": ");
                push_hex_identity(output, binding.installed_code().normalized_identity());
                output.push_str(", \"artifact\": ");
                push_hex_identity(output, binding.artifact().normalized_identity());
                output.push_str(", \"entry_stub\": ");
                push_hex_identity(output, binding.entry().normalized_identity());
                output.push_str(", \"contributing_machines\": [");
                push_identity_set(
                    output,
                    binding
                        .contributing_machines()
                        .iter()
                        .map(|machine| machine.get()),
                );
                output.push(']');
            }
            omega_external_roots::StackLocalEvidence::AdmittedProvider {
                validation_receipt,
                ..
            } => {
                output.push_str(
                    ", \"origin\": \"admitted_provider\", \"provider_validation_receipt\": ",
                );
                push_hex_identity(output, validation_receipt.normalized_identity());
            }
        }
        output.push('}');
    }
    output.push(']');
    output.push_str(", \"validation_receipt\": ");
    push_hex_identity(
        output,
        record.stack.validation_receipt.normalized_identity(),
    );
    output.push_str("}, \"logical_fuel\": {\"schedule_marker\": ");
    output.push_str(&record.logical_fuel.schedule.marker().to_string());
    output.push_str(", \"provision\": ");
    push_hex_identity(output, record.logical_fuel.provision.normalized_identity());
    output.push_str(", \"ceiling_units\": ");
    output.push_str(&record.logical_fuel.ceiling_units.to_string());
    output.push_str(", \"composed_units\": ");
    output.push_str(&record.logical_fuel.realization.units().to_string());
    output.push_str(", \"root_summary\": ");
    push_hex_identity(
        output,
        record.logical_fuel.realization.root().normalized_identity(),
    );
    output.push_str(", \"composition_fingerprint\": ");
    push_hex_identity(
        output,
        record.logical_fuel.realization.composition_fingerprint(),
    );
    output.push_str(", \"provider_summaries\": [");
    push_identity_set(
        output,
        record
            .logical_fuel
            .realization
            .summaries()
            .iter()
            .map(|identity| identity.normalized_identity()),
    );
    output.push_str("], \"summary_evidence\": [");
    for (index, (identity, summary)) in record
        .logical_fuel
        .realization
        .summary_evidence()
        .enumerate()
    {
        if index != 0 {
            output.push_str(", ");
        }
        output.push_str("{\"summary\": ");
        push_hex_identity(output, identity.normalized_identity());
        output.push_str(", \"provider\": ");
        push_hex_identity(output, summary.provider.normalized_identity());
        output.push_str(", \"local_units\": ");
        output.push_str(&summary.local_evidence.units().to_string());
        match &summary.local_evidence {
            omega_external_roots::FixedFuelLocalEvidence::TerminalEntry(binding) => {
                let certificate = binding.certificate();
                output
                    .push_str(", \"origin\": \"terminal_entry\", \"terminal_vocabulary_marker\": ");
                output.push_str(
                    &certificate
                        .terminal_psi()
                        .vocabulary_marker
                        .get()
                        .to_string(),
                );
                output.push_str(", \"terminal_fingerprint\": \"");
                output.push_str(&certificate.terminal_psi().program_fingerprint.to_string());
                output.push_str("\", \"entry\": ");
                push_hex_identity(output, certificate.entry().get());
                output.push_str(", \"installed_code\": ");
                push_hex_identity(output, binding.installed_code().normalized_identity());
                output.push_str(", \"artifact\": ");
                push_hex_identity(output, binding.artifact().normalized_identity());
                output.push_str(", \"entry_stub\": ");
                push_hex_identity(output, binding.entry().normalized_identity());
            }
            omega_external_roots::FixedFuelLocalEvidence::TerminalSegment(binding) => {
                let certificate = binding.certificate();
                output.push_str(
                    ", \"origin\": \"terminal_segment\", \"terminal_vocabulary_marker\": ",
                );
                output.push_str(
                    &certificate
                        .terminal_psi()
                        .vocabulary_marker
                        .get()
                        .to_string(),
                );
                output.push_str(", \"terminal_fingerprint\": \"");
                output.push_str(&certificate.terminal_psi().program_fingerprint.to_string());
                output.push_str("\", \"machine\": ");
                push_hex_identity(output, certificate.machine().get());
                output.push_str(", \"start_block\": ");
                push_hex_identity(output, certificate.start_block().get());
                output.push_str(", \"end_edge\": ");
                push_hex_identity(output, certificate.end_edge().get());
                output.push_str(", \"installed_code\": ");
                push_hex_identity(output, binding.installed_code().normalized_identity());
                output.push_str(", \"artifact\": ");
                push_hex_identity(output, binding.artifact().normalized_identity());
                output.push_str(", \"entry_stub\": ");
                push_hex_identity(output, binding.entry().normalized_identity());
            }
            omega_external_roots::FixedFuelLocalEvidence::AdmittedProvider {
                validation_receipt,
                ..
            } => {
                output.push_str(
                    ", \"origin\": \"admitted_provider\", \"provider_validation_receipt\": ",
                );
                push_hex_identity(output, validation_receipt.normalized_identity());
            }
        }
        output.push('}');
    }
    output.push_str("], \"provider_validation_receipts\": [");
    push_identity_set(
        output,
        record
            .logical_fuel
            .realization
            .provider_receipts()
            .iter()
            .map(|identity| identity.normalized_identity()),
    );
    output.push_str("], \"validation_receipt\": ");
    push_hex_identity(
        output,
        record.logical_fuel.validation_receipt.normalized_identity(),
    );
    output.push_str("}, \"machine_state\": {\"ceiling\": {\"interrupted_state_bits\": ");
    push_hex_u16(output, record.boundary.state.interrupted_state.bits());
    output.push_str(", \"saved_state_bits\": ");
    push_hex_u16(output, record.boundary.state.saved_state.bits());
    output.push_str(", \"restored_state_bits\": ");
    push_hex_u16(output, record.boundary.state.restored_state.bits());
    output.push_str(", \"permitted_transitive_use_bits\": ");
    push_hex_u16(
        output,
        record.boundary.state.permitted_transitive_use.bits(),
    );
    output.push_str("}, \"realized_bits\": ");
    push_hex_u16(
        output,
        record.machine_state.realization.machine_state().bits(),
    );
    output.push_str(", \"realized_registers\": ");
    push_register_set_json(output, record.machine_state.realization.registers());
    output.push_str(", \"validation_receipt\": ");
    push_hex_identity(
        output,
        record
            .machine_state
            .validation_receipt
            .normalized_identity(),
    );
    output.push_str("}}, \"component_pins\": [");
    for (index, pin) in record.component_pins.iter().enumerate() {
        if index != 0 {
            output.push_str(", ");
        }
        output.push_str("{\"contract\": ");
        push_hex_identity(output, pin.contract.normalized_identity());
        output.push_str(", \"artifact\": ");
        push_hex_identity(output, pin.artifact.normalized_identity());
        output.push_str(", \"provider\": ");
        push_hex_identity(output, pin.provider.normalized_identity());
        output.push_str(", \"version\": ");
        push_hex_identity(output, pin.version.normalized_identity());
        output.push('}');
    }
    output.push_str("]}");
}

fn push_boundary_plan_json(output: &mut String, plan: &BoundaryEntryPlan) {
    output.push_str("{\"call\": {\"policy\": \"");
    output.push_str(calling_policy_name(plan.call.policy));
    output.push_str("\", \"parameters\": [");
    for (index, placement) in plan.call.parameters.iter().enumerate() {
        if index != 0 {
            output.push_str(", ");
        }
        push_value_placement_json(output, placement);
    }
    output.push_str("], \"result\": ");
    if let Some(result) = &plan.call.result {
        push_value_placement_json(output, result);
    } else {
        output.push_str("null");
    }
    output.push_str(", \"ordinary_clobbers\": ");
    push_register_set_json(output, &plan.call.ordinary_clobbers);
    output.push_str(", \"stack_alignment\": ");
    output.push_str(&plan.call.stack_alignment.to_string());
    output.push_str(", \"shadow_bytes\": ");
    output.push_str(&plan.call.shadow_bytes.to_string());
    output.push_str(", \"entry_control\": ");
    push_entry_control_json(output, plan.call.entry_control);
    output.push_str("}, \"state\": {\"initial_regime\": ");
    push_machine_regime_json(output, plan.state.initial_regime);
    output.push_str(", \"interrupted_state_bits\": ");
    push_hex_u16(output, plan.state.interrupted_state.bits());
    output.push_str(", \"saved_state_bits\": ");
    push_hex_u16(output, plan.state.saved_state.bits());
    output.push_str(", \"restored_state_bits\": ");
    push_hex_u16(output, plan.state.restored_state.bits());
    output.push_str(", \"permitted_transitive_use_bits\": ");
    push_hex_u16(output, plan.state.permitted_transitive_use.bits());
    output.push_str(", \"stack\": ");
    push_entry_stack_json(output, plan.state.stack);
    output.push_str(", \"preemption\": ");
    push_preemption_json(output, plan.state.preemption);
    output.push_str("}}");
}

fn push_value_placement_json(output: &mut String, placement: &ValuePlacement) {
    output.push_str("{\"shape\": ");
    push_value_shape_json(output, placement.shape);
    output.push_str(", \"locations\": [");
    for (index, location) in placement.locations.iter().enumerate() {
        if index != 0 {
            output.push_str(", ");
        }
        push_value_location_json(output, *location);
    }
    output.push_str("]}");
}

/// Machine-readable form of one already-normalized calling-plan placement.
/// Consumers may embed this object in a larger artifact without rebuilding
/// target register, stack, indirect-copy, or aggregate-shape vocabulary.
pub fn value_placement_json(placement: &ValuePlacement) -> String {
    let mut output = String::new();
    push_value_placement_json(&mut output, placement);
    output
}

fn push_value_shape_json(output: &mut String, shape: ValueShape) {
    output.push_str("{\"class\": ");
    match shape.class {
        ValueClass::Integer => output.push_str("\"integer\""),
        ValueClass::Float => output.push_str("\"float\""),
        ValueClass::HomogeneousFloatAggregate { members } => {
            output.push_str("{\"homogeneous_float_aggregate\": ");
            output.push_str(&members.to_string());
            output.push('}');
        }
        ValueClass::SystemVAggregate { first, second } => {
            output.push_str("{\"system_v_aggregate\": [\"");
            output.push_str(system_v_class_name(first));
            output.push_str("\", \"");
            output.push_str(system_v_class_name(second));
            output.push_str("\"]}");
        }
    }
    output.push_str(", \"byte_size\": ");
    output.push_str(&shape.byte_size.to_string());
    output.push_str(", \"alignment\": ");
    output.push_str(&shape.alignment.to_string());
    output.push('}');
}

fn push_value_location_json(output: &mut String, location: ValueLocation) {
    match location {
        ValueLocation::Register {
            register,
            value_byte_offset,
            byte_size,
        } => {
            output.push_str("{\"register\": ");
            push_register_json(output, register);
            output.push_str(", \"value_byte_offset\": ");
            output.push_str(&value_byte_offset.to_string());
            output.push_str(", \"byte_size\": ");
            output.push_str(&byte_size.to_string());
            output.push('}');
        }
        ValueLocation::Stack {
            stack_byte_offset,
            value_byte_offset,
            byte_size,
            alignment,
        } => {
            output.push_str("{\"stack_byte_offset\": ");
            output.push_str(&stack_byte_offset.to_string());
            output.push_str(", \"value_byte_offset\": ");
            output.push_str(&value_byte_offset.to_string());
            output.push_str(", \"byte_size\": ");
            output.push_str(&byte_size.to_string());
            output.push_str(", \"alignment\": ");
            output.push_str(&alignment.to_string());
            output.push('}');
        }
        ValueLocation::Indirect {
            pointer,
            copy_stack_byte_offset,
            byte_size,
            alignment,
        } => {
            output.push_str("{\"indirect\": {\"pointer\": ");
            push_indirect_pointer_json(output, pointer);
            output.push_str(", \"copy_stack_byte_offset\": ");
            if let Some(offset) = copy_stack_byte_offset {
                output.push_str(&offset.to_string());
            } else {
                output.push_str("null");
            }
            output.push_str(", \"byte_size\": ");
            output.push_str(&byte_size.to_string());
            output.push_str(", \"alignment\": ");
            output.push_str(&alignment.to_string());
            output.push_str("}}");
        }
    }
}

fn push_indirect_pointer_json(output: &mut String, pointer: IndirectPointerLocation) {
    match pointer {
        IndirectPointerLocation::Register(register) => {
            output.push_str("{\"register\": ");
            push_register_json(output, register);
            output.push('}');
        }
        IndirectPointerLocation::Stack {
            stack_byte_offset,
            alignment,
        } => {
            output.push_str("{\"stack_byte_offset\": ");
            output.push_str(&stack_byte_offset.to_string());
            output.push_str(", \"alignment\": ");
            output.push_str(&alignment.to_string());
            output.push('}');
        }
    }
}

fn push_register_set_json(output: &mut String, registers: &RegisterSet) {
    output.push('[');
    for (index, register) in registers.as_slice().iter().enumerate() {
        if index != 0 {
            output.push_str(", ");
        }
        push_register_json(output, *register);
    }
    output.push(']');
}

fn push_register_json(output: &mut String, register: MachineRegister) {
    output.push('"');
    match register {
        MachineRegister::X86Rax => output.push_str("x86_rax"),
        MachineRegister::X86Rcx => output.push_str("x86_rcx"),
        MachineRegister::X86Rdx => output.push_str("x86_rdx"),
        MachineRegister::X86Rbx => output.push_str("x86_rbx"),
        MachineRegister::X86Rsp => output.push_str("x86_rsp"),
        MachineRegister::X86Rbp => output.push_str("x86_rbp"),
        MachineRegister::X86Rsi => output.push_str("x86_rsi"),
        MachineRegister::X86Rdi => output.push_str("x86_rdi"),
        MachineRegister::X86R8 => output.push_str("x86_r8"),
        MachineRegister::X86R9 => output.push_str("x86_r9"),
        MachineRegister::X86R10 => output.push_str("x86_r10"),
        MachineRegister::X86R11 => output.push_str("x86_r11"),
        MachineRegister::X86R12 => output.push_str("x86_r12"),
        MachineRegister::X86R13 => output.push_str("x86_r13"),
        MachineRegister::X86R14 => output.push_str("x86_r14"),
        MachineRegister::X86R15 => output.push_str("x86_r15"),
        MachineRegister::X86Xmm(index) => output.push_str(&format!("x86_xmm{index}")),
        MachineRegister::Aarch64X(index) => output.push_str(&format!("aarch64_x{index}")),
        MachineRegister::Aarch64V(index) => output.push_str(&format!("aarch64_v{index}")),
    }
    output.push('"');
}

fn push_entry_control_json(output: &mut String, control: EntryControl) {
    match control {
        EntryControl::CallReturn => output.push_str("\"call_return\""),
        EntryControl::InterruptReturn => output.push_str("\"interrupt_return\""),
        EntryControl::SupervisorCall {
            number_register,
            immediate,
        } => {
            output.push_str("{\"supervisor_call\": {\"number_register\": ");
            push_register_json(output, number_register);
            output.push_str(", \"immediate\": ");
            output.push_str(&immediate.to_string());
            output.push_str("}}");
        }
    }
}

fn push_machine_regime_json(output: &mut String, regime: MachineRegime) {
    match regime {
        MachineRegime::X86Long64 => output.push_str("\"x86_long64\""),
        MachineRegime::Aarch64A64 { exception_level } => {
            output.push_str("{\"aarch64_a64\": {\"exception_level\": ");
            output.push_str(&exception_level.to_string());
            output.push_str("}}");
        }
    }
}

fn push_entry_stack_json(output: &mut String, stack: EntryStack) {
    match stack {
        EntryStack::Interrupted => output.push_str("\"interrupted\""),
        EntryStack::Dedicated { class } => {
            output.push_str("{\"dedicated\": {\"class\": ");
            output.push_str(&class.to_string());
            output.push_str("}}");
        }
        EntryStack::ProviderSelected => output.push_str("\"provider_selected\""),
    }
}

fn push_preemption_json(output: &mut String, preemption: Preemption) {
    match preemption {
        Preemption::NotApplicable => output.push_str("\"not_applicable\""),
        Preemption::Masked => output.push_str("\"masked\""),
        Preemption::Nestable { maximum_depth } => {
            output.push_str("{\"nestable\": {\"maximum_depth\": ");
            output.push_str(&maximum_depth.to_string());
            output.push_str("}}");
        }
        Preemption::ProviderDefined => output.push_str("\"provider_defined\""),
    }
}

const fn calling_policy_name(policy: CallingPolicy) -> &'static str {
    match policy {
        CallingPolicy::MicrosoftX64 => "microsoft_x64",
        CallingPolicy::SystemVAMD64 => "system_v_amd64",
        CallingPolicy::Aapcs64 => "aapcs64",
        CallingPolicy::LinuxSyscallX86_64 => "linux_syscall_x86_64",
        CallingPolicy::LinuxSyscallAarch64 => "linux_syscall_aarch64",
    }
}

const fn system_v_class_name(class: SystemVEightbyteClass) -> &'static str {
    match class {
        SystemVEightbyteClass::Integer => "integer",
        SystemVEightbyteClass::Sse => "sse",
    }
}

fn push_identity_set(output: &mut String, identities: impl IntoIterator<Item = u64>) {
    for (index, identity) in identities.into_iter().enumerate() {
        if index != 0 {
            output.push_str(", ");
        }
        push_hex_identity(output, identity);
    }
}

fn push_hex_identity(output: &mut String, identity: u64) {
    output.push('"');
    output.push_str(&format!("0x{identity:016x}"));
    output.push('"');
}

fn push_hex_u16(output: &mut String, bits: u16) {
    output.push('"');
    output.push_str(&format!("0x{bits:04x}"));
    output.push('"');
}

fn html_report(title: &str, contents: &str) -> String {
    let mut html = String::new();
    html.push_str("<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n");
    html.push_str("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n");
    html.push_str("<title>");
    html.push_str(&escape_html(title));
    html.push_str("</title>\n<style>\n");
    html.push_str(REPORT_STYLE);
    html.push_str("</style>\n</head>\n<body>\n<aside>\n<h1>");
    html.push_str(&escape_html(title));
    html.push_str("</h1>\n");
    push_report_nav(&mut html);
    html.push_str("</aside>\n<main><pre>");
    html.push_str(&escape_html(contents));
    html.push_str("</pre></main>\n</body>\n</html>\n");
    html
}

fn push_report_nav(html: &mut String) {
    html.push_str("<nav class=\"phase-nav\" aria-label=\"Pipeline stages\"><a target=\"_top\" href=\"00_pipeline.html\">Index</a>");
    for (number, label, id) in REPORT_LINKS {
        html.push_str("<a target=\"_top\" href=\"00_pipeline.html#");
        html.push_str(&escape_html(id));
        html.push_str("\"><span>");
        html.push_str(&escape_html(number));
        html.push_str("</span> ");
        html.push_str(&escape_html(label));
        html.push_str("</a>");
    }
    html.push_str("</nav>\n");
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const REPORT_LINKS: &[(&str, &str, &str)] = &[
    ("00", "Timings", "timings"),
    ("02", "Syntax", "syntax"),
    ("03", "Symbols", "symbols"),
    ("04", "Typed", "typed"),
    ("05", "Checked", "checked"),
    ("06", "State Graph", "state-graph"),
    ("07", "Control Flow", "control-flow"),
    ("08", "Abstract Operations", "abstract-operations"),
    ("09", "Target Operations", "target-operations"),
    (
        "10",
        "Assigned Target Operations",
        "assigned-target-operations",
    ),
    ("11", "Machine Instructions", "machine-instructions"),
    ("12", "Emission", "emission"),
];

const REPORT_STYLE: &str = r#"
:root {
  --bg: #101318;
  --panel: #171d25;
  --panel-border: #2a3442;
  --text: #eef3fb;
  --muted: #9caaba;
}
* { box-sizing: border-box; }
body {
  min-height: 100vh;
  margin: 0;
  background: radial-gradient(circle at 20% 0%, #253144 0, #101318 42%);
  color: var(--text);
  display: grid;
  grid-template-columns: minmax(280px, 22vw) 1fr;
  font: 14px/1.45 ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
}
aside {
  border-right: 1px solid var(--panel-border);
  background: color-mix(in srgb, var(--panel) 92%, transparent);
  min-height: 100vh;
  padding: 18px;
}
h1 { margin: 0 0 16px; font-size: 18px; letter-spacing: 0.04em; }
.phase-nav {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}
.phase-nav a {
  border: 1px solid #303d50;
  border-radius: 999px;
  color: #d8e2ef;
  font-size: 11px;
  line-height: 1;
  padding: 7px 9px;
  text-decoration: none;
}
.phase-nav a:hover { background: #263247; border-color: #8ab4ff; }
.phase-nav span { color: var(--muted); }
main {
  min-width: 0;
  overflow: auto;
  padding: 28px;
}
pre {
  background: rgba(13, 17, 23, 0.82);
  border: 1px solid #283343;
  border-radius: 18px;
  color: #d8e2ef;
  line-height: 1.45;
  margin: 0;
  min-height: calc(100vh - 56px);
  overflow: auto;
  padding: 24px;
  white-space: pre;
}
"#;

fn temp_path_for(path: &Path) -> PathBuf {
    path.with_file_name(format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("omega-artifact"),
        std::process::id()
    ))
}

fn write_source_file_artifact(output: &mut String, file: &SourceFileArtifact) {
    let item_range = if file.item_count == 0 {
        String::from("-")
    } else {
        format!("{}..{}", file.first_item, file.first_item + file.item_count)
    };

    output.push_str(&format!(
        "{:<4} {:>8} {:>7} {:>7} {:>7} {:>11} {}\n",
        file.id,
        format_bytes(file.byte_count as u64),
        file.line_count,
        file.non_empty_line_count,
        file.item_count,
        item_range,
        file.path.display()
    ));
}

fn write_ast_file_artifact(output: &mut String, file: &AstFileArtifact) {
    output.push_str(&format!("## {}\n", file.path.display()));
    output.push_str(&format!(
        "tables: statements {} transition_targets {} expressions {} type_references {} type_constraints {}\n",
        file.statement_count,
        file.transition_target_count,
        file.expression_count,
        file.type_reference_count,
        file.type_constraint_count
    ));

    if !file.item_range_valid {
        output.push_str("invalid item range\n\n");
        return;
    }

    if file.item_summaries.is_empty() {
        output.push_str("items: none\n\n");
        return;
    }

    for (index, summary) in file.item_summaries.iter().enumerate() {
        output.push_str(&format!(
            "- item {}: {}\n",
            file.first_item + index,
            summary
        ));
    }

    output.push('\n');
}

fn format_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;

    let bytes = bytes as f64;
    if bytes >= GIB {
        format!("{:.2} GiB", bytes / GIB)
    } else if bytes >= MIB {
        format!("{:.2} MiB", bytes / MIB)
    } else if bytes >= KIB {
        format!("{:.2} KiB", bytes / KIB)
    } else {
        format!("{} B", bytes as u64)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseTiming {
    pub phase: String,
    pub microseconds: u128,
    pub allocations: AllocationDelta,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceLoadArtifact {
    pub item_count: usize,
    pub files: Vec<SourceFileArtifact>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SourceFileArtifact {
    pub id: usize,
    pub path: PathBuf,
    pub first_item: usize,
    pub item_count: usize,
    pub byte_count: usize,
    pub line_count: usize,
    pub non_empty_line_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AstArtifact {
    pub file_count: usize,
    pub item_count: usize,
    pub identity: AstIdentityArtifact,
    pub files: Vec<AstFileArtifact>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AstIdentityArtifact {
    pub owned_identifier_strings: usize,
    pub identifiers: usize,
    pub source_identifiers: usize,
    pub generated_identifiers: usize,
    pub path_members: usize,
    pub string_literals: usize,
    pub float_literals: usize,
    pub source_float_literals: usize,
    pub generated_float_literals: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AstFileArtifact {
    pub path: PathBuf,
    pub first_item: usize,
    pub statement_count: usize,
    pub transition_target_count: usize,
    pub expression_count: usize,
    pub type_reference_count: usize,
    pub type_constraint_count: usize,
    pub item_summaries: Vec<String>,
    pub item_range_valid: bool,
}

/// Compatibility report for `wire data` protocol schemas (chapter 20): field
/// tables, retired numbers, declared version eras, and per-era verdicts along
/// the VERSION CHAIN (each era against its successor; the newest era against
/// the current schema body). Built from typed trees by the compiler pipeline;
/// this crate only owns the artifact shape and rendering.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WireProtocolReport {
    pub schemas: Vec<WireSchemaReportEntry>,
    pub demands: Vec<WireCompatibilityDemandReportEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WireSchemaReportEntry {
    pub name: String,
    pub normalized_schema_identity: u64,
    /// Whether the compiler exposed generated codec entries for this schema.
    /// Identity-numbered ordinary data may carry both this realization fact
    /// and its normalized reflected schema identity in the same merged row.
    pub synthesized_codec: bool,
    pub encoding: Option<String>,
    pub codec_requirement: Option<String>,
    pub codec_requirement_identity: Option<u64>,
    pub encode_requirement: Option<String>,
    pub encode_requirement_identity: Option<u64>,
    pub normalized_plan_identity: Option<u64>,
    pub encode_obligations: Vec<String>,
    pub realization_origin: Option<WireRealizationOrigin>,
    pub trust_class: Option<WireTrustClass>,
    pub realization_evidence: Vec<String>,
    /// The era discriminator the CURRENT body encodes (decision 10): the
    /// number of declared version blocks (0 for an unversioned schema).
    pub current_era: u64,
    pub fields: Vec<WireFieldReportEntry>,
    pub reserved: Vec<u64>,
    pub cases: Vec<WireCaseReportEntry>,
    pub retired_cases: Vec<u64>,
    pub versions: Vec<WireVersionReportEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireRealizationOrigin {
    Authored,
    Generated { generator: String },
    Foreign { provider: String },
}

impl WireRealizationOrigin {
    fn describe(&self) -> String {
        match self {
            Self::Authored => "authored".to_owned(),
            Self::Generated { generator } => format!("generated by {generator}"),
            Self::Foreign { provider } => format!("foreign provider `{provider}`"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireTrustClass {
    Derived,
    Admitted { authority: String },
}

impl WireTrustClass {
    fn describe(&self) -> String {
        match self {
            Self::Derived => "derived".to_owned(),
            Self::Admitted { authority } => format!("admitted by {authority}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WireFieldReportEntry {
    pub number: u64,
    pub name: String,
    pub relevance: WireFieldRelevance,
    pub type_display: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WireFieldRelevance {
    #[default]
    Relevant,
    Erased,
}

impl WireFieldRelevance {
    pub fn is_erased(self) -> bool {
        matches!(self, Self::Erased)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WireCaseReportEntry {
    pub number: u64,
    pub name: String,
    pub payload_fields: Vec<WireFieldReportEntry>,
    pub retired_payload_identities: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WireVersionReportEntry {
    pub name: String,
    /// The era discriminator payloads of this declared version carry: its
    /// zero-based position in the declaration-ordered version chain.
    pub era: u64,
    /// The next era in the version chain this era's verdicts compare against:
    /// the following declared version, or `current` for the newest era.
    pub successor: String,
    pub fields: Vec<WireFieldReportEntry>,
    pub reserved: Vec<u64>,
    pub verdicts: WireCompatibilityVerdicts,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WireCompatibilityVerdicts {
    pub compatible: Vec<String>,
    /// Cross-era type changes on a stable field number: legal evolution (the
    /// era discriminator selects the old decode table), surfaced as a report
    /// verdict instead of a compile error.
    pub requires_migration: Vec<String>,
    pub reserved: Vec<String>,
    pub incompatible: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WireCompatibilityDemandReportEntry {
    pub edge: String,
    pub lineage: String,
    pub local_schema: String,
    pub peer_schema: String,
    pub codec: String,
    pub unknown_member_behavior: String,
    pub readability: WireCompatibilityFactReport,
    pub writability: WireCompatibilityFactReport,
    pub unknown_preservation: WireCompatibilityFactReport,
    pub canonicality: WireCompatibilityFactReport,
    pub migration_coverage: WireCompatibilityFactReport,
    pub satisfied: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WireCompatibilityFactReport {
    pub required: bool,
    pub satisfied: bool,
    pub detail: String,
}

/// The chapter-10 TRUST REPORT (GR5): one row per admitted semantic
/// commitment, carrying its provenance tier. Dev-active rows (own-package
/// claims, not yet root-granted) carry the STANDING WARNING the grant
/// locality rule promises; root-granted rows name the grant. "The report
/// sees every grant, private or public."
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustReportRow {
    /// The commitment, consumer-rendered (`domain introduction: Meters`).
    pub commitment: String,
    /// `own-package (dev-active)` or `root grant`.
    pub provenance: String,
    /// Exact published contract identity for one local accepted machine.
    /// Domains, provider commitments, and unmatched imported grants have no
    /// machine contract and retain `None` rather than a synthesized identity.
    pub machine_contract_fingerprint: Option<u64>,
    /// Exact normalized universal-template identity for one generic accepted
    /// machine. Non-generic accepted machines and every other row retain
    /// `None`; their ordinary checked contract identity remains separate.
    pub machine_template_fingerprint: Option<u64>,
    /// Exact published service-reach ceiling for one local accepted machine.
    /// `Some(Vec::new())` is the explicit public negative guarantee; rows that
    /// do not describe a local accepted machine retain `None`.
    pub machine_service_reach: Option<Vec<String>>,
    /// Exact published direct synchronous invocation ceiling for one local
    /// accepted machine. `Some(Vec::new())` is explicit public omission; rows
    /// that do not describe a local accepted machine retain `None`.
    pub machine_synchronous_invocations: Option<Vec<String>>,
    /// Exact published suspension ceiling for one local accepted machine.
    /// `Some(false)` is the public negative guarantee; rows that do not
    /// describe a local accepted machine retain `None`.
    pub machine_may_suspend: Option<bool>,
    /// Exact published worker-blocking ceiling for one local accepted machine.
    /// `Some(false)` is the public negative guarantee; rows that do not
    /// describe a local accepted machine retain `None`.
    pub machine_may_block: Option<bool>,
    /// Exact premise-free published termination guarantee for one local
    /// accepted machine. `Some(false)` is published `NoGuarantee`; rows that
    /// do not describe a local accepted machine retain `None`.
    pub machine_terminates_guarantee: Option<bool>,
    /// Exact canonical published crash buckets for one local accepted machine.
    /// `Some(Vec::new())` is the public no-crash ceiling; rows that do not
    /// describe a local accepted machine retain `None`.
    pub machine_crash_routes: Option<Vec<TrustCrashRouteBucket>>,
    /// Dev-active rows warn until the root grants them.
    pub standing_warning: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustCrashCause {
    Trap,
    Abort,
}

impl TrustCrashCause {
    fn as_str(self) -> &'static str {
        match self {
            Self::Trap => "Trap",
            Self::Abort => "Abort",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrustCrashRouteGuard {
    Truth,
    PredicateIdentity(Vec<u8>),
}

impl TrustCrashRouteGuard {
    fn report_text(&self) -> String {
        match self {
            Self::Truth => "true".to_owned(),
            Self::PredicateIdentity(bytes) => {
                let mut identity = String::from("0x");
                for byte in bytes {
                    identity.push_str(&format!("{byte:02x}"));
                }
                identity
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustCrashRouteBucket {
    pub cause: TrustCrashCause,
    pub alternative_guards: Vec<TrustCrashRouteGuard>,
}

/// One exact requirement supplied by a normalized provider-plan row.
///
/// This is the claim-free provider blast-radius carrier: readable names remain
/// separate from canonical overload identity, and admission provenance is
/// copied rather than inferred from the requirement or schema spelling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustProviderRequirementRow {
    pub provider_plan: String,
    pub provider_plan_fingerprint: u64,
    /// Exact normalized provider type; empty denotes a free external leaf.
    pub provider_type: String,
    /// Exact normalized target; empty denotes all targets.
    pub target: String,
    /// Verbatim provider-plan provenance input. Empty remains explicit absence
    /// and is never repaired from the plan, provider, or schema name.
    pub provider_origin_package: String,
    /// Exact selected boundary-service schema identity.
    pub service_schema: String,
    /// Exact evaluated calling contract identity, when the requirement has one.
    pub calling_plan_fingerprint: Option<u64>,
    pub selected: bool,
    pub requirement_owner: String,
    pub requirement_identity: String,
    pub method: String,
    /// Exact positional normalized semantic parameter identities.
    pub parameter_type_identities: Vec<String>,
    /// Exact normalized semantic result identity; `None` means no result.
    pub result_type_identity: Option<String>,
    /// Exact normalized boundary-service reach for this requirement.
    pub service_reach: Vec<String>,
    /// Exact normalized direct invocation bindings. This is not reach closure.
    pub synchronous_invocations: Vec<String>,
    pub may_suspend: bool,
    pub may_block: bool,
    /// Exact existing public `terminates;` guarantee on the bodyless
    /// requirement. This carries no progress-profile or private witness data.
    pub terminates_guarantee: bool,
    pub realization: TrustProviderRealization,
    pub provenance: String,
    pub grant_selectors: Vec<String>,
    pub standing_warning: bool,
}

/// Exact normalized realization selected by one provider-plan row.
///
/// This remains structured so the durable trust artifact distinguishes checked
/// Omega adapters from opaque/raw leaves without parsing a debug rendering or
/// inferring mechanism from the provider name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrustProviderRealization {
    Import { library: String, symbol: String },
    Syscall { number: i64 },
    CompilerIntrinsic { machine: String },
    VtableSlot { index: i64 },
    VtableField { table: String, field: String },
    TableFunction { table: String, field: String },
    CheckedAdapter { machine: String },
}

impl TrustProviderRealization {
    fn report_text(&self) -> String {
        match self {
            Self::Import { library, symbol } => {
                format!("import `{library}` symbol `{symbol}`")
            }
            Self::Syscall { number } => format!("syscall {number}"),
            Self::CompilerIntrinsic { machine } => {
                format!("compiler intrinsic realization `{machine}`")
            }
            Self::VtableSlot { index } => format!("vtable slot {index}"),
            Self::VtableField { table, field } => format!("vtable field `{table}.{field}`"),
            Self::TableFunction { table, field } => {
                format!("table function `{table}.{field}`")
            }
            Self::CheckedAdapter { machine } => format!("checked adapter `{machine}`"),
        }
    }
}

/// One exact routed qualification carried by a normalized provider plan.
///
/// These rows keep the durable trust artifact at least as specific as the
/// admitted claim. They are derived from structured provider-plan schema, not
/// parsed from display types or reconstructed from a compact plan identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustQualificationRow {
    pub provider_plan: String,
    pub provider_plan_fingerprint: u64,
    /// Exact normalized provider type; empty denotes a free external leaf.
    pub provider_type: String,
    /// Exact normalized target; empty denotes all targets.
    pub target: String,
    /// Verbatim provider-plan provenance input, independent from grant status.
    pub provider_origin_package: String,
    /// Exact selected boundary-service schema identity.
    pub service_schema: String,
    /// Exact evaluated calling contract identity, when the requirement has one.
    pub calling_plan_fingerprint: Option<u64>,
    pub selected: bool,
    /// Readable semantic owner of the exact requirement. This remains
    /// separate from the canonical overload identity because an inherited
    /// requirement's owner can differ from the selected service schema.
    pub requirement_owner: String,
    pub requirement_identity: String,
    pub method: String,
    /// `parameter:N` for an accepted entry claim or `result` for a returned
    /// qualification.
    pub subject: String,
    /// `accepts` or `returns`.
    pub authority_flow: String,
    pub domain: String,
    pub effective_carry: String,
    pub predicate_discharge_required: bool,
    pub provenance: String,
    /// Exact authored root-grant selectors that activated this selected plan.
    /// The provider-plan fingerprint above remains the selected semantic
    /// identity; these strings retain its source-level grant provenance.
    pub grant_selectors: Vec<String>,
    pub standing_warning: bool,
}

/// One checked instantiation of a universal generic accepted machine. These
/// rows consume no additional grant: they retain the exact template identity
/// and selected machine-contract identities for audit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustGenericAcceptedInstanceRow {
    pub template_commitment: String,
    pub template_fingerprint: u64,
    pub instance_fingerprint: u64,
    /// Exact normalized checked contract of the specialized machine instance.
    /// This is independent from the specialization-tuple identity above and
    /// from every selected static-machine argument contract below.
    pub instance_contract_fingerprint: u64,
    pub type_argument_identities: Vec<String>,
    pub const_argument_identities: Vec<String>,
    pub machine_argument_contract_fingerprints: Vec<u64>,
    pub conformance_argument_fingerprints: Vec<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrustReport {
    /// Deterministic identity of the complete retained selected-provider set.
    pub selected_provider_closure_fingerprint: u64,
    pub rows: Vec<TrustReportRow>,
    pub generic_accepted_instances: Vec<TrustGenericAcceptedInstanceRow>,
    pub provider_requirements: Vec<TrustProviderRequirementRow>,
    pub qualifications: Vec<TrustQualificationRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BoundaryReport {
    pub targets: Arena<BoundaryTarget>,
    pub contracts: Arena<BoundaryContract>,
    pub unchecked_policies: Arena<UncheckedBoundaryPolicy>,
    pub capability_blast_radius: Arena<CapabilityBlastRadius>,
}

/// Theoretical blast radius for a single boundary capability: whether it is an
/// approved provider edge or an in-package (application-minted) provider, and
/// the authority-flow verbs it participates in. The capability value itself is
/// the authority; service names are deliberately absent.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CapabilityBlastRadius {
    pub capability: String,
    pub approved_provider: bool,
    pub uses: usize,
    pub returns: usize,
    pub acquires: usize,
    pub stores: usize,
    pub derives: usize,
    /// Exact checked flow sites. Counts above are only a summary of these rows.
    pub flows: Vec<CapabilityBlastRadiusFlow>,
}

/// One checked capability-flow fact retained in the boundary artifact.
///
/// The readable state labels are presentation metadata beside the canonical
/// overload identities of their owning machines. Statement and call ordinals
/// disambiguate repeated uses within an operation; `via` distinguishes a
/// propagated helper route from a direct boundary use.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CapabilityBlastRadiusFlow {
    pub state: String,
    pub machine_overload_identity: String,
    pub authority_flow: String,
    pub statement_index: usize,
    pub call_ordinal: usize,
    pub via: Option<CapabilityBlastRadiusRoute>,
}

/// One helper route through which a capability flow was propagated. Keeping
/// the presentation label and exact owner identity in one value prevents an
/// artifact row from retaining either half alone.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CapabilityBlastRadiusRoute {
    pub state: String,
    pub machine_overload_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BoundaryTarget {
    pub name: String,
    pub host_provider: String,
    pub host_settings: usize,
    pub checked_boundaries: usize,
    pub unchecked_boundaries: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BoundaryContract {
    pub capability: String,
    pub state: String,
    pub boundary: String,
    pub requires_count: usize,
    pub ensures_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UncheckedBoundaryPolicy {
    pub target: String,
    pub name: String,
}

/// Build the source/backend audit surface around the exact Build-selected
/// entry. With no selected entry, the report must not invent one from a source
/// name.
pub fn build_backend_surface_report(
    program: &CheckedTrees,
    selected_entry_machine: Option<&str>,
) -> BackendSurfaceReport {
    let mut report = BackendSurfaceReport::default();

    for machine in program.machines() {
        collect_machine(&mut report, program, machine);
    }

    let selected = selected_entry_machine.and_then(|name| entry_point_for_machine(program, name));
    if let Some(entry) = selected {
        report.entry_points.insert(entry);
    }

    report
}

fn collect_machine(report: &mut BackendSurfaceReport, program: &CheckedTrees, machine: &Machine) {
    report.machines.insert(BackendMachineSurface {
        name: machine.name.to_string(),
        contained_machines: program
            .facts
            .carry
            .contained_fields_for_machine(machine.symbol)
            .len(),
        owned_data: program.machine_owned_data(machine).len(),
        states: program.machine_states(machine).len(),
    });
}

fn entry_point_for_machine(program: &CheckedTrees, name: &str) -> Option<BackendEntryPoint> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == name)?;
    let state = program.machine_states(machine).first()?;
    Some(BackendEntryPoint {
        machine: machine.name.as_str().to_owned(),
        state: state.name.as_str().to_owned(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutableFinalization {
    pub executable_path: PathBuf,
    pub status: ExecutableFinalizationStatus,
    pub command: Vec<String>,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutableFinalizationStatus {
    AlreadyExecutable,
}

pub fn finalize_emitted_image_output(
    target: NativeTarget,
    emitted_output: &EmittedImageOutput,
    output_path: &Path,
) -> Result<ExecutableFinalization, Diagnostic> {
    if emitted_output.kind == ImageOutputKind::DirectExecutable {
        mark_executable_if_needed(output_path)?;
        return Ok(ExecutableFinalization {
            executable_path: output_path.to_path_buf(),
            status: ExecutableFinalizationStatus::AlreadyExecutable,
            command: Vec::new(),
            stdout: "native output is already an executable image".to_owned(),
            stderr: String::new(),
        });
    }

    Err(Diagnostic::error(format!(
        "native output `{}` is {:?} for {:?}; Omega does not invoke external linkers, so this target must emit a direct executable image",
        emitted_output.format, emitted_output.kind, target
    )))
}

#[cfg(unix)]
fn mark_executable_if_needed(path: &Path) -> Result<(), Diagnostic> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = std::fs::metadata(path)
        .map_err(|error| {
            Diagnostic::error(format!(
                "failed to read executable permissions {}: {error}",
                path.display()
            ))
        })?
        .permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).map_err(|error| {
        Diagnostic::error(format!(
            "failed to mark executable {}: {error}",
            path.display()
        ))
    })
}

#[cfg(not(unix))]
fn mark_executable_if_needed(_path: &Path) -> Result<(), Diagnostic> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use omega_calling_conventions::{
        CallSignature, CallingPolicy, EntryStack, MachineRegister, MachineState, MachineStateSet,
        ProviderExitRealization, RegisterSet, StateFootprintEvidence, ValueShape,
        evaluate_ordinary_boundary_entry_plan,
    };
    use omega_executable_installation::{
        Artifact, ArtifactContentId, ArtifactEntry, ArtifactId, ContainerLimits,
        DecodedArtifactContainer, EntrySetId, InstallationDiagnostic, InstalledCodeId,
        MachineContractSetId, MachineFootprintId, PlacementPlanId, RelocationSetId,
        decode_executable_container, normalized_decoded_content_identity,
    };
    use omega_external_roots::{
        AcknowledgementPolicyId, ComponentArtifactId, ComponentContractId, ComponentProviderId,
        ComponentVersionPin, ComponentVersionPinId, ExternalRootDiagnostic, ExternalRootId,
        FixedFuelCall, FixedFuelProviderSummary, FuelProvisionId, FuelScheduleIdentity,
        FuelValidationReceiptId, InstalledRootRecord, LogicalFuelResourceColumn,
        MachineStateResourceColumn, NestingRelationId, OpaqueProviderExitAssurance,
        ProviderExecutionId, ProviderFuelSummaryId, ProviderFuelValidationReceiptId,
        ProviderPlanId, ProviderStackSummary, RootAdmissionId, RootEffectId, RootProviderId,
        RootSlotId, RootSlotOwnerId, StackNestingRelation, StackResourceColumn,
        StackValidationReceiptId, StateValidationReceiptId, TrustReceiptId,
        compose_artifact_stacks, compose_fixed_fuel,
    };
    use omega_target::Architecture;
    use psi_checked_trees::CheckedTrees;
    use psi_checked_trees::machine::Machine;
    use psi_checked_trees::name::Identifier;
    use psi_checked_trees::state::State;
    use psi_layout_plans::{EntryStubId, PlacementConstraints, PlacementPhase};
    use psi_symbols::SymbolHandle;

    use super::{
        ArtifactWriter, TrustCrashCause, TrustCrashRouteBucket, TrustCrashRouteGuard,
        TrustGenericAcceptedInstanceRow, TrustProviderRealization, TrustProviderRequirementRow,
        TrustQualificationRow, TrustReport, TrustReportRow, build_backend_surface_report,
        external_root_records_manifest_json, value_placement_json,
    };

    #[test]
    fn accepted_machine_service_reach_distinguishes_public_empty_from_non_machine_rows() {
        let root = std::env::temp_dir().join(format!(
            "omega-trust-accepted-reach-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let writer = ArtifactWriter::new(&root).expect("artifact writer");
        let report = TrustReport {
            rows: vec![
                TrustReportRow {
                    commitment: "accepted fact: quiet_axiom".to_owned(),
                    provenance: "root grant (build.omg)".to_owned(),
                    machine_contract_fingerprint: Some(0xabcd),
                    machine_template_fingerprint: None,
                    machine_service_reach: Some(Vec::new()),
                    machine_synchronous_invocations: Some(Vec::new()),
                    machine_may_suspend: Some(false),
                    machine_may_block: Some(false),
                    machine_terminates_guarantee: Some(false),
                    machine_crash_routes: Some(Vec::new()),
                    standing_warning: false,
                },
                TrustReportRow {
                    commitment: "domain introduction: Meters".to_owned(),
                    provenance: "root grant (build.omg)".to_owned(),
                    machine_contract_fingerprint: None,
                    machine_template_fingerprint: None,
                    machine_service_reach: None,
                    machine_synchronous_invocations: None,
                    machine_may_suspend: None,
                    machine_may_block: None,
                    machine_terminates_guarantee: None,
                    machine_crash_routes: None,
                    standing_warning: false,
                },
                TrustReportRow {
                    commitment: "accepted fact: guarded_axiom".to_owned(),
                    provenance: "root grant (build.omg)".to_owned(),
                    machine_contract_fingerprint: Some(0xbcde),
                    machine_template_fingerprint: None,
                    machine_service_reach: Some(Vec::new()),
                    machine_synchronous_invocations: Some(Vec::new()),
                    machine_may_suspend: Some(false),
                    machine_may_block: Some(false),
                    machine_terminates_guarantee: Some(false),
                    machine_crash_routes: Some(vec![
                        TrustCrashRouteBucket {
                            cause: TrustCrashCause::Trap,
                            alternative_guards: vec![
                                TrustCrashRouteGuard::PredicateIdentity(vec![1]),
                                TrustCrashRouteGuard::PredicateIdentity(vec![2]),
                            ],
                        },
                        TrustCrashRouteBucket {
                            cause: TrustCrashCause::Abort,
                            alternative_guards: vec![TrustCrashRouteGuard::Truth],
                        },
                    ]),
                    standing_warning: false,
                },
            ],
            generic_accepted_instances: vec![TrustGenericAcceptedInstanceRow {
                template_commitment: "admitted".to_owned(),
                template_fingerprint: 0x1111,
                instance_fingerprint: 0x2222,
                instance_contract_fingerprint: 0xaaaa,
                type_argument_identities: vec!["named(name(Card))".to_owned()],
                const_argument_identities: vec!["named(name(1))".to_owned()],
                machine_argument_contract_fingerprints: vec![0x3333],
                conformance_argument_fingerprints: vec![0x4444],
            }],
            ..Default::default()
        };

        writer
            .write_trust_report(&report)
            .expect("trust report output");
        let output =
            std::fs::read_to_string(root.join("trust_report.md")).expect("written trust report");
        let accepted = output
            .lines()
            .find(|line| line.contains("accepted fact: quiet_axiom"))
            .expect("accepted fact row");
        let domain = output
            .lines()
            .find(|line| line.contains("domain introduction: Meters"))
            .expect("domain row");
        let guarded = output
            .lines()
            .find(|line| line.contains("accepted fact: guarded_axiom"))
            .expect("guarded accepted fact row");

        assert!(accepted.contains("service reach: none"));
        assert!(accepted.contains("synchronous invocations: none"));
        assert!(accepted.contains("may suspend: no"));
        assert!(accepted.contains("may block: no"));
        assert!(accepted.contains("termination guarantee: no"));
        assert!(accepted.contains("crash routes: none"));
        assert!(guarded.contains("crash routes: Trap[0x01 | 0x02], Abort[true]"));
        assert!(output.contains("accepted template: admitted [0000000000001111]"));
        assert!(output.contains("instance: 0000000000002222"));
        assert!(output.contains("instance contract: 000000000000aaaa"));
        assert!(output.contains("type argument identities: named(name(Card))"));
        assert!(output.contains("const argument identities: named(name(1))"));
        assert!(output.contains("machine argument contracts: 0000000000003333"));
        assert!(output.contains("conformance arguments: 0000000000004444"));
        assert!(!domain.contains("service reach:"));
        assert!(!domain.contains("synchronous invocations:"));
        assert!(!domain.contains("may suspend:"));
        assert!(!domain.contains("may block:"));
        assert!(!domain.contains("termination guarantee:"));
        assert!(!domain.contains("crash routes:"));
        std::fs::remove_dir_all(root).expect("remove test artifact directory");
    }

    #[test]
    fn trust_report_keeps_inherited_requirement_owner_separate_from_overload_identity() {
        let root = std::env::temp_dir().join(format!(
            "omega-trust-owner-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let writer = ArtifactWriter::new(&root).expect("artifact writer");
        let report = TrustReport {
            selected_provider_closure_fingerprint: 0xabcd,
            rows: Vec::new(),
            generic_accepted_instances: Vec::new(),
            provider_requirements: Vec::new(),
            qualifications: vec![TrustQualificationRow {
                provider_plan: "RootProvider::satisfies::Root".to_owned(),
                provider_plan_fingerprint: 0x1234,
                provider_type: "RootProvider".to_owned(),
                target: "windows_x64".to_owned(),
                provider_origin_package: "omega::providers::root".to_owned(),
                service_schema: "Root".to_owned(),
                calling_plan_fingerprint: Some(0xfeed),
                selected: false,
                requirement_owner: "Base".to_owned(),
                requirement_identity:
                    "named-callable(path(Base::enter), parameters(), result(none))".to_owned(),
                method: "enter".to_owned(),
                subject: "parameter:0".to_owned(),
                authority_flow: "accepts".to_owned(),
                domain: "Token::Granted".to_owned(),
                effective_carry: "strict".to_owned(),
                predicate_discharge_required: false,
                provenance: "own-package (dev-active)".to_owned(),
                grant_selectors: Vec::new(),
                standing_warning: true,
            }],
        };

        writer
            .write_trust_report(&report)
            .expect("trust report output");
        let output =
            std::fs::read_to_string(root.join("trust_report.md")).expect("written trust report");

        assert!(output.contains("requirement owner: Base"));
        assert!(output.contains("selected provider closure: 000000000000abcd"));
        assert!(output.contains("provider type: RootProvider"));
        assert!(output.contains("target: windows_x64"));
        assert!(output.contains("provider origin package: omega::providers::root"));
        assert!(output.contains("own-package (dev-active)"));
        assert!(output.contains("service schema: Root"));
        assert!(output.contains("calling plan: 000000000000feed"));
        assert!(output.contains("selected: no"));
        assert!(output.contains("requirement identity: named-callable(path(Base::enter)"));
        assert!(!output.contains("requirement owner: Root"));
        std::fs::remove_dir_all(root).expect("remove test artifact directory");
    }

    #[test]
    fn trust_report_keeps_claim_free_provider_requirement_blast_radius_exact() {
        let root = std::env::temp_dir().join(format!(
            "omega-trust-provider-requirement-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let writer = ArtifactWriter::new(&root).expect("artifact writer");
        let report = TrustReport {
            selected_provider_closure_fingerprint: 0x5678,
            rows: Vec::new(),
            generic_accepted_instances: Vec::new(),
            provider_requirements: vec![TrustProviderRequirementRow {
                provider_plan: "RootProvider::satisfies::Root".to_owned(),
                provider_plan_fingerprint: 0x1234,
                provider_type: String::new(),
                target: String::new(),
                provider_origin_package: String::new(),
                service_schema: "Root".to_owned(),
                calling_plan_fingerprint: None,
                selected: true,
                requirement_owner: "Base".to_owned(),
                requirement_identity:
                    "named-callable(path(Base::enter), parameters(), result(none))".to_owned(),
                method: "enter".to_owned(),
                parameter_type_identities: vec!["Token in Granted".to_owned()],
                result_type_identity: Some("Token".to_owned()),
                service_reach: vec!["Clock".to_owned(), "Storage".to_owned()],
                synchronous_invocations: vec!["Callback".to_owned()],
                may_suspend: true,
                may_block: false,
                terminates_guarantee: true,
                realization: TrustProviderRealization::VtableSlot { index: 4 },
                provenance: "root grant (build.omg)".to_owned(),
                grant_selectors: vec!["Root".to_owned()],
                standing_warning: false,
            }],
            qualifications: Vec::new(),
        };

        writer
            .write_trust_report(&report)
            .expect("trust report output");
        let output =
            std::fs::read_to_string(root.join("trust_report.md")).expect("written trust report");

        assert!(output.contains("provider requirements: 1"));
        assert!(output.contains("selected provider closure: 0000000000005678"));
        assert!(output.contains("provider plan: RootProvider::satisfies::Root [0000000000001234]"));
        assert!(output.contains("provider type: <free external>"));
        assert!(output.contains("target: <all>"));
        assert!(output.contains("provider origin package: <none>"));
        assert!(output.contains("service schema: Root"));
        assert!(output.contains("calling plan: <none>"));
        assert!(output.contains("selected: yes"));
        assert!(output.contains("requirement owner: Base"));
        assert!(output.contains("requirement identity: named-callable(path(Base::enter)"));
        assert!(output.contains("method: enter"));
        assert!(output.contains("parameter types: Token in Granted"));
        assert!(output.contains("result type: Token"));
        assert!(output.contains("service reach: Clock, Storage"));
        assert!(output.contains("synchronous invocations: Callback"));
        assert!(output.contains("may suspend: yes"));
        assert!(output.contains("may block: no"));
        assert!(output.contains("termination guarantee: yes"));
        assert!(output.contains("realization: vtable slot 4"));
        assert!(output.contains("root grant (build.omg)"));
        assert!(output.contains("grant selectors: Root"));
        assert!(!output.contains("requirement owner: Root"));
        assert!(!output.contains("STANDING WARNING"));
        std::fs::remove_dir_all(root).expect("remove test artifact directory");
    }

    #[test]
    fn trust_provider_realizations_distinguish_checked_and_opaque_leaves() {
        assert_eq!(
            TrustProviderRealization::CheckedAdapter {
                machine: "ConsoleProvider::write".to_owned(),
            }
            .report_text(),
            "checked adapter `ConsoleProvider::write`"
        );
        assert_eq!(
            TrustProviderRealization::Syscall { number: 60 }.report_text(),
            "syscall 60"
        );
    }

    #[test]
    fn value_placement_json_retains_indirect_copy_geometry() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::MicrosoftX64,
            &CallSignature {
                parameters: vec![ValueShape::integer(16, 8)],
                result: None,
            },
        )
        .expect("Microsoft x64 aggregate placement");
        let json = value_placement_json(&boundary.plan().call.parameters[0]);

        assert!(json.contains("\"indirect\""));
        assert!(json.contains("\"copy_stack_byte_offset\": 32"));
        assert!(json.contains("\"byte_size\": 16"));
    }

    fn root_id<T>(identity: u64, constructor: fn(u64) -> Result<T, ExternalRootDiagnostic>) -> T {
        constructor(identity).expect("normalized root identity")
    }

    fn fuel_schedule() -> FuelScheduleIdentity {
        FuelScheduleIdentity::new(1).expect("canonical test fuel schedule")
    }

    fn install_id<T>(
        identity: u64,
        constructor: fn(u64) -> Result<T, InstallationDiagnostic>,
    ) -> T {
        constructor(identity).expect("normalized installation identity")
    }

    fn entry_id(identity: u64) -> EntryStubId {
        EntryStubId::from_normalized_identity(identity).expect("normalized entry identity")
    }

    fn executable_container_fixture() -> Artifact {
        let artifact_id = install_id(900, ArtifactId::from_normalized_identity);
        let contracts = install_id(901, MachineContractSetId::from_normalized_identity);
        let footprint = install_id(902, MachineFootprintId::from_normalized_identity);
        let placement_plan = install_id(903, PlacementPlanId::from_normalized_identity);
        let entry_set = install_id(904, EntrySetId::from_normalized_identity);
        let relocation_set = install_id(905, RelocationSetId::from_normalized_identity);
        let code = vec![0xc3];
        let entries = vec![ArtifactEntry::from_canonical_decode(entry_id(906), 0)];
        let placement_constraints =
            PlacementConstraints::new(None, 1, PlacementPhase::Load, None, None)
                .expect("placement constraints");
        let decoded = DecodedArtifactContainer {
            format_marker: omega_executable_installation::OMEGA_EXECUTABLE_CONTAINER_MARKER,
            total_length: 1,
            artifact: artifact_id,
            content: install_id(907, ArtifactContentId::from_normalized_identity),
            architecture: Architecture::X86_64,
            code_length: code.len() as u64,
            code: code.clone(),
            contracts,
            declared_footprint: footprint,
            placement_plan,
            placement_constraints,
            entry_set,
            entries: entries.clone(),
            relocation_set,
            relocations: Vec::new(),
            proof_payload: omega_executable_installation::normalized_proof_payload_identity(b""),
            proof: Vec::new(),
            sections: Vec::new(),
        };
        let content =
            normalized_decoded_content_identity(&decoded).expect("normalized content identity");
        Artifact::from_canonical_decode(
            artifact_id,
            content,
            Architecture::X86_64,
            code,
            contracts,
            footprint,
            placement_plan,
            placement_constraints,
            entry_set,
            entries,
            relocation_set,
            Vec::new(),
        )
        .expect("canonical artifact")
    }

    #[test]
    fn writes_canonical_executable_container_atomically() {
        let root = std::env::temp_dir().join(format!(
            "omega-artifact-container-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let writer = ArtifactWriter::new(&root).expect("artifact writer");
        let limits = ContainerLimits {
            max_total_bytes: 64 * 1024,
            max_sections: 16,
            max_section_bytes: 32 * 1024,
            max_relocations: 64,
        };
        let artifact = executable_container_fixture();

        let path = writer
            .write_executable_container("program.omega-artifact", &artifact, b"proof", limits)
            .expect("canonical artifact output");
        let bytes = std::fs::read(&path).expect("written artifact bytes");
        let decoded =
            decode_executable_container(&bytes, limits).expect("written bytes remain canonical");

        assert_eq!(decoded.artifact(), &artifact);
        assert_eq!(decoded.proof(), b"proof");
        assert!(!root.join(".program.omega-artifact.tmp").exists());
        std::fs::remove_dir_all(root).expect("remove test artifact directory");
    }

    #[test]
    fn missing_entry_selection_does_not_infer_main_machine() {
        let mut program = CheckedTrees::default();
        let mut machine = Machine {
            symbol: SymbolHandle::default(),
            name: Identifier::generated("Main::main"),
            attached_data: None,
            owned_data: Default::default(),
            satisfies: Default::default(),
            states: Default::default(),
            ..Default::default()
        };
        program.typed.push_machine_state(
            &mut machine,
            State {
                symbol: SymbolHandle::default(),
                name: Identifier::generated("main"),
                parameters: Default::default(),
                return_type: Default::default(),
                ..Default::default()
            },
        );
        program.typed.push_machine(machine);

        let report = build_backend_surface_report(&program, None);

        assert!(report.entry_points.is_empty());
        assert_eq!(report.machines.len(), 1);
    }

    #[test]
    fn explicit_entry_selection_controls_surface_report() {
        let mut program = CheckedTrees::default();
        for (machine_name, state_name) in [("Main::main", "main"), ("Application::launch", "start")]
        {
            let mut machine = Machine {
                name: Identifier::generated(machine_name),
                ..Default::default()
            };
            program.typed.push_machine_state(
                &mut machine,
                State {
                    name: Identifier::generated(state_name),
                    ..Default::default()
                },
            );
            program.typed.push_machine(machine);
        }

        let report = build_backend_surface_report(&program, Some("Application::launch"));
        let entries = report
            .entry_points
            .iter()
            .map(|(_, entry)| (entry.machine.as_str(), entry.state.as_str()))
            .collect::<Vec<_>>();

        assert_eq!(entries, [("Application::launch", "start")]);
    }

    #[test]
    fn counts_contained_machines_from_attached_data_fields() {
        let worker_data_symbol = SymbolHandle::from_arena_index(1);
        let main_data_symbol = SymbolHandle::from_arena_index(2);
        let worker_machine_symbol = SymbolHandle::from_arena_index(3);
        let main_machine_symbol = SymbolHandle::from_arena_index(4);
        let worker_field_symbol = SymbolHandle::from_arena_index(5);
        let mut program = CheckedTrees::default();
        let worker_type = program.typed.type_reference_table.insert(
            psi_checked_trees::types::TypeReferenceNode::Named {
                symbol: worker_data_symbol,
                name: Identifier::generated("Worker"),
            },
        );

        program
            .typed
            .push_data_definition(psi_checked_trees::data::DataDefinition {
                symbol: worker_data_symbol,
                name: Identifier::generated("Worker"),
                ..Default::default()
            });
        let mut main_data = psi_checked_trees::data::DataDefinition {
            symbol: main_data_symbol,
            name: Identifier::generated("Main"),
            ..Default::default()
        };
        program.typed.push_data_member(
            &mut main_data,
            psi_checked_trees::data::DataMember::Field(psi_checked_trees::data::DataField {
                identity: None,
                symbol: worker_field_symbol,
                name: Identifier::generated("worker"),
                relevance: Default::default(),
                type_reference: worker_type,
            }),
        );
        program.typed.push_data_definition(main_data);
        program.typed.push_machine(Machine {
            symbol: worker_machine_symbol,
            name: Identifier::generated("Worker::run"),
            attached_data: Some(Identifier::generated("Worker")),
            ..Default::default()
        });
        program.typed.push_machine(Machine {
            symbol: main_machine_symbol,
            name: Identifier::generated("Main::main"),
            attached_data: Some(Identifier::generated("Main")),
            ..Default::default()
        });
        let targets = program.facts.carry.contained_targets.insert_many([
            psi_checked_trees::ContainedMachineTargetFact {
                machine: worker_machine_symbol,
            },
        ]);
        let fields = program.facts.carry.contained_fields.insert_many([
            psi_checked_trees::ContainedMachineFieldFact {
                field: worker_field_symbol,
                data: worker_data_symbol,
                type_reference: worker_type,
                targets,
            },
        ]);
        program.facts.carry.machine_topologies.insert(
            psi_checked_trees::MachineCarryTopologyFact {
                machine: worker_machine_symbol,
                fields: psi_arena::HandleSpan::empty(),
            },
        );
        program.facts.carry.machine_topologies.insert(
            psi_checked_trees::MachineCarryTopologyFact {
                machine: main_machine_symbol,
                fields,
            },
        );

        let report = build_backend_surface_report(&program, None);
        let main = report
            .machines
            .iter()
            .find_map(|(_, machine)| (machine.name == "Main::main").then_some(machine))
            .expect("main machine surface");

        assert_eq!(main.contained_machines, 1);
    }

    #[test]
    fn external_root_manifest_is_complete_normalized_and_address_free() {
        let boundary = evaluate_ordinary_boundary_entry_plan(
            CallingPolicy::SystemVAMD64,
            &CallSignature {
                parameters: vec![ValueShape::integer(8, 8)],
                result: None,
            },
        )
        .expect("boundary plan");
        let leaf = FixedFuelProviderSummary::from_admitted_provider(
            root_id(21, ProviderFuelSummaryId::from_normalized_identity),
            root_id(22, RootProviderId::from_normalized_identity),
            fuel_schedule(),
            4,
            BTreeSet::new(),
            root_id(
                23,
                ProviderFuelValidationReceiptId::from_normalized_identity,
            ),
        );
        let work_root = FixedFuelProviderSummary::from_admitted_provider(
            root_id(20, ProviderFuelSummaryId::from_normalized_identity),
            root_id(8, RootProviderId::from_normalized_identity),
            fuel_schedule(),
            3,
            BTreeSet::from([FixedFuelCall {
                callee: leaf.identity,
                maximum_invocations: 2,
            }]),
            root_id(
                24,
                ProviderFuelValidationReceiptId::from_normalized_identity,
            ),
        );
        let composed_fuel =
            compose_fixed_fuel(work_root.identity, [&work_root, &leaf]).expect("fixed fuel");
        let root_identity = root_id(1, ExternalRootId::from_normalized_identity);
        let nesting_identity = root_id(11, NestingRelationId::from_normalized_identity);
        let stack_summary = ProviderStackSummary::from_admitted_provider(
            root_identity,
            root_id(8, RootProviderId::from_normalized_identity),
            EntryStack::ProviderSelected,
            2048,
            16,
            root_id(29, StackValidationReceiptId::from_normalized_identity),
        );
        let composed_stack = compose_artifact_stacks(
            &StackNestingRelation {
                identity: nesting_identity,
                edges: BTreeSet::new(),
            },
            [&stack_summary],
        )
        .expect("stack composition")
        .demand(root_identity)
        .expect("root stack demand")
        .clone();
        let record = InstalledRootRecord {
            root: root_identity,
            normalized_root_identity: 0x101,
            entry: entry_id(2),
            installed_code: install_id(3, InstalledCodeId::from_normalized_identity),
            artifact: install_id(4, ArtifactId::from_normalized_identity),
            slot: root_id(5, RootSlotId::from_normalized_identity),
            owner: root_id(6, RootSlotOwnerId::from_normalized_identity),
            admission: root_id(7, RootAdmissionId::from_normalized_identity),
            provider_execution: root_id(30, ProviderExecutionId::from_normalized_identity),
            provider_execution_fingerprint: 0x3030,
            provider_exit_assurance: OpaqueProviderExitAssurance::AcceptedClaim {
                realization: ProviderExitRealization {
                    control: boundary.plan().call.entry_control,
                    restored_state: boundary.plan().state.restored_state,
                },
                validation_receipt: root_id(10, TrustReceiptId::from_normalized_identity),
            },
            provider_exit_assurance_fingerprint: 0x3031,
            provider_plan: root_id(31, ProviderPlanId::from_normalized_identity),
            requirement_identity: "TestRoot::entry".into(),
            entry_claims: Vec::new(),
            acknowledgement_parameter_index: None,
            interrupt_mask_guard_claim: None,
            boundary_contract_fingerprint: boundary.contract_fingerprint(),
            boundary: boundary.plan().clone(),
            provider: root_id(8, RootProviderId::from_normalized_identity),
            effects: BTreeSet::from([root_id(9, RootEffectId::from_normalized_identity)]),
            trust_receipts: BTreeSet::from([root_id(10, TrustReceiptId::from_normalized_identity)]),
            nesting_relation: nesting_identity,
            acknowledgement_policy: Some(root_id(
                12,
                AcknowledgementPolicyId::from_normalized_identity,
            )),
            stack: StackResourceColumn {
                ceiling_bytes: 8192,
                realization: composed_stack,
                validation_receipt: root_id(25, StackValidationReceiptId::from_normalized_identity),
            },
            logical_fuel: LogicalFuelResourceColumn {
                schedule: fuel_schedule(),
                provision: root_id(28, FuelProvisionId::from_normalized_identity),
                ceiling_units: 64,
                realization: composed_fuel,
                validation_receipt: root_id(26, FuelValidationReceiptId::from_normalized_identity),
            },
            machine_state: MachineStateResourceColumn {
                realization: StateFootprintEvidence::new(
                    RegisterSet::new([MachineRegister::X86Rax]),
                    MachineStateSet::new([MachineState::Flags]),
                ),
                validation_receipt: root_id(27, StateValidationReceiptId::from_normalized_identity),
            },
            component_pins: BTreeSet::from([ComponentVersionPin {
                contract: root_id(13, ComponentContractId::from_normalized_identity),
                artifact: root_id(14, ComponentArtifactId::from_normalized_identity),
                provider: root_id(15, ComponentProviderId::from_normalized_identity),
                version: root_id(16, ComponentVersionPinId::from_normalized_identity),
            }]),
        };

        let first = external_root_records_manifest_json(0x202, &[&record]);
        let second = external_root_records_manifest_json(0x202, &[&record]);
        let parsed: serde_json::Value = serde_json::from_str(&first).expect("valid JSON manifest");

        assert_eq!(first, second);
        assert_eq!(parsed["root_count"], 1);
        assert_eq!(
            parsed["roots"][0]["normalized_root_identity"],
            "0x0000000000000101"
        );
        assert_eq!(parsed["roots"][0]["entry"], "0x0000000000000002");
        assert_eq!(
            parsed["roots"][0]["provider_execution"],
            "0x000000000000001e"
        );
        assert_eq!(parsed["roots"][0]["provider_plan"], "0x000000000000001f");
        assert_eq!(
            parsed["roots"][0]["boundary_plan"]["call"]["policy"],
            "system_v_amd64"
        );
        assert_eq!(
            parsed["roots"][0]["boundary_plan"]["state"]["stack"],
            "provider_selected"
        );
        assert_eq!(
            parsed["roots"][0]["resources"]["stack"]["composed_wcsu_bytes"],
            2048
        );
        assert_eq!(
            parsed["roots"][0]["resources"]["stack"]["summary_evidence"][0]["origin"],
            "admitted_provider"
        );
        assert_eq!(
            parsed["roots"][0]["resources"]["logical_fuel"]["composed_units"],
            11
        );
        assert_eq!(
            parsed["roots"][0]["resources"]["logical_fuel"]["schedule_marker"],
            1
        );
        assert_eq!(
            parsed["roots"][0]["resources"]["logical_fuel"]["provision"],
            "0x000000000000001c"
        );
        assert_eq!(
            parsed["roots"][0]["resources"]["logical_fuel"]["summary_evidence"][0]["origin"],
            "admitted_provider"
        );
        assert_eq!(
            parsed["roots"][0]["resources"]["logical_fuel"]["summary_evidence"][1]["origin"],
            "admitted_provider"
        );
        assert!(
            parsed["roots"][0]["resources"]
                .get("structural_work")
                .is_none()
        );
        assert_eq!(
            parsed["roots"][0]["resources"]["machine_state"]["realized_registers"][0],
            "x86_rax"
        );
        assert_eq!(
            parsed["roots"][0]["resources"]["machine_state"]["ceiling"]["permitted_transitive_use_bits"],
            "0x0007"
        );
        assert_eq!(
            parsed["roots"][0]["resources"]["stack"]["validation_receipt"],
            "0x0000000000000019"
        );
        assert_eq!(parsed["roots"][0]["effects"][0], "0x0000000000000009");
        assert_eq!(
            parsed["roots"][0]["component_pins"][0]["version"],
            "0x0000000000000010"
        );
        assert!(!first.contains("entry_address"));
        assert!(!first.contains("code_address"));
        assert!(!first.contains("ranking"));
        assert!(!first.contains("codegen"));
    }
}
