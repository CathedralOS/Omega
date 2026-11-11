//! The generics experiment of the lowered-form checker, on real programs.
//!
//! For each `main.omg` given on the command line it checks the fixture with
//! the ordinary pipeline (the only way to obtain the frozen typed trees and
//! the bound entry) and elaborates the entry's closure. For every machine in
//! that closure that stage 04 generated for a static application of a
//! generic machine, it then:
//!
//! 1. elaborates the generic machine once with its implementation parameters
//!    rigid, and discharges it against the requirements' contracts;
//! 2. elaborates and discharges one conformance witness per selected
//!    implementation, which is that implementation's refinement proof;
//! 3. carries the generic body's certificates into the generated machine
//!    with `lowered_discharge::reverify_specialization`, where the kernel
//!    decides each against the generated machine's own goal and hypotheses.
//!
//! `concrete` lines show, for comparison, what a search over the generated
//! machine alone proves, with the selected implementation's own contract.
//!
//! Tab-separated lines:
//!
//!     program <path> current=<accepted|rejected> route=<..>
//!     generic <path> <first specialization> <obligation> <kind> <proved|refused> <detail>
//!     concrete <path> <specialization> <obligation> <kind> <proved|refused> <detail>
//!     witness <path> <specialization> <obligation> <kind> <proved|refused> <detail>
//!     carried <path> <specialization> <obligation> <kind> <accepted|refused> <detail>
//!
//! A program the current checker rejects still has typed trees when stage
//! 04 produced them, and elaboration proves nothing, so the same three
//! steps run on it and the `generic` lines show what the lowered-form
//! checker refuses. This is a measuring tool, not a gate, and not part of
//! the shipped CLI.

#[path = "../tests/support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use std::path::Path;

use lowered_discharge::{Selection, discharge, reverify_specialization};
use semantic_vocabulary::MachineId;
use structure_elaboration::Elaboration;
use terminal_check_tables::obligations::ObligationKind;

fn one_line(text: impl std::fmt::Display) -> String {
    text.to_string()
        .replace(['\t', '\n'], " ")
        .chars()
        .take(300)
        .collect()
}

fn kind(
    elaboration: &Elaboration,
    machine: MachineId,
    obligation: semantic_vocabulary::ObligationId,
) -> &'static str {
    let row = elaboration
        .tables
        .machines
        .get(machine)
        .and_then(|tables| tables.obligations.get(obligation));
    match row.map(|row| row.kind) {
        Some(ObligationKind::OperationSafety { .. }) => "safety",
        Some(ObligationKind::CallRequirement { .. }) => "requirement",
        Some(ObligationKind::NormalGuarantee { .. }) => "guarantee",
        Some(ObligationKind::PublishedGuarantee { .. }) => "published",
        Some(ObligationKind::DataDomain { .. }) => "domain",
        Some(_) => "other",
        None => "missing",
    }
}

/// One line per obligation of `machine` that takes a certificate.
fn report(
    label: &str,
    main: &Path,
    subject: u64,
    elaboration: &Elaboration,
    discharged: &lowered_discharge::Discharge,
    machine: MachineId,
    lines: &mut Vec<String>,
) -> bool {
    let mut all = true;
    let Some(result) = discharged.machines.get(machine) else {
        return false;
    };
    for (obligation, verdict) in result.verdicts.iter() {
        let kind = kind(elaboration, machine, obligation);
        if kind == "published" {
            continue;
        }
        let (outcome, detail) = match verdict {
            Ok(_) => ("proved", "-".to_owned()),
            Err(refusal) => {
                all = false;
                (
                    "refused",
                    format!("{:?}: {}", refusal.reason, one_line(&refusal.detail)),
                )
            }
        };
        lines.push(format!(
            "{label}\t{}\t{subject}\t{}\t{kind}\t{outcome}\t{detail}",
            main.display(),
            obligation.get()
        ));
    }
    all
}

fn probe(main: &Path, lines: &mut Vec<String>) -> String {
    use fixture_package_inputs::FixtureFrontierCheck;
    let (frontier, current) =
        match fixture_package_inputs::check_fixture_keeping_typed_frontier(main, "linux_x86_64") {
            FixtureFrontierCheck::Accepted { frontier, .. } => (frontier, "accepted"),
            FixtureFrontierCheck::Rejected { frontier, .. } => (frontier, "rejected"),
            FixtureFrontierCheck::ReviewRefused { frontier, .. } => (frontier, "review-refused"),
            FixtureFrontierCheck::StoppedBeforeTypedFrontier(diagnostics) => {
                return format!(
                    "current=none\troute=unreached\t{}",
                    one_line(format!("{:?}", diagnostics.first()))
                );
            }
        };
    let head = format!("current={current}");
    let typed = &frontier.typed;
    let Some(entry) = frontier.entries.first().copied() else {
        return format!("{head}\troute=no-entry");
    };
    let mut consumer = match structure_elaboration::elaborate_closure(typed, entry) {
        Ok(elaboration) => elaboration,
        Err(refusal) => return format!("{head}\troute=outside\t{}", one_line(&refusal.detail)),
    };
    let mut callers: Vec<MachineId> = consumer
        .selections
        .iter()
        .map(|selected| selected.caller)
        .collect();
    callers.dedup();
    if callers.is_empty() {
        return format!("{head}\troute=no-specialization");
    }
    let entry_machine = MachineId::new(1).expect("1 is nonzero");
    // Each generic machine is elaborated and discharged once, however many
    // applications of it the program makes.
    let mut generics: Vec<(
        symbols::SymbolHandle,
        Elaboration,
        lowered_discharge::Discharge,
    )> = Vec::new();
    let mut carried_all = true;
    for specialization in callers {
        let Some(template) = consumer
            .machines
            .get(specialization)
            .and_then(|symbol| structure_elaboration::template_of(typed, *symbol))
        else {
            return format!("{head}\troute=defect\tspecialization without a template");
        };
        // 1. The generic body, once, against its requirements alone.
        if !generics.iter().any(|(symbol, _, _)| *symbol == template) {
            let mut generic = match structure_elaboration::elaborate_generic(typed, template) {
                Ok(generic) => generic,
                Err(refusal) => {
                    return format!(
                        "{head}\troute=outside\tgeneric: {}",
                        one_line(&refusal.detail)
                    );
                }
            };
            let found = match discharge(&mut generic) {
                Ok(found) => found,
                Err(refusal) => {
                    return format!(
                        "{head}\troute=defect\tgeneric: {}",
                        one_line(&refusal.detail)
                    );
                }
            };
            report(
                "generic",
                main,
                specialization.get(),
                &generic,
                &found,
                entry_machine,
                lines,
            );
            generics.push((template, generic, found));
        }
        let (_, generic, found) = generics
            .iter()
            .find(|(symbol, _, _)| *symbol == template)
            .expect("pushed above");
        // What a search over this generated machine alone proves, with the
        // selected implementation's own contract: what a use-by-use check
        // accepts. Its closure is elaborated on its own, since the route
        // does not yet serve every machine of the whole program.
        if let Some(symbol) = consumer.machines.get(specialization).copied()
            && let Ok(mut alone) = structure_elaboration::elaborate_closure(typed, symbol)
            && let Ok(searched) = discharge(&mut alone)
        {
            report(
                "concrete",
                main,
                specialization.get(),
                &alone,
                &searched,
                entry_machine,
                lines,
            );
        }
        // 2. Each selected implementation's refinement proof.
        let selected: Vec<_> = consumer
            .selections
            .iter()
            .filter(|selected| selected.caller == specialization)
            .copied()
            .collect();
        let mut witnesses = Vec::new();
        for selected in &selected {
            let Some(requirement) = generic
                .machines
                .iter()
                .find(|(machine, symbol)| {
                    **symbol == selected.requirement && generic.requirements.contains(machine)
                })
                .map(|(machine, _)| machine)
            else {
                return format!(
                    "{head}\troute=defect\tselection names no requirement of the generic form"
                );
            };
            let Some(implementation) = consumer.machines.get(selected.implementation).copied()
            else {
                return format!("{head}\troute=defect\tselection names no machine");
            };
            let mut witness = match structure_elaboration::elaborate_witness(
                typed,
                selected.declaring_trait,
                selected.requirement,
                implementation,
            ) {
                Ok(witness) => witness,
                Err(refusal) => {
                    return format!(
                        "{head}\troute=outside\twitness: {}",
                        one_line(&refusal.detail)
                    );
                }
            };
            let refinement = match discharge(&mut witness) {
                Ok(refinement) => refinement,
                Err(refusal) => {
                    return format!(
                        "{head}\troute=defect\twitness: {}",
                        one_line(&refusal.detail)
                    );
                }
            };
            report(
                "witness",
                main,
                specialization.get(),
                &witness,
                &refinement,
                entry_machine,
                lines,
            );
            witnesses.push((requirement, witness, refinement));
        }
        let selections: Vec<Selection<'_>> = witnesses
            .iter()
            .map(|(requirement, witness, refinement)| Selection {
                requirement: *requirement,
                witness,
                refinement,
            })
            .collect();
        // 3. Carry the generic body's certificates; the kernel re-decides.
        let verdicts = match reverify_specialization(
            generic,
            found,
            entry_machine,
            &mut consumer,
            specialization,
            &selections,
        ) {
            Ok(verdicts) => verdicts,
            Err(refusal) => {
                return format!("{head}\troute=defect\tcarry: {}", one_line(&refusal.detail));
            }
        };
        for (obligation, verdict) in &verdicts {
            let (outcome, detail) = match verdict {
                Ok(_) => ("accepted", "-".to_owned()),
                Err(refusal) => {
                    carried_all = false;
                    (
                        "refused",
                        format!("{:?}: {}", refusal.reason, one_line(&refusal.detail)),
                    )
                }
            };
            lines.push(format!(
                "carried\t{}\t{}\t{}\t{}\t{outcome}\t{detail}",
                main.display(),
                specialization.get(),
                obligation.get(),
                kind(&consumer, specialization, *obligation),
            ));
        }
    }
    format!(
        "{head}\troute=carried-{}",
        if carried_all { "accepted" } else { "refused" }
    )
}

fn main() {
    for argument in std::env::args().skip(1) {
        let main = Path::new(&argument);
        let mut lines = Vec::new();
        let line =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| probe(main, &mut lines)))
                .unwrap_or_else(|_| "panicked".to_owned());
        println!("program\t{argument}\t{line}");
        for line in lines {
            println!("{line}");
        }
    }
}
