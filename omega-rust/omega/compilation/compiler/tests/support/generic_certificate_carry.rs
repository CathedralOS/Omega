//! The three steps of the generics experiment on one fixture, shared by the
//! `generic_certificate_carry` example, which prints them, and the
//! `lowered_route_generics` test, which pins them: the generic body checked
//! once, each selected implementation's refinement proof, and the generic
//! body's certificates carried into each machine stage 04 generated from it.
//! The including crate declares `unbuilt_frontier` at its root: the typed
//! trees come from the path that runs no Build.

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

/// The outcome of the three steps on the program in `main`, whose Build
/// binds `main`: `route=<outcome>`, with one line per obligation in `lines`.
pub fn probe(main: &Path, lines: &mut Vec<String>) -> String {
    let frontier = match crate::unbuilt_frontier::frontier(main, None, &[]) {
        Ok(frontier) => frontier,
        Err(stopped) => {
            return format!(
                "route=unreached\t{}",
                one_line(format!("{:?}", stopped.first()))
            );
        }
    };
    let typed = &frontier.typed;
    let Some(entry) = frontier.entries.first().copied() else {
        return "route=no-entry".to_owned();
    };
    let entry_machine = MachineId::new(1).expect("1 is nonzero");
    // Every machine stage 04 generated for a static application, each with
    // the closure it is carried into: one that selects an implementation,
    // binds a value parameter, applies a type, or several of these. They
    // are the generated machines of the entry's closure when the route
    // lowers that; when it does not (the entry uses a construct outside the
    // subset), each machine generated from a declaration of the entry's own
    // source file is the root of a closure of its own.
    let mut roots: Vec<(Elaboration, MachineId)> = Vec::new();
    match structure_elaboration::elaborate_closure(typed, entry) {
        Ok(closure) => {
            for (machine, symbol) in closure.machines.iter() {
                if structure_elaboration::template_of(typed, *symbol).is_some() {
                    roots.push((closure.clone(), machine));
                }
            }
        }
        Err(entry_refusal) => {
            let file = |symbol| {
                typed
                    .symbols
                    .symbol_provenance_source_span(symbol)
                    .map(|span| span.source_id)
            };
            for specialization in &typed.machine_specializations {
                if file(specialization.template).is_none()
                    || file(specialization.template) != file(entry)
                {
                    continue;
                }
                match structure_elaboration::elaborate_closure(typed, specialization.instance) {
                    Ok(closure) => roots.push((closure, entry_machine)),
                    Err(refusal) => {
                        return format!("route=outside\t{}", one_line(&refusal.detail));
                    }
                }
            }
            if roots.is_empty() {
                return format!("route=outside\t{}", one_line(&entry_refusal.detail));
            }
        }
    }
    if roots.is_empty() {
        return "route=no-specialization".to_owned();
    }
    // Each generic machine is elaborated and discharged once, however many
    // applications of it the program makes: one check form, or for a finite
    // type family one per alternative it lists.
    let mut generics: Vec<(
        symbols::SymbolHandle,
        Vec<(Elaboration, lowered_discharge::Discharge)>,
    )> = Vec::new();
    let mut carried_all = true;
    for (mut consumer, specialization) in roots {
        let Some(template) = consumer
            .machines
            .get(specialization)
            .and_then(|symbol| structure_elaboration::template_of(typed, *symbol))
        else {
            return "route=defect\tspecialization without a template".to_owned();
        };
        // 1. The generic body, once, against its requirements alone.
        if !generics.iter().any(|(symbol, _)| *symbol == template) {
            let forms = match structure_elaboration::elaborate_generic(typed, template) {
                Ok(generic) => vec![generic],
                Err(opaque) => match structure_elaboration::elaborate_alternatives(typed, template)
                {
                    Ok(alternatives) => alternatives,
                    Err(_) => {
                        return format!(
                            "route=outside\tgeneric: {}",
                            one_line(&opaque.detail)
                        );
                    }
                },
            };
            let mut checked = Vec::new();
            for mut form in forms {
                let found = match discharge(&mut form) {
                    Ok(found) => found,
                    Err(refusal) => {
                        return format!(
                            "route=defect\tgeneric: {}",
                            one_line(&refusal.detail)
                        );
                    }
                };
                // A generic body is checked for every use it admits, so an
                // obligation it refuses rejects the program even when no
                // generated machine needs it (a family's unused alternative).
                carried_all &= report(
                    "generic",
                    main,
                    specialization.get(),
                    &form,
                    &found,
                    entry_machine,
                    lines,
                );
                checked.push((form, found));
            }
            generics.push((template, checked));
        }
        let (_, forms) = generics
            .iter()
            .find(|(symbol, _)| *symbol == template)
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
        let selected: Vec<_> = consumer
            .selections
            .iter()
            .filter(|selected| selected.caller == specialization)
            .cloned()
            .collect();
        // The certificates of the one check form this machine is: for a
        // family, the alternative whose body it is. Which one is decided by
        // the carrier's own correspondence of the two lowered bodies, never
        // by the type's spelling; a form it does not correspond to carries
        // nothing, and the kernel re-decides whatever is carried.
        let mut outcome = None;
        for (generic, found) in forms {
            // 2. Each selected implementation's refinement proof.
            let mut witnesses = Vec::new();
            for selected in &selected {
                let Some(requirement) = generic
                    .machines
                    .iter()
                    .find(|(machine, symbol)| {
                        **symbol == selected.requirement
                            && generic.requirements.contains(machine)
                            && generic.value_arguments.get(*machine)
                                == Some(&selected.value_arguments)
                    })
                    .map(|(machine, _)| machine)
                else {
                    return "route=defect\tselection names no requirement of the generic form".to_owned();
                };
                let Some(implementation) = consumer.machines.get(selected.implementation).copied()
                else {
                    return "route=defect\tselection names no machine".to_owned();
                };
                let mut witness = match structure_elaboration::elaborate_witness(
                    typed,
                    selected.declaring_trait,
                    selected.requirement,
                    &selected.value_arguments,
                    implementation,
                ) {
                    Ok(witness) => witness,
                    Err(refusal) => {
                        return format!(
                            "route=outside\twitness: {}",
                            one_line(&refusal.detail)
                        );
                    }
                };
                let refinement = match discharge(&mut witness) {
                    Ok(refinement) => refinement,
                    Err(refusal) => {
                        return format!(
                            "route=defect\twitness: {}",
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
            let carried = reverify_specialization(
                generic,
                found,
                entry_machine,
                &mut consumer,
                specialization,
                &selections,
            );
            let corresponds = carried.is_ok();
            outcome = Some(carried);
            if corresponds {
                break;
            }
        }
        let verdicts = match outcome {
            Some(Ok(verdicts)) => verdicts,
            Some(Err(refusal)) => {
                return format!("route=defect\tcarry: {}", one_line(&refusal.detail));
            }
            None => return "route=defect\tno check form".to_owned(),
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
        "route=carried-{}",
        if carried_all { "accepted" } else { "refused" }
    )
}
