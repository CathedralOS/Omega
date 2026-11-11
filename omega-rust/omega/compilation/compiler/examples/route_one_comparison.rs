//! Route one's side-by-side comparison: the current checker and the checker on
//! the lowered form, on the same corpus programs.
//!
//! For each `main.omg` given on the command line it checks the fixture with
//! the ordinary pipeline (the current checker's verdict, and the only way to
//! obtain the frozen typed trees and the bound entries), then elaborates the
//! compilation with `structure_elaboration` and discharges it with
//! `lowered_discharge`, twice: with every kind of knowledge, and without
//! branch conditions, to count what branch facts buy. Each route is timed by
//! wall clock around its own calls only. Tab-separated lines:
//!
//!     program <path> current=<accepted|rejected|stopped> current_us=<n> route=<..> route_us=<n> ...
//!     obligation <path> <machine> <obligation> <kind> <verdict> <values from> <detail>
//!     diagnostic <path> <machine> <obligation> <reject|incomplete> <site> <message>
//!     use <path> <machine> <operation or edge> unestablished <detail>
//!
//! `route=` is `lowered_discharge::judge`'s verdict: `accepted`, `rejected`
//! (one `diagnostic` line per unproved obligation and one `use` line per use
//! of storage that is not established, the compile errors the program gets on
//! this route), `incomplete` (the prover has no rule for some
//! goal's form), `fallback` (an unsupported obligation hands the closure to
//! the current checker) or `defect`.
//!
//! The route is given everything the check claims: the Build's entries and
//! every declaration of the fixture's package, used or not. A program the
//! current checker rejects is compared as well (`current=rejected`), through
//! the typed-frontier seam; `current=stopped` means no typed trees exist
//! (`route=unreached`). This is a measuring tool, not a
//! gate, and not part of the shipped CLI.

#[path = "../tests/support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use std::path::Path;
use std::time::Instant;

use lowered_discharge::{Knowledge, RouteVerdict, UnprovedObligation, discharge_with, judge};
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
    use fixture_package_inputs::FixtureFrontierCheck;
    let started = Instant::now();
    let checked =
        fixture_package_inputs::check_fixture_keeping_typed_frontier(main, "linux_x86_64");
    let current_us = started.elapsed().as_micros();
    // A program the current checker rejects is compared too: the seam keeps
    // the typed trees that checker judged.
    let (frontier, current) = match checked {
        FixtureFrontierCheck::Accepted { frontier, .. } => (frontier, "accepted"),
        FixtureFrontierCheck::Rejected { frontier, .. }
        | FixtureFrontierCheck::ReviewRefused { frontier, .. } => (frontier, "rejected"),
        FixtureFrontierCheck::StoppedBeforeTypedFrontier(errors) => {
            return format!(
                "current=stopped\tcurrent_us={current_us}\troute=unreached\t{}",
                one_line(format!("{:?}", errors.first()))
            );
        }
    };
    let head = format!("current={current}\tcurrent_us={current_us}");
    if frontier.entries.is_empty() {
        return format!("{head}\troute=no-entry");
    }
    let typed = &frontier.typed;

    let started = Instant::now();
    let elaborated = structure_elaboration::elaborate(
        typed,
        structure_elaboration::Compilation {
            entries: &frontier.entries,
            subject: frontier.subject,
            selected: &frontier.selected,
        },
    );
    let mut elaboration = match elaborated {
        Ok(elaboration) => elaboration,
        Err(refusal) => {
            return format!("{head}\troute=outside\t{}", one_line(&refusal.detail));
        }
    };
    let for_verdict = elaboration.clone();
    let mut without_branches = elaboration.clone();
    let mut intervals_only = elaboration.clone();
    let mut consumer = elaboration.clone();
    let discharge_started = Instant::now();
    let discharged = discharge_with(&mut elaboration, Knowledge::All);
    let discharge_us = discharge_started.elapsed().as_micros();
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
    // Re-verifying the saved certificates in a consumer elaborated from the
    // same typed trees: goal and hypotheses recomputed, no search.
    let reverify_started = Instant::now();
    let mut reverified = 0_usize;
    for (machine, result) in discharged.machines.iter() {
        for (obligation, verdict) in result.verdicts.iter() {
            if let Ok(lowered_discharge::Discharged::Certificate(evidence)) = verdict
                && lowered_discharge::reverify(
                    &mut consumer,
                    machine,
                    obligation,
                    &evidence.certificate,
                )
                .is_ok()
            {
                reverified += 1;
            }
        }
    }
    let reverify_us = reverify_started.elapsed().as_micros();
    let held = |run: &Option<lowered_discharge::Discharge>, machine, obligation| {
        run.as_ref()
            .and_then(|run: &lowered_discharge::Discharge| run.machines.get(machine))
            .and_then(|run| run.verdicts.get(obligation))
            .is_some_and(Result::is_ok)
    };

    // obligations, judged, core refused, unproved, unsupported, needed a
    // relation, needed a branch fact, published clauses held
    let mut counts = [0_usize; 8];
    // Uses of storage judged for initialization, and how many of them are
    // unestablished. Each of those is a compile error by the new route: a
    // rejection of the program, not a construct outside the subset.
    let mut uses = 0_usize;
    let mut unestablished = 0_usize;
    let mut gated = 0_usize;
    for (machine, result) in discharged.machines.iter() {
        uses += result.uses.len();
        for refusal in result.unestablished() {
            unestablished += 1;
            lines.push(format!(
                "use\t{}\t{}\t{:?}\tunestablished\t{}",
                main.display(),
                machine.get(),
                refusal.concerns,
                one_line(&refusal.detail),
            ));
        }
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
                Some(ObligationKind::DataDomain { .. }) => "domain",
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
                // A zero establishment of a gated type: nothing is owed
                // there, and each use its contents reach is unestablished.
                Ok(lowered_discharge::Discharged::Gated(why)) => {
                    gated += 1;
                    ("gated".to_owned(), None, one_line(&why.detail))
                }
                Err(refusal) => {
                    let outcome = match refusal.reason {
                        RefusalReason::Unproved(_) => {
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
    // The route's own verdict, the one a router takes: an unproved obligation
    // rejects the program with a diagnostic, an unsupported one hands the
    // closure back to the current checker.
    let route = match judge(Ok(for_verdict)) {
        Ok(RouteVerdict::Accepted(_)) => "accepted",
        // The unestablished uses were reported above, one `use` line each.
        Ok(RouteVerdict::Rejected { unproved, .. }) => {
            report(main, &unproved, "reject", lines);
            "rejected"
        }
        Ok(RouteVerdict::Incomplete(unproved)) => {
            report(main, &unproved, "incomplete", lines);
            "incomplete"
        }
        Ok(RouteVerdict::FallBack(_)) => "fallback",
        Err(_) => "defect",
    };
    format!(
        "{head}\troute={route}\troute_us={route_us}\tdischarge_us={discharge_us}\treverify_us={reverify_us}\treverified={reverified}\tobligations={}\tjudged={}\tcore_refused={}\tunproved={}\trefused_other={}\tneeded_relation={}\tneeded_branch={}\tpublished_held={}\tuses={uses}\tunestablished={unestablished}\tgated={gated}",
        counts[0], counts[1], counts[2], counts[3], counts[4], counts[5], counts[6], counts[7]
    )
}

/// One `diagnostic` line per unproved obligation, with its outcome.
fn report(main: &Path, unproved: &[UnprovedObligation], outcome: &str, lines: &mut Vec<String>) {
    for entry in unproved {
        let site = match entry.site {
            structure_elaboration::SourceOrigin::Authored(span) => {
                format!("bytes {}..{}", span.span.start, span.span.end)
            }
            other => format!("{other:?}"),
        };
        lines.push(format!(
            "diagnostic\t{}\t{}\t{}\t{outcome}\t{site}\t{}",
            main.display(),
            entry.machine.get(),
            entry.obligation.get(),
            one_line(entry.message())
        ));
    }
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
