//! A machine that establishes a record in activation-local storage and calls
//! a borrowed receiver materializes that slot's stable address into the call.
//! The validated frame layout records the exact loaned-local roster, replay
//! recovers it from the physical instructions rather than the producer's
//! claims, every materialized address resolves through the rostered slot's
//! committed coordinates, and an omitted, invented, reordered, or
//! misattributed loan fails closed — while the artifact still reaches
//! ordinary callable publication on every admitted target.

use crate::tests::{
    AdmissionProfile, NativeTarget, OptimizationSelections, canonical_artifact,
    compiler_baseline_request_v1, optimize_artifact_sections,
    stage_function_fragment_frame_application, stage_optimized_fixed_frame_text_section,
    stage_optimized_function_fragment_emission, stage_optimized_relocation_free_object_container,
    stage_optimized_verified_physical_pipeline_with_provider_executions,
    stage_validated_optimized_object_artifact, stage_validated_optimized_ordinary_callable_entry,
    validate_optimized_ordinary_callable_entry, validate_target_frame_layout,
};
use semantic_vocabulary::{OperationId, PlaceId};
use terminal_production::{
    TerminalMachineSelection, TerminalProductionCustody, TerminalProductionTimings,
};

const SOURCE: &str =
    include_str!("../../../../../omega/pass/structural/local_record_receivers/main.omg");

/// Produce the `sum_local` artifact: the entry establishes a `Pair` record in
/// activation-local storage and calls its borrowed `total` receiver, so the
/// record slot's stable address is loaned across the call.
fn produce() -> (Vec<u8>, Vec<u8>) {
    let tokens = source_files_to_tokens::Lexer::new(SOURCE)
        .tokenize()
        .unwrap();
    let syntax = tokens_to_syntax_trees::parse_syntax_trees(&tokens).unwrap();
    let resolved = syntax_trees_to_symbol_resolved_trees::resolve(
        syntax_trees_to_symbol_resolved_trees::ResolutionRequest::new(&syntax),
    )
    .unwrap();
    let typed =
        symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved).unwrap();
    let checked = typed_trees_to_checked_trees::lower_typed_trees(
        typed,
        &typed_trees_to_checked_trees::CheckingRequest::settled(),
    )
    .unwrap();
    let artifact = terminal_production::TerminalProductionRequest::new(
        &checked,
        TerminalMachineSelection::Name("sum_local"),
    )
    .produce(TerminalProductionCustody::artifact_only(
        &mut TerminalProductionTimings::default(),
    ))
    .unwrap()
    .into_artifact();
    let artifact =
        terminal_codec::CanonicalTerminalArtifact::from_bytes(&artifact.to_bytes()).unwrap();
    let module = terminal_codec::decode_module(artifact.semantic_bytes()).unwrap();
    let proof = terminal_codec::decode_proof_bundle(artifact.proof_bytes()).unwrap();
    terminal_verifier::verify_module(&module, &proof, &AdmissionProfile::default()).unwrap();
    (
        artifact.semantic_bytes().to_vec(),
        artifact.proof_bytes().to_vec(),
    )
}

/// Recover the stable-address-loan roster a canonical layout must record for
/// one function: every activation-local slot whose address is materialized
/// into a value, minus allocator spill slots whose only materializations are
/// private reload windows.
fn expected_loans(
    function: &physical_instructions::PostAllocationMachineFunction,
) -> Vec<selected_instructions::LocalStorageSlotId> {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match instruction.address {
            Some(physical_instructions::PhysicalAddressOperation::FrameAddress {
                slot: selected_instructions::FrameStorageSlotId::Local(slot),
                ..
            }) if !matches!(
                slot,
                selected_instructions::LocalStorageSlotId::Spill { .. }
            ) =>
            {
                Some(slot)
            }
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[test]
fn loaned_local_addresses_replay_through_ordinary_callable_entry() {
    let (semantic, proof) = produce();
    for target in [
        NativeTarget::linux_x64(),
        NativeTarget::windows_x64(),
        NativeTarget::uefi_x64(),
        NativeTarget::linux_arm64(),
        NativeTarget::macos_arm64(),
    ] {
        let optimized = optimize_artifact_sections(
            &semantic,
            &proof,
            &AdmissionProfile::default(),
            compiler_baseline_request_v1(&OptimizationSelections::new([]).unwrap()),
        )
        .unwrap();
        let physical = stage_optimized_verified_physical_pipeline_with_provider_executions(
            optimized,
            target,
            &[],
        )
        .unwrap_or_else(|error| panic!("{target:?} physical: {error:?}"));
        let realization = physical.fixed_frame_for_test();
        let layout = realization.frame().plan();
        let machine_plan = realization.machine().machine().plan();
        let mut loaned = 0usize;
        for (row, function) in layout.functions.iter().zip(&machine_plan.functions) {
            assert_eq!(row.machine, function.machine, "{target:?}");
            // The recorded roster is exactly the set of non-spill local
            // slots whose addresses the physical instructions materialize.
            let expected = expected_loans(function);
            assert_eq!(
                row.stable_address_loans, expected,
                "{target:?} {:?}: {row:?}",
                row.machine
            );
            // Every loaned slot is a placed local slot inside the committed
            // extent — a loan can never name storage the frame did not
            // commit, and canonical order carries no duplicates.
            let committed = row.frame_size_bytes - row.red_zone_resident_bytes;
            assert!(
                row.stable_address_loans
                    .windows(2)
                    .all(|pair| pair[0] < pair[1]),
                "{target:?} {:?}: roster is not canonical",
                row.machine
            );
            for loan in &row.stable_address_loans {
                let placed = row
                    .local_storage_slots
                    .iter()
                    .find(|slot| slot.id == *loan)
                    .unwrap_or_else(|| panic!("{target:?}: unplaced loan {loan:?}"));
                assert!(
                    placed.frame_offset_bytes >= row.outgoing_abi_area.byte_size
                        && placed.frame_offset_bytes + u64::from(placed.size_bytes) <= committed,
                    "{target:?} {:?}: loan {loan:?} escapes the committed extent: {row:?}",
                    row.machine
                );
            }
            loaned += usize::from(!expected.is_empty());
        }
        assert!(
            loaned >= 1,
            "{target:?}: the receiver program must loan local storage"
        );
        // The produced layout replays exactly; a roster that omits a loan,
        // invents one, misattributes storage, or loses canonical order
        // fails closed on every target.
        let plan = layout.clone();
        let current = realization.allocation().current();
        let validate = |candidate: machine_code::TargetFrameLayoutPlan| {
            validate_target_frame_layout(
                realization.machine(),
                realization.requirements(),
                realization.storage(),
                current.register_environment(),
                candidate,
            )
        };
        assert!(validate(plan.clone()).is_ok(), "{target:?}");
        let row = layout
            .functions
            .iter()
            .position(|function| !function.stable_address_loans.is_empty())
            .unwrap_or_else(|| panic!("{target:?}: no loaned function"));
        let mut omitted = plan.clone();
        omitted.functions[row].stable_address_loans.pop();
        assert!(
            validate(omitted).is_err(),
            "{target:?}: an omitted loan is not canonical"
        );
        let mut invented = plan.clone();
        invented.functions[row].stable_address_loans.push(
            selected_instructions::LocalStorageSlotId::Structural {
                operation: OperationId::new(9_999_999).unwrap(),
                place: PlaceId::new(9_999_999).unwrap(),
            },
        );
        assert!(
            validate(invented).is_err(),
            "{target:?}: an invented loan is not canonical"
        );
        let mut misattributed = plan.clone();
        misattributed.functions[row].stable_address_loans =
            vec![selected_instructions::LocalStorageSlotId::Boundary {
                operation: OperationId::new(9_999_998).unwrap(),
            }];
        assert!(
            validate(misattributed).is_err(),
            "{target:?}: a loan naming different storage is not canonical"
        );
        let mut reordered = plan.clone();
        reordered.functions[row].stable_address_loans.reverse();
        if reordered.functions[row].stable_address_loans != plan.functions[row].stable_address_loans
        {
            assert!(
                validate(reordered).is_err(),
                "{target:?}: the canonical roster is ordered"
            );
        }
        // The same program still reaches ordinary callable publication on
        // every admitted target, loans resolved against the committed frame.
        let emitted = stage_optimized_function_fragment_emission(
            physical.into_function_fragment_emission_source(),
        )
        .unwrap_or_else(|error| panic!("{target:?} emission: {error:?}"));
        let framed = stage_function_fragment_frame_application(emitted)
            .unwrap_or_else(|error| panic!("{target:?} frame: {error:?}"));
        let text = stage_optimized_fixed_frame_text_section(framed)
            .unwrap_or_else(|error| panic!("{target:?} text: {error:?}"));
        let object = stage_optimized_relocation_free_object_container(text)
            .unwrap_or_else(|error| panic!("{target:?} object: {error:?}"));
        let artifact = stage_validated_optimized_object_artifact(
            canonical_artifact(&semantic, &proof),
            object,
        )
        .unwrap_or_else(|error| panic!("{target:?} artifact: {error:?}"));
        let callable = stage_validated_optimized_ordinary_callable_entry(artifact)
            .unwrap_or_else(|error| panic!("{target:?} callable: {error:?}"));
        assert_eq!(
            validate_optimized_ordinary_callable_entry(&callable).unwrap(),
            callable.custody(),
            "{target:?}"
        );
    }
}
