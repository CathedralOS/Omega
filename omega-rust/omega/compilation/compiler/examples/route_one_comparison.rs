//! Route one's side-by-side comparison: the current checker and the checker on
//! the lowered form, on the same corpus programs.
//!
//! For each `main.omg` given on the command line it checks the fixture with
//! the ordinary pipeline through the typed-frontier seam (the current
//! checker's verdict and diagnostics, the frozen typed trees and the bound
//! entry, for rejected programs as well as accepted ones), then elaborates
//! the entry with `structure_elaboration` and discharges it with
//! `lowered_discharge`, twice: with every kind of knowledge, and without
//! branch conditions, to count what branch facts buy. Each route is timed by
//! wall clock around its own calls only. Tab-separated lines:
//!
//!     program <path> current=<accepted|rejected|review-refused> current_us=<n> root=<..> route=<..> route_us=<n> ...
//!     obligation <path> <machine> <obligation> <kind> <verdict> <values from> <detail>
//!     diagnostic <path> <machine> <obligation> <reject|incomplete> <site> <message> site=<class>
//!     use <path> <machine> <operation or edge> unestablished <detail> <site> site=<class>
//!     current-diagnostic <path> <site> <message>
//!     quality <path> root=<..> fragment=<all|part|none|absent> site=<class> expected=<fragments>
//!
//! `route=` is `lowered_discharge::judge`'s verdict: `accepted`, `rejected`
//! (one `diagnostic` line per unproved obligation and one `use` line per use
//! of storage that is not established, the compile errors the program gets on
//! this route), `incomplete` (the prover has no rule for some
//! goal's form), `fallback` (an unsupported obligation hands the closure to
//! the current checker) or `defect`. Before a verdict exists it is `outside`
//! (elaboration refused the closure; the refusal reason leads the detail),
//! `refused-module` (discharge refused it whole, with `reason=`), `no-entry`,
//! or `unreached` when the compilation stopped before Psi 04 produced typed
//! trees.
//!
//! **Roots.** The route elaborates the Build's bound entry and the machines
//! it calls (`root=entry`). Most `fail/` fixtures bind no entry, and a defect
//! in a machine the entry never calls is outside that closure. With
//! `--every-machine` as the first argument, each machine declared in a source
//! file that a current diagnostic names is also elaborated as the root of its
//! own closure (`root=machine:<name>`, one more `program` line each): what
//! the route says once it elaborates every checked machine.
//!
//! **Diagnostic quality.** A program both checkers reject gets one `quality`
//! line that weighs the new route's errors against the current checker's:
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
//!   none to compare with. Each `diagnostic` and `use` line carries its own.
//!
//! `route_one_comparison --tally < output.tsv` reads this tool's own output,
//! from one run or several concatenated, and prints the three diagnostic
//! counts for the programs the current checker rejects: rejected at all by
//! the new route, of those the pinned fragment present, and of those the
//! site matched. It prints them for the entry's closure alone and, when the
//! run used `--every-machine`, again over all roots: a fixture is rejected
//! when any closure is, and takes the best fragment and site among its
//! closures.
//!
//! This is a measuring tool, not a gate, and not part of the shipped CLI.

#[path = "../tests/support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use std::collections::BTreeMap;
use std::io::BufRead;
use std::path::{Path, PathBuf};
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

/// Checks one fixture and judges each root's closure on the new route. One
/// `program` line body per root; the first is always the entry's.
fn probe(main: &Path, every_machine: bool, lines: &mut Vec<String>) -> Vec<String> {
    use fixture_package_inputs::FixtureFrontierCheck;
    let started = Instant::now();
    let checked =
        fixture_package_inputs::check_fixture_keeping_typed_frontier(main, "linux_x86_64");
    let current_us = started.elapsed().as_micros();
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
        FixtureFrontierCheck::StoppedBeforeTypedFrontier(diagnostics) => {
            return vec![format!(
                "current=rejected\tcurrent_us={current_us}\troot=entry\troute=unreached\t{}",
                one_line(format!(
                    "{:?}",
                    diagnostics.first().map(|first| &first.message)
                ))
            )];
        }
    };
    let sites = CurrentSites::of(&current_diagnostics);
    for diagnostic in &current_diagnostics {
        lines.push(format!(
            "current-diagnostic\t{}\t{}\t{}",
            main.display(),
            sites.describe_current(diagnostic),
            one_line(&diagnostic.message)
        ));
    }
    let head = format!("current={current}\tcurrent_us={current_us}");
    let typed = &frontier.typed;
    let mut programs = Vec::new();
    let mut roots = Vec::new();
    let entry = frontier.entries.first().copied();
    match entry {
        Some(entry) => roots.push(("entry".to_owned(), entry)),
        None => programs.push(format!("{head}\troot=entry\troute=no-entry")),
    }
    if every_machine {
        // The route elaborates only the entry's call closure today. Rooting
        // it at each machine of a source file the current checker's
        // diagnostics name measures what it says once it elaborates every
        // checked machine, and reaches fixtures that bind no entry.
        for machine in typed.machines() {
            let diagnosed = typed
                .symbols
                .symbol_source_span(machine.symbol)
                .is_some_and(|declared| {
                    sites
                        .spans
                        .iter()
                        .any(|current| current.source_id == declared.source_id)
                });
            if diagnosed && Some(machine.symbol) != entry {
                roots.push((format!("machine:{}", machine.name.as_str()), machine.symbol));
            }
        }
    }
    for (label, root) in roots {
        let judged = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            probe_root(main, typed, root, &label, current, &sites, lines)
        }))
        .unwrap_or_else(|_| "route=panicked".to_owned());
        programs.push(format!("{head}\troot={label}\t{judged}"));
    }
    programs
}

/// Elaborates `root`'s closure, discharges it and returns the fields of its
/// `program` line from `route=` on.
fn probe_root(
    main: &Path,
    typed: &typed_trees::TypedTrees,
    root: symbols::SymbolHandle,
    label: &str,
    current: &str,
    sites: &CurrentSites,
    lines: &mut Vec<String>,
) -> String {
    let started = Instant::now();
    let elaborated = structure_elaboration::elaborate(typed, root);
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
    let discharged = discharge_with(&mut elaboration, Knowledge::All);
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
        Ok(RouteVerdict::Rejected {
            unproved,
            unestablished,
        }) => {
            let mut best = report(main, &unproved, "reject", sites, lines);
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
        Ok(RouteVerdict::FallBack(_)) => "fallback",
        Err(_) => "defect",
    };
    format!(
        "route={route}\troute_us={route_us}\tobligations={}\tjudged={}\tcore_refused={}\tunproved={}\trefused_other={}\tneeded_relation={}\tneeded_branch={}\tpublished_held={}\tuses={uses}\tunestablished={unestablished}\tgated={gated}",
        counts[0], counts[1], counts[2], counts[3], counts[4], counts[5], counts[6], counts[7]
    )
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

/// The source spans the current checker's diagnostics name, and the text of
/// each source they name, to turn a byte offset into a line.
struct CurrentSites {
    spans: Vec<source::SourceSpan>,
    texts: BTreeMap<usize, (PathBuf, String)>,
}

impl CurrentSites {
    fn of(diagnostics: &[Diagnostic]) -> Self {
        let mut texts = BTreeMap::new();
        let mut spans = Vec::new();
        for diagnostic in diagnostics {
            let Some(span) = diagnostic.source_span else {
                continue;
            };
            spans.push(span);
            if let Some(location) = &diagnostic.resolved_location
                && let Ok(text) = std::fs::read_to_string(&location.path)
            {
                texts
                    .entry(span.source_id.0)
                    .or_insert((location.path.clone(), text));
            }
        }
        Self { spans, texts }
    }

    /// The one-based line of a byte offset, when the source's text is known.
    fn line(&self, span: source::SourceSpan) -> Option<usize> {
        let (_, text) = self.texts.get(&span.source_id.0)?;
        let prefix = text.as_bytes().get(..span.span.start)?;
        Some(1 + prefix.iter().filter(|byte| **byte == b'\n').count())
    }

    fn describe_span(&self, span: source::SourceSpan) -> String {
        let name = self.texts.get(&span.source_id.0).map_or_else(
            || format!("source {}", span.source_id.0),
            |(path, _)| {
                path.file_name()
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

/// One `diagnostic` line per unproved obligation, with its outcome and how
/// its site compares with the current checker's. Returns the best site class,
/// or `None` when there is no unproved obligation.
fn report(
    main: &Path,
    unproved: &[UnprovedObligation],
    outcome: &str,
    sites: &CurrentSites,
    lines: &mut Vec<String>,
) -> Option<SiteClass> {
    let mut best: Option<SiteClass> = None;
    for entry in unproved {
        let class = sites.class(Some(entry.site));
        best = Some(best.map_or(class, |best| best.min(class)));
        lines.push(format!(
            "diagnostic\t{}\t{}\t{}\t{outcome}\t{}\t{}\tsite={}",
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
/// programs the current checker rejects: once for the entry's closure alone,
/// and once more over every root when the run used `--every-machine`.
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
        let entry_root = field("root=") == "entry";
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
        "roots: the bound entry",
        "roots: the entry and every machine of a diagnosed source file",
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
