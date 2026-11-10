//! Route one's corpus probe for structure-only elaboration.
//!
//! For each `main.omg` given on the command line: check the fixture with the
//! ordinary pipeline (only to obtain its frozen typed trees and its bound
//! program entry; no checker fact is read), elaborate the entry with
//! `structure_elaboration`, then encode the unsealed module, load it into the
//! Terminal interpreter and run it. One tab-separated line per fixture:
//!
//!     <path> <stage> <detail> [<expected exit>]
//!
//! where stage is `check-failed`, `no-entry`, `refused`, `encode-refused`,
//! `load-refused`, `run-failed` or `ran`. A `summary` line per elaborated
//! fixture counts its lowered rows. This is a measuring tool, not a gate.

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

fn probe(main: &Path) -> String {
    let fixture = main.parent().map_or(main, |parent| parent);
    let expected = std::fs::read_to_string(fixture.join("expected_native_exit.txt"))
        .map(|text| text.trim().to_owned())
        .unwrap_or_default();
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
    let checked = match checked {
        Ok(checked) => checked,
        Err(errors) => return format!("check-failed\t{}", first_line(errors.first())),
    };
    let roots = checked.build().config.program_roots();
    let Some(entry) = roots.first().copied() else {
        return "no-entry\t-".to_owned();
    };
    let program = checked.source_inspection().program();
    if std::env::var_os("STRUCTURE_ELABORATION_DUMP").is_some() {
        dump_transitions(&program.typed);
    }
    let elaboration = match structure_elaboration::elaborate(&program.typed, entry) {
        Ok(elaboration) => elaboration,
        Err(refusal) => return format!("refused\t{}", refusal.detail),
    };
    let mut counts = [0_usize; 4];
    let mut values = 0;
    let mut edges = 0;
    for tables in (1..=elaboration.tables.machines.len() as u64)
        .filter_map(|raw| semantic_vocabulary::PsiSemanticId::new(raw))
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
        Err(error) => return format!("encode-refused\t{}\t{summary}", first_line(error)),
    };
    let mut execution =
        match TerminalExecution::start_artifact(&bytes, &[], TerminalStructuralInputs::default()) {
            Ok(execution) => execution,
            Err(error) => return format!("load-refused\t{}\t{summary}", first_line(error)),
        };
    match execution.resume(&mut WorkMeter::with_budget(10_000_000), &mut NoEffects) {
        Ok(status) => format!(
            "ran\t{}\t{summary}\texpected={expected}",
            first_line(status)
        ),
        Err(error) => format!("run-failed\t{}\t{summary}", first_line(error)),
    }
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

fn main() {
    for argument in std::env::args().skip(1) {
        let main = Path::new(&argument);
        let line =
            std::panic::catch_unwind(|| probe(main)).unwrap_or_else(|_| "panicked\t-".to_owned());
        println!("{argument}\t{line}");
    }
}
