//! A record declared `dropped by` some machine must be disposed of by that
//! machine and no other, and any machine of the record's own package may
//! still take it apart with a complete consuming pattern. Stage 05 writes
//! the selected machine on the record's type and on every cleanup it
//! schedules, and the lowered-form checker judges the disposal at that
//! exact machine, including that machine's own `requires` clause.
//!
//! The spec rules are in `wiki/spec/language/ownership.md`: "The selected
//! nominal cleanup runs first" ("Construction and disposal order"), and a
//! record with a selected cleanup "cannot be partially moved, and only an
//! authorized complete consuming pattern takes it apart" ("Paths and
//! control flow").
//!
//! The programs declare two machines of the same shape, `finish` and
//! `release`, and select one of them, so the shape alone cannot say which
//! machine disposes of a `Lease`.

#[path = "support/source_handoff.rs"]
mod program;

use program::Program;
use terminal_psi::{TerminalAffineCleanupAction, Terminator};

fn lease(selected: &str, finish_requires: &str) -> String {
    format!(
        "data Lease dropped by {selected} {{ mark: u8; }} \
         machine finish(x: Lease) {finish_requires} {{ let {{ mark as _ }} = move x; }} \
         machine release(y: Lease) {{ let {{ mark as _ }} = move y; }} \
         machine use_it(r: Lease) {{}}"
    )
}

/// The cleanup each return of `use_it` runs: the machine it invokes, and
/// whether it disposes of the whole formal.
fn invoked(program: &Program) -> Vec<(String, bool)> {
    let use_it = program.machine("use_it");
    use_it
        .blocks
        .iter()
        .flat_map(|block| match &block.terminator {
            Terminator::ReturnUnit {
                cleanup_actions, ..
            } => cleanup_actions.as_slice(),
            _ => &[],
        })
        .map(|action| {
            let TerminalAffineCleanupAction::InvokeOwned(invocation) = action else {
                panic!("the whole `Lease` reaches its cleanup: {action:?}")
            };
            (
                program.machine_named(invocation.cleanup_machine),
                invocation.subject.path.is_empty(),
            )
        })
        .collect()
}

/// The machine stage 05 recorded on the `Lease` type as its cleanup.
fn selected(program: &Program) -> String {
    let declared: Vec<_> = program
        .elaboration()
        .module
        .types
        .structural_types
        .iter()
        .filter_map(|declared| declared.nominal_cleanup.as_ref())
        .map(|selection| program.machine_named(selection.cleanup_machine))
        .collect();
    let [machine] = declared.as_slice() else {
        panic!("one type selects a cleanup: {declared:?}")
    };
    machine.clone()
}

/// `dropped by finish` makes `finish` the type's cleanup and the machine
/// `use_it` invokes on its whole formal; `dropped by release` makes it
/// `release`. In both programs the other machine also takes a `Lease`
/// apart with `let { mark as _ } = move ...;`, which its own package may
/// do, and the checker accepts both.
#[test]
fn the_return_invokes_the_cleanup_the_declaration_selects() {
    for (chosen, other) in [("finish", "release"), ("release", "finish")] {
        let program = Program::of(&lease(chosen, ""));
        assert_eq!(selected(&program), chosen);
        assert_eq!(invoked(&program), [(chosen.to_owned(), true)]);
        program.judged(|_| {}).accepted();

        // The lowered return invokes the same-shaped machine the
        // declaration did not select. The checker compares the invocation
        // with the selection stage 05 retained on the type.
        let other = program.elaboration().module.machines[program.position(other)].id;
        let refusal = program
            .judged_in("use_it", |machine| {
                for block in &mut machine.blocks {
                    if let Terminator::ReturnUnit {
                        cleanup_actions, ..
                    } = &mut block.terminator
                    {
                        for action in cleanup_actions {
                            if let TerminalAffineCleanupAction::InvokeOwned(invocation) = action {
                                invocation.cleanup_machine = other;
                            }
                        }
                    }
                }
            })
            .refused();
        assert!(
            refusal
                .detail
                .contains("owned cleanup does not match its retained carrier selection"),
            "{refusal:?}"
        );
    }
}

/// The selected cleanup's own `requires` clause is owed where the cleanup
/// runs: `finish` requires `x.mark == 1`, which nothing about `use_it`'s
/// formal establishes, so the program is rejected. Selecting `release`
/// instead, the same requirement on `finish` is owed nowhere.
#[test]
fn the_selected_cleanup_owes_its_own_requirement_at_the_disposal() {
    let requires = "requires x.mark == 1";
    let rejection = Program::of(&lease("finish", requires))
        .judged(|_| {})
        .rejected();
    assert_eq!(
        rejection.unproved,
        ["CallRequirement { invocation: InvocationId(1), clause: 0 }"],
        "{rejection:?}"
    );
    Program::of(&lease("release", requires))
        .judged(|_| {})
        .accepted();
}
