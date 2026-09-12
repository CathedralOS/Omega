//! Selected existing owners retain transfer identity through result observation.

use super::{TerminalExecutionResult, TerminalScalarValue, check_source, execute};

const SOURCE: &str =
    include_str!("../../../../../../tests/omega/pass/expressions/owned_match_values/main.omg");

#[test]
fn owned_match_transfers_only_the_selected_existing_local() {
    for selected in [true, false] {
        let (_, execution) = execute(SOURCE, &[TerminalScalarValue::Boolean(selected)]);
        assert_eq!(
            execution.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(selected)),
        );
    }
}

#[test]
fn exclusive_arms_can_transfer_the_same_owned_source() {
    let source = SOURCE.replace("false -> right", "false -> left");
    for selected in [true, false] {
        let (_, execution) = execute(&source, &[TerminalScalarValue::Boolean(selected)]);
        assert_eq!(
            execution.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(true)),
        );
    }
}

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
    let original = check_source(SOURCE).expect("owned selection checks");
    checked_trees_to_lowered_psi::lower_machine(&original, "choose")
        .expect("original selection reaches independent lowering");
    for mutation in 0..6 {
        let mut changed = original.clone();
        let ownership = &mut changed.facts.flow.ownership;
        let (handle, receipt) = ownership.owned_selections.iter().next().unwrap();
        let receipt = receipt.clone();
        let first_transfer = receipt.transfers.start();
        let first_source = receipt.sources.start();
        let other_source =
            arena::Handle::from_parts(first_source.arena_index() + 1, first_source.generation());
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
                ownership.owned_selections.get_mut(handle).transfers = arena::HandleSpan::default()
            }
            4 => {
                ownership.owned_selections.get_mut(handle).death =
                    language_semantics::PermissionEventSource::Statement { statement_index: 2 }
            }
            5 => ownership.owned_selections = arena::Arena::default(),
            _ => unreachable!(),
        }
        assert!(
            checked_trees_to_lowered_psi::lower_machine(&changed, "choose").is_err(),
            "selection receipt mutation {mutation}"
        );
    }
}

#[test]
fn possibly_moved_sources_cannot_be_observed_after_selection() {
    let source = SOURCE.replace("result in Choice::Some", "left in Choice::Some");
    let errors = check_source(&source).expect_err("possibly transferred source is unavailable");
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("may have been transferred")),
        "{errors:?}"
    );
}

#[test]
fn duplicate_prior_receipts_cannot_launder_a_fresh_origin_as_unknown() {
    let source = "data Choice { case Empty; case Some(value: u32); }
        machine choose(selected: bool, other: bool) -> bool {
            let first: Choice = match selected {
                true -> Choice::Some { value: 37 }, false -> Choice::Empty
            };
            let result: Choice = match other { true -> first, false -> first };
            result in Choice::Some
        }";
    let original = super::check_source(source).expect("fresh origin followed by selection checks");
    checked_trees_to_lowered_psi::lower_machine(&original, "choose")
        .expect("unmodified source reaches independent lowering");

    let mut changed = original.clone();
    let (_, selection) = changed
        .facts
        .flow
        .ownership
        .owned_selections
        .iter()
        .next()
        .expect("one existing-owner selection");
    let selection = selection.clone();
    let source_handle = selection.sources.start();
    let incoming = changed
        .facts
        .flow
        .ownership
        .selection_sources
        .get(source_handle)
        .clone();
    assert!(!incoming.origin_selection.is_valid());
    assert!(matches!(
        incoming.provenance,
        language_semantics::PermissionProvenance::Established { .. }
    ));
    let source_expression = changed
        .facts
        .values
        .structural_values
        .root_at(selection.state, incoming.statement_ordinal)
        .expect("fresh source has a retained structural producer")
        .expression;
    let bogus_prior = checked_trees::FlowOwnedSelectionReceipt {
        machine: selection.machine,
        state: selection.state,
        statement_ordinal: incoming.statement_ordinal,
        expression: source_expression,
        destination: incoming.symbol,
        type_reference: selection.type_reference,
        transfers: arena::HandleSpan::empty(),
        sources: arena::HandleSpan::empty(),
        death: language_semantics::PermissionEventSource::StateExit,
    };
    let ownership = &mut changed.facts.flow.ownership;
    let bogus_handle = ownership.owned_selections.append(bogus_prior.clone());
    ownership.owned_selections.append(bogus_prior);
    // Ambiguous lookup used to look absent at the fresh producer, while an
    // unchecked direct handle authorized Unknown at its subsequent consumer.
    assert!(
        ownership
            .owned_selection_at(selection.state, incoming.statement_ordinal)
            .is_none()
    );
    let incoming = ownership.selection_sources.get_mut(source_handle);
    incoming.provenance = language_semantics::PermissionProvenance::Unknown;
    incoming.origin_selection = bogus_handle;

    let error = checked_trees_to_lowered_psi::lower_machine(&changed, "choose")
        .expect_err("duplicate prior receipts cannot fabricate origin authority");
    assert!(
        format!("{error:?}").contains("selected ownership lost its prior selection origin"),
        "expected duplicate-receipt rejection, got {error:?}"
    );
}

#[test]
fn untouched_local_remains_observable_after_selection() {
    let source = SOURCE
        .replace("false -> right", "false -> left")
        .replace("result in Choice::Some", "right in Choice::Empty");
    for selected in [true, false] {
        let (_, execution) = execute(&source, &[TerminalScalarValue::Boolean(selected)]);
        assert_eq!(
            execution.value(),
            TerminalExecutionResult::Scalar(TerminalScalarValue::Boolean(true))
        );
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
    let baseline_checked = check_source(baseline)
        .unwrap_or_else(|errors| panic!("direct borrowed-survivor source: {errors:#?}"));
    let selected_checked = check_source(&selected)
        .unwrap_or_else(|errors| panic!("selected borrowed-survivor source: {errors:#?}"));

    // Both reject at the existing borrowed-local lowering boundary. This is
    // not evidence of address-stable transport, nor a language prohibition.
    for checked in [&baseline_checked, &selected_checked] {
        let error = checked_trees_to_lowered_psi::lower_machine(checked, "choose")
            .expect_err("borrowed-local lowering remains an implementation dependency");
        assert!(
            matches!(
                error,
                checked_trees_to_lowered_psi::LoweringError::Unsupported(
                    "machine has no source-independent checked scalar control plan"
                )
            ),
            "{error:?}"
        );
    }
}
