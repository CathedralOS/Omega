//! Route one's side-by-side comparison: the current checker and the checker on
//! the lowered form, on the same corpus programs.
//!
//! For each `main.omg` given on the command line it checks the fixture with
//! the ordinary pipeline (the current checker's verdict, and the only way to
//! obtain the frozen typed trees and the bound entry), then elaborates the
//! entry with `structure_elaboration` and discharges it with
//! `lowered_discharge`, twice: with every kind of knowledge, and without
//! branch conditions, to count what branch facts buy. Each route is timed by
//! wall clock around its own calls only. Tab-separated lines:
//!
//!     program <path> current=<accepted|rejected> current_us=<n> route=<..> route_us=<n> ...
//!     obligation <path> <machine> <obligation> <kind> <verdict> <values from> <detail>
//!
//! A program the current checker rejects has no typed trees here, so the new
//! route cannot run on it (`route=unreached`). This is a measuring tool, not a
//! gate, and not part of the shipped CLI.

#[path = "../tests/support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use std::path::Path;
use std::time::Instant;

use lowered_discharge::{Knowledge, discharge_with};
use proof_admission::{AcceptedFactRoute, MathematicalCoreDecision};
use semantic_vocabulary::{IntegerMathTerm, Proposition, ScalarTerm, ValueId};
use terminal_check_tables::obligations::ObligationKind;
use terminal_check_tables::values::ValueOrigin;
use terminal_check_tables::{MachineTables, RefusalReason};

fn one_line(text: impl std::fmt::Display) -> String {
    text.to_string()
        .replace(['\t', '\n'], " ")
        .chars()
        .take(300)
        .collect()
}

/// The values a goal names, in the subset's term forms.
fn values(proposition: &Proposition, out: &mut Vec<ValueId>) {
    fn term(scalar: &ScalarTerm, out: &mut Vec<ValueId>) {
        match scalar {
            ScalarTerm::Value { id, .. } => out.push(*id),
            ScalarTerm::ExactIntegerAdd { left, right, .. }
            | ScalarTerm::ExactIntegerSubtract { left, right, .. }
            | ScalarTerm::ExactIntegerMultiply { left, right, .. } => {
                term(left, out);
                term(right, out);
            }
            _ => {}
        }
    }
    fn math(integer: &IntegerMathTerm, out: &mut Vec<ValueId>) {
        match integer {
            IntegerMathTerm::MathValue { value, .. } => out.push(*value),
            IntegerMathTerm::Embed(inner) => term(inner, out),
            IntegerMathTerm::IntegerLiteral(_) => {}
            IntegerMathTerm::Add(left, right)
            | IntegerMathTerm::Subtract(left, right)
            | IntegerMathTerm::Multiply(left, right) => {
                math(left, out);
                math(right, out);
            }
            IntegerMathTerm::ShiftLeft { value, count } => {
                math(value, out);
                math(count, out);
            }
        }
    }
    match proposition {
        Proposition::Equal(left, right)
        | Proposition::LessThan(left, right)
        | Proposition::LessOrEqual(left, right) => {
            term(left, out);
            term(right, out);
        }
        Proposition::IntegerMathEqual(left, right)
        | Proposition::IntegerMathLessThan(left, right)
        | Proposition::IntegerMathLessOrEqual(left, right) => {
            math(left, out);
            math(right, out);
        }
        Proposition::Conjunction(parts) | Proposition::Disjunction(parts) => {
            for part in parts {
                values(part, out);
            }
        }
        _ => {}
    }
}

/// Where each named value comes from: a formal, a constant or the named
/// operation. Structure only; this is how a refusal is classified.
fn origins(
    machine: &terminal_psi::TerminalMachine,
    tables: &MachineTables,
    named: &[ValueId],
) -> String {
    let mut kinds: Vec<&'static str> = named
        .iter()
        .map(
            |value| match tables.values.get(*value).map(|row| row.origin) {
                Some(ValueOrigin::MachineParameter { .. }) => "formal",
                Some(ValueOrigin::BlockParameter { .. }) => "join",
                Some(ValueOrigin::MachineResult) => "result",
                Some(ValueOrigin::EntryContent { .. }) => "entry-content",
                Some(ValueOrigin::Operation(operation)) => machine
                    .blocks
                    .iter()
                    .flat_map(|block| &block.operations)
                    .find(|candidate| candidate.id == operation)
                    .map_or("unknown", |found| {
                        terminal_semantics::OperationSemanticTag::for_operation(&found.kind).name()
                    }),
                None => "no-row",
            },
        )
        .collect();
    kinds.sort_unstable();
    kinds.dedup();
    if kinds.is_empty() {
        "-".to_owned()
    } else {
        kinds.join(",")
    }
}

/// The operands of an exact operation, the subset's only partial operations.
fn exact_operands(kind: &terminal_psi::OperationKind) -> Vec<ValueId> {
    use terminal_psi::OperationKind as Kind;
    match kind {
        Kind::ExactIntegerAdd { left, right, .. }
        | Kind::ExactIntegerSubtract { left, right, .. }
        | Kind::ExactIntegerMultiply { left, right, .. }
        | Kind::ExactIntegerDivide { left, right, .. }
        | Kind::ExactIntegerRemainder { left, right, .. } => vec![*left, *right],
        Kind::ExactIntegerShiftLeft { value, count, .. }
        | Kind::ExactIntegerShiftRight { value, count, .. } => vec![*value, *count],
        _ => Vec::new(),
    }
}

fn probe(main: &Path, lines: &mut Vec<String>) -> String {
    let started = Instant::now();
    let checked = (|| {
        let snapshot = fixture_package_inputs::fixture_build_snapshot(main)?;
        let discovered = fixture_package_inputs::discover_fixture(
            main,
            package_compilation::BuildExecutionInputs {
                snapshot,
                ..Default::default()
            },
        )?;
        let target = discovered
            .targets
            .iter()
            .find(|target| target.as_str() == "linux_x86_64")
            .or_else(|| discovered.targets.first())
            .cloned();
        let profile = target::TargetProfile::from_omega_target_name(target.as_deref())
            .map_err(|error| vec![error])?;
        let mut prepared = fixture_package_inputs::review_fixture(
            discovered,
            &[profile],
            compiler::CompilationProduct::Check,
        )?;
        fixture_package_inputs::check_fixture(
            &mut prepared,
            profile.target_name(),
            Default::default(),
        )
    })();
    let current_us = started.elapsed().as_micros();
    let checked = match checked {
        Ok(checked) => checked,
        Err(errors) => {
            return format!(
                "current=rejected\tcurrent_us={current_us}\troute=unreached\t{}",
                one_line(format!("{:?}", errors.first()))
            );
        }
    };
    let head = format!("current=accepted\tcurrent_us={current_us}");
    let Some(entry) = checked.build().config.program_roots().first().copied() else {
        return format!("{head}\troute=no-entry");
    };
    let typed = &checked.source_inspection().program().typed;

    let started = Instant::now();
    let elaborated = structure_elaboration::elaborate(typed, entry);
    let mut elaboration = match elaborated {
        Ok(elaboration) => elaboration,
        Err(refusal) => {
            return format!("{head}\troute=outside\t{}", one_line(&refusal.detail));
        }
    };
    let mut without_branches = elaboration.clone();
    let mut intervals_only = elaboration.clone();
    let discharged = discharge_with(&mut elaboration, Knowledge::All);
    let route_us = started.elapsed().as_micros();
    let discharged = match discharged {
        Ok(discharged) => discharged,
        Err(refusal) => {
            return format!(
                "{head}\troute=refused-module\troute_us={route_us}\t{}",
                one_line(&refusal.detail)
            );
        }
    };
    let without = discharge_with(&mut without_branches, Knowledge::WithoutBranchConditions).ok();
    let intervals = discharge_with(&mut intervals_only, Knowledge::LiteralIntervals).ok();
    let held = |run: &Option<lowered_discharge::Discharge>, machine, obligation| {
        run.as_ref()
            .and_then(|run: &lowered_discharge::Discharge| run.machines.get(machine))
            .and_then(|run| run.verdicts.get(obligation))
            .is_some_and(Result::is_ok)
    };

    // obligations, judged, core refused, unproved, unsupported, needed a
    // relation, needed a branch fact, published clauses held
    let mut counts = [0_usize; 8];
    for (machine, result) in discharged.machines.iter() {
        let Some(tables) = elaboration.tables.machines.get(machine) else {
            continue;
        };
        let Some(lowered) = elaboration.module.machines.iter().find(|m| m.id == machine) else {
            continue;
        };
        for (obligation, verdict) in result.verdicts.iter() {
            counts[0] += 1;
            let kind = match tables.obligations.get(obligation).map(|row| row.kind) {
                Some(ObligationKind::OperationSafety { .. }) => "safety",
                Some(ObligationKind::CallRequirement { .. }) => "requirement",
                Some(ObligationKind::NormalGuarantee { .. }) => "guarantee",
                Some(ObligationKind::PublishedGuarantee { .. }) => "published",
                Some(_) => "other",
                None => "missing",
            };
            let (outcome, goal, detail) = match verdict {
                Ok(lowered_discharge::Discharged::EveryExit(exits)) => {
                    counts[7] += 1;
                    (format!("every-exit:{}", exits.len()), None, "-".to_owned())
                }
                Ok(lowered_discharge::Discharged::Certificate(evidence)) => {
                    let core = match &evidence.fact.route {
                        AcceptedFactRoute::CertificateDerived { acceptance, .. } => {
                            match &acceptance.mathematical_core {
                                MathematicalCoreDecision::Judged(_) => {
                                    counts[1] += 1;
                                    "discharged-judged".to_owned()
                                }
                                MathematicalCoreDecision::Refused(family) => {
                                    counts[2] += 1;
                                    format!("discharged-core-refused:{family}")
                                }
                            }
                        }
                        other => format!("unexpected-route:{other:?}"),
                    };
                    // What it needed: literal intervals alone, a relation
                    // between values, or a branch fact.
                    let needed = if held(&intervals, machine, obligation) {
                        "interval"
                    } else if held(&without, machine, obligation) {
                        counts[5] += 1;
                        "relation"
                    } else {
                        counts[6] += 1;
                        "branch"
                    };
                    (
                        format!("{core}+{needed}"),
                        Some(evidence.goal.clone()),
                        "-".to_owned(),
                    )
                }
                Err(refusal) => {
                    let outcome = match refusal.reason {
                        RefusalReason::Unproved => {
                            counts[3] += 1;
                            "unproved"
                        }
                        _ => {
                            counts[4] += 1;
                            "refused-other"
                        }
                    };
                    (outcome.to_owned(), None, one_line(&refusal.detail))
                }
            };
            let mut named = Vec::new();
            match &goal {
                Some(goal) => values(goal, &mut named),
                None => {
                    // The operands of a refused operation, or the actuals of
                    // a refused call requirement.
                    if let Some(ObligationKind::OperationSafety { operation }) =
                        tables.obligations.get(obligation).map(|row| row.kind)
                        && let Some(found) = lowered
                            .blocks
                            .iter()
                            .flat_map(|block| &block.operations)
                            .find(|candidate| candidate.id == operation)
                    {
                        named.extend(exact_operands(&found.kind));
                        let tag =
                            terminal_semantics::OperationSemanticTag::for_operation(&found.kind)
                                .name();
                        lines.push(format!(
                            "obligation\t{}\t{}\t{}\t{kind}:{tag}\t{outcome}\t{}\t{detail}",
                            main.display(),
                            machine.get(),
                            obligation.get(),
                            origins(lowered, tables, &named),
                        ));
                        continue;
                    }
                }
            }
            lines.push(format!(
                "obligation\t{}\t{}\t{}\t{kind}\t{outcome}\t{}\t{detail}",
                main.display(),
                machine.get(),
                obligation.get(),
                origins(lowered, tables, &named),
            ));
        }
    }
    let route = if counts[3] + counts[4] == 0 {
        "accepted"
    } else {
        "refused"
    };
    format!(
        "{head}\troute={route}\troute_us={route_us}\tobligations={}\tjudged={}\tcore_refused={}\tunproved={}\trefused_other={}\tneeded_relation={}\tneeded_branch={}\tpublished_held={}",
        counts[0], counts[1], counts[2], counts[3], counts[4], counts[5], counts[6], counts[7]
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
