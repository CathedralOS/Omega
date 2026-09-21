//! A ranked cyclic machine may re-establish an unrestricted scalar array on
//! every traversal and pass the member-produced payload to a call as an
//! owned argument, or re-establish an empty affine record — the
//! composed-control spelling of a trivial affine local — whose custody must
//! be disposed again before the place can be re-armed. The verifier admits
//! the cycle only through the establishment arm, and the interpreter
//! replaces the stored payload each traversal instead of treating the place
//! as established once per activation.

use proof_admission::AdmissionProfile;
use semantic_vocabulary::{IntegerSign, IntegerType, IntegerValue, StructuralPlaceKind};
use terminal_interpreter::{
    TerminalExecutionResult, TerminalScalarValue, interpret_terminal_artifact,
};
use terminal_psi::{
    OperationKind, OperationResult, StructuralAccess, StructuralMultiplicity, TerminalModule,
    TerminalRankedScc, Terminator,
};

use super::checked_source;

/// `scan` re-enters its body through the recursive edge, so `first` reads
/// whichever payload the deepest traversal established: `[0, scale]` at the
/// base, not the entry iteration's `[3, scale]`. A stale or rejected
/// re-establishment changes the result or fails interpretation outright.
const CYCLE_SOURCE: &str = r#"
    machine first(row: [u64; 2]) -> u64 { row[0] }
    machine scan(remaining: u64 [0..=5], scale: u64 [0..=10]) -> u64
    terminates by remaining -> Nat::Descending in 0..6;
    {
        let v: u64 = first([remaining, scale]);
        transition remaining > 0 {
            true -> scan(remaining - 1, scale)
            _ -> v
        }
    }
"#;

#[test]
fn cyclic_unrestricted_scalar_array_reestablishes_and_feeds_an_owned_argument() {
    let checked = checked_source(CYCLE_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "scan")
        .expect("ranked cycle carrying a scalar-array establishment lowers");
    let module = &lowered.semantic_module;
    let scan = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("entry scan");
    let establishment = scan
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::EstablishScalarArray { .. }))
        .expect("the cyclic body retains one scalar-array establishment");
    let place = establishment
        .result
        .structural()
        .expect("array establishment produces a structural result")
        .place;
    assert_eq!(
        establishment.result.structural().unwrap().multiplicity,
        StructuralMultiplicity::Unrestricted
    );
    let call = scan
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::CallStructuralScalar { .. }))
        .expect("the cyclic body retains the owned-argument scalar call");
    let OperationKind::CallStructuralScalar {
        structural_arguments,
        ..
    } = &call.kind
    else {
        unreachable!()
    };
    assert_eq!(structural_arguments.len(), 1);
    assert_eq!(structural_arguments[0].place, place);
    assert_eq!(structural_arguments[0].access, StructuralAccess::Owned);

    terminal_verifier::verify_module(module, &lowered.proof_bundle, &AdmissionProfile::default())
        .expect("the cyclic scalar-array machine passes independent verification");

    let semantic = terminal_codec::encode_module(module).expect("encode semantics");
    let proof =
        terminal_codec::encode_proof_section(module, &lowered.proof_bundle).expect("encode proof");
    let unsigned = |value: u64| TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        value: IntegerValue::Unsigned(value.into()),
    };
    for remaining in 0..=5u64 {
        let TerminalExecutionResult::Scalar(result) = interpret_terminal_artifact(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[unsigned(remaining), unsigned(7)],
        )
        .expect("re-established cyclic array payload interprets") else {
            panic!("scan returns a scalar")
        };
        // The base traversal's establishment stores `[0, 7]`; `first` reads
        // element 0, so the result is always the deepest `remaining` — zero.
        // Any earlier payload surviving across traversals would leak the
        // entry `remaining` into the result instead.
        assert_eq!(
            result,
            unsigned(0),
            "remaining={remaining}: the deepest traversal's payload wins",
        );
    }
}

/// `scan` declares `marker`, an empty affine local, ahead of the transition,
/// so composed lowering establishes the empty record inside the cyclic
/// component rather than through the entry-sequence
/// `EstablishTrivialAffineLocal` spelling. The frontier replay proves the
/// affine place dies on every edge that could re-enter the establishment:
/// the member-block result is carried forward through owned block
/// parameters and discarded before the backedge can re-arm it.
const AFFINE_CYCLE_SOURCE: &str = r#"
    data Marker {}
    machine first(row: [u64; 2]) -> u64 { row[0] }
    machine scan(remaining: u64 [0..=5], scale: u64 [0..=10]) -> u64
    terminates by remaining -> Nat::Descending in 0..6;
    {
        let marker: Marker = Marker {};
        let v: u64 = first([remaining, scale]);
        transition remaining > 0 {
            true -> scan(remaining - 1, scale)
            _ -> v
        }
    }
"#;

#[test]
fn cyclic_affine_empty_record_reestablishes_inside_the_component() {
    let checked = checked_source(AFFINE_CYCLE_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "scan")
        .expect("ranked cycle carrying an affine empty local lowers");
    let module = &lowered.semantic_module;
    let scan = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("entry scan");
    let Some(TerminalRankedScc::Natural(components)) = &scan.ranked_scc else {
        panic!("the authored cycle carries natural ranking evidence");
    };
    let members = components[0]
        .ranks
        .iter()
        .map(|rank| rank.block)
        .collect::<std::collections::BTreeSet<_>>();
    let establishment = scan
        .blocks
        .iter()
        .find(|block| {
            block.operations.iter().any(|operation| {
                matches!(&operation.kind, OperationKind::EstablishRecord { fields }
                if fields.is_empty()
                    && operation
                        .result
                        .structural()
                        .is_some_and(|result| {
                            result.multiplicity == StructuralMultiplicity::Affine
                                && result.claims.is_empty()
                        }))
            })
        })
        .expect("the cyclic body retains one empty affine establishment");
    assert!(
        members.contains(&establishment.id),
        "the affine empty record is established by a cyclic member block",
    );

    terminal_verifier::verify_module(module, &lowered.proof_bundle, &AdmissionProfile::default())
        .expect("the cyclic affine-record machine passes independent verification");

    let semantic = terminal_codec::encode_module(module).expect("encode semantics");
    let proof =
        terminal_codec::encode_proof_section(module, &lowered.proof_bundle).expect("encode proof");
    let unsigned = |value: u64| TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        value: IntegerValue::Unsigned(value.into()),
    };
    for remaining in 0..=5u64 {
        let TerminalExecutionResult::Scalar(result) = interpret_terminal_artifact(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[unsigned(remaining), unsigned(7)],
        )
        .expect("re-armed affine local interprets") else {
            panic!("scan returns a scalar")
        };
        assert_eq!(result, unsigned(0));
    }
}

/// Removing one disposal obligation from a member edge keeps the affine
/// place live across the backedge, so the next traversal's establishment
/// would produce an already-owned place. Verification must reject the
/// drifted custody instead of trusting the admission.
#[test]
fn cyclic_affine_empty_record_rejects_when_an_edge_drops_its_disposal() {
    let checked = checked_source(AFFINE_CYCLE_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "scan")
        .expect("ranked cycle carrying an affine empty local lowers");
    let module = &lowered.semantic_module;
    let mut rosters = 0;
    for block_index in 0..module.machines[0].blocks.len() {
        for edge_index in 0..2 {
            let mut drifted = module.clone();
            let block = &mut drifted.machines[0].blocks[block_index];
            let removed = match &mut block.terminator {
                Terminator::Jump {
                    trivial_affine_discards,
                    ..
                } if edge_index == 0 && !trivial_affine_discards.is_empty() => {
                    Some(trivial_affine_discards.remove(0))
                }
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    let edge = if edge_index == 0 {
                        when_true
                    } else {
                        when_false
                    };
                    (!edge.trivial_affine_discards.is_empty())
                        .then(|| edge.trivial_affine_discards.remove(0))
                }
                _ => None,
            };
            let Some(_) = removed else { continue };
            rosters += 1;
            assert!(
                terminal_verifier::verify_module(
                    &drifted,
                    &lowered.proof_bundle,
                    &AdmissionProfile::default(),
                )
                .is_err(),
                "edge {edge_index} of block {block_index} lost an affine disposal",
            );
        }
    }
    assert!(
        rosters > 0,
        "the affine local's custody produces edge disposal rosters to strip",
    );
}

/// Re-spell `scan`'s member `marker` establishment into the direct
/// `EstablishTrivialAffineLocal` form the ordinary checked-machine
/// statement path emits: the declared place is identical, so every disposal
/// roster and the frontier's custody replay stay as the composed spelling
/// produced them — only the operation kind, its `Unit` result, and the
/// place's declaration kind change.
fn respell_member_record_as_trivial_affine_local(
    module: &mut TerminalModule,
) -> semantic_vocabulary::PlaceId {
    let entry = module.entry;
    let machine = module
        .machines
        .iter_mut()
        .find(|machine| machine.id == entry)
        .expect("entry scan");
    let (block_index, operation_index, picked) = machine
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .operations
                .iter()
                .enumerate()
                .find_map(|(operation_index, operation)| {
                    if !matches!(
                        &operation.kind,
                        OperationKind::EstablishRecord { fields } if fields.is_empty()
                    ) {
                        return None;
                    }
                    operation
                        .result
                        .structural()
                        .filter(|result| result.multiplicity == StructuralMultiplicity::Affine)
                        .map(|result| (block_index, operation_index, result.place))
                })
        })
        .expect("the cyclic body retains one empty affine establishment");
    let structural_type = machine
        .structural_places
        .iter()
        .find(|place| place.id == picked)
        .and_then(|place| match place.kind {
            StructuralPlaceKind::OperationResult {
                structural_type, ..
            } => Some(structural_type),
            _ => None,
        })
        .expect("the member establishment's result place declares its type");
    let declaration_ordinal = machine
        .structural_places
        .iter()
        .filter(|place| matches!(place.kind, StructuralPlaceKind::TrivialAffineLocal { .. }))
        .count() as u32;
    machine.blocks[block_index].operations[operation_index].kind =
        OperationKind::EstablishTrivialAffineLocal {
            destination: picked,
        };
    machine.blocks[block_index].operations[operation_index].result = OperationResult::Unit;
    machine
        .structural_places
        .iter_mut()
        .find(|place| place.id == picked)
        .expect("the declared place exists")
        .kind = StructuralPlaceKind::TrivialAffineLocal {
        declaration_ordinal,
        structural_type,
        construction: None,
    };
    picked
}

/// The same cycle, with the member `marker` establishment re-spelled to the
/// direct `EstablishTrivialAffineLocal` operation: the eligibility fence
/// admits the unit-result establishment arm, the frontier replay enforces
/// the identical re-arm lifecycle, and the interpreter observes the same
/// result.
#[test]
fn cyclic_trivial_affine_local_reestablishes_inside_the_component() {
    let checked = checked_source(AFFINE_CYCLE_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "scan")
        .expect("ranked cycle carrying an affine empty local lowers");
    let mut module = lowered.semantic_module.clone();
    let picked = respell_member_record_as_trivial_affine_local(&mut module);
    let scan = module
        .machines
        .iter()
        .find(|machine| machine.id == module.entry)
        .expect("entry scan");
    let Some(TerminalRankedScc::Natural(components)) = &scan.ranked_scc else {
        panic!("the authored cycle carries natural ranking evidence");
    };
    let members = components[0]
        .ranks
        .iter()
        .map(|rank| rank.block)
        .collect::<std::collections::BTreeSet<_>>();
    let establishment = scan
        .blocks
        .iter()
        .find(|block| {
            block.operations.iter().any(|operation| {
                matches!(
                    &operation.kind,
                    OperationKind::EstablishTrivialAffineLocal { destination }
                        if *destination == picked
                )
            })
        })
        .expect("the cyclic body retains the direct affine establishment");
    assert!(
        members.contains(&establishment.id),
        "the trivial affine local is established by a cyclic member block",
    );

    terminal_verifier::verify_module(&module, &lowered.proof_bundle, &AdmissionProfile::default())
        .expect("the cyclic trivial-affine-local machine passes independent verification");

    let semantic = terminal_codec::encode_module(&module).expect("encode semantics");
    let proof =
        terminal_codec::encode_proof_section(&module, &lowered.proof_bundle).expect("encode proof");
    let unsigned = |value: u64| TerminalScalarValue::Integer {
        scalar_type: IntegerType::new(IntegerSign::Unsigned, 64).unwrap(),
        value: IntegerValue::Unsigned(value.into()),
    };
    for remaining in 0..=5u64 {
        let TerminalExecutionResult::Scalar(result) = interpret_terminal_artifact(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            &[unsigned(remaining), unsigned(7)],
        )
        .expect("re-armed trivial affine local interprets") else {
            panic!("scan returns a scalar")
        };
        assert_eq!(result, unsigned(0));
    }
}

/// The same drift as the empty-record rejection, now against the direct
/// spelling: removing one disposal obligation from a member edge keeps the
/// affine place live across the backedge, so the next traversal's
/// establishment would produce an already-owned place. Verification must
/// reject the drifted custody instead of trusting the admission.
#[test]
fn cyclic_trivial_affine_local_rejects_when_an_edge_drops_its_disposal() {
    let checked = checked_source(AFFINE_CYCLE_SOURCE);
    let lowered = checked_trees_to_lowered_psi::lower_machine(&checked, "scan")
        .expect("ranked cycle carrying an affine empty local lowers");
    let mut module = lowered.semantic_module.clone();
    respell_member_record_as_trivial_affine_local(&mut module);
    let mut rosters = 0;
    for block_index in 0..module.machines[0].blocks.len() {
        for edge_index in 0..2 {
            let mut drifted = module.clone();
            let block = &mut drifted.machines[0].blocks[block_index];
            let removed = match &mut block.terminator {
                Terminator::Jump {
                    trivial_affine_discards,
                    ..
                } if edge_index == 0 && !trivial_affine_discards.is_empty() => {
                    Some(trivial_affine_discards.remove(0))
                }
                Terminator::Conditional {
                    when_true,
                    when_false,
                    ..
                } => {
                    let edge = if edge_index == 0 {
                        when_true
                    } else {
                        when_false
                    };
                    (!edge.trivial_affine_discards.is_empty())
                        .then(|| edge.trivial_affine_discards.remove(0))
                }
                _ => None,
            };
            let Some(_) = removed else { continue };
            rosters += 1;
            assert!(
                terminal_verifier::verify_module(
                    &drifted,
                    &lowered.proof_bundle,
                    &AdmissionProfile::default(),
                )
                .is_err(),
                "edge {edge_index} of block {block_index} lost an affine disposal",
            );
        }
    }
    assert!(
        rosters > 0,
        "the trivial affine local's custody produces edge disposal rosters to strip",
    );
}
