use super::{RECORD_SOURCE, SOURCE};
use crate::value_dispatch::{TerminalExecutionResult, TerminalScalarValue, execute, unsigned};
use checked_trees_to_lowered_psi::TerminalMachineSelection;

#[test]
fn nested_selections_share_the_result_continuation() {
    let source = SOURCE
        .replace("selected: bool", "selected: bool, inner: bool")
        .replace(
            "true -> left",
            "true -> match inner { true -> left, false -> right }",
        );
    for selected in [true, false] {
        for inner in [true, false] {
            let (module, execution) = execute(
                &source,
                &[
                    TerminalScalarValue::Boolean(selected),
                    TerminalScalarValue::Boolean(inner),
                ],
            );
            assert_eq!(
                execution.value(),
                TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(selected && inner))
            );
            let machine = module
                .machines
                .iter()
                .find(|machine| machine.id == module.entry)
                .unwrap();
            assert_eq!(
                machine
                    .blocks
                    .iter()
                    .filter(|block| !block.structural_parameters.is_empty())
                    .count(),
                1,
                "nested selection uses one result/residual continuation, not a path product"
            );
        }
    }
}

#[test]
fn owned_selection_receipts_reject_changed_origin_arm_roster_and_death() {
    for source in [SOURCE, RECORD_SOURCE] {
        let original =
            crate::front_end::checked_program_result(source).expect("owned selection checks");
        checked_trees_to_lowered_psi::lower_machine(
            &original,
            TerminalMachineSelection::Name("choose"),
        )
        .expect("original selection reaches independent lowering");
        for mutation in 0..6 {
            let mut changed = original.clone();
            let ownership = &mut changed.facts.flow.ownership;
            let (handle, receipt) = ownership.owned_selections.iter().next().unwrap();
            let receipt = receipt.clone();
            let first_transfer = receipt.transfers.start();
            let first_source = receipt.sources.start();
            let other_source = arena::Handle::from_parts(
                first_source.arena_index() + 1,
                first_source.generation(),
            );
            match mutation {
                0 => {
                    ownership.selection_sources.get_mut(first_source).provenance =
                        ownership.selection_sources.get(other_source).provenance
                }
                1 => {
                    ownership
                        .selection_transfers
                        .get_mut(first_transfer)
                        .source_arm = arena::Handle::invalid()
                }
                2 => {
                    let transfer = ownership.selection_transfers.get_mut(first_transfer);
                    transfer.source = if transfer.source == first_source {
                        other_source
                    } else {
                        first_source
                    };
                }
                3 => {
                    ownership.owned_selections.get_mut(handle).transfers =
                        arena::HandleSpan::default()
                }
                4 => {
                    ownership.owned_selections.get_mut(handle).death =
                        language_semantics::PermissionEventSource::Statement { statement_index: 2 }
                }
                5 => ownership.owned_selections = arena::Arena::default(),
                _ => unreachable!(),
            }
            assert!(
                checked_trees_to_lowered_psi::lower_machine(
                    &changed,
                    TerminalMachineSelection::Name("choose")
                )
                .is_err(),
                "selection receipt mutation {mutation}"
            );
        }
    }
}

const CHAINED_SOURCE: &str = "data Choice { case Empty; case Some(value: u32); }
    machine choose(selected: bool, other: bool) -> bool {
        let left: Choice = Choice::Some { value: 37 };
        let right: Choice = Choice::Empty;
        let a: Choice = match selected { true -> left, false -> right };
        let third: Choice = Choice::Some { value: 9 };
        let b: Choice = match other { true -> a, false -> third };
        b in Choice::Some
    }";

#[test]
fn chained_owned_selection_rejects_substituted_origin_receipt() {
    let checked =
        crate::front_end::checked_program_result(CHAINED_SOURCE).expect("chained selection checks");
    let (second, receipt) = checked
        .facts
        .flow
        .ownership
        .owned_selections
        .iter()
        .nth(1)
        .expect("the second match keeps its receipt");
    let sources = checked
        .facts
        .flow
        .ownership
        .selection_sources
        .span(receipt.sources)
        .expect("chained source span");
    let chained = sources
        .iter()
        .position(|source| source.origin_selection.is_valid())
        .expect("the second match records the prior destination's origin");
    assert_eq!(
        sources[chained].provenance,
        language_semantics::PermissionProvenance::Unknown
    );
    for mutation in 0..2 {
        let mut changed = checked.clone();
        let ownership = &mut changed.facts.flow.ownership;
        let source = ownership
            .selection_sources
            .get_mut(arena::Handle::from_parts(
                receipt.sources.start().arena_index() + chained as u32,
                receipt.sources.start().generation(),
            ));
        match mutation {
            // A missing origin handle does not authorize the Unknown
            // provenance row.
            0 => source.origin_selection = arena::Handle::invalid(),
            // The second receipt is a valid handle but names the wrong
            // destination for this source's statement.
            _ => source.origin_selection = second,
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(
                &changed,
                TerminalMachineSelection::Name("choose")
            )
            .is_err(),
            "chained origin mutation {mutation}"
        );
    }
}

const PROJECTED_CHAINED_SOURCE: &str = "data Payload { left: u64; right: u64; }
    data Pair { first: Payload; second: Payload; }
    machine choose(selected: bool, other: bool) -> u64 {
        let x: Pair = Pair {
            first: Payload { left: 1, right: 2 },
            second: Payload { left: 3, right: 4 }
        };
        let y: Pair = Pair {
            first: Payload { left: 5, right: 6 },
            second: Payload { left: 7, right: 8 }
        };
        let a: Pair = match selected { true -> x, false -> y };
        let b: Pair = Pair {
            first: Payload { left: 9, right: 10 },
            second: Payload { left: 11, right: 12 }
        };
        let result: Payload = match other {
            true -> a.first,
            false -> b.second
        };
        result.left ^ result.right
    }";

#[test]
fn projected_move_of_a_prior_selection_result_executes_the_selected_child() {
    for selected in [true, false] {
        for other in [true, false] {
            let (module, execution) = execute(
                PROJECTED_CHAINED_SOURCE,
                &[
                    TerminalScalarValue::Boolean(selected),
                    TerminalScalarValue::Boolean(other),
                ],
            );
            // The first match moves one of the Pair locals into `a`; the
            // second projects `a.first` out of the join block parameter on
            // its true arm and `b.second` out of a plain local otherwise.
            let first = if selected {
                (1u128, 2u128)
            } else {
                (5u128, 6u128)
            };
            let second = (11u128, 12u128);
            let (left, right) = if other { first } else { second };
            assert_eq!(
                execution.value(),
                TerminalExecutionResult::Scalar(unsigned(left ^ right)),
                "selected={selected} other={other}"
            );
            let machine = module
                .machines
                .iter()
                .find(|machine| machine.id == module.entry)
                .unwrap();
            assert_eq!(
                machine
                    .blocks
                    .iter()
                    .filter(|block| !block.structural_parameters.is_empty())
                    .count(),
                2,
                "each selection keeps its own result/residual continuation"
            );
        }
    }
}

#[test]
fn borrowed_untouched_local_keeps_the_lowering_boundary_explicit() {
    let baseline = r#"
        data Choice { case Empty; case Some(value: u32); }
        machine choose(selected: bool) -> bool {
            let left: Choice = Choice::Some { value: 37 };
            let right: Choice = Choice::Empty;
            let view: &Choice = &right;
            view in Choice::Empty
        }
    "#;
    let selected = baseline.replace(
        "view in Choice::Empty",
        "let result: Choice = match selected { true -> left, false -> left };
         view in Choice::Empty",
    );
    let baseline_checked = crate::front_end::checked_program_result(baseline)
        .unwrap_or_else(|errors| panic!("direct borrowed-survivor source: {errors:#?}"));
    let selected_checked = crate::front_end::checked_program_result(&selected)
        .unwrap_or_else(|errors| panic!("selected borrowed-survivor source: {errors:#?}"));

    // Both reject at the existing borrowed-local lowering boundary. This is
    // not evidence of address-stable transport, nor a language prohibition.
    for checked in [&baseline_checked, &selected_checked] {
        let error = checked_trees_to_lowered_psi::lower_machine(
            checked,
            TerminalMachineSelection::Name("choose"),
        )
        .expect_err("borrowed-local lowering remains an implementation dependency");
        assert!(
            matches!(
                error,
                checked_trees_to_lowered_psi::LoweringError::InvalidUnitMachinePlan { reason, .. }
                    if reason.starts_with(
                        "machine has no source-independent checked scalar control plan"
                    )
            ),
            "{error:?}"
        );
    }
}
