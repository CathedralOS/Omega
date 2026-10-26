use diagnostics::Diagnostic;

pub fn validate_behavior_plan(
    program: &dyn crate::CheckingInput,
    operational: &flow_effects::OperationalPlan,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let service_reaches = crate::infer_service_reaches(program, operational);

    validate_pure_discards(program, operational, &service_reaches, &mut diagnostics);
    validate_asm_intrinsic_declarations(program, &service_reaches, &mut diagnostics);
    validate_service_reach_ceilings(program, &service_reaches, &mut diagnostics);

    for machine_summary in operational.machines() {
        let Some(machine) = program.machine(machine_summary.symbol) else {
            continue;
        };

        if crate::value_custody::cleanup::is_reserved_cleanup(program, machine)
            && (machine.suspends
                || machine.blocks
                || machine_summary.transitive_may_suspend
                || machine_summary.transitive_may_block)
        {
            diagnostics.push(Diagnostic::error(format!(
                "cleanup machine `{}` must be transitively non-suspending and nonblocking",
                machine.name
            )));
        }

        // Requirements, boundaries, accepted contracts, and external
        // realizations publish closed operational ceilings. Omission is an
        // authored `false`, unlike omission on a private checked body, where
        // both axes are inferred. Validate the declaration-free body fixed
        // points so a published machine cannot launder behavior through a
        // local helper or a pinned requirement.
        if machine.is_public
            || machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
        {
            if machine_summary.body_may_suspend && !machine.suspends {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` omits `suspends;` from its published contract, but its body may suspend",
                    machine.name
                )));
            }
            if machine_summary.body_may_block && !machine.blocks {
                diagnostics.push(Diagnostic::error(format!(
                    "machine `{}` omits `blocks;` from its published contract, but its body may block",
                    machine.name
                )));
            }
        }
    }

    crate::program_validation::finish_diagnostics(diagnostics)
}

fn validate_service_reach_ceilings(
    program: &dyn crate::CheckingInput,
    service_reaches: &flow_effects::ServiceReachInferencePlan,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for machine in program.machines() {
        // A family interface's envelope is derived from all separately checked
        // members. It has no body whose inferred row could violate an authored
        // declaration copied from one prototype.
        if machine.target_family_interface {
            continue;
        }
        let Some(summary) = service_reaches.for_machine(machine.symbol) else {
            continue;
        };
        let declared = program
            .service_reach_rows
            .services(machine.service_reach_row);
        // The same fixed point carries ordinary helpers, callback contracts,
        // selected cleanup and target-family envelopes. Checking only direct
        // boundary calls lets a helper launder a service through an empty
        // ceiling. Omission requests inference; a written empty row still
        // bounds this complete derived row by the empty set.
        let ceiling_present = program
            .authored_service_reach_rows_for(machine.symbol)
            .next()
            .is_some();
        if machine.supply_mode == language_semantics::MachineSupplyMode::CheckedBody
            && !ceiling_present
        {
            continue;
        }
        let required = service_reaches.services(summary.inferred_transitive);
        let missing = required
            .iter()
            .copied()
            .filter(|service| !declared.contains(service))
            .collect::<Vec<_>>();
        if missing.is_empty() {
            continue;
        }
        let names = missing
            .iter()
            .map(|service| {
                program
                    .service_reaches
                    .definition(*service)
                    .map(|definition| definition.name.as_str())
                    .unwrap_or("<unknown canonical service>")
            })
            .collect::<Vec<_>>();
        diagnostics.push(Diagnostic::error(format!(
            "machine `{}` publishes service reach `{}` but its checked body reaches undeclared service{} `{}`",
            machine.name,
            format_service_row(program, declared),
            if names.len() == 1 { "" } else { "s" },
            names.join(" + "),
        )));
    }
}

fn format_service_row(
    program: &dyn crate::CheckingInput,
    services: &[language_semantics::ServiceReachId],
) -> String {
    if services.is_empty() {
        return "<none>".to_owned();
    }
    services
        .iter()
        .map(|service| {
            program
                .service_reaches
                .definition(*service)
                .map(|definition| definition.name.as_str())
                .unwrap_or("<unknown canonical service>")
        })
        .collect::<Vec<_>>()
        .join(" + ")
}

mod asm_discharge;
use asm_discharge::validate_asm_intrinsic_declarations;
pub use asm_discharge::{AsmAuthorityAdmission, validate_asm_discharge};

mod pure_discards;
use pure_discards::validate_pure_discards;
