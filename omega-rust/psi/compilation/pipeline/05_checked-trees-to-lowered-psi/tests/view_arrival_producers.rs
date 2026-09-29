//! A state that receives an element view reads it with the extent of the
//! producer every arrival transferred. The producer supplies no fact: Terminal
//! reconstruction relates the successor's length read to the dominating
//! establishment or subslice, the producer searches a certificate against those
//! reconstructed axioms, and the reloaded artifact is verified independently.
use checked_trees_to_lowered_psi::TerminalMachineSelection;
use proof_admission::AdmissionProfile;
use terminal_codec::{encode_module, encode_proof_section};
use terminal_psi::OperationKind;

fn lower(source: &str) -> Result<lowered_psi::LoweredPsi, String> {
    let checked = crate::front_end::checked_program(source);
    checked_trees_to_lowered_psi::lower_machine(
        &checked,
        TerminalMachineSelection::Name("Root::enter"),
    )
    .map_err(|error| format!("{error:?}"))
}

/// Every element read whose view is a block parameter, with its bound
/// obligation.
fn arrival_reads(lowered: &lowered_psi::LoweredPsi) -> Vec<semantic_vocabulary::ObligationId> {
    lowered
        .semantic_module
        .machines
        .iter()
        .flat_map(|machine| {
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter_map(move |operation| match operation.kind {
                    OperationKind::ElementViewRead {
                        source, obligation, ..
                    } if machine.structural_places.iter().any(|place| {
                        place.id == source
                            && matches!(
                                place.kind,
                                semantic_vocabulary::StructuralPlaceKind::BlockParameter { .. }
                            )
                    }) =>
                    {
                        Some(obligation)
                    }
                    _ => None,
                })
        })
        .collect()
}

/// Reload the emitted artifact, require a retained certificate for every
/// arrival read, and verify the whole module without producer state.
fn verify_arrival_reads(source: &str) {
    let lowered = lower(source).expect("the arrival read lowers");
    let reads = arrival_reads(&lowered);
    assert!(!reads.is_empty(), "the fixture reads a transferred view");
    for obligation in reads {
        assert!(
            lowered
                .proof_bundle
                .evidence
                .iter()
                .any(|evidence| evidence.obligation == obligation),
            "the producer certifies arrival read bound {obligation:?}"
        );
    }
    let module = terminal_codec::decode_module(&encode_module(&lowered.semantic_module).unwrap())
        .expect("reload module");
    let proof = terminal_codec::decode_proof_bundle(
        &encode_proof_section(&lowered.semantic_module, &lowered.proof_bundle).unwrap(),
    )
    .expect("reload proof");
    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default())
        .expect("the arrival read verifies independently");
}

#[test]
fn an_established_view_bounds_a_fixed_read_in_its_successor_state() {
    verify_arrival_reads(
        r#"
        boundary trait Output {
            machine count(value: u64) reaches Output;
        }
        data Entry {
            value: i32;
        }
        data Root {
            entries: [Entry; 4];
        }
        machine Root::enter(&mut self) reaches Output {
            self.entries[0].value = 7;
            let entry_view: &[Entry] = self.entries.as_slice();
            transition { _ -> check_first(entry_view) }
            state check_first(&mut self, entries: &[Entry]) {
                transition entries[0].value == 7 {
                    true -> good()
                    false -> bad()
                }
            }
            state good(&mut self) { Output::count(121); }
            state bad(&mut self) { Output::count(122); }
        }
    "#,
    );
}

const TAIL: &str = r#"
        boundary trait Output {
            machine count(value: u64) reaches Output;
        }
        data Entry {
            value: i32;
        }
        data Root {
            entries: [Entry; 4];
        }
        machine Root::enter(&mut self) reaches Output {
            self.entries[2].value = 37;
            let view: &[Entry] = self.entries.as_slice();
            let tail: &[Entry] = view[1..];
            transition { _ -> choose(tail, 1) }
            state choose(&mut self, entries: &[Entry], index: u64) {
                transition entries[index].value == 37 {
                    true -> good()
                    false -> bad()
                }
            }
            state good(&mut self) { Output::count(207); }
            state bad(&mut self) { Output::count(208); }
        }
    "#;

#[test]
fn a_tail_subslice_bounds_a_transferred_index_in_its_successor_state() {
    verify_arrival_reads(TAIL);
}

/// The retained certificate for the tail read cites the subslice's extent
/// equation. Retargeting the arrival to the whole view changes what the
/// verifier reconstructs, so the unchanged certificate no longer replays.
#[test]
fn a_retargeted_arrival_invalidates_the_tail_certificate() {
    let lowered = lower(TAIL).expect("the tail arrival lowers");
    let mut module = lowered.semantic_module.clone();
    let machine = module
        .machines
        .iter_mut()
        .find(|machine| {
            machine
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .any(|operation| {
                    matches!(operation.kind, OperationKind::ElementViewSubslice { .. })
                })
        })
        .expect("the entry machine subslices its view");
    let (whole, tail) = machine
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find_map(|operation| match operation.kind {
            OperationKind::ElementViewSubslice { source, .. } => {
                Some((source, operation.result.structural()?.place))
            }
            _ => None,
        })
        .expect("the tail subslice");
    let mut retargeted = 0;
    for block in &mut machine.blocks {
        let arguments = match &mut block.terminator {
            terminal_psi::Terminator::Jump {
                structural_arguments,
                ..
            } => structural_arguments,
            _ => continue,
        };
        for argument in arguments
            .iter_mut()
            .filter(|argument| argument.place == tail)
        {
            argument.place = whole;
            retargeted += 1;
        }
    }
    assert_eq!(retargeted, 1, "one arrival transfers the tail");
    terminal_verifier::verify_module(
        &lowered.semantic_module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect("the authored arrival verifies");
    let error = terminal_verifier::verify_module(
        &module,
        &lowered.proof_bundle,
        &AdmissionProfile::default(),
    )
    .expect_err("the tail certificate cannot certify the whole view's arrival");
    let reads = arrival_reads(&lowered);
    assert!(
        matches!(
            error,
            terminal_verifier::VerificationError::RejectedEvidence { obligation, .. }
                if reads.contains(&obligation)
        ),
        "the arrival read's certificate is rejected: {error:?}"
    );
}
