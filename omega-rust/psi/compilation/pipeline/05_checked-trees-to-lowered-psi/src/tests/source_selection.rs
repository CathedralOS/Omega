//! Source closure ownership travels with the selected lowering result.

use crate::TerminalMachineSelection;
use crate::machine_lowering::machine_dispatch::{lower_selected_machine, select_terminal_machine};
use crate::producer_result::{SourceMappedLowered, SourceMapping};

#[test]
fn shared_scalar_catalog_preserves_distinct_results_and_their_guarantee() {
    use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue};
    use terminal_interpreter::{
        AcceptTerminalEffects, TerminalExecutionResult, TerminalScalarValue,
        TerminalStructuralInputs,
    };

    let checked = crate::front_end::checked_program(
        r#"
        machine choose(first: bool) -> u64 ensures result >= 20 {
            transition first {
                true -> twenty()
                false -> thirty()
            }
            state twenty() -> u64 { 20 }
            state thirty() -> u64 { 30 }
        }
        "#,
    );
    let lowered = crate::lower_machine(&checked, TerminalMachineSelection::Name("choose"))
        .expect("the shared catalog retains each scalar exit's guarantee");
    let entry = lowered
        .semantic_module
        .machines
        .iter()
        .find(|machine| machine.id == lowered.semantic_module.entry)
        .unwrap();
    assert_eq!(entry.contract.ensures.len(), 1);
    let semantic_bytes = terminal_codec::encode_module(&lowered.semantic_module).unwrap();
    let proof_bytes =
        terminal_codec::encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle)
            .unwrap();
    for (first, expected) in [(true, 20), (false, 30)] {
        let result = terminal_interpreter::interpret_terminal_artifact_measured(
            &semantic_bytes,
            &proof_bytes,
            &proof_admission::AdmissionProfile::default(),
            &[TerminalScalarValue::Boolean(first)],
            TerminalStructuralInputs::default(),
            &mut AcceptTerminalEffects,
        )
        .expect("both independently verified scalar exits execute");
        assert_eq!(
            result.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Integer {
                scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
                value: IntegerValue::Unsigned(expected),
            })
        );
    }
}

#[test]
fn scalar_state_local_calls_retain_their_exact_borrow_custody() {
    let checked = crate::front_end::checked_program(
        r#"
        machine increment(value: &mut u64) {
            value = ((value as u64 in Wrapping) + 1) as u64;
        }
        machine enter() -> u64 {
            let mut value: u64 = 41;
            increment(&mut value);
            let observed: u64 = value;
            transition { _ -> report(observed) }
            state report(observed: u64) -> u64 { observed }
        }
        "#,
    );
    let lowered = crate::lower_machine(&checked, TerminalMachineSelection::Name("enter"))
        .expect("state-local storage composes with Unit calls and scalar returns");
    let module = terminal_codec::decode_module(
        &terminal_codec::encode_module(&lowered.semantic_module).unwrap(),
    )
    .unwrap();
    let proof = terminal_codec::decode_proof_bundle(
        &terminal_codec::encode_proof_bundle(&lowered.proof_bundle).unwrap(),
    )
    .unwrap();
    terminal_verifier::verify_module(
        &module,
        &proof,
        &proof_admission::AdmissionProfile::default(),
    )
    .expect("source-free state-local call replay");

    let access = checked
        .facts
        .borrow
        .argument_accesses
        .iter()
        .find_map(|(handle, row)| {
            (row.kind == checked_trees::BorrowAccessKind::Mutable).then_some(handle)
        })
        .expect("authored mutable call loan");
    for substitute_root in [false, true] {
        let mut changed = checked.clone();
        let row = changed.facts.borrow.argument_accesses.get_mut(access);
        if substitute_root {
            row.root_symbol = symbols::SymbolHandle::default();
        } else {
            row.kind = checked_trees::BorrowAccessKind::Read;
        }
        assert!(
            crate::lower_machine(&changed, TerminalMachineSelection::Name("enter")).is_err(),
            "another referent or shared access cannot justify the mutable call"
        );
    }
}

#[test]
fn source_projection_modes_preserve_their_distinct_custody_authority() {
    let checked = crate::front_end::checked_program(
        "machine leaf(value: u32) -> u32 { value }
         machine enter(value: u32) -> u32 { leaf(value) }",
    );
    let selection =
        select_terminal_machine(&checked, TerminalMachineSelection::Name("enter")).unwrap();
    let lowered = lower_selected_machine(&checked, selection).unwrap();
    let sources = &lowered.source_machines;
    let expected = sources
        .iter()
        .copied()
        .zip(
            lowered
                .terminal
                .semantic_module
                .machines
                .iter()
                .map(|machine| machine.id),
        )
        .collect::<Vec<_>>();
    assert_eq!(
        lowered.source_mapping.exact_owners(),
        Some(expected.as_slice())
    );
    assert!(SourceMapping::EntryOnly.exact_owners().is_none());
    assert_eq!(
        SourceMapping::EntryOnly
            .projection_sources(&lowered.terminal, selection.machine)
            .unwrap(),
        vec![(selection.machine, lowered.terminal.semantic_module.entry)],
    );
    let exact = SourceMapping::ExactCatalog(expected.clone());
    assert_eq!(exact.exact_owners(), Some(expected.as_slice()));
    assert_eq!(
        exact
            .projection_sources(&lowered.terminal, selection.machine)
            .unwrap(),
        expected,
    );
}

#[test]
fn exact_source_catalog_reorders_owners_and_rejects_incomplete_or_duplicate_bindings() {
    let checked = crate::front_end::checked_program(
        "boundary trait Host { machine touch(); }
         machine leaf() reaches Host { Host::touch(); }
         machine enter() reaches Host { leaf(); }",
    );
    let selection =
        select_terminal_machine(&checked, TerminalMachineSelection::Name("enter")).unwrap();
    let lowered = lower_selected_machine(&checked, selection).unwrap();
    let owners = lowered.source_mapping.exact_owners().unwrap();
    let mut reversed = owners.to_vec();
    reversed.reverse();
    assert_eq!(
        SourceMappedLowered::new(lowered.terminal.clone(), reversed)
            .unwrap()
            .source_machine_ids,
        owners,
    );
    let mut duplicate_source = owners.to_vec();
    duplicate_source[1].0 = duplicate_source[0].0;
    let mut duplicate_target = owners.to_vec();
    duplicate_target[1].1 = duplicate_target[0].1;
    let mut foreign_target = owners.to_vec();
    foreign_target[1].1 = semantic_vocabulary::MachineId::new(9000).unwrap();
    for invalid in [
        owners[..1].to_vec(),
        duplicate_source,
        duplicate_target,
        foreign_target,
    ] {
        assert!(SourceMappedLowered::new(lowered.terminal.clone(), invalid).is_err());
    }
}
