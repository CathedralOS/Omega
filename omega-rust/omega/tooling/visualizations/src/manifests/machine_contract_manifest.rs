//! Machine contracts, specializations, and implementation evidence.

#[cfg(test)]
mod tests;

use crate::encoding::manifest_coordinates::machine_overload_identity;
use crate::encoding::manifest_values::{
    push_claim_identity_json, push_json_string, push_json_strings,
};
use checked_trees::CheckedTrees;
use symbols::SymbolHandle;
use typed_trees::machine::Machine;
use typed_trees::state::State;

/// Externally inspectable machine-contract artifact. The
/// object shape is the firewall: authored interface identity and checked
/// implementation evidence are siblings, never one flattened bag. Consumers
/// pin `contract.report_fingerprint`; proof/debug tooling may inspect
/// `implementation` without changing that identity.
pub fn machine_contract_manifest_json(program: &CheckedTrees) -> String {
    let specializations = validated_manifest_specializations(program);
    let crash_capsules = validated_manifest_crash_capsules(program);
    let mut json = String::from("{\n  \"machines\": [");
    for (index, machine) in program.machines().iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        let contract = exact_manifest_machine_contract(program, machine);
        let service_reach = exact_manifest_service_reach(program, machine);
        let synchronous_invocation = exact_manifest_synchronous_invocation(program, machine);
        let suspension = exact_manifest_suspension(program, machine);
        let blocking = exact_manifest_blocking(program, machine);
        let termination = exact_manifest_termination(program, machine);
        let mutation = exact_manifest_mutation(program, machine);
        json.push_str("\n    {\n      \"machine\": ");
        push_json_string(&mut json, machine.name.as_str());
        json.push_str(",\n      \"machine_overload_identity\": ");
        push_json_string(
            &mut json,
            &machine_overload_identity(program, machine.symbol).unwrap_or_else(|| {
                panic!(
                    "checked machine contract `{}` must have an exact overload identity",
                    machine.name
                )
            }),
        );

        json.push_str(",\n      \"contract\": {");
        json.push_str("\n        \"report_fingerprint\": \"0x");
        json.push_str(&format!("{:016x}", contract.report_fingerprint));
        json.push_str("\",\n        \"supply\": ");
        push_json_string(&mut json, supply_mode_name(machine.supply_mode));
        json.push_str(",\n        \"service_reach\": ");
        push_service_reach_plan_json(&mut json, program, service_reach);
        json.push_str(",\n        \"synchronous_invocation\": ");
        push_synchronous_invocation_plan_json(&mut json, synchronous_invocation, false);
        json.push_str(",\n        \"suspension\": ");
        push_suspension_plan_json(&mut json, suspension);
        json.push_str(",\n        \"blocking\": ");
        push_blocking_plan_json(&mut json, blocking);
        json.push_str(",\n        \"crashes\": ");
        push_crash_plan_json(&mut json, &contract.crash);
        json.push_str(",\n        \"termination\": ");
        push_termination_interface_json(&mut json, &termination.interface);
        json.push_str("\n      }");

        json.push_str(",\n      \"implementation\": {");
        json.push_str("\n        \"checked_may_suspend\": ");
        json.push_str(if suspension.checked_may_suspend {
            "true"
        } else {
            "false"
        });
        json.push_str(",\n        \"checked_may_block\": ");
        json.push_str(if blocking.checked_may_block {
            "true"
        } else {
            "false"
        });
        json.push_str(",\n        \"checked_service_reach\": ");
        push_service_row_json(&mut json, program, service_reach.checked_inferred);
        json.push_str(",\n        \"checked_synchronous_invocations\": ");
        push_string_array(&mut json, &synchronous_invocation.checked_inferred);
        let state_write_frames = mutation.state_write_frames.as_slice();
        json.push_str(",\n        \"inferred_write_frames\": [");
        for (frame_index, state_frame) in state_write_frames.iter().enumerate() {
            if frame_index > 0 {
                json.push(',');
            }
            let state_name = mutation_frame_state_name(program, machine, state_frame.state);
            json.push_str("\n          {\"state\": ");
            push_json_string(&mut json, state_name);
            json.push_str(", \"completeness\": ");
            push_json_string(
                &mut json,
                match state_frame.frame.completeness() {
                    facts::WriteFrameCompleteness::Complete => "complete",
                    facts::WriteFrameCompleteness::Opaque => "opaque",
                },
            );
            json.push_str(", \"fingerprint\": \"0x");
            json.push_str(&format!(
                "{:016x}",
                state_frame.frame.compatibility_report_fingerprint()
            ));
            json.push_str("\", \"paths\": [");
            push_json_strings(&mut json, state_frame.frame.paths());
            json.push_str("]}");
        }
        if !state_write_frames.is_empty() {
            json.push('\n');
            json.push_str("        ");
        }
        json.push(']');
        json.push_str(",\n        \"checked_crash_sites\": [");
        for (site_index, site) in contract.crash.checked_sites().iter().enumerate() {
            if site_index > 0 {
                json.push(',');
            }
            let location = site.location();
            let state_name = exact_manifest_crash_source_state(
                program,
                machine,
                location.state(),
                location.statement_ordinal(),
                "site",
            )
            .name
            .as_str();
            json.push_str("\n          {\"state\": ");
            push_json_string(&mut json, state_name);
            json.push_str(", \"statement_ordinal\": ");
            json.push_str(&location.statement_ordinal().to_string());
            json.push_str(", \"cause\": ");
            push_json_string(
                &mut json,
                match site.cause() {
                    checked_trees::CrashCause::Trap => "Trap",
                    checked_trees::CrashCause::Abort => "Abort",
                },
            );
            json.push_str(", \"path_guard_conjuncts\": [");
            for (guard_index, predicate) in site.path_guard_conjuncts().iter().enumerate() {
                if guard_index > 0 {
                    json.push_str(", ");
                }
                let mut identity = String::from("0x");
                for byte in predicate.canonical_bytes() {
                    identity.push_str(&format!("{byte:02x}"));
                }
                push_json_string(&mut json, &identity);
            }
            json.push(']');
            json.push_str(", \"path_guard_consequences\": [");
            for (guard_index, predicate) in site.path_guard_consequences().iter().enumerate() {
                if guard_index > 0 {
                    json.push_str(", ");
                }
                let mut identity = String::from("0x");
                for byte in predicate.canonical_bytes() {
                    identity.push_str(&format!("{byte:02x}"));
                }
                push_json_string(&mut json, &identity);
            }
            json.push(']');
            json.push_str(", \"guard_covering_buckets\": [");
            for (coverage_index, bucket) in site.guard_covering_buckets().iter().enumerate() {
                if coverage_index > 0 {
                    json.push_str(", ");
                }
                json.push_str(&bucket.get().to_string());
            }
            json.push(']');
            json.push_str(", \"covering_buckets\": [");
            for (coverage_index, (bucket, _)) in
                contract.crash.covering_buckets_for_site(site).enumerate()
            {
                if coverage_index > 0 {
                    json.push_str(", ");
                }
                json.push_str(&bucket.get().to_string());
            }
            json.push(']');
            json.push_str(", \"frontier_lower_bound\": [");
            for (claim_index, claim) in site.frontier_lower_bound().iter().enumerate() {
                if claim_index > 0 {
                    json.push_str(", ");
                }
                push_claim_identity_json(&mut json, program, *claim);
            }
            json.push(']');
            json.push('}');
        }
        if !contract.crash.checked_sites().is_empty() {
            json.push('\n');
            json.push_str("        ");
        }
        json.push(']');
        json.push_str(",\n        \"checked_crash_calls\": [");
        for (call_index, call) in contract.crash.checked_calls().iter().enumerate() {
            if call_index > 0 {
                json.push(',');
            }
            let location = call.location();
            let state_name = exact_manifest_crash_call_source(program, machine, call)
                .name
                .as_str();
            let target = exact_manifest_crash_target(
                program,
                call.target_machine(),
                call.target_state(),
                "checked crash call",
            );
            json.push_str("\n          {\"state\": ");
            push_json_string(&mut json, state_name);
            json.push_str(", \"statement_ordinal\": ");
            json.push_str(&location.statement_ordinal().to_string());
            json.push_str(", \"call_ordinal\": ");
            json.push_str(&location.call_ordinal().to_string());
            json.push_str(", \"target_machine\": ");
            push_json_string(&mut json, &target.owner_label);
            json.push_str(", \"target_callable_overload_identity\": ");
            push_json_string(&mut json, &target.overload_identity);
            json.push_str(", \"target_state\": ");
            push_json_string(&mut json, &target.state_label);
            json.push_str(", \"target_contract_report_fingerprint\": \"0x");
            json.push_str(&format!(
                "{:016x}",
                call.target_contract_report_fingerprint()
            ));
            json.push_str("\", \"path_guard_conjuncts\": [");
            for (guard_index, predicate) in call.path_guard_conjuncts().iter().enumerate() {
                if guard_index > 0 {
                    json.push_str(", ");
                }
                push_crash_predicate_identity_json(&mut json, predicate);
            }
            json.push_str("], \"path_guard_consequences\": [");
            for (guard_index, predicate) in call.path_guard_consequences().iter().enumerate() {
                if guard_index > 0 {
                    json.push_str(", ");
                }
                push_crash_predicate_identity_json(&mut json, predicate);
            }
            json.push_str("], \"surviving_buckets\": [");
            push_crash_buckets_json(&mut json, call.surviving_buckets());
            json.push_str("]}");
        }
        if !contract.crash.checked_calls().is_empty() {
            json.push('\n');
            json.push_str("        ");
        }
        json.push(']');
        json.push(',');
        json.push_str("\n        \"checked_termination\": ");
        push_termination_json(&mut json, &termination.checked_summary);
        json.push_str(",\n        \"resolved_ranking_view\": ");
        push_json_string(
            &mut json,
            termination
                .implementation_witness
                .as_ref()
                .map_or("", |witness| witness.view_path.as_str()),
        );
        if let Some(witness) = termination.implementation_witness.as_ref() {
            json.push_str(",\n        \"ranking_witness\": {\n          \"subjects\": [");
            push_json_strings(&mut json, &witness.subjects);
            json.push_str("],\n          \"view\": ");
            push_json_string(&mut json, &witness.view_path);
            json.push_str(",\n          \"view_arguments\": [");
            push_json_strings(&mut json, &witness.view_arguments);
            json.push(']');
            if let Some(range) = witness.rank_range.as_ref() {
                json.push_str(",\n          \"rank_range\": {\"floor\": ");
                push_json_string(&mut json, &range.floor);
                json.push_str(", \"ceiling\": ");
                push_json_string(&mut json, &range.ceiling);
                json.push_str(", \"ceiling_inclusive\": ");
                json.push_str(if range.ceiling_inclusive {
                    "true"
                } else {
                    "false"
                });
                json.push('}');
            }
            json.push_str("\n        }");
        }
        json.push_str("\n      }\n    }");
    }
    json.push_str("\n  ],\n  \"crash_contract_capsules\": [");
    for (index, row) in crash_capsules.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        let capsule = row.capsule;
        json.push_str("\n    {\"target_machine\": ");
        push_json_string(&mut json, &row.target.owner_label);
        json.push_str(", \"target_callable_overload_identity\": ");
        push_json_string(&mut json, &row.target.overload_identity);
        json.push_str(", \"target_state\": ");
        push_json_string(&mut json, &row.target.state_label);
        json.push_str(", \"target_contract_report_fingerprint\": \"0x");
        json.push_str(&format!(
            "{:016x}",
            capsule.target_contract_report_fingerprint()
        ));
        json.push_str("\", \"published_buckets\": [");
        push_crash_buckets_json(&mut json, capsule.published_buckets());
        json.push_str("]}");
    }
    if !crash_capsules.is_empty() {
        json.push('\n');
        json.push_str("  ");
    }
    json.push_str("],\n  \"specializations\": [");
    for (index, row) in specializations.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        let specialization = row.specialization;
        json.push_str("\n    {\n      \"template\": ");
        push_json_string(&mut json, row.template.name.as_str());
        json.push_str(",\n      \"instance\": ");
        push_json_string(&mut json, row.instance.name.as_str());
        json.push_str(",\n      \"instance_report_fingerprint\": \"0x");
        json.push_str(&format!("{:016x}", specialization.report_fingerprint));
        json.push_str("\",\n      \"instance_contract_report_fingerprint\": \"0x");
        json.push_str(&format!(
            "{:016x}",
            row.instance_contract_report_fingerprint
        ));
        json.push_str("\",\n      \"instance_contract_commitment\": \"");
        for byte in row.instance_contract_commitment.as_bytes() {
            json.push_str(&format!("{byte:02x}"));
        }
        json.push_str("\",\n      \"template_contract_report_fingerprint\": \"0x");
        json.push_str(&format!(
            "{:016x}",
            specialization.template_contract_report_fingerprint
        ));
        json.push_str("\",\n      \"template_contract_commitment\": \"");
        for byte in specialization.template_contract_commitment.as_bytes() {
            json.push_str(&format!("{byte:02x}"));
        }
        json.push_str("\",\n      \"accepted_template_commitment\": ");
        if let Some(commitment) = specialization.accepted_template_commitment.as_deref() {
            push_json_string(&mut json, commitment);
        } else {
            json.push_str("null");
        }
        json.push_str(",\n      \"type_arguments\": [");
        push_json_strings(&mut json, &specialization.type_arguments);
        json.push_str("],\n      \"const_arguments\": [");
        push_json_strings(&mut json, &specialization.const_arguments);
        json.push_str("],\n      \"type_argument_identities\": [");
        push_json_strings(&mut json, &specialization.type_argument_identities);
        json.push_str("],\n      \"const_argument_identities\": [");
        push_json_strings(&mut json, &specialization.const_argument_identities);
        json.push_str("],\n      \"machine_argument_contract_report_fingerprints\": [");
        for (identity_index, identity) in specialization
            .machine_argument_contract_report_fingerprints
            .iter()
            .enumerate()
        {
            if identity_index > 0 {
                json.push_str(", ");
            }
            push_json_string(&mut json, &format!("0x{identity:016x}"));
        }
        json.push_str("],\n      \"conformance_argument_report_fingerprints\": [");
        for (identity_index, identity) in specialization
            .conformance_argument_report_fingerprints
            .iter()
            .enumerate()
        {
            if identity_index > 0 {
                json.push_str(", ");
            }
            push_json_string(&mut json, &format!("0x{identity:016x}"));
        }
        json.push_str("],\n      \"machine_argument_contract_commitments\": [");
        for (identity_index, argument) in specialization.machine_arguments.iter().enumerate() {
            if identity_index > 0 {
                json.push_str(", ");
            }
            let owner = program
                .machines()
                .iter()
                .find(|machine| {
                    machine.symbol == *argument
                        || program
                            .machine_states(machine)
                            .iter()
                            .any(|state| state.symbol == *argument)
                })
                .expect("specialization machine argument must retain one exact owner");
            let commitment = exact_manifest_machine_contract(program, owner).commitment;
            let text = commitment
                .as_bytes()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            push_json_string(&mut json, &text);
        }
        json.push_str("],\n      \"conformance_argument_commitments\": [");
        for (identity_index, application) in
            specialization.conformance_applications.iter().enumerate()
        {
            if identity_index > 0 {
                json.push_str(", ");
            }
            let text = application
                .commitment
                .as_bytes()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            push_json_string(&mut json, &text);
        }
        json.push_str("]\n    }");
    }
    json.push_str("\n  ]\n}\n");
    json
}

fn exact_manifest_machine_contract<'program>(
    program: &'program CheckedTrees,
    machine: &checked_trees::machine::Machine,
) -> &'program checked_trees::MachineContractPlan {
    let mut matches = program
        .facts
        .contract_plans
        .machines
        .iter()
        .filter(|plan| plan.machine == machine.symbol);
    let plan = matches.next().unwrap_or_else(|| {
        panic!(
            "machine contract manifest `{}` is missing its exact machine contract row",
            machine.name
        )
    });
    assert!(
        matches.next().is_none(),
        "machine contract manifest `{}` has duplicate exact machine contract rows",
        machine.name
    );
    plan
}

fn exact_manifest_service_reach(
    program: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
) -> language_semantics::ServiceReachPlan {
    let mut matches = program
        .facts
        .service_reaches
        .machines()
        .iter()
        .filter(|fact| fact.machine == machine.symbol);
    let fact = matches.next().unwrap_or_else(|| {
        panic!(
            "machine contract manifest `{}` is missing its exact service-reach row",
            machine.name
        )
    });
    assert!(
        matches.next().is_none(),
        "machine contract manifest `{}` has duplicate exact service-reach rows",
        machine.name
    );
    language_semantics::ServiceReachPlan {
        interface: fact.interface,
        checked_inferred: fact.inferred_transitive,
    }
}

fn exact_manifest_synchronous_invocation<'program>(
    program: &'program CheckedTrees,
    machine: &checked_trees::machine::Machine,
) -> &'program language_semantics::SynchronousInvocationPlan {
    let mut matches = program
        .facts
        .synchronous_invocations
        .machines
        .iter()
        .filter(|fact| fact.machine == machine.symbol);
    let fact = matches.next().unwrap_or_else(|| {
        panic!(
            "machine contract manifest `{}` is missing its exact synchronous-invocation row",
            machine.name
        )
    });
    assert!(
        matches.next().is_none(),
        "machine contract manifest `{}` has duplicate exact synchronous-invocation rows",
        machine.name
    );
    &fact.plan
}

fn exact_manifest_suspension(
    program: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
) -> language_semantics::SuspensionPlan {
    let mut matches = program
        .facts
        .suspensions
        .machines
        .iter()
        .filter(|fact| fact.machine == machine.symbol);
    let fact = matches.next().unwrap_or_else(|| {
        panic!(
            "machine contract manifest `{}` is missing its exact suspension row",
            machine.name
        )
    });
    assert!(
        matches.next().is_none(),
        "machine contract manifest `{}` has duplicate exact suspension rows",
        machine.name
    );
    fact.plan
}

fn exact_manifest_blocking(
    program: &CheckedTrees,
    machine: &checked_trees::machine::Machine,
) -> language_semantics::BlockingPlan {
    let mut matches = program
        .facts
        .blocking
        .machines
        .iter()
        .filter(|fact| fact.machine == machine.symbol);
    let fact = matches.next().unwrap_or_else(|| {
        panic!(
            "machine contract manifest `{}` is missing its exact blocking row",
            machine.name
        )
    });
    assert!(
        matches.next().is_none(),
        "machine contract manifest `{}` has duplicate exact blocking rows",
        machine.name
    );
    fact.plan
}

fn exact_manifest_termination<'program>(
    program: &'program CheckedTrees,
    machine: &checked_trees::machine::Machine,
) -> &'program language_semantics::MachineTerminationPlan {
    let mut matches = program
        .facts
        .termination
        .machines
        .iter()
        .filter(|fact| fact.machine == machine.symbol);
    let fact = matches.next().unwrap_or_else(|| {
        panic!(
            "machine contract manifest `{}` is missing its exact termination row",
            machine.name
        )
    });
    assert!(
        matches.next().is_none(),
        "machine contract manifest `{}` has duplicate exact termination rows",
        machine.name
    );
    &fact.plan
}

fn exact_manifest_mutation<'program>(
    program: &'program CheckedTrees,
    machine: &checked_trees::machine::Machine,
) -> &'program checked_trees::MachineMutationFact {
    let mut matches = program
        .facts
        .mutation
        .machines
        .iter()
        .filter(|fact| fact.machine == machine.symbol);
    let fact = matches.next().unwrap_or_else(|| {
        panic!(
            "machine contract manifest `{}` is missing its exact mutation row",
            machine.name
        )
    });
    assert!(
        matches.next().is_none(),
        "machine contract manifest `{}` has duplicate exact mutation rows",
        machine.name
    );
    let states = program.machine_states(machine);
    assert_eq!(
        fact.state_write_frames.len(),
        states.len(),
        "machine contract manifest `{}` mutation frames must cover its exact typed state table one-for-one",
        machine.name
    );
    for (frame, state) in fact.state_write_frames.iter().zip(states) {
        assert_eq!(
            frame.state, state.symbol,
            "machine contract manifest `{}` mutation frames must retain exact typed state-table carrier order",
            machine.name
        );
    }
    fact
}

struct ValidatedManifestSpecialization<'program> {
    specialization: &'program typed_trees::typed_trees::MachineSpecialization,
    template: &'program Machine,
    instance: &'program Machine,
    instance_contract_report_fingerprint: u64,
    instance_contract_commitment: checked_trees::MachineContractCommitment,
}

struct ValidatedManifestCrashTarget {
    owner_label: String,
    state_label: String,
    overload_identity: String,
    is_requirement: bool,
}

struct ValidatedManifestCrashCapsule<'program> {
    capsule: &'program checked_trees::CrashContractCapsule,
    target: ValidatedManifestCrashTarget,
}

fn exact_manifest_crash_target(
    program: &CheckedTrees,
    target_machine: SymbolHandle,
    target_state: SymbolHandle,
    source_kind: &str,
) -> ValidatedManifestCrashTarget {
    let local_owners = program
        .machines()
        .iter()
        .filter(|machine| machine.symbol == target_machine)
        .collect::<Vec<_>>();
    assert!(
        local_owners.len() <= 1,
        "{source_kind} has duplicate exact local target-machine owners"
    );
    let trait_owners = program
        .traits()
        .iter()
        .filter(|definition| definition.symbol == target_machine)
        .collect::<Vec<_>>();
    assert!(
        trait_owners.len() <= 1,
        "{source_kind} has duplicate exact trait target owners"
    );

    let mut candidates = Vec::new();
    if let Some(machine) = local_owners.first().copied() {
        let states = program
            .machine_states(machine)
            .iter()
            .filter(|state| state.symbol == target_state)
            .collect::<Vec<_>>();
        assert!(
            states.len() <= 1,
            "{source_kind} has duplicate exact local target states"
        );
        if let Some(state) = states.first().copied() {
            let overload_identity = program
                .normalized_machine_overload_identity(machine)
                .unwrap_or_else(|| {
                    panic!("{source_kind} local target must retain an exact overload identity")
                })
                .identity();
            candidates.push(ValidatedManifestCrashTarget {
                owner_label: machine.name.as_str().to_owned(),
                state_label: state.name.as_str().to_owned(),
                overload_identity,
                is_requirement: false,
            });
        }
    }

    let mut generic_targets = Vec::new();
    if target_machine == target_state {
        for declaring_machine in program.machines() {
            for parameter in program.machine_type_parameters(declaring_machine) {
                let typed_trees::data::TypeParameterKind::Machine { contract } = &parameter.kind
                else {
                    continue;
                };
                if parameter.symbol != target_state {
                    continue;
                }
                let signature = program
                    .machine_parameter_contract_view(contract)
                    .expect(
                        "checked machine-parameter contract must retain a valid requirement identity",
                    )
                    .signature();
                let label = parameter.name.as_str();
                generic_targets.push(ValidatedManifestCrashTarget {
                    owner_label: label.to_owned(),
                    state_label: label.to_owned(),
                    overload_identity: program
                        .normalized_machine_parameter_overload_identity(
                            declaring_machine,
                            signature,
                        )
                        .identity(),
                    is_requirement: true,
                });
            }
        }
    }
    assert!(
        generic_targets.len() <= 1,
        "{source_kind} has duplicate exact generic requirement targets"
    );
    let owner_category_count = usize::from(!local_owners.is_empty())
        + usize::from(!generic_targets.is_empty())
        + usize::from(!trait_owners.is_empty());
    assert!(
        owner_category_count <= 1,
        "{source_kind} target owner must resolve to one retained callable category"
    );
    candidates.extend(generic_targets);

    if let Some(definition) = trait_owners.first().copied() {
        let signatures = program
            .trait_machine_signatures(definition)
            .iter()
            .filter(|signature| signature.symbol == target_state)
            .collect::<Vec<_>>();
        assert!(
            signatures.len() <= 1,
            "{source_kind} has duplicate exact trait target signatures"
        );
        if let Some(signature) = signatures.first().copied() {
            candidates.push(ValidatedManifestCrashTarget {
                owner_label: definition.name.as_str().to_owned(),
                state_label: signature.name.as_str().to_owned(),
                overload_identity: program
                    .normalized_trait_requirement_overload_identity(definition, signature)
                    .identity(),
                is_requirement: true,
            });
        }
    }

    let mut candidates = candidates.into_iter();
    let target = candidates.next().unwrap_or_else(|| {
        panic!("{source_kind} must name one exact retained callable target coordinate")
    });
    assert!(
        candidates.next().is_none(),
        "{source_kind} must name exactly one retained callable target category"
    );
    target
}

fn validated_manifest_crash_capsules(
    program: &CheckedTrees,
) -> Vec<ValidatedManifestCrashCapsule<'_>> {
    let mut coordinates = Vec::new();
    program
        .facts
        .contract_plans
        .crash_capsules
        .iter()
        .map(|capsule| {
            let coordinate = (capsule.target_machine(), capsule.target_state());
            assert!(
                !coordinates.contains(&coordinate),
                "machine contract manifest crash capsules have duplicate exact target coordinates"
            );
            coordinates.push(coordinate);
            let target = exact_manifest_crash_target(
                program,
                capsule.target_machine(),
                capsule.target_state(),
                "crash contract capsule",
            );
            assert!(
                target.is_requirement,
                "crash contract capsule target must be an exact requirement owner/signature pair"
            );
            ValidatedManifestCrashCapsule { capsule, target }
        })
        .collect()
}

fn exact_manifest_specialization_machine<'program>(
    program: &'program CheckedTrees,
    symbol: SymbolHandle,
    role: &str,
) -> &'program Machine {
    let mut matches = program
        .machines()
        .iter()
        .filter(|machine| machine.symbol == symbol);
    let machine = matches.next().unwrap_or_else(|| {
        panic!("machine contract manifest specialization is missing its exact typed {role} machine")
    });
    assert!(
        matches.next().is_none(),
        "machine contract manifest specialization has duplicate exact typed {role} machines"
    );
    machine
}

fn validated_manifest_specializations(
    program: &CheckedTrees,
) -> Vec<ValidatedManifestSpecialization<'_>> {
    let mut instance_symbols = Vec::new();
    let mut validated = Vec::with_capacity(program.machine_specializations.len());
    for specialization in &program.machine_specializations {
        assert!(
            !instance_symbols.contains(&specialization.instance),
            "machine contract manifest specializations have duplicate exact instance rows"
        );
        instance_symbols.push(specialization.instance);
        let template =
            exact_manifest_specialization_machine(program, specialization.template, "template");
        let instance =
            exact_manifest_specialization_machine(program, specialization.instance, "instance");
        validated.push(ValidatedManifestSpecialization {
            specialization,
            template,
            instance,
            instance_contract_report_fingerprint:
                specialization_instance_contract_report_fingerprint(program, instance),
            instance_contract_commitment: exact_manifest_machine_contract(program, instance)
                .commitment,
        });
    }
    validated
}

fn exact_manifest_crash_source_state<'program>(
    program: &'program CheckedTrees,
    machine: &Machine,
    state_symbol: SymbolHandle,
    statement_ordinal: u32,
    source_kind: &str,
) -> &'program State {
    let mut states = program
        .machine_states(machine)
        .iter()
        .filter(|state| state.symbol == state_symbol);
    let state = states.next().unwrap_or_else(|| {
        panic!("checked crash {source_kind} source state must belong to its exact contract machine")
    });
    assert!(
        states.next().is_none(),
        "checked crash {source_kind} source state must resolve uniquely within its exact contract machine"
    );
    let statement_index = usize::try_from(statement_ordinal)
        .expect("checked crash source statement ordinal exceeds retained index range");
    assert!(
        program
            .statement_table
            .statements(state.statement_nodes)
            .get(statement_index)
            .is_some(),
        "checked crash {source_kind} statement must belong to its exact typed state"
    );
    state
}

fn exact_manifest_crash_call_source<'program>(
    program: &'program CheckedTrees,
    machine: &Machine,
    call: &checked_trees::CheckedCrashCallSite,
) -> &'program State {
    let location = call.location();
    let state = exact_manifest_crash_source_state(
        program,
        machine,
        location.state(),
        location.statement_ordinal(),
        "call",
    );
    let mut flow_states = program
        .facts
        .flow
        .control
        .states
        .iter()
        .filter(|(_, flow)| {
            flow.machine_symbol == machine.symbol && flow.state_symbol == state.symbol
        });
    let flow_state = flow_states
        .next()
        .map(|(_, flow)| flow)
        .unwrap_or_else(|| panic!("checked crash call must name one exact checked flow state"));
    assert!(
        flow_states.next().is_none(),
        "checked crash call must name exactly one checked flow state"
    );
    let calls = program
        .facts
        .flow
        .control
        .calls
        .span(flow_state.calls)
        .expect("checked crash call flow state must retain an exact valid call span");
    let statement_index = usize::try_from(location.statement_ordinal())
        .expect("checked crash call statement ordinal exceeds retained index range");
    let call_ordinal = usize::try_from(location.call_ordinal())
        .expect("checked crash call ordinal exceeds retained index range");
    let mut flow_calls = calls.iter().filter(|flow_call| {
        flow_call.statement_index == statement_index && flow_call.call_ordinal == call_ordinal
    });
    let flow_call = flow_calls
        .next()
        .unwrap_or_else(|| panic!("checked crash call must name one exact checked flow call"));
    assert!(
        flow_calls.next().is_none(),
        "checked crash call must name exactly one checked flow call"
    );
    assert_eq!(
        flow_call.target_symbol,
        call.target_state(),
        "checked crash call must retain its exact checked flow target"
    );
    state
}

fn mutation_frame_state_name<'program>(
    program: &'program CheckedTrees,
    machine: &Machine,
    state_symbol: SymbolHandle,
) -> &'program str {
    program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_symbol)
        .map(|state| state.name.as_str())
        .expect("checked mutation write-frame state must belong to its exact fact machine")
}

fn specialization_instance_contract_report_fingerprint(
    program: &CheckedTrees,
    instance: &Machine,
) -> u64 {
    exact_manifest_machine_contract(program, instance).report_fingerprint
}

fn supply_mode_name(mode: language_semantics::MachineSupplyMode) -> &'static str {
    use language_semantics::MachineSupplyMode;
    match mode {
        MachineSupplyMode::CheckedBody => "checked_body",
        MachineSupplyMode::Requirement => "requirement",
        MachineSupplyMode::TopLevelRequirement => "top_level_requirement",
        MachineSupplyMode::Boundary => "boundary",
        MachineSupplyMode::AdmissionClaim => "admission_claim",
        MachineSupplyMode::ExternalRealization { .. } => "external-realization",
    }
}

fn push_suspension_plan_json(json: &mut String, plan: language_semantics::SuspensionPlan) {
    use language_semantics::SuspensionInterface;
    match plan.interface {
        SuspensionInterface::InternalInferred => {
            json.push_str("{\"interface\": \"internal_inferred\"}");
        }
        SuspensionInterface::PublishedMaySuspend(value) => {
            json.push_str("{\"interface\": \"published_ceiling\", \"may_suspend\": ");
            json.push_str(if value { "true" } else { "false" });
            json.push('}');
        }
    }
}

fn push_service_reach_plan_json(
    json: &mut String,
    program: &CheckedTrees,
    plan: language_semantics::ServiceReachPlan,
) {
    use language_semantics::ServiceReachInterface;
    match plan.interface {
        ServiceReachInterface::InternalInferred => {
            json.push_str("{\"interface\": \"internal_inferred\"}");
        }
        ServiceReachInterface::PublishedCeiling(row) => {
            json.push_str("{\"interface\": \"published_ceiling\", \"services\": ");
            push_service_row_json(json, program, row);
            json.push('}');
        }
    }
}

fn push_synchronous_invocation_plan_json(
    json: &mut String,
    plan: &language_semantics::SynchronousInvocationPlan,
    include_checked: bool,
) {
    use language_semantics::SynchronousInvocationInterface;
    json.push_str("{\"interface\": ");
    push_json_string(
        json,
        match plan.interface {
            SynchronousInvocationInterface::InternalInferred => "internal_inferred",
            SynchronousInvocationInterface::PublishedCeiling => "published_ceiling",
        },
    );
    json.push_str(", \"targets\": ");
    push_string_array(json, &plan.published);
    if include_checked {
        json.push_str(", \"checked\": ");
        push_string_array(json, &plan.checked_inferred);
    }
    json.push('}');
}

fn push_string_array(json: &mut String, values: &[String]) {
    json.push('[');
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            json.push_str(", ");
        }
        push_json_string(json, value);
    }
    json.push(']');
}

fn push_service_row_json(
    json: &mut String,
    program: &CheckedTrees,
    row: language_semantics::ServiceReachRowId,
) {
    let reaches = &program.facts.service_reaches;
    json.push('[');
    for (index, service) in reaches.rows.services(row).iter().enumerate() {
        if index > 0 {
            json.push_str(", ");
        }
        let name = reaches
            .services
            .definition(*service)
            .map(|definition| definition.name.as_str())
            .unwrap_or("<unknown-service>");
        push_json_string(json, name);
    }
    json.push(']');
}

fn push_blocking_plan_json(json: &mut String, plan: language_semantics::BlockingPlan) {
    use language_semantics::BlockingInterface;
    match plan.interface {
        BlockingInterface::InternalInferred => {
            json.push_str("{\"interface\": \"internal_inferred\"}");
        }
        BlockingInterface::PublishedMayBlock(value) => {
            json.push_str("{\"interface\": \"published_ceiling\", \"may_block\": ");
            json.push_str(if value { "true" } else { "false" });
            json.push('}');
        }
    }
}

fn push_crash_plan_json(json: &mut String, plan: &checked_trees::CrashPlan) {
    json.push_str("{\"interface\": ");
    push_json_string(
        json,
        match plan.interface() {
            checked_trees::CrashInterface::InternalInferred => "internal_inferred",
            checked_trees::CrashInterface::PublishedCeiling => "published_ceiling",
        },
    );
    json.push_str(", \"buckets\": [");
    push_crash_buckets_json(json, plan.published());
    json.push_str("]}");
}

fn push_crash_buckets_json(json: &mut String, buckets: &[checked_trees::CrashRouteBucket]) {
    for (bucket_index, bucket) in buckets.iter().enumerate() {
        if bucket_index > 0 {
            json.push_str(", ");
        }
        json.push_str("{\"cause\": ");
        push_json_string(
            json,
            match bucket.cause() {
                checked_trees::CrashCause::Trap => "Trap",
                checked_trees::CrashCause::Abort => "Abort",
            },
        );
        json.push_str(", \"alternative_guards\": [");
        for (guard_index, guard) in bucket.alternative_guards().iter().enumerate() {
            if guard_index > 0 {
                json.push_str(", ");
            }
            match guard {
                checked_trees::CrashRouteGuard::Truth => push_json_string(json, "true"),
                checked_trees::CrashRouteGuard::Predicate(predicate) => {
                    push_crash_predicate_identity_json(json, predicate);
                }
            }
        }
        json.push_str("]}");
    }
}

fn push_crash_predicate_identity_json(
    json: &mut String,
    predicate: &checked_trees::CrashPredicateIdentity,
) {
    let mut identity = String::from("0x");
    for byte in predicate.canonical_bytes() {
        identity.push_str(&format!("{byte:02x}"));
    }
    push_json_string(json, &identity);
}

fn push_termination_json(json: &mut String, guarantee: &language_semantics::TerminationGuarantee) {
    use language_semantics::TerminationGuarantee;
    match guarantee {
        TerminationGuarantee::NoGuarantee => json.push_str("{\"kind\": \"no_guarantee\"}"),
        TerminationGuarantee::Terminates { premises } => {
            json.push_str("{\"kind\": \"terminates\", \"premises\": [");
            for (index, premise) in premises.iter().enumerate() {
                if index > 0 {
                    json.push_str(", ");
                }
                json.push_str("{\"profile\": ");
                json.push_str(&premise.profile.0.to_string());
                json.push_str(", \"subject_root\": ");
                json.push_str(&premise.subject.root.arena_index().to_string());
                json.push_str(", \"subject_projections\": [");
                for (projection_index, projection) in premise.subject.projections.iter().enumerate()
                {
                    if projection_index > 0 {
                        json.push_str(", ");
                    }
                    json.push_str(&projection.arena_index().to_string());
                }
                json.push_str("]}");
            }
            json.push_str("]}");
        }
    }
}

fn push_termination_interface_json(
    json: &mut String,
    interface: &language_semantics::TerminationInterface,
) {
    use language_semantics::TerminationInterface;
    match interface {
        TerminationInterface::InternalDerived => {
            json.push_str("{\"interface\": \"internal_derived\"}");
        }
        TerminationInterface::Published(guarantee) => {
            json.push_str("{\"interface\": \"published\", \"guarantee\": ");
            push_termination_json(json, guarantee);
            json.push('}');
        }
    }
}
