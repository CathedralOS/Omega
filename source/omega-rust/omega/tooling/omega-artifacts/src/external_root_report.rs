//! Canonical provider/runtime-owned external-root ledger presentation.

use omega_external_roots::{InstalledRootLedger, InstalledRootRecord};

use super::calling_plan_json::push_hex_u16;
use psi_diagnostics::Diagnostic;

use super::{ArtifactWriter, calling_plan_json};

impl ArtifactWriter {
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

pub(crate) fn external_root_records_manifest_json(
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
    output.push_str(", \"normalized_root_report_identity\": ");
    push_hex_identity(output, record.normalized_root_report_identity);
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
    output.push_str(", \"provider_execution_report_fingerprint\": ");
    push_hex_identity(output, record.provider_execution_report_fingerprint);
    output.push_str(", \"provider_plan\": ");
    push_hex_identity(output, record.provider_plan.normalized_identity());
    output.push_str(", \"native_fuel\": {\"kind\": ");
    output.push('"');
    output.push_str(match record.native_fuel_kind {
        omega_external_roots::NativeFuelRealizationKind::FixedProvision => "fixed_provision",
        omega_external_roots::NativeFuelRealizationKind::DynamicMetering => "dynamic_metering",
        omega_external_roots::NativeFuelRealizationKind::Interpreted => "interpreted",
    });
    output.push('"');
    output.push_str(", \"report_fingerprint\": ");
    push_hex_identity(output, record.native_fuel_report_fingerprint);
    output.push('}');
    output.push_str(", \"selected_provider_closure_report_fingerprint\": ");
    push_hex_identity(output, record.selected_provider_closure_report_fingerprint);
    output.push_str(", \"selected_provider_closure_digest\": ");
    push_hex_digest(output, record.selected_provider_closure_digest.as_bytes());
    output.push_str(", \"boundary_contract_report_fingerprint\": ");
    push_hex_identity(output, record.boundary_contract_report_fingerprint);
    output.push_str(", \"boundary_plan\": ");
    calling_plan_json::push_boundary_plan_json(output, &record.boundary);
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
    calling_plan_json::push_entry_stack_json(output, record.boundary.state.stack);
    let root_input = record
        .stack
        .realization
        .input(record.root)
        .expect("installed stack composition retains its root input");
    let root_demand = record
        .stack
        .realization
        .demand(record.root)
        .expect("installed stack composition retains its root demand");
    output.push_str(", \"local_wcsu_bytes\": ");
    output.push_str(&root_input.pure().body_wcsu_bytes.to_string());
    output.push_str(", \"composed_wcsu_bytes\": ");
    output.push_str(
        &root_demand
            .domains()
            .map(|(_, demand)| demand.bytes)
            .max()
            .unwrap_or_default()
            .to_string(),
    );
    output.push_str(", \"alignment\": ");
    output.push_str(
        &root_demand
            .domains()
            .map(|(_, demand)| demand.alignment)
            .max()
            .unwrap_or_default()
            .to_string(),
    );
    output.push_str(", \"composed_domains\": [");
    for (index, (domain, demand)) in root_demand.domains().enumerate() {
        if index != 0 {
            output.push_str(", ");
        }
        output.push_str("{\"domain\": ");
        push_stack_domain_json(output, domain);
        output.push_str(", \"bytes\": ");
        output.push_str(&demand.bytes.to_string());
        output.push_str(", \"alignment\": ");
        output.push_str(&demand.alignment.to_string());
        output.push('}');
    }
    output.push(']');
    output.push_str(", \"composition_fingerprint\": ");
    push_hex_identity(output, record.stack.realization.fingerprint());
    output.push_str(", \"artifact_composition_fingerprint\": ");
    push_hex_identity(output, record.stack.realization.composition().fingerprint());
    output.push_str(", \"contributing_roots\": [");
    push_identity_set(
        output,
        root_demand
            .contributing_roots()
            .iter()
            .map(|identity| identity.normalized_identity()),
    );
    output.push_str("], \"provider_validation_receipts\": [");
    let mut stack_receipts = std::collections::BTreeSet::new();
    for (_, input) in record.stack.realization.inputs() {
        if let Some(receipt) = input.body_evidence().provider_validation_receipt() {
            stack_receipts.insert(receipt);
        }
        if let Some(receipt) = input.realization_evidence().validation_receipt() {
            stack_receipts.insert(receipt);
        }
    }
    push_identity_set(
        output,
        stack_receipts
            .iter()
            .map(|identity| identity.normalized_identity()),
    );
    output.push_str("], \"summary_evidence\": [");
    for (index, (root, input)) in record.stack.realization.inputs().enumerate() {
        if index != 0 {
            output.push_str(", ");
        }
        output.push_str("{\"root\": ");
        push_hex_identity(output, root.normalized_identity());
        output.push_str(", \"provider\": ");
        push_hex_identity(output, input.provider().normalized_identity());
        output.push_str(", \"local_wcsu_bytes\": ");
        output.push_str(&input.pure().body_wcsu_bytes.to_string());
        output.push_str(", \"alignment\": ");
        output.push_str(&input.pure().body_wcsu_alignment.to_string());
        match input.body_evidence() {
            omega_external_roots::StackLocalEvidence::TerminalEntry(binding) => {
                output.push_str(", \"origin\": \"entry\", \"terminal_vocabulary_marker\": ");
                output.push_str(&binding.psi().vocabulary_marker.get().to_string());
                output.push_str(", \"terminal_fingerprint\": \"");
                output.push_str(&binding.psi().program_fingerprint.to_string());
                output.push_str("\", \"entry\": ");
                push_hex_identity(output, binding.machine_entry().get());
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
        let entry_evidence = input.realization_evidence();
        output.push_str(", \"arrival_origin\": \"");
        output.push_str(match entry_evidence.arrival_origin() {
            omega_external_roots::ArrivalStackRealizationOrigin::NoHardwareArrival => {
                "no_hardware_arrival"
            }
            omega_external_roots::ArrivalStackRealizationOrigin::X86_64TargetRule => {
                "x86_64_target_rule"
            }
            omega_external_roots::ArrivalStackRealizationOrigin::OpaqueProvider => {
                "opaque_provider"
            }
        });
        output.push_str("\", \"adapter_origin\": \"");
        output.push_str(match entry_evidence.adapter_origin() {
            omega_external_roots::AdapterStackRealizationOrigin::None => "none",
            omega_external_roots::AdapterStackRealizationOrigin::OpaqueProvider => {
                "opaque_provider"
            }
        });
        output.push_str("\", \"entry_target\": \"");
        output.push_str(match entry_evidence.architecture() {
            omega_target::Architecture::X86_64 => "x86_64",
            omega_target::Architecture::Aarch64 => "aarch64",
        });
        output.push_str("\", \"entry_installed_code\": ");
        push_hex_identity(
            output,
            entry_evidence.installed_code().normalized_identity(),
        );
        output.push_str(", \"entry_artifact\": ");
        push_hex_identity(output, entry_evidence.artifact().normalized_identity());
        output.push_str(", \"entry_stub\": ");
        push_hex_identity(output, entry_evidence.entry().normalized_identity());
        output.push_str(", \"entry_boundary_contract\": ");
        push_hex_identity(output, entry_evidence.boundary_contract_fingerprint());
        output.push_str(", \"body_domains\": [");
        for (domain_index, (context, domain)) in entry_evidence.body_domains().iter().enumerate() {
            if domain_index > 0 {
                output.push_str(", ");
            }
            output.push_str("{\"context\": ");
            push_hex_identity(output, context.get());
            output.push_str(", \"domain\": ");
            push_stack_domain_ref_json(output, *domain);
            output.push('}');
        }
        output.push(']');
        output.push_str(", \"entry_realization_fingerprint\": ");
        push_hex_identity(output, entry_evidence.realization().fingerprint());
        output.push_str(", \"target_arrival_rule_fingerprint\": ");
        if let Some(fingerprint) = entry_evidence.target_rule_fingerprint() {
            push_hex_identity(output, fingerprint);
        } else {
            output.push_str("null");
        }
        output.push_str(", \"opaque_validation_receipt\": ");
        if let Some(receipt) = entry_evidence.validation_receipt() {
            push_hex_identity(output, receipt.normalized_identity());
        } else {
            output.push_str("null");
        }
        output.push_str(", \"arrival_contexts\": ");
        push_entry_stack_realization_json(output, entry_evidence.realization());
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
                output.push_str(", \"origin\": \"entry\", \"terminal_vocabulary_marker\": ");
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
    calling_plan_json::push_register_set_json(output, record.machine_state.realization.registers());
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

fn push_entry_stack_realization_json(
    output: &mut String,
    realization: &omega_calling_conventions::ValidatedEntryStackRealization,
) {
    output.push('[');
    for (context_index, context) in realization.realization().contexts.iter().enumerate() {
        if context_index != 0 {
            output.push_str(", ");
        }
        output.push_str("{\"context\": ");
        push_hex_identity(output, context.context.get());
        output.push_str(", \"epochs\": [");
        for (epoch_index, epoch) in context.epochs.iter().enumerate() {
            if epoch_index != 0 {
                output.push_str(", ");
            }
            output.push_str("{\"stage\": \"");
            output.push_str(match epoch.stage {
                omega_calling_conventions::EntryStackStage::Enter => "enter",
                omega_calling_conventions::EntryStackStage::Body => "body",
                omega_calling_conventions::EntryStackStage::Exit => "exit",
            });
            output.push_str("\", \"active_domain\": ");
            push_stack_domain_ref_json(output, epoch.active_domain);
            output.push_str(", \"occupancy\": [");
            for (occupancy_index, occupancy) in epoch.occupancy_by_domain.iter().enumerate() {
                if occupancy_index != 0 {
                    output.push_str(", ");
                }
                output.push_str("{\"domain\": ");
                push_stack_domain_ref_json(output, occupancy.domain);
                output.push_str(", \"bytes\": ");
                output.push_str(&occupancy.bytes.to_string());
                output.push_str(", \"alignment\": ");
                output.push_str(&occupancy.alignment.to_string());
                output.push('}');
            }
            output.push_str("], \"nesting\": ");
            push_preemption_json(output, epoch.nesting);
            output.push('}');
        }
        output.push_str("]}");
    }
    output.push(']');
}

fn push_stack_domain_ref_json(
    output: &mut String,
    domain: omega_calling_conventions::StackDomainRef,
) {
    match domain {
        omega_calling_conventions::StackDomainRef::Interrupted => {
            output.push_str("{\"kind\": \"interrupted\"}")
        }
        omega_calling_conventions::StackDomainRef::Dedicated { class } => {
            output.push_str("{\"kind\": \"dedicated\", \"class\": ");
            output.push_str(&class.to_string());
            output.push('}');
        }
        omega_calling_conventions::StackDomainRef::ProviderSelected => {
            output.push_str("{\"kind\": \"provider_selected\"}")
        }
    }
}

fn push_stack_domain_json(output: &mut String, domain: omega_external_roots::StackDomain) {
    match domain {
        omega_external_roots::StackDomain::Interrupted => {
            output.push_str("{\"kind\": \"interrupted\"}")
        }
        omega_external_roots::StackDomain::Dedicated { class } => {
            output.push_str("{\"kind\": \"dedicated\", \"class\": ");
            output.push_str(&class.to_string());
            output.push('}');
        }
        omega_external_roots::StackDomain::ProviderSelected => {
            output.push_str("{\"kind\": \"provider_selected\"}")
        }
    }
}

fn push_preemption_json(output: &mut String, preemption: omega_calling_conventions::Preemption) {
    match preemption {
        omega_calling_conventions::Preemption::NotApplicable => {
            output.push_str("{\"kind\": \"not_applicable\"}")
        }
        omega_calling_conventions::Preemption::Masked => output.push_str("{\"kind\": \"masked\"}"),
        omega_calling_conventions::Preemption::Nestable { maximum_depth } => {
            output.push_str("{\"kind\": \"nestable\", \"maximum_depth\": ");
            output.push_str(&maximum_depth.to_string());
            output.push('}');
        }
        omega_calling_conventions::Preemption::ProviderDefined => {
            output.push_str("{\"kind\": \"provider_defined\"}")
        }
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

fn push_hex_digest(output: &mut String, digest: &[u8; 32]) {
    output.push('"');
    output.push_str("0x");
    for byte in digest {
        output.push_str(&format!("{byte:02x}"));
    }
    output.push('"');
}
