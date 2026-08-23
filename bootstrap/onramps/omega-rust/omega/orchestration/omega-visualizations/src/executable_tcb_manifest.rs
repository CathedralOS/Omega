use omega_effects::{
    ExecutableEntryOrigin, ExecutableIdentity, ExecutableTcbEntry, ExecutionScope,
    ImplementationEvidence, IncompleteCause, OpaqueInProcessBinding, ProviderIdentity,
    ScopeCompleteness, SelectedProviderPlanFacts,
};
use std::fmt::Write;

/// Stable artifact surface for the executable TCB facts derivable from the
/// exact selected-provider closure.
pub fn executable_tcb_manifest_json(selected: &SelectedProviderPlanFacts) -> String {
    let manifest = selected.executable_tcb_manifest();
    executable_tcb_manifest_value_json(&manifest)
}

/// Stable artifact surface for a manifest after static/runtime union.
pub fn executable_tcb_manifest_value_json(
    manifest: &omega_effects::ExecutableTcbManifest,
) -> String {
    let mut json = String::from("{\n  \"known_entries\": [");
    for (index, entry) in manifest.known_entries.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push_str("\n    {");
        push_entry_json(&mut json, entry);
        json.push_str("\n    }");
    }
    if !manifest.known_entries.is_empty() {
        json.push('\n');
        json.push_str("  ");
    }
    json.push_str("],\n  \"completeness\": {");
    match &manifest.completeness {
        ScopeCompleteness::Complete {
            scope,
            selected_provider_closure_identity,
            opaque_closure_evidence,
            runtime_closure_evidence,
        } => {
            json.push_str("\n    \"status\": \"complete\",\n    \"scope\": ");
            push_execution_scope_json(&mut json, *scope);
            json.push_str(",\n    \"evidence\": [\n      {\"kind\": \"selected_provider_closure\", \"identity\": ");
            push_json_string(
                &mut json,
                &format!("0x{selected_provider_closure_identity:016x}"),
            );
            json.push('}');
            for evidence in opaque_closure_evidence {
                json.push_str(",\n      {\"kind\": \"admitted_opaque_executable_closure\", \"provider_plan_identity\": ");
                push_json_string(
                    &mut json,
                    &format!("0x{:016x}", evidence.provider_plan_identity),
                );
                json.push_str(", \"method\": ");
                push_json_string(&mut json, &evidence.method);
                json.push_str(", \"requirement_identity\": ");
                push_json_string(&mut json, &evidence.requirement_identity);
                json.push_str(", \"evidence_identity\": ");
                push_json_string(&mut json, &evidence.evidence_identity);
                json.push('}');
            }
            push_runtime_closure_evidence_json(&mut json, runtime_closure_evidence, true);
            json.push_str("\n    ]");
        }
        ScopeCompleteness::Incomplete {
            scope,
            causes,
            opaque_closure_evidence,
            runtime_closure_evidence,
        } => {
            json.push_str("\n    \"status\": \"incomplete\",\n    \"scope\": ");
            push_execution_scope_json(&mut json, *scope);
            json.push_str(",\n    \"causes\": [");
            for (index, cause) in causes.iter().enumerate() {
                if index > 0 {
                    json.push(',');
                }
                push_incomplete_cause_json(&mut json, cause);
            }
            if !causes.is_empty() {
                json.push('\n');
                json.push_str("    ");
            }
            json.push(']');
            json.push_str(",\n    \"evidence\": [");
            for (index, evidence) in opaque_closure_evidence.iter().enumerate() {
                if index > 0 {
                    json.push(',');
                }
                json.push_str("\n      {\"kind\": \"admitted_opaque_executable_closure\", \"provider_plan_identity\": ");
                push_json_string(
                    &mut json,
                    &format!("0x{:016x}", evidence.provider_plan_identity),
                );
                json.push_str(", \"method\": ");
                push_json_string(&mut json, &evidence.method);
                json.push_str(", \"requirement_identity\": ");
                push_json_string(&mut json, &evidence.requirement_identity);
                json.push_str(", \"evidence_identity\": ");
                push_json_string(&mut json, &evidence.evidence_identity);
                json.push('}');
            }
            push_runtime_closure_evidence_json(
                &mut json,
                runtime_closure_evidence,
                !opaque_closure_evidence.is_empty(),
            );
            if !opaque_closure_evidence.is_empty() || !runtime_closure_evidence.is_empty() {
                json.push('\n');
                json.push_str("    ");
            }
            json.push(']');
        }
    }
    json.push_str("\n  }\n}\n");
    json
}

/// Stable artifact surface retaining the caller manifest and each separately
/// evaluated isolated-provider manifest.
pub fn executable_tcb_manifest_set_json(set: &omega_effects::ExecutableTcbManifestSet) -> String {
    let mut json = String::from("{\n  \"root_manifest\": ");
    json.push_str(executable_tcb_manifest_value_json(set.root()).trim());
    json.push_str(",\n  \"isolated_scopes\": [");
    for (index, isolated) in set.isolated().iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push_str("\n    {\"manifest_receipt_identity\": ");
        push_json_string(&mut json, isolated.manifest_receipt_identity());
        json.push_str(", \"manifest\": ");
        json.push_str(executable_tcb_manifest_value_json(isolated.manifest()).trim());
        json.push('}');
    }
    if !set.isolated().is_empty() {
        json.push('\n');
        json.push_str("  ");
    }
    json.push_str("]\n}\n");
    json
}

fn push_incomplete_cause_json(json: &mut String, cause: &IncompleteCause) {
    json.push_str("\n      {\n        \"provider\": ");
    match cause {
        IncompleteCause::SelectedOpaqueProvider {
            provider_identity,
            provider_plan_identity,
            method,
            requirement_identity,
            binding,
        } => {
            push_provider_identity_json(json, provider_identity);
            json.push_str(",\n        \"provider_plan_identity\": ");
            push_json_string(json, &format!("0x{provider_plan_identity:016x}"));
            json.push_str(",\n        \"method\": ");
            push_json_string(json, method);
            json.push_str(",\n        \"requirement_identity\": ");
            push_json_string(json, requirement_identity);
            json.push_str(",\n        \"reason\": \"uncontained_opaque_in_process_provider\",\n        \"binding\": ");
            push_opaque_binding_json(json, binding);
        }
        IncompleteCause::OmegaRuntimeAdmission {
            provider_identity,
            provider_plan_identity,
            executable_identity,
            admission_receipt_identity,
        } => {
            push_provider_identity_json(json, provider_identity);
            json.push_str(",\n        \"provider_plan_identity\": ");
            push_json_string(json, &format!("0x{provider_plan_identity:016x}"));
            json.push_str(",\n        \"executable_identity\": ");
            push_json_string(json, executable_identity);
            json.push_str(",\n        \"admission_receipt_identity\": ");
            push_json_string(json, admission_receipt_identity);
            json.push_str(
                ",\n        \"reason\": \"runtime_admission_without_executable_closure\"",
            );
        }
    }
    json.push_str("\n      }");
}

fn push_runtime_closure_evidence_json(
    json: &mut String,
    evidence: &[omega_effects::RuntimeExecutableClosureEvidence],
    mut has_prior: bool,
) {
    for evidence in evidence {
        if has_prior {
            json.push(',');
        }
        has_prior = true;
        json.push_str("\n      {\"kind\": \"omega_runtime_executable_closure\", \"provider\": ");
        push_provider_identity_json(json, &evidence.provider_identity);
        json.push_str(", \"provider_plan_identity\": ");
        push_json_string(json, &format!("0x{:016x}", evidence.provider_plan_identity));
        json.push_str(", \"executable_identity\": ");
        push_json_string(json, &evidence.executable_identity);
        json.push_str(", \"admission_receipt_identity\": ");
        push_json_string(json, &evidence.admission_receipt_identity);
        json.push_str(", \"evidence_identity\": ");
        push_json_string(json, &evidence.evidence_identity);
        json.push('}');
    }
}

fn push_entry_json(json: &mut String, entry: &ExecutableTcbEntry) {
    json.push_str("\n      \"provider\": ");
    push_provider_identity_json(json, &entry.provider_identity);
    json.push_str(",\n      \"provider_plan_identity\": ");
    push_json_string(json, &format!("0x{:016x}", entry.provider_plan_identity));
    json.push_str(",\n      \"selected_requirement\": ");
    if let Some(requirement) = &entry.selected_requirement {
        json.push_str("{\"method\": ");
        push_json_string(json, &requirement.method);
        json.push_str(", \"identity\": ");
        push_json_string(json, &requirement.requirement_identity);
        json.push('}');
    } else {
        json.push_str("null");
    }
    json.push_str(",\n      \"executable\": ");
    match &entry.executable_identity {
        ExecutableIdentity::CurrentArtifactMachine(machine) => {
            json.push_str("{\"kind\": \"current_artifact_machine\", \"identity\": ");
            push_json_string(json, machine);
            json.push('}');
        }
        ExecutableIdentity::CurrentArtifactIntrinsic { target, machine } => {
            json.push_str("{\"kind\": \"current_artifact_intrinsic\", \"target\": ");
            push_json_string(json, target);
            json.push_str(", \"identity\": ");
            push_json_string(json, machine);
            json.push('}');
        }
        ExecutableIdentity::PinnedOpaqueArtifact(identity) => {
            json.push_str("{\"kind\": \"pinned_opaque_artifact\", \"identity\": ");
            push_json_string(json, identity);
            json.push('}');
        }
        ExecutableIdentity::IsolatedProviderEndpoint {
            scope_identity,
            endpoint_identity,
        } => {
            json.push_str("{\"kind\": \"isolated_provider_endpoint\", \"scope_identity\": ");
            push_json_string(json, &format!("0x{scope_identity:016x}"));
            json.push_str(", \"identity\": ");
            push_json_string(json, endpoint_identity);
            json.push('}');
        }
    }
    json.push_str(",\n      \"implementation_evidence\": ");
    match &entry.implementation_evidence {
        ImplementationEvidence::CheckedBody { machine } => {
            json.push_str("{\"class\": \"checked_body\", \"identity\": ");
            push_json_string(json, machine);
            json.push('}');
        }
        ImplementationEvidence::CompilerKnown { target, machine } => {
            json.push_str("{\"class\": \"compiler_known\", \"target\": ");
            push_json_string(json, target);
            json.push_str(", \"identity\": ");
            push_json_string(json, machine);
            json.push('}');
        }
        ImplementationEvidence::AdmittedOpaque { receipt_identity } => {
            json.push_str("{\"class\": \"admitted_opaque\", \"receipt_identity\": ");
            push_json_string(json, receipt_identity);
            json.push('}');
        }
        ImplementationEvidence::AdmittedIsolatedEndpoint {
            endpoint_receipt_identity,
            isolated_manifest_receipt_identity,
        } => {
            json.push_str(
                "{\"class\": \"admitted_isolated_endpoint\", \"endpoint_receipt_identity\": ",
            );
            push_json_string(json, endpoint_receipt_identity);
            json.push_str(", \"isolated_manifest_receipt_identity\": ");
            push_json_string(json, isolated_manifest_receipt_identity);
            json.push('}');
        }
    }
    json.push_str(",\n      \"origin\": ");
    push_json_string(
        json,
        match entry.origin {
            ExecutableEntryOrigin::StaticSelection => "static_selection",
            ExecutableEntryOrigin::OmegaRuntimeAdmission => "omega_runtime_admission",
        },
    );
    json.push_str(",\n      \"execution_scope\": ");
    push_execution_scope_json(json, entry.execution_scope);
    json.push_str(",\n      \"containment\": [");
    for (index, evidence) in entry.containment.iter().enumerate() {
        if index > 0 {
            json.push_str(", ");
        }
        json.push_str("{\"guarantee\": ");
        push_json_string(json, containment_guarantee_name(evidence.guarantee));
        json.push_str(", \"evidence_identity\": ");
        push_json_string(json, &evidence.evidence_identity);
        json.push('}');
    }
    json.push(']');
}

fn push_provider_identity_json(json: &mut String, identity: &ProviderIdentity) {
    match identity {
        ProviderIdentity::NominalType(name) => {
            json.push_str("{\"kind\": \"nominal_type\", \"identity\": ");
            push_json_string(json, name);
        }
        ProviderIdentity::FreeExternalPlan(name) => {
            json.push_str("{\"kind\": \"free_external_plan\", \"identity\": ");
            push_json_string(json, name);
        }
    }
    json.push('}');
}

fn push_opaque_binding_json(json: &mut String, binding: &OpaqueInProcessBinding) {
    match binding {
        OpaqueInProcessBinding::Import { library, symbol } => {
            json.push_str("{\"kind\": \"import\", \"library\": ");
            push_json_string(json, library);
            json.push_str(", \"symbol\": ");
            push_json_string(json, symbol);
        }
        OpaqueInProcessBinding::VtableSlot { index } => {
            let _ = write!(json, "{{\"kind\": \"vtable_slot\", \"index\": {index}");
        }
        OpaqueInProcessBinding::VtableField { table, field } => {
            json.push_str("{\"kind\": \"vtable_field\", \"table\": ");
            push_json_string(json, table);
            json.push_str(", \"field\": ");
            push_json_string(json, field);
        }
        OpaqueInProcessBinding::TableFunction { table, field } => {
            json.push_str("{\"kind\": \"table_function\", \"table\": ");
            push_json_string(json, table);
            json.push_str(", \"field\": ");
            push_json_string(json, field);
        }
    }
    json.push('}');
}

fn push_execution_scope_json(json: &mut String, scope: ExecutionScope) {
    match scope {
        ExecutionScope::CallerAddressSpace => push_json_string(json, "caller_address_space"),
        ExecutionScope::IsolatedProvider(identity) => {
            json.push_str("{\"kind\": \"isolated_provider\", \"identity\": ");
            push_json_string(json, &format!("0x{identity:016x}"));
            json.push('}');
        }
    }
}

const fn containment_guarantee_name(
    guarantee: omega_effects::ContainmentGuarantee,
) -> &'static str {
    match guarantee {
        omega_effects::ContainmentGuarantee::MemoryIsolation => "memory_isolation",
        omega_effects::ContainmentGuarantee::ForcibleTermination => "forcible_termination",
        omega_effects::ContainmentGuarantee::FaultContainment => "fault_containment",
        omega_effects::ContainmentGuarantee::BoundedResources => "bounded_resources",
    }
}

fn push_json_string(output: &mut String, value: &str) {
    output.push('"');
    for ch in value.chars() {
        match ch {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            c if c.is_control() => {
                let _ = write!(output, "\\u{:04x}", c as u32);
            }
            c => output.push(c),
        }
    }
    output.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_effects::provider_plan::{
        ProviderBinding, ProviderPlan, ProviderPlanRow, ServiceMethod, ServiceSchema,
    };

    fn selected(binding: ProviderBinding) -> SelectedProviderPlanFacts {
        let plan = ProviderPlan {
            name: "selected".into(),
            provider_type: "SelectedProvider".into(),
            target: "test-target".into(),
            schema: ServiceSchema {
                trait_name: "Storage".into(),
                methods: vec![ServiceMethod {
                    name: "read".into(),
                    requirement_owner: "Storage".into(),
                    requirement_identity: "Storage::read".into(),
                    parameter_count: 0,
                    parameter_type_identities: Vec::new(),
                    entry_claims: Vec::new(),
                    has_result: false,
                    result_type_identity: None,
                    result_claims: Vec::new(),
                    service_reach: vec!["Storage".into()],
                    synchronous_invocations: Vec::new(),
                    may_suspend: false,
                    may_block: false,
                    terminates_guarantee: false,
                    termination_premises: Vec::new(),
                    calling_plan_fingerprint: None,
                }],
            },
            rows: vec![ProviderPlanRow {
                method: "read".into(),
                requirement_identity: "Storage::read".into(),
                binding,
            }],
            origin_package: "test".into(),
        };
        SelectedProviderPlanFacts::from_selection(&[plan], &["selected".into()])
            .expect("selected provider")
    }

    #[test]
    fn artifact_separates_known_entries_from_attributed_completeness() {
        let json = executable_tcb_manifest_json(&selected(ProviderBinding::Import {
            library: "opaque.dll".into(),
            symbol: "read".into(),
        }));

        assert!(json.contains("\"known_entries\": []"));
        assert!(json.contains("\"status\": \"incomplete\""));
        assert!(json.contains("\"reason\": \"uncontained_opaque_in_process_provider\""));
        assert!(json.contains("\"provider_plan_identity\": \"0x"));
        assert!(!json.contains("omega_runtime_admission"));
    }

    #[test]
    fn artifact_reports_pinned_opaque_identity_and_independent_receipts() {
        let selected = selected(ProviderBinding::Import {
            library: "platform".into(),
            symbol: "read".into(),
        });
        let plan_identity = selected.plans()[0].identity_fingerprint();
        let selected = selected
            .with_opaque_executable_admissions([
                omega_effects::OpaqueExecutableAdmissionCandidate {
                    provider_plan_identity: plan_identity,
                    method: "read".into(),
                    requirement_identity: "Storage::read".into(),
                    binding: OpaqueInProcessBinding::Import {
                        library: "platform".into(),
                        symbol: "read".into(),
                    },
                    executable_identity: "platform-baseline:read-v1".into(),
                    implementation_evidence_identity: "receipt:binary-v1".into(),
                    execution_scope: ExecutionScope::CallerAddressSpace,
                    containment: vec![omega_effects::ContainmentEvidence {
                        guarantee: omega_effects::ContainmentGuarantee::BoundedResources,
                        evidence_identity: "receipt:quota-v1".into(),
                    }],
                    executable_closure_evidence_identity: Some("receipt:closed-loader-v1".into()),
                },
            ])
            .expect("exact opaque admission");

        let json = executable_tcb_manifest_json(&selected);
        assert!(json.contains("\"kind\": \"pinned_opaque_artifact\""));
        assert!(json.contains(
            "\"selected_requirement\": {\"method\": \"read\", \"identity\": \"Storage::read\"}"
        ));
        assert!(json.contains("\"identity\": \"platform-baseline:read-v1\""));
        assert!(json.contains("\"class\": \"admitted_opaque\""));
        assert!(json.contains("\"guarantee\": \"bounded_resources\""));
        assert!(json.contains("\"kind\": \"admitted_opaque_executable_closure\""));
        assert!(json.contains("\"status\": \"complete\""));
    }

    #[test]
    fn artifact_reports_only_mediated_runtime_entries_and_their_closure() {
        let static_manifest = SelectedProviderPlanFacts::default().executable_tcb_manifest();
        let mut ledger =
            omega_effects::OmegaRuntimeExecutableLedger::new(ExecutionScope::CallerAddressSpace)
                .expect("valid caller scope");
        ledger
            .admit(omega_effects::OmegaRuntimeExecutableAdmissionCandidate {
                provider_identity: ProviderIdentity::NominalType("RuntimePlugin".into()),
                provider_plan_identity: 91,
                executable_identity: "sha256:runtime-plugin-v1".into(),
                implementation_evidence_identity: "receipt:implementation-v1".into(),
                admission_receipt_identity: "receipt:omega-loader-v1".into(),
                execution_scope: ExecutionScope::CallerAddressSpace,
                containment: Vec::new(),
                executable_closure_evidence_identity: Some("receipt:closed-loader-v1".into()),
            })
            .expect("mediated runtime admission");
        let manifest = ledger
            .union_with_static_manifest(&static_manifest)
            .expect("matching caller scope");

        let json = executable_tcb_manifest_value_json(&manifest);
        assert!(json.contains("\"origin\": \"omega_runtime_admission\""));
        assert!(json.contains("\"selected_requirement\": null"));
        assert!(json.contains("\"identity\": \"sha256:runtime-plugin-v1\""));
        assert!(json.contains("\"kind\": \"omega_runtime_executable_closure\""));
        assert!(json.contains("\"admission_receipt_identity\": \"receipt:omega-loader-v1\""));
    }

    #[test]
    fn artifact_keeps_isolated_scope_manifest_separate_from_root_endpoint() {
        let root = SelectedProviderPlanFacts::default().executable_tcb_manifest();
        let isolated = SelectedProviderPlanFacts::default()
            .with_execution_scope(ExecutionScope::IsolatedProvider(501))
            .expect("isolated scope")
            .executable_tcb_manifest();
        let mut set = omega_effects::ExecutableTcbManifestSet::new(root).expect("root manifest");
        set.attach_isolated_scope(omega_effects::IsolatedExecutableScopeCandidate {
            provider_identity: ProviderIdentity::NominalType("SandboxedCodec".into()),
            provider_plan_identity: 501,
            endpoint_identity: "endpoint:codec-v1".into(),
            endpoint_receipt_identity: "receipt:endpoint-v1".into(),
            isolated_manifest_receipt_identity: "receipt:isolated-manifest-v1".into(),
            isolated_scope_identity: 501,
            containment: Vec::new(),
            isolated_manifest: isolated,
        })
        .expect("exact isolated attachment");

        let json = executable_tcb_manifest_set_json(&set);
        assert!(json.contains("\"kind\": \"isolated_provider_endpoint\""));
        assert!(json.contains("\"kind\": \"isolated_provider\""));
        assert!(json.contains("\"manifest_receipt_identity\": \"receipt:isolated-manifest-v1\""));
        assert_eq!(json.matches("\"root_manifest\"").count(), 1);
        assert_eq!(json.matches("\"isolated_scopes\"").count(), 1);
    }
}
