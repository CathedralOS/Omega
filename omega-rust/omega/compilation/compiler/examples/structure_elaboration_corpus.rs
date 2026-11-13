//! Route one's corpus probe for structure-only elaboration.
//!
//! For each `main.omg` given on the command line: check the fixture with the
//! ordinary pipeline through the typed-frontier seam (only to obtain its frozen
//! typed trees, its bound program entry and the current checker's verdict; no
//! checker fact is read), then elaborate the entry with `structure_elaboration`
//! whether or not the current checker accepted the program. An accepted
//! program's unsealed module is also encoded, loaded into the Terminal
//! interpreter and run. One tab-separated line per fixture:
//!
//!     <path> <stage> <detail> [<summary> <expected exit>] reached04=.. current=.. route=.. [reason=..]
//!
//! where stage is `check-failed`, `no-entry`, `refused`, `encode-refused`,
//! `load-refused`, `run-failed` or `ran`, as before the seam existed, so counts
//! by stage stay comparable with earlier runs. The trailing fields are new:
//! `reached04` says whether Psi 04 produced typed trees; `current` is the
//! current checker's verdict (`accepted`, `rejected`, `review-refused` when the
//! checker accepted and the fixture harness then refused, or `none`); `route`
//! is the elaboration result (`lowered`, `refused` with `reason`, `no-entry`,
//! or `none` when there were no trees). `total` lines follow the fixtures.
//! This is a measuring tool, not a gate.

#[path = "../tests/support/fixture_package_inputs.rs"]
mod fixture_package_inputs;

use std::path::Path;

use psi_work::WorkMeter;
use terminal_check_tables::obligations::ObligationKind;
use terminal_interpreter::{
    TerminalEffect, TerminalEffectHandler, TerminalEffectRejection, TerminalExecution,
    TerminalStructuralInputs,
};

struct NoEffects;

impl TerminalEffectHandler for NoEffects {
    fn handle_effect(&mut self, _: &TerminalEffect) -> Result<(), TerminalEffectRejection> {
        Err(TerminalEffectRejection::new(
            "the scalar subset performs no effect",
        ))
    }
}

fn first_line(text: impl std::fmt::Debug) -> String {
    format!("{text:?}")
        .replace(['\t', '\n'], " ")
        .chars()
        .take(400)
        .collect()
}

/// One fixture's result: the stage and detail fields this probe has always
/// printed, then what the seam and the new route reported.
struct Probed {
    legacy: String,
    reached: &'static str,
    current: &'static str,
    route: &'static str,
    reason: Option<String>,
}

impl Probed {
    fn line(&self) -> String {
        let mut line = format!(
            "{}\treached04={}\tcurrent={}\troute={}",
            self.legacy, self.reached, self.current, self.route
        );
        if let Some(reason) = &self.reason {
            line.push_str("\treason=");
            line.push_str(reason);
        }
        line
    }
}

fn probe(main: &Path) -> Probed {
    use fixture_package_inputs::FixtureFrontierCheck;
    let fixture = main.parent().map_or(main, |parent| parent);
    let expected = std::fs::read_to_string(fixture.join("expected_native_exit.txt"))
        .map(|text| text.trim().to_owned())
        .unwrap_or_default();
    let check_failed = |errors: &[diagnostics::Diagnostic]| {
        format!("check-failed\t{}", first_line(errors.first()))
    };
    // `failed` is the stage field of a program the ordinary check did not
    // return: it stays `check-failed` even when the new route goes on to
    // lower the program, so earlier counts by stage are unchanged.
    let (frontier, current, failed) =
        match fixture_package_inputs::check_fixture_keeping_typed_frontier(main, "linux_x86_64") {
            FixtureFrontierCheck::Accepted { frontier, .. } => (frontier, "accepted", None),
            FixtureFrontierCheck::Rejected {
                frontier,
                diagnostics,
            } => (frontier, "rejected", Some(check_failed(&diagnostics))),
            FixtureFrontierCheck::ReviewRefused {
                frontier,
                diagnostics,
            } => (frontier, "review-refused", Some(check_failed(&diagnostics))),
            FixtureFrontierCheck::StoppedBeforeTypedFrontier(diagnostics) => {
                return Probed {
                    legacy: check_failed(&diagnostics),
                    reached: "no",
                    current: "none",
                    route: "none",
                    reason: None,
                };
            }
        };
    let probed = |legacy: String, route, reason| Probed {
        legacy: failed.clone().unwrap_or(legacy),
        reached: "yes",
        current,
        route,
        reason,
    };
    if frontier.entries.is_empty() {
        return probed("no-entry\t-".to_owned(), "no-entry", None);
    }
    if std::env::var_os("STRUCTURE_ELABORATION_DUMP").is_some() {
        dump_transitions(&frontier.typed);
    }
    let elaboration = match structure_elaboration::elaborate(
        &frontier.typed,
        structure_elaboration::Compilation {
            entries: &frontier.entries,
            subjects: &[frontier.subject],
            purpose: source::DependencyScope::Product,
            selected: &frontier.selected,
            bindings: &frontier.bindings,
        },
    ) {
        Ok(elaboration) => elaboration,
        Err(refusal) => {
            return probed(
                format!("refused\t{}", refusal.detail),
                "refused",
                Some(format!("{:?}: {}", refusal.reason, refusal.detail)),
            );
        }
    };
    if failed.is_some() {
        // The current checker did not accept this program, so there is no
        // expected run to compare with; the new route's prover decides it.
        return probed(String::new(), "lowered", None);
    }
    let mut counts = [0_usize; 4];
    let mut values = 0;
    let mut edges = 0;
    for tables in (1..=elaboration.tables.machines.len() as u64)
        .filter_map(semantic_vocabulary::PsiSemanticId::new)
        .filter_map(|machine| elaboration.tables.machines.get(machine))
    {
        values += tables.values.len();
        edges += tables.edges.len();
        for raw in 1..=tables.obligations.len() as u64 {
            let Some(row) = semantic_vocabulary::PsiSemanticId::new(raw)
                .and_then(|obligation| tables.obligations.get(obligation))
            else {
                continue;
            };
            let slot = match row.kind {
                ObligationKind::OperationSafety { .. } => 0,
                ObligationKind::CallRequirement { .. } => 1,
                ObligationKind::NormalGuarantee { .. } => 2,
                _ => 3,
            };
            counts[slot] += 1;
        }
    }
    let summary = format!(
        "machines={} values={values} edges={edges} safety={} requirement={} guarantee={} other={}",
        elaboration.machines.len(),
        counts[0],
        counts[1],
        counts[2],
        counts[3]
    );
    let bytes = match terminal_codec::encode_module(&elaboration.module) {
        Ok(bytes) => bytes,
        Err(error) => {
            return probed(
                format!("encode-refused\t{}\t{summary}", first_line(error)),
                "lowered",
                None,
            );
        }
    };
    let mut execution =
        match TerminalExecution::start_artifact(&bytes, &[], TerminalStructuralInputs::default()) {
            Ok(execution) => execution,
            Err(error) => {
                return probed(
                    format!("load-refused\t{}\t{summary}", first_line(error)),
                    "lowered",
                    None,
                );
            }
        };
    let legacy = match execution.resume(&mut WorkMeter::with_budget(10_000_000), &mut NoEffects) {
        Ok(status) => format!(
            "ran\t{}\t{summary}\texpected={expected}",
            first_line(status)
        ),
        Err(error) => format!("run-failed\t{}\t{summary}", first_line(error)),
    };
    probed(legacy, "lowered", None)
}

/// Diagnostic only: each transition's guard and pattern handles, so a reader
/// rule can be checked against what the typed trees actually hold.
fn dump_transitions(typed: &typed_trees::TypedTrees) {
    use typed_trees::statement::{StatementNode, TransitionGuardNode};
    let kind = |handle: typed_trees::expression::ExpressionHandle| {
        if handle.is_valid() {
            let node = format!("{:?}", typed.expression_table.expression(handle));
            format!("{handle:?}={}", node.chars().take(160).collect::<String>())
        } else {
            "invalid".to_owned()
        }
    };
    for machine in typed.machines() {
        for state in typed.machine_states.span(machine.states).unwrap_or(&[]) {
            for statement in typed.statement_table.statements(state.statement_nodes) {
                let StatementNode::Transition(transition) = statement else {
                    continue;
                };
                let guard = match transition.guard {
                    TransitionGuardNode::Always => "Always".to_owned(),
                    TransitionGuardNode::When(guard) => kind(guard),
                };
                eprintln!("dump {} guard {guard}", machine.name);
                for pattern in typed
                    .statement_table
                    .transition_patterns(transition.patterns)
                {
                    eprintln!(
                        "dump   pattern subject {} guard {} condition {}",
                        kind(pattern.subject),
                        kind(pattern.pattern.guard),
                        kind(pattern.pattern.condition)
                    );
                }
            }
        }
    }
}

/// Adds one to `key`'s count. The keys are report labels and nothing is
/// decided on them.
fn tally(counts: &mut Vec<(String, usize)>, key: &str) {
    match counts.iter_mut().find(|(seen, _)| seen == key) {
        Some((_, count)) => *count += 1,
        None => counts.push((key.to_owned(), 1)),
    }
}

fn main() {
    let mut stages = Vec::new();
    let mut frontiers = Vec::new();
    let mut verdicts = Vec::new();
    let mut rejected_routes = Vec::new();
    let mut rejected_reasons = Vec::new();
    for argument in std::env::args().skip(1) {
        let main = Path::new(&argument);
        let probed = std::panic::catch_unwind(|| probe(main)).unwrap_or_else(|_| Probed {
            legacy: "panicked\t-".to_owned(),
            reached: "unknown",
            current: "unknown",
            route: "unknown",
            reason: None,
        });
        println!("{argument}\t{}", probed.line());
        tally(&mut stages, probed.legacy.split('\t').next().unwrap_or(""));
        tally(&mut frontiers, probed.reached);
        tally(&mut verdicts, probed.current);
        if probed.current == "rejected" {
            tally(&mut rejected_routes, probed.route);
            if let Some(reason) = &probed.reason {
                tally(&mut rejected_reasons, reason);
            }
        }
    }
    // Totals: the stage counts this probe has always implied, then the
    // programs the seam made visible. `rejected-*` rows cover only programs
    // that reached Psi 04 and that the current checker then rejected.
    for (name, counts) in [
        ("stage", &mut stages),
        ("reached04", &mut frontiers),
        ("current", &mut verdicts),
        ("rejected-route", &mut rejected_routes),
        ("rejected-refusal", &mut rejected_reasons),
    ] {
        counts.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
        for (key, count) in counts.iter() {
            println!("total\t{name}\t{key}\t{count}");
        }
    }
}
