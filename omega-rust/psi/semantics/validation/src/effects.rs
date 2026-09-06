use diagnostics::Diagnostic;
use typed_trees::TypedTrees;

pub fn validate_behavior_plan(
    program: &TypedTrees,
    operational: &flow_effects::OperationalPlan,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let service_reaches = crate::infer_service_reaches(program, operational);

    validate_pure_discards(program, operational, &service_reaches, &mut diagnostics);
    validate_asm_intrinsic_declarations(program, operational, &service_reaches, &mut diagnostics);
    validate_service_reach_ceilings(program, &service_reaches, &mut diagnostics);

    for machine_summary in operational.machines() {
        let Some(machine) = program
            .machines()
            .iter()
            .find(|machine| machine.symbol == machine_summary.symbol)
        else {
            continue;
        };

        if machine.attached_data.is_some()
            && machine.name.as_str().ends_with("::drop")
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

    crate::finish_diagnostics(diagnostics)
}

fn validate_service_reach_ceilings(
    program: &TypedTrees,
    service_reaches: &flow_effects::ServiceReachInferencePlan,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for machine in program.machines() {
        let publishes = machine.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
            || machine.is_public
            || !program
                .service_reach_rows
                .services(machine.service_reach_row)
                .is_empty();
        if !publishes {
            continue;
        }
        let Some(summary) = service_reaches.for_machine(machine.symbol) else {
            continue;
        };
        let published = service_reaches.services(summary.published);
        let missing = service_reaches
            .services(summary.inferred_transitive)
            .iter()
            .copied()
            .filter(|service| !published.contains(service))
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
            format_service_row(program, published),
            if names.len() == 1 { "" } else { "s" },
            names.join(" + "),
        )));
    }
}

fn format_service_row(
    program: &TypedTrees,
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
pub use asm_discharge::validate_asm_discharge;
use asm_discharge::validate_asm_intrinsic_declarations;

mod pure_discards;
use pure_discards::validate_pure_discards;
