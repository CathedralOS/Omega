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
//!     program <path> current=<accepted|rejected|review-refused|stopped> current_us=<n> route=<..> route_us=<n> ...
//!     obligation <path> <machine> <obligation> <kind> <verdict> <values from> <detail>
//!     diagnostic <path> <machine> <obligation> <reject|incomplete> <site> <message>
//!     use <path> <machine> <operation or edge> unestablished <detail>
//!     conflict <path> <machine> <loan obligation> <conflict> <message> <site>
//!     window <path> <machine> <opening obligation> <consumption point> <closed-..|open:..>
//!     current-diagnostic <path> <site> <message>
//!     fallback <path> root=<..> <construct> <refusal detail>
//!     quality <path> root=<..> fragment=<all|part|none|absent> site=<class> expected=<fragments>
//!
//! Every `program` line carries `root=`: `compilation` when the route judges
//! what the check claims (`structure_elaboration::elaborate`), or
//! `machine:<name>` for one machine's closure (below). `diagnostic` and `use`
//! lines end with `site=<class>`, how that error's site compares with the
//! current checker's diagnostics.
//!
//! A `diagnostic` for a value consumed inside an open invariant window
//! carries `consumed at <site>` after the opening write's site.
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
//! (`route=unreached`).
//! If source checking succeeds but the fixture's later entry, service or
//! permission review refuses, `current=review-refused` preserves that boundary.
//! Its diagnostic remains visible, but it is not tallied as a source rejection.
//!
//! **Fixtures that bind no entry.** A Terminal module names one entry, so a
//! compilation with none has no module and is not elaborated
//! (`route=no-entry`); most `fail/` fixtures are like that. With
//! `--every-machine` as the first argument, each machine declared in a
//! source file under the fixture's directory (other than its `build.omg`), or
//! in a source file a current diagnostic names, is instead elaborated as the
//! root of its own closure (`structure_elaboration::elaborate_closure`, one
//! more `program` line each, `root=machine:<name>`).
//!
//! **Diagnostic quality.** A program both checkers reject gets one `quality`
//! line per root the route rejects, weighing its errors against the current
//! checker's:
//!
//! - `fragment=` compares the route's messages with the fixture's
//!   `expected.txt` by the corpus runner's rule (every nonempty line must
//!   occur in some message): `all`, `part`, `none`, or `absent` when the
//!   fixture pins no fragment. Text only: whether a miss names the same
//!   obligation in other words is for a reader to classify.
//! - `site=` is the best match between any route error's source span and any
//!   current diagnostic's: `exact`, `overlap` (the spans intersect),
//!   `same-line`, `different`, `unpositioned:<reason>` when no route error
//!   has a position, or `current-unpositioned` when the current checker gave
//!   none to compare with.
//!
//! `route_one_comparison --tally < output.tsv` reads this tool's own output,
//! from one run or several concatenated, and prints the three diagnostic
//! counts for the programs the current checker rejects: rejected at all by
//! the new route, of those the pinned fragment present, and of those the
//! site matched. A fixture with several roots is rejected when any closure
//! is, and takes the best fragment and site among its closures. A rejection
//! at a site the current checker never names may be of another construct
//! than the one the fixture pins; `route_one_measures.md` classifies those.
//!
//! This is a measuring tool, not a gate, and not part of the shipped CLI.

#[path = "../tests/support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use std::collections::BTreeMap;
use std::io::BufRead;
use std::path::Path;
use std::time::Instant;

use diagnostics::Diagnostic;
use structure_elaboration::SourceOrigin;

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

/// The operands of an exact operation or cast, the subset's only partial
/// operations.
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
        Kind::IntegerExactCast { operand, .. } => vec![*operand],
        _ => Vec::new(),
    }
}

/// Checks one fixture and judges what the route can on it. One `program`
/// line body per root.
fn probe(main: &Path, every_machine: bool, lines: &mut Vec<String>) -> Vec<String> {
    use fixture_package_inputs::FixtureFrontierCheck;
    let started = Instant::now();
    let checked =
        fixture_package_inputs::check_fixture_keeping_typed_frontier(main, "linux_x86_64");
    let current_us = started.elapsed().as_micros();
    // A program the current checker rejects is compared too: the seam keeps
    // the typed trees that checker judged, and its diagnostics.
    let (frontier, current, current_diagnostics) = match checked {
        FixtureFrontierCheck::Accepted { frontier, .. } => (frontier, "accepted", Vec::new()),
        FixtureFrontierCheck::Rejected {
            frontier,
            diagnostics,
        } => (frontier, "rejected", diagnostics),
        FixtureFrontierCheck::ReviewRefused {
            frontier,
            diagnostics,
        } => (frontier, "review-refused", diagnostics),
        FixtureFrontierCheck::StoppedBeforeTypedFrontier(errors) => {
            return vec![format!(
                "current=stopped\tcurrent_us={current_us}\troot=compilation\troute=unreached\t{}",
                one_line(format!("{:?}", errors.first().map(|first| &first.message)))
            )];
        }
    };
    let typed = &frontier.typed;
    let sites = CurrentSites::of(&current_diagnostics, &typed.symbols);
    for diagnostic in &current_diagnostics {
        lines.push(format!(
            "current-diagnostic\t{}\t{}\t{}",
            main.display(),
            sites.describe_current(diagnostic),
            one_line(&diagnostic.message)
        ));
    }
    let head = format!("current={current}\tcurrent_us={current_us}");
    let formation_started = Instant::now();
    match lowered_discharge::check_bound_callable_shapes(typed, &frontier.bindings) {
        Ok(failures) if !failures.is_empty() => {
            let mut best: Option<SiteClass> = None;
            for failure in &failures {
                let site = failure
                    .source_span
                    .map(structure_elaboration::SourceOrigin::Authored);
                let class = sites.class(site);
                best = Some(best.map_or(class, |best| best.min(class)));
                lines.push(format!(
                    "formation\t{}\t{}\t{}\tsite={}",
                    main.display(),
                    one_line(&failure.detail),
                    sites.describe_route(site),
                    class.name()
                ));
            }
            if current == "rejected" {
                let messages: Vec<_> = failures
                    .iter()
                    .map(|failure| failure.detail.clone())
                    .collect();
                let (fragment, expected) = pinned_fragments(main, &messages);
                let site = if sites.spans.is_empty() {
                    "current-unpositioned".to_owned()
                } else {
                    best.map_or_else(|| "-".to_owned(), SiteClass::name)
                };
                lines.push(format!(
                    "quality\t{}\troot=compilation\tfragment={fragment}\tsite={site}\texpected={}",
                    main.display(),
                    one_line(expected)
                ));
            }
            return vec![format!(
                "{head}\troot=compilation\troute=rejected\troute_us={}\tformations={}",
                formation_started.elapsed().as_micros(),
                failures.len()
            )];
        }
        Err(defect) => {
            return vec![format!(
                "{head}\troot=compilation\troute=defect\t{}",
                one_line(defect.detail)
            )];
        }
        Ok(_) => {}
    }
    let judged = |label: &str,
                  lines: &mut Vec<String>,
                  elaborate: &dyn Fn() -> terminal_check_tables::Checked<
        structure_elaboration::Elaboration,
    >| {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let started = Instant::now();
            let elaborated = elaborate();
            judge_elaboration(main, elaborated, started, label, current, &sites, lines)
        }))
        .unwrap_or_else(|_| "route=panicked".to_owned())
    };
    if !frontier.entries.is_empty() {
        let compilation = structure_elaboration::Compilation {
            entries: &frontier.entries,
            subjects: &[frontier.subject],
            purpose: source::DependencyScope::Product,
            selected: &frontier.selected,
            bindings: &frontier.bindings,
        };
        let line = judged("compilation", lines, &|| {
            structure_elaboration::elaborate(typed, compilation)
        });
        return vec![format!("{head}\troot=compilation\t{line}")];
    }
    let mut programs = vec![format!("{head}\troot=compilation\troute=no-entry")];
    if !every_machine {
        return programs;
    }
    // The checked copy lives in a session directory holding `source/<fixture
    // path>`, so a source is the fixture's own when one of its directories
    // ends with the fixture's directory as given.
    let fixture = main
        .parent()
        .filter(|fixture| !fixture.as_os_str().is_empty());
    for machine in typed.machines() {
        let Some(declared) = typed.symbols.symbol_source_span(machine.symbol) else {
            continue;
        };
        let own = fixture.is_some_and(|fixture| {
            typed.symbols.source_file(declared).is_some_and(|file| {
                // `build.omg` declares the Build, which the product check
                // does not judge.
                file.path.file_name() != Some(std::ffi::OsStr::new("build.omg"))
                    && file
                        .path
                        .ancestors()
                        .any(|directory| directory.ends_with(fixture))
            })
        });
        let diagnosed = sites
            .spans
            .iter()
            .any(|current| current.source_id == declared.source_id);
        if !(own || diagnosed) {
            continue;
        }
        let label = format!("machine:{}", machine.name.as_str());
        let line = judged(&label, lines, &|| {
            structure_elaboration::elaborate_closure(typed, machine.symbol)
        });
        programs.push(format!("{head}\troot={label}\t{line}"));
    }
    programs
}

/// Discharges one elaboration and returns the fields of its `program` line
/// from `route=` on.
fn judge_elaboration(
    main: &Path,
    elaborated: terminal_check_tables::Checked<structure_elaboration::Elaboration>,
    started: Instant,
    label: &str,
    current: &str,
    sites: &CurrentSites<'_>,
    lines: &mut Vec<String>,
) -> String {
    let mut elaboration = match elaborated {
        Ok(elaboration) => elaboration,
        Err(refusal) => {
            return format!(
                "route=outside\t{}",
                one_line(format!("{:?}: {}", refusal.reason, refusal.detail))
            );
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
                "route=refused-module\troute_us={route_us}\treason={}\t{}",
                one_line(format!("{:?}", refusal.reason)),
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
    let mut opened = 0_usize;
    let mut consumed = 0_usize;
    let mut closed = 0_usize;
    for (machine, result) in discharged.machines.iter() {
        uses += result.uses.len();
        for window in &result.windows {
            consumed += 1;
            let outcome = match &window.verdict {
                Ok(evidence) => {
                    closed += 1;
                    match &evidence.fact.route {
                        AcceptedFactRoute::CertificateDerived { acceptance, .. } => {
                            match &acceptance.mathematical_core {
                                MathematicalCoreDecision::Judged(_) => "closed-judged".to_owned(),
                                MathematicalCoreDecision::Refused(family) => {
                                    format!("closed-core-refused:{family}")
                                }
                            }
                        }
                        other => format!("unexpected-route:{other:?}"),
                    }
                }
                Err(refusal) => format!("open:{:?}:{}", refusal.reason, one_line(&refusal.detail)),
            };
            lines.push(format!(
                "window\t{}\t{}\t{}\t{:?}\t{outcome}",
                main.display(),
                machine.get(),
                window.opened_by.get(),
                window.at,
            ));
        }
        // Each unestablished use is printed with its site by the verdict.
        unestablished += result.unestablished().count();
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
                Some(ObligationKind::ArrivalInvariant { .. }) => "arrival",
                Some(ObligationKind::CrashCoverage { .. }) => "crash",
                Some(ObligationKind::WellFoundedness { .. }) => "well-founded",
                Some(ObligationKind::RankComparison { .. }) => "rank",
                Some(ObligationKind::Claim { .. }) => "claim",
                Some(ObligationKind::Loan { .. }) => "loan",
                Some(ObligationKind::RestorationWindow { .. }) => "restoration",
                Some(ObligationKind::DomainDebt { .. }) => "domain-debt",
                Some(ObligationKind::EstablishmentDebt { .. }) => "establishment-debt",
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
                // A store or zero establishment not shown to keep a clause:
                // it opens an invariant window, owed at each consumption
                // point the window reaches open (one `window` line each).
                Ok(lowered_discharge::Discharged::Window(why)) => {
                    opened += 1;
                    ("window".to_owned(), None, one_line(&why.detail))
                }
                Ok(lowered_discharge::Discharged::Preserved) => {
                    ("preserved".to_owned(), None, "-".to_owned())
                }
                // A loan judged by structure: no certificate.
                Ok(lowered_discharge::Discharged::Lent) => {
                    ("lent".to_owned(), None, "-".to_owned())
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
                    if let Some(ObligationKind::OperationSafety {
                        site:
                            terminal_check_tables::obligations::ExecutionSite::Operation(operation),
                    }) = tables.obligations.get(obligation).map(|row| row.kind)
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
    let weakened_joins: usize = discharged
        .machines
        .values()
        .map(|machine| machine.weakened_joins.len())
        .sum();
    // The route's own verdict, the one a router takes: an unproved obligation
    // rejects the program with a diagnostic, an unsupported one hands the
    // closure back to the current checker.
    let route = match judge(Ok(for_verdict)) {
        Ok(RouteVerdict::Accepted(_)) => "accepted",
        Ok(RouteVerdict::Rejected {
            formations,
            unproved,
            unestablished,
            conflicts,
        }) => {
            debug_assert!(
                formations.is_empty(),
                "body-only judgment has no binding formation pass"
            );
            let mut best = report(main, &unproved, "reject", sites, lines);
            for entry in &conflicts {
                let class = sites.class(Some(entry.site));
                lines.push(format!(
                    "conflict\t{}\t{}\t{}\t{:?}\t{}\t{}\tsite={}",
                    main.display(),
                    entry.machine.get(),
                    entry.loan.get(),
                    entry.conflict,
                    one_line(entry.message()),
                    sites.describe_route(Some(entry.site)),
                    class.name(),
                ));
                best = Some(best.map_or(class, |best| best.min(class)));
            }
            for entry in &unestablished {
                let class = sites.class(entry.site);
                lines.push(format!(
                    "use\t{}\t{}\t{:?}\tunestablished\t{}\t{}\tsite={}",
                    main.display(),
                    entry.machine.get(),
                    entry.at,
                    one_line(&entry.detail),
                    sites.describe_route(entry.site),
                    class.name(),
                ));
                best = Some(best.map_or(class, |best| best.min(class)));
            }
            if current == "rejected" {
                let messages: Vec<String> = unproved
                    .iter()
                    .map(UnprovedObligation::message)
                    .chain(unestablished.iter().map(|entry| entry.message()))
                    .chain(conflicts.iter().map(|entry| entry.message()))
                    .collect();
                let (fragment, expected) = pinned_fragments(main, &messages);
                lines.push(format!(
                    "quality\t{}\troot={label}\tfragment={fragment}\tsite={}\texpected={}",
                    main.display(),
                    if sites.spans.is_empty() {
                        "current-unpositioned".to_owned()
                    } else {
                        best.map_or_else(|| "-".to_owned(), SiteClass::name)
                    },
                    one_line(expected),
                ));
            }
            "rejected"
        }
        Ok(RouteVerdict::Incomplete(unproved)) => {
            report(main, &unproved, "incomplete", sites, lines);
            "incomplete"
        }
        Ok(RouteVerdict::FallBack(refusal)) => {
            lines.push(format!(
                "fallback\t{}\troot={label}\t{:?}\t{}",
                main.display(),
                refusal.concerns,
                one_line(&refusal.detail)
            ));
            "fallback"
        }
        Err(_) => "defect",
    };
    format!(
        "route={route}\troute_us={route_us}\tdischarge_us={discharge_us}\treverify_us={reverify_us}\treverified={reverified}\tobligations={}\tjudged={}\tcore_refused={}\tunproved={}\trefused_other={}\tneeded_relation={}\tneeded_branch={}\tpublished_held={}\tuses={uses}\tunestablished={unestablished}\twindows={opened}\tconsumed={consumed}\tclosed={closed}\tweakened_joins={weakened_joins}",
        counts[0], counts[1], counts[2], counts[3], counts[4], counts[5], counts[6], counts[7]
    )
}

/// One `diagnostic` line per unproved obligation, with its outcome and how
/// its site compares with the current checker's. Returns the best site class,
/// or `None` when there is no unproved obligation.
fn report(
    main: &Path,
    unproved: &[UnprovedObligation],
    outcome: &str,
    sites: &CurrentSites<'_>,
    lines: &mut Vec<String>,
) -> Option<SiteClass> {
    let mut best: Option<SiteClass> = None;
    for entry in unproved {
        let class = sites.class(Some(entry.site));
        best = Some(best.map_or(class, |best| best.min(class)));
        // An unclosed window names its consumption point beside the
        // opening write.
        let consumed = match entry.consumed {
            Some(consumed) => format!(
                "\tconsumed at {}",
                sites.describe_route(Some(consumed.site))
            ),
            None => String::new(),
        };
        lines.push(format!(
            "diagnostic\t{}\t{}\t{}\t{outcome}\t{}{consumed}\t{}\tsite={}",
            main.display(),
            entry.machine.get(),
            entry.obligation.get(),
            sites.describe_route(Some(entry.site)),
            one_line(entry.message()),
            class.name(),
        ));
    }
    best
}

/// How a route error's source site compares with the current checker's
/// diagnostics, best first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SiteClass {
    Exact,
    Overlap,
    SameLine,
    Different,
    Unpositioned(Option<structure_elaboration::Unpositioned>),
}

impl SiteClass {
    fn rank(self) -> u8 {
        match self {
            Self::Exact => 0,
            Self::Overlap => 1,
            Self::SameLine => 2,
            Self::Different => 3,
            Self::Unpositioned(_) => 4,
        }
    }

    /// The better of two classes; the first on a tie.
    fn min(self, other: Self) -> Self {
        if other.rank() < self.rank() {
            other
        } else {
            self
        }
    }

    fn name(self) -> String {
        match self {
            Self::Exact => "exact".to_owned(),
            Self::Overlap => "overlap".to_owned(),
            Self::SameLine => "same-line".to_owned(),
            Self::Different => "different".to_owned(),
            Self::Unpositioned(Some(why)) => format!("unpositioned:{why:?}"),
            Self::Unpositioned(None) => "unpositioned:NoCorrespondence".to_owned(),
        }
    }
}

/// The source spans the current checker's diagnostics name, and the
/// compilation's own source map, to name the file and line of any span.
struct CurrentSites<'a> {
    spans: Vec<source::SourceSpan>,
    symbols: &'a symbols::SymbolTable,
}

impl<'a> CurrentSites<'a> {
    fn of(diagnostics: &[Diagnostic], symbols: &'a symbols::SymbolTable) -> Self {
        let spans = diagnostics
            .iter()
            .filter_map(|diagnostic| diagnostic.source_span)
            .collect();
        Self { spans, symbols }
    }

    /// The one-based line of a span's start, from the source it lies in.
    fn line(&self, span: source::SourceSpan) -> Option<usize> {
        let file = self.symbols.source_file(span)?;
        let prefix = file.source.as_bytes().get(..span.span.start)?;
        Some(1 + prefix.iter().filter(|byte| **byte == b'\n').count())
    }

    fn describe_span(&self, span: source::SourceSpan) -> String {
        let name = self.symbols.source_file(span).map_or_else(
            || format!("source {}", span.source_id.0),
            |file| {
                file.path
                    .file_name()
                    .map_or_else(String::new, |name| name.to_string_lossy().into_owned())
            },
        );
        match self.line(span) {
            Some(line) => format!("{name}:{line} bytes {}..{}", span.span.start, span.span.end),
            None => format!("{name} bytes {}..{}", span.span.start, span.span.end),
        }
    }

    fn describe_current(&self, diagnostic: &Diagnostic) -> String {
        diagnostic
            .source_span
            .map_or_else(|| "no-span".to_owned(), |span| self.describe_span(span))
    }

    fn describe_route(&self, site: Option<SourceOrigin>) -> String {
        match site {
            Some(SourceOrigin::Authored(span)) => self.describe_span(span),
            Some(SourceOrigin::Unpositioned(why)) => format!("Unpositioned({why:?})"),
            None => "no-correspondence".to_owned(),
        }
    }

    /// The best match between one route site and any current diagnostic.
    fn class(&self, site: Option<SourceOrigin>) -> SiteClass {
        let span = match site {
            Some(SourceOrigin::Authored(span)) => span,
            Some(SourceOrigin::Unpositioned(why)) => return SiteClass::Unpositioned(Some(why)),
            None => return SiteClass::Unpositioned(None),
        };
        self.spans
            .iter()
            .map(|current| {
                if current.source_id != span.source_id {
                    SiteClass::Different
                } else if current.span.start == span.span.start && current.span.end == span.span.end
                {
                    SiteClass::Exact
                } else if current.span.start < span.span.end && span.span.start < current.span.end {
                    SiteClass::Overlap
                } else if self.line(*current).is_some() && self.line(*current) == self.line(span) {
                    SiteClass::SameLine
                } else {
                    SiteClass::Different
                }
            })
            .reduce(SiteClass::min)
            .unwrap_or(SiteClass::Different)
    }
}

/// Weighs the route's messages against the fixture's `expected.txt` by the
/// corpus runner's rule: every nonempty line must occur in some message.
fn pinned_fragments(main: &Path, messages: &[String]) -> (&'static str, String) {
    let expected = main
        .parent()
        .and_then(|fixture| std::fs::read_to_string(fixture.join("expected.txt")).ok())
        .unwrap_or_default();
    let fragments: Vec<&str> = expected
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    let found = fragments
        .iter()
        .filter(|fragment| messages.iter().any(|message| message.contains(**fragment)))
        .count();
    let class = if fragments.is_empty() {
        "absent"
    } else if found == fragments.len() {
        "all"
    } else if found > 0 {
        "part"
    } else {
        "none"
    };
    (class, fragments.join(" | "))
}

/// What `--tally` keeps for one fixture and one choice of roots.
#[derive(Default)]
struct Tallied {
    routes: Vec<String>,
    fragment: Option<usize>,
    site: Option<(usize, String)>,
}

const FRAGMENTS: [&str; 4] = ["all", "part", "none", "absent"];
const SITES: [&str; 6] = [
    "exact",
    "overlap",
    "same-line",
    "different",
    "unpositioned",
    "current-unpositioned",
];

impl Tallied {
    /// One verdict for the fixture from its roots' verdicts. A rejection by
    /// any closure rejects the program; a closure the route does not serve
    /// hands that closure back to the current checker.
    fn route(&self) -> String {
        let has = |name: &str| self.routes.iter().any(|route| route.starts_with(name));
        let first = |name: &str| self.routes.iter().find(|route| route.starts_with(name));
        for decided in ["rejected", "incomplete"] {
            if has(decided) {
                return decided.to_owned();
            }
        }
        for undecided in ["fallback", "refused-module", "defect", "panicked"] {
            if let Some(route) = first(undecided) {
                return route.clone();
            }
        }
        match (has("accepted"), first("outside")) {
            (true, Some(_)) => "accepted, other closures outside".to_owned(),
            (true, None) => "accepted".to_owned(),
            (false, Some(outside)) => outside.clone(),
            (false, None) => self
                .routes
                .first()
                .cloned()
                .unwrap_or_else(|| "-".to_owned()),
        }
    }
}

/// Reads this tool's output and prints the three diagnostic counts for the
/// programs the current checker rejects: once for what the check claims
/// alone, and once more over every root when the run used `--every-machine`.
fn tally(input: impl BufRead) {
    // Per fixture: the entry root alone, then every root.
    let mut fixtures: BTreeMap<String, [Tallied; 2]> = BTreeMap::new();
    let mut programs = std::collections::BTreeSet::new();
    let mut machine_roots = false;
    for line in input.lines().map_while(Result::ok) {
        let fields: Vec<&str> = line.split('\t').collect();
        let field = |name: &str| {
            fields
                .iter()
                .find_map(|field| field.strip_prefix(name))
                .unwrap_or("-")
                .to_owned()
        };
        let Some(path) = fields.get(1) else {
            continue;
        };
        let entry_root = field("root=") == "compilation";
        machine_roots |= !entry_root && field("root=") != "-";
        match fields.first().copied() {
            Some("program") => {
                programs.insert((*path).to_owned());
                if field("current=") != "rejected" {
                    continue;
                }
                let mut route = field("route=");
                if route == "outside" {
                    // The refusal reason leads the field after `route=`.
                    let detail = fields
                        .iter()
                        .position(|field| *field == "route=outside")
                        .and_then(|at| fields.get(at + 1));
                    let reason = detail.and_then(|detail| detail.split(':').next());
                    route = format!("outside:{}", reason.unwrap_or("-"));
                } else if route == "refused-module" {
                    route = format!("refused-module:{}", field("reason="));
                }
                let both = fixtures.entry((*path).to_owned()).or_default();
                if entry_root {
                    both[0].routes.push(route.clone());
                }
                both[1].routes.push(route);
            }
            Some("quality") => {
                let fragment = FRAGMENTS
                    .iter()
                    .position(|class| *class == field("fragment="));
                let named = field("site=");
                let site = SITES
                    .iter()
                    .position(|class| named.starts_with(class))
                    .map(|rank| (rank, named));
                let both = fixtures.entry((*path).to_owned()).or_default();
                for tallied in both.iter_mut().skip(usize::from(!entry_root)) {
                    tallied.fragment = match (tallied.fragment, fragment) {
                        (Some(old), Some(new)) => Some(old.min(new)),
                        (old, new) => old.or(new),
                    };
                    tallied.site = match (tallied.site.take(), site.clone()) {
                        (Some(old), Some(new)) => Some(if new.0 < old.0 { new } else { old }),
                        (old, new) => old.or(new),
                    };
                }
            }
            _ => {}
        }
    }
    println!("programs\t{}", programs.len());
    println!("current checker rejects\t{}", fixtures.len());
    let choices = [
        "roots: what the check claims (fixtures that bind an entry)",
        "roots: as above, and each own machine of a fixture that binds no entry",
    ];
    for (choice, title) in choices.iter().enumerate() {
        if choice == 1 && !machine_roots {
            break;
        }
        let mut routes: BTreeMap<String, usize> = BTreeMap::new();
        let mut fragments: BTreeMap<&str, usize> = BTreeMap::new();
        let mut sites: BTreeMap<String, usize> = BTreeMap::new();
        for both in fixtures.values() {
            let tallied = &both[choice];
            let route = tallied.route();
            if route == "rejected" {
                let fragment = tallied.fragment.map_or("-", |rank| FRAGMENTS[rank]);
                *fragments.entry(fragment).or_default() += 1;
                let site = tallied
                    .site
                    .as_ref()
                    .map_or_else(|| "-".to_owned(), |(_, name)| name.clone());
                *sites.entry(site).or_default() += 1;
            }
            *routes.entry(route).or_default() += 1;
        }
        let lowered: usize = routes
            .iter()
            .filter(|(route, _)| {
                !route.starts_with("outside")
                    && !matches!(route.as_str(), "unreached" | "no-entry" | "-")
            })
            .map(|(_, count)| count)
            .sum();
        let rejected = routes.get("rejected").copied().unwrap_or(0);
        println!("\n{title}");
        for (route, count) in &routes {
            println!("  route={route}\t{count}");
        }
        println!("lowered by the new route\t{lowered}");
        println!("1. rejected by the new route\t{rejected} of {lowered}");
        let all = fragments.get("all").copied().unwrap_or(0);
        println!("2. message contains every pinned fragment\t{all} of {rejected}");
        for (class, count) in &fragments {
            println!("  fragment={class}\t{count}");
        }
        let right: usize = sites
            .iter()
            .filter(|(class, _)| matches!(class.as_str(), "exact" | "overlap" | "same-line"))
            .map(|(_, count)| count)
            .sum();
        println!("3. site matches the current checker's\t{right} of {rejected}");
        for (class, count) in &sites {
            println!("  site={class}\t{count}");
        }
    }
}

fn main() {
    let mut arguments = std::env::args().skip(1).peekable();
    if arguments.peek().map(String::as_str) == Some("--tally") {
        tally(std::io::stdin().lock());
        return;
    }
    let every_machine = arguments
        .next_if(|first| first == "--every-machine")
        .is_some();
    for argument in arguments {
        let main = Path::new(&argument);
        let mut lines = Vec::new();
        let programs = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            probe(main, every_machine, &mut lines)
        }))
        .unwrap_or_else(|_| vec!["panicked".to_owned()]);
        for program in programs {
            println!("program\t{argument}\t{program}");
        }
        for line in lines {
            println!("{line}");
        }
    }
}
