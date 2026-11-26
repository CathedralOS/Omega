//! A Product driver settles provider defaults and supplies by reading only
//! the lowered closure of each roster it tries, and lowers the rest of the
//! roster it keeps. Stage 05 therefore lowers a roster in two steps:
//! `lower_roster_closure` reads the closure and lists the conformance rows it
//! owes, and `complete` prepares those witnesses. These tests pin that the
//! closure is readable before completion, that a discarded closure hands its
//! roster back unchanged, and that completion still prepares every witness
//! the roster owes.

#[path = "../../06_lowered-program-to-checked-program/tests/support/source_handoff.rs"]
mod program;

use program::{PACKAGE, Program};
use semantic_vocabulary::PackageKeyIdentity;
use source_compiler::test_support::lowering::source_roster::{
    SourceCompilationRoster, lower_roster_closure,
};
use terminal_check_tables::compilation::TerminalCompilation;

/// One conformance of the package: `Twice::double` must refine `Doubling`'s
/// requirement, which is one witness the package owes whether or not `main`
/// calls it.
const CONFORMANCE: &str = "trait Doubling {
    machine double(x: u8) -> u8 requires x < 100u8;
}
Twice satisfies Doubling {
    machine double(x: u8) -> u8 requires x < 100u8 { x + x }
}
machine main() -> u8 { 1u8 }
";

fn roster(program: &Program) -> SourceCompilationRoster {
    SourceCompilationRoster {
        entries: vec![program.machine_symbol("main")],
        subject: Some(PackageKeyIdentity::from_digest(PACKAGE).unwrap()),
        ..Default::default()
    }
}

#[test]
fn a_roster_closure_is_read_before_its_witnesses_are_prepared() {
    let program = Program::of(CONFORMANCE);
    let roster = roster(&program);
    let closure = lower_roster_closure(roster.clone(), &program.prepared).unwrap();
    let elaboration = closure
        .closure()
        .expect("the closure lowers before any witness is prepared");
    let main = program.machine_symbol("main");
    assert!(
        elaboration
            .machines
            .iter()
            .any(|(_, symbol)| *symbol == main)
    );
    // Settlement that asks for another roster gets this one back as it was.
    assert_eq!(
        format!("{:?}", closure.into_roster()),
        format!("{roster:?}")
    );
}

#[test]
fn completing_a_roster_closure_prepares_every_witness_the_roster_owes() {
    let program = Program::of(CONFORMANCE);
    let closure = lower_roster_closure(roster(&program), &program.prepared).unwrap();
    let lowered = closure.complete();
    let TerminalCompilation::Prepared(prepared) = lowered.program() else {
        panic!("the roster binds no callable shape that could fail to form")
    };
    assert!(prepared.closure.is_ok());
    assert_eq!(prepared.witnesses.len(), 1);
    assert!(prepared.witnesses.iter().all(Result::is_ok));
    assert!(lowered.source().roster.is_some());
}

#[test]
fn modular_witness_stubs_closed_callees_without_lowering_transitive_bodies() {
    let build = |crash_body: &str| {
        format!(
            "trait Check {{
    machine run(x: u8) -> u8 requires x > 0u8 crashes none;
}}
machine transitive(x: u8) -> u8 {{ {crash_body} }}
machine helper(x: u8) -> u8 requires x < 100u8 {{ transitive(x) }}
Impl satisfies Check {{
    machine run(x: u8) -> u8 requires x > 0u8, x < 100u8 crashes none {{ helper(x) }}
}}
machine main() -> u8 {{ 1u8 }}
"
        )
    };
    for (body, expects_trap) in [("x + 1u8", false), ("crash Trap;", true)] {
        let program = Program::of(&build(body));
        let closure = lower_roster_closure(roster(&program), &program.prepared).unwrap();
        let closure_ref = closure.closure().expect("closure must lower");
        let helper = program.machine_symbol("helper");
        let transitive = program.machine_symbol("transitive");
        assert!(closure_ref.machines.iter().any(|(_, sym)| *sym == helper));
        assert!(
            closure_ref
                .machines
                .iter()
                .any(|(_, sym)| *sym == transitive)
        );

        let lowered = closure.complete();
        let TerminalCompilation::Prepared(prepared) = lowered.program() else {
            panic!("the roster binds no callable shape that could fail to form");
        };
        assert_eq!(prepared.witnesses.len(), 1);
        let witness = prepared.witnesses[0]
            .as_ref()
            .expect("modular witness must lower cleanly");
        assert!(
            witness.machines.iter().all(|(_, sym)| *sym != transitive),
            "modular witness must not lower transitive callees"
        );
        let helper_id = witness
            .machines
            .iter()
            .find_map(|(id, sym)| (*sym == helper).then_some(id))
            .expect("witness retains direct callee header");
        assert!(
            witness.requirements.contains(&helper_id),
            "direct closed callee must be stubbed as a bodyless requirement"
        );
        let helper_machine = witness
            .module
            .machine(helper_id)
            .expect("stubbed helper machine");
        assert!(
            helper_machine.blocks.is_empty(),
            "stubbed helper must carry no lowered blocks"
        );
        let has_trap = helper_machine
            .declared_crash_routes
            .as_ref()
            .is_some_and(|routes| {
                routes
                    .iter()
                    .any(|bucket| bucket.cause == semantic_vocabulary::CrashCause::Trap)
            });
        assert_eq!(has_trap, expects_trap);
    }
}
