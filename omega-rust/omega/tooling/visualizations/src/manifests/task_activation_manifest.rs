//! Provider-independent task activation demands and retained carry facts.

use crate::encoding::manifest_coordinates::machine_overload_identity;
use crate::encoding::manifest_values::push_json_string;
use checked_trees::CheckedTrees;
use symbols::SymbolHandle;

/// Provider-independent task activation demands. Runtime/provider admission
/// consumes these normalized facts; the artifact keeps target/layout and
/// canonical carry derivation inspectable without exposing provider handles.
pub fn task_activation_manifest_json(
    program: &CheckedTrees,
    task_activations: &task_plans::TaskActivationPlanSet,
) -> String {
    use checked_trees::machine::Machine;
    use task_plans::TaskStartOperation;

    fn machine_name(machines: &[Machine], symbol: SymbolHandle) -> &str {
        machines
            .iter()
            .find(|machine| machine.symbol == symbol)
            .map(|machine| machine.name.as_str())
            .unwrap_or("<unknown>")
    }
    fn callable_name(program: &CheckedTrees, symbol: SymbolHandle) -> String {
        if let Some(machine) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == symbol)
        {
            return machine.name.as_str().to_owned();
        }
        program
            .traits()
            .iter()
            .find_map(|definition| {
                program
                    .trait_machine_signatures(definition)
                    .iter()
                    .find(|signature| signature.symbol == symbol)
                    .map(|signature| format!("{}::{}", definition.name, signature.name))
            })
            .unwrap_or_else(|| "<unknown>".to_owned())
    }
    let mut json = String::from("{\n  \"activations\": [");
    for (index, activation) in task_activations.as_slice().iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        let plan = activation.plan.candidate();
        json.push_str("\n    {\n      \"operation\": ");
        push_json_string(
            &mut json,
            match activation.operation {
                TaskStartOperation::Start => "start",
                TaskStartOperation::TryStart => "try_start",
            },
        );
        json.push_str(",\n      \"start_requirement\": ");
        push_json_string(
            &mut json,
            &callable_name(program, activation.start_requirement),
        );
        json.push_str(",\n      \"selected_runtime\": {\"provider_plan\": ");
        push_json_string(&mut json, &activation.selected_runtime.provider_plan_name);
        json.push_str(", \"runtime_identity\": \"0x");
        json.push_str(&format!(
            "{:016x}",
            activation.selected_runtime.runtime.normalized_identity()
        ));
        json.push_str("\", \"requirement_identity\": ");
        push_json_string(&mut json, &activation.selected_runtime.requirement_identity);
        json.push('}');
        json.push_str(",\n      \"target_machine\": ");
        push_json_string(
            &mut json,
            machine_name(program.machines(), activation.target_machine),
        );
        json.push_str(",\n      \"target_machine_overload_identity\": ");
        push_json_string(
            &mut json,
            &machine_overload_identity(program, activation.target_machine)
                .expect("task activation must name an exact target machine"),
        );
        json.push_str(",\n      \"specialization_report_fingerprint\": \"0x");
        json.push_str(&format!(
            "{:016x}",
            activation.specialization_report_fingerprint
        ));
        json.push_str("\",\n      \"specialization_commitment\": \"");
        for byte in activation.specialization_commitment.as_bytes() {
            json.push_str(&format!("{byte:02x}"));
        }
        json.push_str("\",\n      \"activation_plan_id\": \"0x");
        json.push_str(&format!(
            "{:016x}",
            activation.plan.normalized_identity().normalized_identity()
        ));
        json.push_str("\",\n      \"machine_contract_id\": \"0x");
        json.push_str(&format!(
            "{:016x}",
            plan.machine_contract.normalized_identity()
        ));
        json.push_str("\",\n      \"entry_id\": \"0x");
        json.push_str(&format!("{:016x}", plan.entry.normalized_identity()));
        json.push_str("\",\n      \"argument_layout_id\": \"0x");
        json.push_str(&format!(
            "{:016x}",
            plan.argument_layout.normalized_identity()
        ));
        json.push_str("\",\n      \"terminal_outcome_layout_id\": \"0x");
        json.push_str(&format!(
            "{:016x}",
            plan.terminal_outcome_layout.normalized_identity()
        ));
        json.push_str("\",\n      \"calling_plan_id\": \"0x");
        json.push_str(&format!("{:016x}", plan.calling_plan.normalized_identity()));
        json.push_str("\",\n      \"stack_plan\": {\"bytes\": ");
        json.push_str(&plan.stack_plan.bytes.to_string());
        json.push_str(", \"alignment\": ");
        json.push_str(&plan.stack_plan.alignment.to_string());
        json.push_str(", \"representation\": \"0x");
        json.push_str(&format!(
            "{:016x}",
            plan.stack_plan.representation.normalized_identity()
        ));
        json.push_str("\"},\n      \"may_suspend\": ");
        json.push_str(if plan.may_suspend { "true" } else { "false" });
        json.push_str(",\n      \"may_block\": ");
        json.push_str(if plan.may_block { "true" } else { "false" });
        json.push_str(",\n      \"canonical_suspension_crossings\": [");
        for (crossing_index, crossing) in plan.canonical_suspension_crossings.iter().enumerate() {
            if crossing_index > 0 {
                json.push(',');
            }
            json.push_str("{\"identity\": \"0x");
            json.push_str(&format!("{:016x}", crossing.identity.get()));
            json.push_str("\", \"suspension_allowed\": ");
            json.push_str(if crossing.suspension_allowed {
                "true"
            } else {
                "false"
            });
            json.push_str(", \"preserve_cpu\": ");
            json.push_str(if crossing.preserve_cpu {
                "true"
            } else {
                "false"
            });
            json.push_str(", \"preserve_host_thread\": ");
            json.push_str(if crossing.preserve_host_thread {
                "true"
            } else {
                "false"
            });
            json.push('}');
        }
        json.push_str("],\n      \"cpu_thread_preservation\": {\"preserve_cpu\": ");
        json.push_str(if plan.carry_obligations.preserve_cpu {
            "true"
        } else {
            "false"
        });
        json.push_str(", \"preserve_host_thread\": ");
        json.push_str(if plan.carry_obligations.preserve_host_thread {
            "true"
        } else {
            "false"
        });
        json.push('}');
        json.push_str(",\n      \"cancellation_required\": ");
        json.push_str(if plan.cancellation_required {
            "true"
        } else {
            "false"
        });
        json.push_str("\n    }");
    }
    json.push_str("\n  ]\n}\n");
    json
}
