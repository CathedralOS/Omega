use super::{checked_program, frame};
use checked_trees::{
    CheckedStoreRoot, CheckedUnitEffectOperationPlan, CheckedUnitStructuralPathSegment,
};

use super::{ShapeCollector, build_structural_scalar_field_store_sequence, machine_binders};
use crate::execution::terminal_unit::calls::structural_scalar_signature;

fn carrier_stores(
    checked: &checked_trees::CheckedTrees,
) -> Option<Vec<CheckedUnitEffectOperationPlan>> {
    let program = &checked.typed;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::inspect")
        .expect("inspect machine");
    let state = &program.machine_states(machine)[0];
    let mut shapes = ShapeCollector::new(program);
    let (_, structural, scalar) = structural_scalar_signature(
        program,
        &mut shapes,
        machine,
        state,
        &machine_binders(program, machine),
        true,
    )
    .expect("inspect signature");
    build_structural_scalar_field_store_sequence(
        program,
        &checked.facts,
        machine,
        state,
        &structural,
        &scalar,
        0,
        None,
    )
}

fn assert_carrier_primitive_store(
    source: &str,
    expected_path: &[CheckedUnitStructuralPathSegment],
) {
    let checked = checked_program(source);
    let program = &checked.typed;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::inspect")
        .expect("inspect machine");
    let state = &program.machine_states(machine)[0];
    let statements = program.statement_table.statements(state.statement_nodes);
    let typed_trees::statement::StatementNode::LocalData(carrier) = &statements[0] else {
        panic!("established carrier local");
    };
    let captured = validation::reference_result_custody::local_record_loans(
        program,
        &checked.facts,
        machine.symbol,
        state,
        0,
    )
    .expect("a mutable carrier retains its exact establishment loans");
    assert_eq!(captured.len(), 1);
    let (_, loan) = &captured[0];
    assert!(
        !checked
            .facts
            .flow
            .borrow_lifetimes
            .weakenings
            .iter()
            .any(|(_, weakening)| {
                weakening.loan == *loan
                    && weakening.source
                        == checked_trees::FlowInvalidationSource::Statement { statement_index: 2 }
                    && weakening.reason == checked_trees::FlowBorrowWeakeningReason::LocalReassigned
            }),
        "a pointee write cannot retire the carrier's loan"
    );
    let stores = carrier_stores(&checked)
        .expect("the metadata call preserves the carrier's reference-field store");
    let [
        CheckedUnitEffectOperationPlan::WriteOnlyPrimitiveStore {
            statement_index,
            destination,
            path,
            ..
        },
    ] = stores.as_slice()
    else {
        panic!("one primitive referent store, got {stores:?}");
    };
    assert_eq!(*statement_index, 2);
    // The live carrier supplies the address and custody. Its static origin
    // must not replace the local root or erase the reference crossing.
    assert_eq!(
        *destination,
        CheckedStoreRoot::Local {
            symbol: carrier.symbol,
        }
    );
    assert_eq!(path, expected_path);
}

#[test]
fn mutable_carrier_establishment_keeps_reference_occurrences_separate() {
    let checked = checked_program(
        r#"
        data Carrier { destination: &mut u64; }
        data Main {}
        machine Main::inspect(destination: &mut u64, replacement: &mut u64) -> u64 {
            let mut carrier: Carrier = Carrier { destination: destination };
            carrier.destination = replacement;
            carrier.destination = 11;
            70
        }
        "#,
    );
    let program = &checked.typed;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::inspect")
        .expect("inspect");
    let state = &program.machine_states(machine)[0];
    assert!(
        validation::reference_result_custody::local_record_loans(
            program,
            &checked.facts,
            machine.symbol,
            state,
            0,
        )
        .is_none(),
        "LocalReassigned release still needs its own publication evidence"
    );
    let initial = checked
        .facts
        .borrow
        .loans
        .iter()
        .find(|(_, loan)| loan.statement_index == 0)
        .expect("initial capture exists independently of publication")
        .0;
    let owner = checked.facts.borrow.loans.get(initial).owner_symbol;
    let replacement = checked
        .facts
        .borrow
        .loans
        .iter()
        .find(|(_, loan)| loan.owner_symbol == owner && loan.statement_index == 1)
        .expect("replacement establishes a different loan occurrence")
        .0;
    let weakenings = &checked.facts.flow.borrow_lifetimes.weakenings;
    assert!(
        weakenings.iter().any(|(_, weakening)| {
            weakening.loan == initial
                && weakening.source
                    == checked_trees::FlowInvalidationSource::Statement { statement_index: 1 }
                && weakening.reason == checked_trees::FlowBorrowWeakeningReason::LocalReassigned
        }),
        "replacement ends the old carrier occurrence"
    );
    assert!(
        !weakenings.iter().any(|(_, weakening)| {
            weakening.loan == replacement
                && weakening.reason == checked_trees::FlowBorrowWeakeningReason::LocalReassigned
        }),
        "neither its own establishment nor a pointee store retires the new loan"
    );
}

#[test]
fn mutable_carrier_establishment_rejects_forged_source_and_occurrence() {
    let mut checked = checked_program(
        r#"
        data Carrier { destination: &mut u64; }
        data Main {}
        machine Main::inspect(destination: &mut u64, other: &mut u64) -> u64 {
            let mut carrier: Carrier = Carrier { destination: destination };
            carrier.destination = 11;
            70
        }
        "#,
    );
    let program = &checked.typed;
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "Main::inspect")
        .expect("inspect");
    let state = &program.machine_states(machine)[0];
    let captured = validation::reference_result_custody::local_record_loans(
        program,
        &checked.facts,
        machine.symbol,
        state,
        0,
    )
    .expect("pointee write retains a publishable initial capture");
    assert_eq!(captured.len(), 1);
    let initial = captured[0].1;
    let machine_symbol = machine.symbol;
    let replacement_root = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.name.as_str() == "other")
        .expect("distinct input")
        .symbol;
    let state = state.clone();
    let original_root = checked.facts.borrow.loans.get(initial).root_symbol;
    assert_ne!(original_root, replacement_root);
    checked.facts.borrow.loans.get_mut(initial).root_symbol = replacement_root;
    assert!(
        validation::reference_result_custody::local_record_loans(
            &checked.typed,
            &checked.facts,
            machine_symbol,
            &state,
            0,
        )
        .is_none(),
        "same-shaped replacement storage cannot authorize the initial capture"
    );
    checked.facts.borrow.loans.get_mut(initial).root_symbol = original_root;
    checked.facts.borrow.loans.get_mut(initial).statement_index = 1;
    assert!(
        validation::reference_result_custody::local_record_loans(
            &checked.typed,
            &checked.facts,
            machine_symbol,
            &state,
            0,
        )
        .is_none(),
        "another occurrence cannot authorize the initial capture"
    );
}

#[test]
fn reference_carrier_primitive_store_follows_metadata_method() {
    assert_carrier_primitive_store(
        r#"
        data Carrier { destination: &mut u64; count: u64; }
        machine Carrier::set_count(carrier: &mut Carrier) { carrier.count = 7; }
        data Main { scratch: u64; }
        machine Main::inspect(destination: &mut u64, observed: &u64) -> u64 {
            let mut carrier: Carrier = Carrier { destination: destination, count: 0 };
            Carrier::set_count(&mut carrier);
            carrier.destination = 11;
            match observed == 59 && carrier.count == 7 { true -> 70, false -> 71 }
        }
        "#,
        &[
            CheckedUnitStructuralPathSegment::Field("destination".into()),
            CheckedUnitStructuralPathSegment::Referent,
        ],
    );
}

#[test]
fn reference_carrier_primitive_store_retains_nested_nonleading_leaf() {
    assert_carrier_primitive_store(
        r#"
        data Window { padding: u64; destination: &mut u64; }
        data Carrier { prefix: u64; window: Window; count: u64; }
        machine Carrier::set_count(carrier: &mut Carrier) { carrier.count = 7; }
        data Main { scratch: u64; }
        machine Main::inspect(destination: &mut u64, observed: &u64) -> u64 {
            let mut carrier: Carrier = Carrier {
                prefix: 3,
                window: Window { padding: 5, destination: destination },
                count: 0
            };
            Carrier::set_count(&mut carrier);
            carrier.window.destination = 11;
            match observed == 59 && carrier.count == 7 { true -> 70, false -> 71 }
        }
        "#,
        &[
            CheckedUnitStructuralPathSegment::Field("window".into()),
            CheckedUnitStructuralPathSegment::Field("destination".into()),
            CheckedUnitStructuralPathSegment::Referent,
        ],
    );
}

#[test]
fn reference_carrier_primitive_store_rejects_shared_reference_permission() {
    let checked = checked_program(
        r#"
            data Carrier { destination: &u64; count: u64; }
            machine Carrier::set_count(carrier: &mut Carrier) { carrier.count = 7; }
            data Main { scratch: u64; }
            machine Main::inspect(destination: &mut u64, observed: &u64) -> u64 {
                let mut carrier: Carrier = Carrier { destination: destination, count: 0 };
                Carrier::set_count(&mut carrier);
                carrier.destination = 11;
                match observed == 59 && carrier.count == 7 { true -> 70, false -> 71 }
            }
        "#,
    );
    let stores = carrier_stores(&checked);
    assert!(
        stores.is_none(),
        "a shared leaf permits neither a primitive store nor another store fallback: {stores:?}"
    );
}

/// A reference-typed receiver field writes under its own path: a borrow moved
/// field-to-field keeps a finite caller-visible frame because every later
/// write through the stored borrow already composes beneath `self.<field>`.
/// The checked frame records `self.alias` and `frame::matches` admits it —
/// the write-frame-agreement gate no longer stops the body at the store.
#[test]
fn reference_field_store_keeps_the_receiver_path_in_a_complete_write_frame() {
    for access in ["&'a mut [u8]", "&'a [u8]"] {
        let source = format!(
            r#"
            data Main<'a> {{
                view: {access};
                alias: {access};
            }}
            machine Main::run(&mut self) {{
                self.alias = self.view;
            }}
        "#
        );
        let checked = checked_program(&source);
        let program = &checked.typed;
        let machine = program
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::run")
            .unwrap();
        let state = &program.machine_states(machine)[0];
        let write_frame = checked
            .facts
            .mutation
            .for_machine(machine.symbol)
            .unwrap()
            .state_write_frames
            .iter()
            .find(|entry| entry.state == state.symbol)
            .expect("state write frame")
            .frame
            .clone();
        assert_eq!(write_frame.paths(), ["self.alias"], "access: {access}");
        assert!(
            frame::matches(program, machine, state, &write_frame, None),
            "access: {access}"
        );
    }
}
