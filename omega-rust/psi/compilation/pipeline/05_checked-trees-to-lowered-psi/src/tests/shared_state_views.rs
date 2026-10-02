//! A state edge may restrict a mutable loan to a shared state view.
//!
//! A named `&mut` parameter passed to a state declaring `&T` keeps its
//! place and narrows its access, exactly as the implicit receiver does for a
//! `&self` state. The shared state reads the place but cannot write it, and
//! Terminal verification accepts the narrowed block parameter.

use super::lower_machine;
use crate::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use terminal_psi::StructuralAccess;

const SOURCE: &str = "
    data Main { level: u8; seen: u8 in Wrapping; }
    machine Main::observe(main: &mut Main) {
        main.level = 3;
        transition { _ -> inspect(main) }
        state inspect(main: &Main) {
            transition main.level == 3 {
                true -> keep(main)
                _ -> keep(main)
            }
        }
        state keep(main: &Main) { }
    }
    machine Main::main(main: &mut Main) { Main::observe(main); }
";

#[test]
fn a_named_mutable_loan_narrows_to_a_shared_state_view() {
    let checked = crate::front_end::checked_program(SOURCE);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::main"))
        .expect("a mutable loan narrows to the shared successor state");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the narrowed shared view verifies");
    assert!(
        lowered
            .semantic_module
            .machines
            .iter()
            .flat_map(|machine| &machine.blocks)
            .flat_map(|block| &block.structural_parameters)
            .any(|parameter| parameter.access == StructuralAccess::SharedBorrow),
        "a successor block declares the shared view"
    );
}

fn lowered_named_loans(source: &str) -> lowered_psi::LoweredPsi {
    let checked = crate::front_end::checked_program(source);
    let lowered = lower_machine(&checked, TerminalMachineSelection::Name("Main::run"))
        .expect("named state loans lower");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("named state loans independently verify");
    lowered
}

#[test]
fn projected_child_releases_before_the_named_parent_restores_access() {
    use terminal_psi::{OperationKind, Terminator};

    let lowered = lowered_named_loans(
        "data Inner { tag: u64; }
         data Main { inner: Inner; }
         machine Main::run(main: &mut Main) {
             main.inner.tag = 5;
             transition { _ -> next(main, &mut main.inner) }
             state next(main: &mut Main, loan: &mut Inner) {
                 loan.tag = 9;
                 transition { _ -> done(main) }
             }
             state done(main: &mut Main) { main.inner.tag = 11; }
         }",
    );
    let mut module = lowered.semantic_module.clone();
    let machine = module
        .machines
        .iter_mut()
        .find(|machine| machine.id == module.entry)
        .expect("entry machine");
    let parent = machine.structural_parameters[0].place;
    let child = machine
        .blocks
        .iter()
        .flat_map(|block| &block.structural_parameters)
        .map(|parameter| parameter.place)
        .collect::<Vec<_>>();
    assert_eq!(
        child.len(),
        1,
        "only the projected child needs a new binding"
    );
    let child = child[0];
    let restored = machine
        .blocks
        .iter_mut()
        .filter(|block| matches!(block.terminator, Terminator::ReturnUnit { .. }))
        .flat_map(|block| &mut block.operations)
        .find(|operation| {
            matches!(&operation.kind,
            OperationKind::StructuralScalarFieldStore { destination, path, .. }
                if *destination == parent && !path.is_empty())
        })
        .expect("the final state writes the restored parent's child field");
    let operation = restored.id;
    let OperationKind::StructuralScalarFieldStore {
        destination, path, ..
    } = &mut restored.kind
    else {
        unreachable!()
    };
    // Keep the field's type and access valid, but use the retired child name.
    // Dropping that child restored the parent; it did not keep both names live.
    *destination = child;
    path.clear();
    assert_eq!(
        terminal_verifier::validate_module(&module).map(|_| ()),
        Err(
            terminal_verifier::ModuleError::OwnedStructuralPlaceNotLiveAtOperation {
                operation,
                place: child,
            }
        ),
    );
}

#[test]
fn unchanged_cyclic_loans_keep_distinct_entry_roots_after_parameter_reordering() {
    let lowered = lowered_named_loans(
        "data Main { count: u64; }
         machine Main::run(left: &mut Main, right: &mut Main, repeat: bool) {
             transition { _ -> step(right, left, repeat) }
             state step(first: &mut Main, second: &mut Main, repeat: bool) {
                 transition repeat {
                     true -> step(first, second, false)
                     _ -> done(first, second)
                 }
             }
             state done(first: &mut Main, second: &mut Main) {
                 first.count = 7;
                 second.count = 9;
             }
         }",
    );
    let machine = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .unwrap();
    assert!(
        machine
            .blocks
            .iter()
            .all(|block| block.structural_parameters.is_empty())
    );
    let roots = machine
        .structural_parameters
        .iter()
        .map(|parameter| parameter.place)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(roots.len(), 2);
    let written = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation.kind {
            terminal_psi::OperationKind::StructuralScalarFieldStore { destination, .. } => {
                Some(destination)
            }
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        written, roots,
        "forwarding must not merge same-typed invocation storage"
    );
}

#[test]
fn a_foreign_arrival_prevents_entry_aliasing_through_the_whole_cycle() {
    let lowered = lowered_named_loans(
        "data Main { count: u64; }
         machine Main::run(left: &mut Main, right: &mut Main, pick: bool, repeat: bool) {
             transition pick {
                 true -> step(left, repeat)
                 _ -> step(right, repeat)
             }
             state step(main: &mut Main, repeat: bool) {
                 transition repeat {
                     true -> step(main, false)
                     _ -> done(main)
                 }
             }
             state done(main: &mut Main) { main.count = 7; }
         }",
    );
    let machine = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .unwrap();
    let roots = machine
        .structural_parameters
        .iter()
        .map(|parameter| parameter.place)
        .collect::<std::collections::BTreeSet<_>>();
    let writes = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match operation.kind {
            terminal_psi::OperationKind::StructuralScalarFieldStore { destination, .. } => {
                Some(destination)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(!writes.is_empty());
    for destination in writes {
        assert!(
            !roots.contains(&destination),
            "mixed arrivals cannot select one invocation root"
        );
        assert!(
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.structural_parameters)
                .any(|parameter| parameter.place == destination)
        );
    }
}
