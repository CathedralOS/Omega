//! Non-authoritative program-storage installation audit records.

use omega_artifacts::ArtifactWriter;
use psi_diagnostics::Diagnostic;
use std::path::Path;

pub const PROGRAM_STORAGE_INSTALLATION_ARTIFACT: &str = "10_program_storage_installation.json";

pub fn program_storage_installation_record_json(
    record: &crate::ProgramStorageInstallationRecord,
) -> String {
    let binding = record.binding();
    let mut output = String::from(
        "{\n  \"authority\": \"non_authoritative_audit_record\",\n  \"installation_status\": \"completed\",\n  \"root_slot\": \"",
    );
    push_normalized_identity(&mut output, binding.root_slot().normalized_identity());
    output.push_str("\",\n  \"semantic_requirement\": ");
    push_json_string(&mut output, binding.requirement_identity());
    output.push_str(",\n  \"semantic_continuation_calling_plan_fingerprint\": \"");
    push_normalized_identity(&mut output, binding.boundary_contract_fingerprint());
    output.push_str("\",\n  \"root_provider_invocation\": ");
    if let Some(invocation) = record.provider_invocation() {
        output.push_str("{\"provider\": \"");
        push_normalized_identity(&mut output, invocation.provider().normalized_identity());
        output.push_str("\", \"provider_plan\": \"");
        push_normalized_identity(
            &mut output,
            invocation.provider_plan().normalized_identity(),
        );
        output.push_str("\", \"invocation\": \"");
        push_normalized_identity(&mut output, invocation.invocation().normalized_identity());
        output.push_str("\"}");
    } else {
        output.push_str("null");
    }
    output.push_str(",\n  \"roots\": [\n    ");
    push_installed_root(
        &mut output,
        "image",
        binding.image().parameter_index(),
        record.image(),
    );
    output.push_str(",\n    ");
    push_installed_root(
        &mut output,
        "initial_storage",
        binding.initial_storage().parameter_index(),
        record.initial_storage(),
    );
    output.push_str("\n  ],\n  \"receiver_storage\": ");
    if let Some(receiver) = record.receiver() {
        output.push_str("{\"status\": \"reserved\", \"initialization\": \"bridge_required\", \"activation\": \"one_exclusive_loan\", \"type_identity\": ");
        push_json_string(&mut output, receiver.type_identity());
        output.push_str(", \"base\": \"");
        push_normalized_identity(&mut output, receiver.base());
        output.push_str("\", \"length\": \"");
        push_normalized_identity(&mut output, receiver.length());
        output.push_str("\", \"end\": \"");
        push_normalized_identity(&mut output, receiver.end());
        output.push_str("\", \"alignment\": ");
        output.push_str(&receiver.alignment().to_string());
        output.push_str(", \"initial_storage_offset\": ");
        output.push_str(&receiver.initial_storage_offset().to_string());
        output.push_str(", \"lineage_root\": \"");
        push_normalized_identity(&mut output, receiver.lineage_root().normalized_identity());
        output.push_str("\"}");
    } else {
        output.push_str("null");
    }
    output.push_str("\n}\n");
    output
}

pub(crate) fn write_program_storage_installation_record(
    artifact_directory: &Path,
    record: &crate::ProgramStorageInstallationRecord,
) -> Result<(), Diagnostic> {
    ArtifactWriter::new(artifact_directory)?.write_text(
        PROGRAM_STORAGE_INSTALLATION_ARTIFACT,
        &program_storage_installation_record_json(record),
    )
}

fn push_installed_root(
    output: &mut String,
    role: &str,
    parameter_index: usize,
    root: &crate::ProgramStorageInstalledExtentRecord,
) {
    output.push_str("{\"role\": ");
    push_json_string(output, role);
    output.push_str(", \"parameter_index\": ");
    output.push_str(&parameter_index.to_string());
    output.push_str(", \"base\": \"");
    push_normalized_identity(output, root.base());
    output.push_str("\", \"length\": \"");
    push_normalized_identity(output, root.length());
    output.push_str("\", \"end\": \"");
    push_normalized_identity(output, root.end());
    output.push_str("\", \"address_space\": \"");
    push_normalized_identity(output, root.address_space().normalized_identity());
    output.push_str("\", \"rights\": [");
    for (index, right) in root.rights().iter().enumerate() {
        if index != 0 {
            output.push_str(", ");
        }
        output.push('"');
        push_normalized_identity(output, right.normalized_identity());
        output.push('"');
    }
    output.push_str("], \"provenance\": \"");
    push_normalized_identity(output, root.provenance().normalized_identity());
    output.push_str("\", \"mapping_era\": \"");
    push_normalized_identity(output, root.mapping_era().normalized_identity());
    output.push_str("\", \"lineage_root\": \"");
    push_normalized_identity(output, root.lineage_root().normalized_identity());
    output.push_str("\", \"origin\": ");
    push_extent_root_origin(output, root.origin());
    output.push('}');
}

fn push_extent_root_origin(output: &mut String, origin: psi_extents::ExtentRootOrigin) {
    match origin {
        psi_extents::ExtentRootOrigin::ProviderIssued(issuance) => {
            let invocation = issuance.invocation();
            output.push_str("{\"kind\": \"provider_issued\", \"issuance\": \"");
            push_normalized_identity(output, issuance.issuance().normalized_identity());
            output.push_str("\", \"backing\": \"");
            push_normalized_identity(output, issuance.backing().normalized_identity());
            output.push_str("\", \"provider\": \"");
            push_normalized_identity(output, issuance.provider().normalized_identity());
            output.push_str("\", \"live_issuance_premise\": \"");
            push_normalized_identity(
                output,
                issuance.live_issuance_premise().normalized_identity(),
            );
            output.push_str("\", \"custody_root\": \"");
            push_normalized_identity(output, issuance.custody_root().normalized_identity());
            output.push_str("\", \"alias_class\": \"");
            push_normalized_identity(output, issuance.alias_class().normalized_identity());
            output.push_str("\", \"correspondence\": \"");
            push_normalized_identity(output, issuance.correspondence().normalized_identity());
            output.push_str("\", \"trust_provenance\": \"");
            push_normalized_identity(output, issuance.trust_provenance().normalized_identity());
            output.push_str("\", \"invocation\": {\"provider_plan\": \"");
            push_normalized_identity(output, invocation.provider_plan().normalized_identity());
            output.push_str("\", \"invocation\": \"");
            push_normalized_identity(output, invocation.invocation().normalized_identity());
            output.push_str("\", \"establishment_route\": \"");
            push_normalized_identity(
                output,
                invocation.establishment_route().normalized_identity(),
            );
            output.push_str("\", \"capacity\": \"");
            push_normalized_identity(output, invocation.capacity().normalized_identity());
            output.push_str("\", \"qualification\": \"");
            push_normalized_identity(output, invocation.qualification().normalized_identity());
            output.push_str("\"}}");
        }
        psi_extents::ExtentRootOrigin::ProgramLocal(origin) => {
            output.push_str("{\"kind\": \"program_local\", \"installed_code\": \"");
            push_normalized_identity(output, origin.installed_code());
            output.push_str("\", \"external_root\": \"");
            push_normalized_identity(output, origin.external_root());
            output.push_str("\", \"root_slot\": \"");
            push_normalized_identity(output, origin.root_slot());
            output.push_str("\", \"schema_identity\": \"");
            push_normalized_identity(output, origin.schema_identity());
            output.push_str("\", \"lifecycle_ledger\": \"");
            push_normalized_identity(output, origin.lifecycle_ledger());
            output.push_str("\", \"lifecycle_epoch\": \"");
            push_normalized_identity(output, origin.lifecycle_epoch());
            output.push_str("\", \"entry_invocation\": \"");
            push_normalized_identity(output, origin.entry_invocation());
            output.push_str("\", \"subject_place\": \"");
            push_normalized_identity(output, origin.subject_place());
            output.push_str("\"}");
        }
    }
}

fn push_normalized_identity(output: &mut String, identity: u64) {
    output.push_str(&format!("0x{identity:016x}"));
}

fn push_json_string(output: &mut String, value: &str) {
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character => output.push(character),
        }
    }
    output.push('"');
}
