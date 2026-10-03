//! Reference holders survive local construction, calls and scalar completion.

use super::signed;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
use terminal_fuel::TerminalFuelMeter;
use terminal_interpreter::{
    AcceptTerminalEffects, TerminalExecution, TerminalExecutionResult, TerminalExecutionStatus,
    TerminalScalarValue, TerminalStructuralInputs,
};
use terminal_production::{
    TerminalMachineSelection, TerminalProductionCustody, TerminalProductionTimings,
};
use terminal_psi::Terminator;

const LOCAL_REFERENCE_RECORD_TRANSFER: &str = "data Cell { value: i32; }
        data View { cell: &mut Cell; }
        machine forward(view: View) -> View { view }
        machine replace(cell: &mut Cell) { cell.value = 70; }
        machine exercise() -> i32 {
            let mut owner: Cell = Cell { value: 12 };
            let borrowed: &mut Cell = &mut owner;
            let held: View = View { cell: borrowed };
            let forwarded: View = forward(held);
            replace(forwarded.cell);
            owner.value
        }";

#[test]
fn local_reference_record_transfer_executes_without_source() {
    let checked = crate::front_end::checked_program(LOCAL_REFERENCE_RECORD_TRANSFER);
    execute_closed_reference_program(&checked, "exercise", 70);
}

#[test]
fn stored_exclusive_endpoint_survives_recursive_calls_and_scalar_completion() {
    let checked = crate::front_end::checked_program(
        "data Wrap where 4 <= remaining, remaining <= 500 { remaining: u64; }
        data Indirect { target: &mut Wrap; }
        machine walk(n: u64, indirect: Indirect)
            requires 0 <= n && n <= 4
            terminates by n -> Nat::Descending in 0..=indirect.target.remaining;
            -> u64 {
            transition n > 0 {
                true -> walk(n - 1, indirect)
                false -> n
            }
        }
        machine exercise() -> u64 {
            let mut owner: Wrap = Wrap { remaining: 6 };
            let target: &mut Wrap = &mut owner;
            let indirect: Indirect = Indirect { target: target };
            let tick: u64 = 40 + 2;
            let outcome: u64 = walk(4, indirect);
            transition outcome == 0 && tick == 42 { true -> (0) _ -> (1) }
        }",
    );
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name("exercise"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("ranked loan carrier reaches normal scalar completion")
    .into_artifact();
    let mut changed_module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let mut removed = 0;
    for machine in &mut changed_module.machines {
        for block in &mut machine.blocks {
            if let Terminator::Return {
                cleanup_actions, ..
            } = &mut block.terminator
            {
                removed += cleanup_actions.len();
                cleanup_actions.clear();
            }
        }
    }
    assert!(removed > 0, "the scalar exits retain disposal obligations");
    assert!(
        terminal_verifier::validate_module(&changed_module).is_err(),
        "missing owned reference cleanup cannot become a successful return"
    );
    let mut changed = checked.clone();
    let mut erased = 0;
    for catalog in [
        &mut changed.facts.flow.terminal_unit_effects.structural_types,
        &mut changed.facts.flow.terminal_scalar_graphs.structural_types,
    ] {
        for declaration in catalog {
            if let checked_trees::CheckedUnitStructuralTypeShape::Record { fields } =
                &mut declaration.shape
            {
                for field in fields {
                    if field.identity == "remaining" {
                        field.field_type = checked_trees::CheckedUnitStructuralFieldType::Scalar(
                            checked_trees::types::PrimitiveType::U64,
                        );
                        erased += 1;
                    }
                }
            }
        }
    }
    assert!(erased > 0);
    assert!(
        terminal_production::TerminalProductionRequest::new(
            &changed,
            TerminalMachineSelection::Name("exercise"),
        )
        .produce(TerminalProductionCustody::artifact_only(
            &mut TerminalProductionTimings::default(),
        ))
        .is_err(),
        "agreeing catalogs cannot erase declared field bounds"
    );
    let mut execution = TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
    )
    .expect("verify ranked loan carrier independently");
    assert_eq!(
        execution
            .resume(
                &mut TerminalFuelMeter::with_allowance(10_000),
                &mut AcceptTerminalEffects,
            )
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(
            TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                value: IntegerValue::Unsigned(0),
            },
        ))
    );
}

fn execute_closed_reference_program(
    checked: &checked_trees::CheckedTrees,
    name: &str,
    expected: i128,
) {
    let artifact = terminal_production::TerminalProductionRequest::new(
        checked,
        TerminalMachineSelection::Name(name),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .expect("ordinary reference and record transfers retain their loans")
    .into_artifact();
    let mut execution = TerminalExecution::start_artifact(
        artifact.semantic_bytes(),
        artifact.proof_bytes(),
        &proof_admission::AdmissionProfile::default(),
        &[],
        TerminalStructuralInputs::default(),
    )
    .expect("verify transferred loan custody independently");
    assert_eq!(
        execution
            .resume(
                &mut TerminalFuelMeter::with_allowance(1_000),
                &mut AcceptTerminalEffects,
            )
            .unwrap(),
        TerminalExecutionStatus::Complete(TerminalExecutionResult::Scalar(signed(expected)),)
    );
}

#[test]
fn local_reference_record_transfer_rejects_substituted_holder() {
    let original = crate::front_end::checked_program(LOCAL_REFERENCE_RECORD_TRANSFER);
    let machine = original
        .machines()
        .iter()
        .find(|machine| machine.name.as_str() == "exercise")
        .unwrap();
    let state = &original.machine_states(machine)[0];
    for statement_index in 2..=3 {
        let loans = validation::reference_result_custody::local_record_loans(
            &original.typed,
            &original.facts,
            machine.symbol,
            state,
            statement_index,
        )
        .expect("each construction transfers one exact live leaf");
        let handle = loans[0].1;
        for mutation in 0..3 {
            let mut changed = original.clone();
            let loan = changed.facts.borrow.loans.get_mut(handle);
            match mutation {
                0 => loan.source_owner_symbol = symbols::SymbolHandle::invalid(),
                1 => loan.source_owner_symbol = loan.root_symbol,
                2 => loan.kind = checked_trees::BorrowAccessKind::Read,
                _ => unreachable!(),
            }
            assert!(
                validation::reference_result_custody::local_record_loans(
                    &changed.typed,
                    &changed.facts,
                    machine.symbol,
                    state,
                    statement_index,
                )
                .is_none(),
                "statement {statement_index}, mutation {mutation}"
            );
            assert!(
                terminal_production::TerminalProductionRequest::new(
                    &changed,
                    TerminalMachineSelection::Name("exercise"),
                )
                .produce(TerminalProductionCustody::artifact_only(
                    &mut TerminalProductionTimings::default(),
                ))
                .is_err(),
                "substituted holder cannot reach Terminal publication"
            );
        }
    }
}
