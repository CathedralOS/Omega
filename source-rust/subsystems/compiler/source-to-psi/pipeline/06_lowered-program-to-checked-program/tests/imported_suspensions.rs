//! A suspended call into another file must wait on the machine the file
//! qualifier selects, and a value read from another file keeps that file's
//! declared type, whether the suspended result is bound to a local first or
//! tested directly. Each program spreads over two or three files of one
//! package; a twin swaps which file holds which machine or changes the
//! imported type, so a compiler that picked a same-named machine from the
//! wrong file, or read the wrong type, gives the twin the wrong verdict.
//!
//! The spec rules: `|alias|::name` selects the declaration of the imported
//! file (`wiki/spec/language/modules.md`, "Names and imports"); a suspended
//! call names the machine it waits on (`wiki/spec/language/effects.md`,
//! "Call-site acknowledgements").

#[path = "support/source_handoff.rs"]
mod program;

use program::{Program, returned};
use terminal_psi::{OperationKind, TerminalMachine};

const BOUND: &str = "let v: bool = suspend |kit|::ready(x); transition v";
const DIRECT: &str = "transition suspend |kit|::ready(x)";

fn files<'a>(kit: &'a str, other: &'a str, main: &'a str) -> [(&'a str, &'a str); 3] {
    [
        ("product/kit.omg", kit),
        ("product/other.omg", other),
        ("product/main.omg", main),
    ]
}

/// The one Boolean constant `machine` returns.
fn constant_returned(machine: &TerminalMachine) -> bool {
    let value = returned(machine);
    let found = machine
        .operations()
        .find(|operation| operation.result.scalar().map(|result| result.id) == Some(value))
        .expect("the returned value is produced in the machine");
    let OperationKind::BooleanConstant { value } = found.kind else {
        panic!("{found:?}")
    };
    value
}

/// The machine the one call of `caller` enters.
fn callee<'a>(program: &'a Program, caller: &str) -> &'a TerminalMachine {
    let call = program::the_operation(program.machine(caller), |kind| {
        matches!(kind, OperationKind::Call { .. })
    });
    let OperationKind::Call { callee, .. } = call.kind else {
        unreachable!()
    };
    program
        .elaboration()
        .module
        .machines
        .iter()
        .find(|machine| machine.id == callee)
        .expect("the callee is lowered")
}

/// `kit.omg` and `other.omg` each declare a public `ready`; one returns
/// `true` and the other `false`. `choose` waits on `|kit|::ready` and
/// `sibling` on `|other|::ready`. Each waits on the `ready` of the file it
/// names, so exchanging the two files' bodies exchanges what each callee
/// returns, and the checker accepts both programs.
#[test]
fn a_suspended_call_waits_on_the_machine_its_file_qualifier_selects() {
    let yes = "public machine ready(x: i32) -> bool suspends { true }";
    let no = "public machine ready(x: i32) -> bool suspends { false }";
    for subject in [BOUND, DIRECT] {
        let main = format!(
            "use kit.omg; use other.omg;
machine choose(x: i32) -> u8 suspends {{ {subject} {{ true -> (17u8), _ -> (23u8), }} }}
machine sibling(x: i32) -> bool suspends {{ suspend |other|::ready(x) }}"
        );
        for (kit, other, kit_answers) in [(yes, no, true), (no, yes, false)] {
            let program = Program::try_of(&files(kit, other, &main))
                .unwrap_or_else(|refusal| panic!("{subject}: {refusal:?}"));
            assert_eq!(
                constant_returned(callee(&program, "choose")),
                kit_answers,
                "{subject}"
            );
            assert_eq!(
                constant_returned(callee(&program, "sibling")),
                !kit_answers,
                "{subject}"
            );
            program.judged(|_| {}).accepted();
        }
    }
}

const READY: &str = "machine wait(x: i32) -> i32 suspends { x }
machine ready(x: i32) -> bool suspends {
    let v: i32 = suspend wait(x); let observed: i32 = |kit|::v(); observed == v
}";

fn choose(subject: &str) -> String {
    format!(
        "use kit.omg;
{READY}
machine choose(x: i32) -> i32 suspends {{
    {subject} {{ true -> accept(), _ -> other(), }}
    machine accept() -> i32 {{ |kit|::v() }}
    machine other() -> i32 {{ 0 }}
}}"
    )
}

/// `ready` and `choose`'s nested `accept` read `|kit|::v()`. With `v`
/// returning an `i32` both are accepted, and each of the two calls enters
/// `kit.omg`'s `v`; with `v` returning a `bool`, the program is refused at
/// the first read, where `ready` stores it in an `i32`.
#[test]
fn a_value_read_from_another_file_keeps_that_files_type() {
    for subject in [
        "let v: bool = suspend ready(x); transition v",
        "transition suspend ready(x)",
    ] {
        let main = choose(subject);
        for value in ["7", "8"] {
            let kit = format!("public machine v() -> i32 {{ {value} }}");
            let program =
                Program::try_of(&[("product/kit.omg", &kit), ("product/main.omg", &main)])
                    .unwrap_or_else(|refusal| panic!("{subject}: {refusal:?}"));
            let v = program.machine("v").id;
            for reader in ["ready", "choose\\accept"] {
                let calls: Vec<_> = program
                    .machine(reader)
                    .operations()
                    .filter_map(|operation| match operation.kind {
                        OperationKind::Call { callee, .. } => Some(callee),
                        _ => None,
                    })
                    .filter(|callee| *callee == v)
                    .collect();
                assert_eq!(calls.len(), 1, "{subject}: {reader}");
            }
            program.judged(|_| {}).accepted();
        }
        let refusal = Program::try_of(&[
            ("product/kit.omg", "public machine v() -> bool { true }"),
            ("product/main.omg", &main),
        ])
        .err()
        .expect("a bool cannot initialize an i32");
        assert_eq!(
            (refusal.stage, refusal.at.as_str()),
            ("lowering", "|kit|::v()")
        );
        assert!(
            refusal
                .message
                .starts_with("Violation: this value has scalar carrier `bool` but the local it initializes requires `i32`"),
            "{}",
            refusal.message
        );
    }
}
